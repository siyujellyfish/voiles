"""以四個公開命令檢查確定初始化與跨型別函式簽章。"""
from pathlib import Path
import sys

from m7_public_cases import verify


def main(body):
    return "fn main()->int{\n" + body + "\n}"


cli = Path(sys.argv[1]).resolve(strict=True)
verify(cli, [(main(body), expected) for body, expected in [
    ("var n:int\nn = 42\nreturn n", 42),
    ("var n:int\nif(true){ n = 42 } else { n = 0 }\nreturn n", 42),
    ("var n:int\nif(false){ return 7 } else { n = 42 }\nreturn n", 42),
    ("var n:int\nif(true){ n = 42 }\nreturn n", "reject"),
    ("var n:int\nif(true){ var n:int = 42 }\nreturn n", "reject"),
    ("var n:int\nn = n + 1\nreturn n", "reject"),
    ("var n:int\nfor i in 0..0{ n = 42 }\nreturn n", "reject"),
    ("var n:int\nwhile(false){ n = 42 }\nreturn n", "reject"),
    ('const s:str = "甲"\nvar i:int\nreturn s[i][0]', "reject"),
    ("var n:int\ntry{ n = 9223372036854775807 + 1 } except EvaluationError{ n = 42 }\nreturn n", 42),
    ("var n:int\ntry{ n = 9223372036854775807 + 1 } except EvaluationError{ return n }\nreturn n", "reject"),
    ("var b:bool\nb = true\nif(b){ return 42 } else { return 0 }", 42),
    ("var f:float\nf = 0.5\nif(f != 0.5){ return 0 } else { return 42 }", 42),
]])

choose = 'fn choose(flag:bool,n:float)->str{ if(flag){ return f"{n:.1f}" } else { return "*" } }\n'
verify(cli, [(choose + main(body), expected) for body, expected in [
    ('const s:str = choose(false,0.5)\nreturn s[0][0]', 42),
    ('if(true){ const s:str = choose(n=0.5,flag=true)\nreturn s[0][0] } else { return 0 }', 48),
    ('const s:str = choose(1,0.5)\nreturn 0', "reject"),
    ('const s:str = choose(true,1)\nreturn 0', "reject"),
]])

verify(cli, [(main(body), expected) for body, expected in [
    ("var n:int = 4\nvar sum:int = 0\nfor i in 1..n{ sum = sum + i\nn = 1 }\nreturn sum", 6),
    ("var n:int = 4\nfor i in n + 1..n{ return 1 }\nreturn 42", 42),
    ("var n:int\nfor i in 0..n{ return i }\nreturn 0", "reject"),
    ("for i in true..3{ return i }\nreturn 0", "reject"),
    ("for i in 0..0.5{ return i }\nreturn 0", "reject"),
]])

finish = "fn finish(n:int)->none{ if(n != 0){ return none } else { return none } }\n"
verify(cli, [(finish + main(body), expected) for body, expected in [
    ("finish(42)\nreturn 42", 42),
    ("finish(n=0)\nreturn 42", 42),
    ("return finish(42)", "reject"),
    ("if(finish(0) != finish(1)){ return 1 } else { return 0 }", "reject"),
    ("finish(false)\nreturn 0", "reject"),
]])
verify(cli, [
    ("fn main()->none{ return none }", "reject"),
    ("fn wrong()->none{ return 1 }\n" + main("return 0"), "reject"),
    (finish + "fn relay()->none{ return finish(42) }\n" + main("relay()\nreturn 42"), 42),
])

verify(cli, [(main("if(" + condition + "){ return 42 } else { return 0 }"), expected)
             for condition, expected in [
    ("2 + 3 == 5 and 1 < 2 and 2 <= 2 and 3 > 2 and 4 >= 4", 42),
    ("0 > 1 or 3 <= 2 or 4 < 4", 0),
    ("false < true", "reject"),
    ('"x" == "x"', "reject"),
    ("1 == true", "reject"),
    ("1 < 2 < 3", "reject"),
]])

