from pathlib import Path
import sys

from m7_public_cases import verify


def main(body):
    return "fn main()->int{\n" + body + "\n}"


def converted(text, category):
    return (
        'fn convert()->int{\nconst text:str = "' + text + '"\nreturn text.to_int()\n}\n'
        + main("try{ return convert() } except " + category + "{ return 7 }")
    )


verify(Path(sys.argv[1]).resolve(strict=True), [
    (main("try{ return 42 } except EvaluationError{ return 1 }"), 42),
    (main("var x:int = 2\ntry{ x = x+40\nx = 9223372036854775807 + 1 } "
          "except EvaluationError{ return x }\nreturn 0"), 42),
    (converted("42", "TypeConversationError"), 42),
    (converted("水母", "TypeConversationError"), 7),
    (converted("水母", "EvaluationError"), "runtime"),
    (main("try{ try{ return 9223372036854775807+1 } except TypeConversationError{ return 1 } } "
          "except EvaluationError{ return 42 }"), 42),
    (main("try{ return 1 } except { return 2 }"), "reject"),
    (main("try{ return 1 } except Unknown{ return 2 }"), "reject"),
    (main("try{ return missing } except EvaluationError{ return 2 }"), "reject"),
])

verify(Path(sys.argv[1]).resolve(strict=True), [
    (main('var s:str = "錯"\nvar n:int = 0\ntry{ n = s.to_int() } '
          'except TypeConversationError{ s = "42"\nn = s.to_int() }\nreturn n'),42),
    (main('const s:str = "甲é*"\nvar i:int = 0\nwhile(i != len(s)){ '
          'if(s[i][0] != 42){ i = i + 1 } else { return 42 } }\nreturn 0'),42),
])

verify(Path(sys.argv[1]).resolve(strict=True), [
    (main('var count:int = 0\nfor i in 0..3{ const t:str = "*"\n'
          'try{ const bad:str = "x"\ncount = bad.to_int() } except TypeConversationError{ '
          'count = count + t[0][0] }\ncontinue }\nreturn count'),126),
    (main('if(true){ const t:str = "*" }\nreturn t[0][0]'),"reject"),
])

verify(Path(sys.argv[1]).resolve(strict=True), [
    (main('try{ return 1 / 0 } except EvaluationError as msg{ return len(msg) + 38 }'),42),
    (main('const s:str = "x"\ntry{ return s.to_int() } except TypeConversationError as msg{ return len(msg) + 36 }'),42),
    (main('try{ return 1 / 0 } except EvaluationError as msg{ msg = "x"\nreturn 0 }'),"reject"),
    (main('try{ const n:int = 1 } except EvaluationError as msg{ return 0 }\nreturn len(msg)'),"reject"),
])
