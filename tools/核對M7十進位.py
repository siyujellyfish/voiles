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