verify(cli, [(main("return " + expression), expected) for expression, expected in [
    ("2 + 5 * 2 ** 3", 42),
    ("2 ** 3 ** 2 - 470", 42),
    ("-2 ** 2 + 46", 42),
    ("(-2) ** 2 + 38", 42),
    ("-7 // 3 + 45", 42),
    ("7 % -3 + 44", 42),
    ("-7 / 3 + 44", 42),
    ("0 ** 0 + 41", 42),
    ("1 // 0", "runtime"),
    ("1 % 0", "runtime"),
    ("2 ** -1", "runtime"),
    ("9223372036854775807 * 2", "runtime"),
    ("false * 2", "reject"),
]])

verify(cli, [(main(body), expected) for body, expected in [
    ("var n:int = 40\nreturn n+=2", 42),
    ("const n:int = 40\nreturn n+=2", "reject"),
    ("var n:int\nreturn n+=2", "reject"),
    ("var n:int = 9223372036854775807\ntry{ return n+=1 } except EvaluationError{ return n - 9223372036854775765 }", 42),
]])

lexer = """struct Token{ var start:int
const _tag:int = 3 }
fn scan(n:int)->list[Token]{
    var tokens:list[Token] = []
    tokens.append(Token(start=n))
    return tokens.move()
}
fn first(tokens:list[Token])->int{ with tokens[0] as t{ return t.start } }"""
program = "import lexer\n"
verify(cli, [(program + main(body), expected) for body, expected in [
    ("var xs:list[lexer.Token] = lexer.scan(42)\nreturn lexer.first(xs.move())", 42),
    ("var xs:list[lexer.Token] = lexer.scan(42)\nwith xs[0] as t{ return t.start }", 42),
    ("var xs:list[lexer.Token] = lexer.scan(42)\nwith xs[0] as t{ return t._tag }", "reject"),
    ("var xs:list[Missing] = []\nreturn len(xs)", "reject"),
    ("var xs:list[lexer.Token] = []\nxs.append(42)\nreturn 0", "reject"),
]], {"lexer.voil": lexer})

verify(cli, [(main(body), expected) for body, expected in [
    ("match(1+1){ case 1{ return 0 } case 2{ return 42 } case _{ return 3 } }",42),
    ("match(-9223372036854775808){ case -0x8000000000000000{ return 42 } case _{ return 0 } }",42),
    ("match(-1){ case -1+0{ return 1 } case _{ return 42 } }","reject"),
    ("match(false){ case true{ return 0 } case false{ return 42 } }",42),
    ("var n:int\nmatch(1){ case 1{ n = 42 } case _{ n = 3 } }\nreturn n",42),
    ("var n:int\nmatch(1){ case 1{ n = 42 } }\nreturn n","reject"),
    ("match(1){ case 1{ return 0 } case 0x1{ return 1 } }\nreturn 0","reject"),
    ("match(1){ case true{ return 1 } }\nreturn 0","reject"),
]])

counter = "share i:int = 40\nfn next()->int{ return i+=1 }"
verify(cli, [
    ("import counter\n" + main("const first:int = counter.next()\nreturn counter.i + counter.next() - first"),42),
    ("import counter\n" + main("match(counter.next()){ case 0{ return 0 } case 41{ return counter.i + 1 } case _{ return 0 } }"),42),
    ("import i from counter as number\n" + main("return number+=1"),"reject"),
], {"counter.voil": counter})

verify(cli, [
    ("share start:int = -0x8000000000000000\n" + main("return start + 9223372036854775807 + 43"),42),
    ("share start:int = -9223372036854775809\n" + main("return start"),"reject"),
    ("import float\n" + main("const x:float = float.nan()\nreturn 0"),"reject"),
], {"float.voil":"fn marker()->int{ return 42 }"})
