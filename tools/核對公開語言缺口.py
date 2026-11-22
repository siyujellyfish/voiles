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
