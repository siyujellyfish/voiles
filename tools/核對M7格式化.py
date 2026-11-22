from pathlib import Path
import sys

from m7_public_cases import verify


def program(template, result="s[-1][0]"):
    return (
        'fn main()->int{\nvar n:int = 42\nconst name:str = "若"\n'
        + 'const s:str = f"' + template + '"\nreturn ' + result + '\n}'
    )


verify(Path(sys.argv[1]).resolve(strict=True), [
    (program("{name:s}{{{n:d}}}{n:x}"), 97),
    (program("{name}", "s[0][0]"), 232),
    (program("{{{n}}}"), 125),
    (program("{missing}"), "reject"),
    (program("{name:d}"), "reject"),
    (program("{n:s}"), "reject"),
    (program("{n"), "reject"),
    (program("n}"), "reject"),
])

def scalars(declaration, index):
    return ('fn main()->int{\nvar n:float = 2.125\nvar b:bool = true\n'
            + declaration + '\nn = 3.0\nb = false\nreturn s[' + str(index) + '][0]\n}')


verify(Path(sys.argv[1]).resolve(strict=True), [
    (scalars('const s:str = t"{n:.2f}/{b}"',3), ord("2")),
    (scalars('const s:str = f"{n:.2f}/{b}"',5), ord("t")),
    (scalars('const s:str = f"{n}"',4), ord("5")),
    (scalars('var s:str = t"{b}"',0),"reject"),
    (scalars('const s:str = f"{b:x}"',0),"reject"),
])

verify(Path(sys.argv[1]).resolve(strict=True), [
    (program("{(n+1)-2:x}"),ord("9")),
    (scalars('const s:str = f"{n + 0.25:.2f}"',3),ord("8")),
    (program("{n + 9223372036854775807}"),"runtime"),
    (program("{n + true}"),"reject"),
])

verify(Path(sys.argv[1]).resolve(strict=True), [
    ('fn main()->int{ var text:str = "a"\nfor i in 0..3{ const digit:str = f"{i+1}"\n'
     'text[0] = digit }\nreturn text[0][0] }',ord("3")),
    ('fn main()->int{ var n:int = 0\nwhile(n != 2){ const t:str = t"{n}"\n'
     'n = t.to_int()+1 }\nreturn n }',2),
])
