from pathlib import Path
import sys

from m7_public_cases import verify


def equal(expression, expected):
    return (
        "fn sum(a:float,b:float)->float{ return a + b }\n"
        + "fn main()->int{\nvar x:float = " + expression
        + "\nif(x != " + expected + "){ return 1 } else { return 42 }\n}"
    )


tiny = "0." + "0" * 397 + "1"
huge = "9" * 16 + "0" * 369 + ".0"
verify(Path(sys.argv[1]).resolve(strict=True), [
    (equal("sum(0.1,0.2)", "0.3"), 42),
    (equal("sum(1.234567890123456,0.0000000000000005)", "1.234567890123456"), 42),
    (equal("sum(1.234567890123455,0.0000000000000005)", "1.234567890123456"), 42),
    (equal("0.0 - 0.1 + 0.3", "0.2"), 42),
    (equal("sum(" + tiny + "," + tiny + ")", tiny[:-1] + "2"), 42),
    (equal("0." + "0" * 398 + "5", "0.0"), 42),
    (equal("sum(" + huge + "," + huge + ")", "0.0"), "runtime"),
    (equal("sum(1,0.2)", "1.2"), "reject"),
    (equal("1" + "0" * 385 + ".0", "0.0"), "reject"),
])

verify(Path(sys.argv[1]).resolve(strict=True), [
    ('fn main()->int{ if(-0.5 * -0.5 != 0.25){return 0}else{return 42} }',42),
    ('fn main()->int{ return -9223372036854775808 + 9223372036854775807 + 43 }',42),
    ('fn main()->int{ return -9223372036854775808 / -1 }',"runtime"),
    ('fn main()->int{ if(0.1 * 0.2 != 0.02){return 0}else{return 42} }',42),
    ('fn main()->int{ if(1.0 / 3.0 != 0.3333333333333333){return 0}else{return 42} }',42),
    ('fn main()->int{ var x:float = 1.0 / 0.0\nreturn 0 }',"runtime"),
    ('fn main()->int{ var x:float = 2.0 * 3\nreturn 0 }',"reject"),
])

verify(Path(sys.argv[1]).resolve(strict=True), [
    ('fn main()->int{ const x:float = float.nan()\nif(x != x){return 42}else{return 0} }',42),
    (equal("float.infinity()+1.0","float.infinity()"),42),
    ('fn main()->int{ const n:float = float.infinity()-float.infinity()\nif(n != n){return 42}else{return 0} }',42),
    (equal("-0.0","0.0"),42),
    ('fn main()->int{ const n:float = -0.0\nconst s:str = f"{n:.1f}"\nreturn s[0][0] }',45),
])
