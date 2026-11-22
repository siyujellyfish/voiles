"""驗證結構整筆移交、純值複製、欄位可寫性與 UTF-8 位元組。"""
from pathlib import Path
import sys
from m7_public_cases import verify

cli = Path(sys.argv[1]).resolve(strict=True)
schema = 'struct Pair{\nvar n:int = 0\nvar b:bool = false\n}\n'
schema += 'struct Box{\nvar s:str = "甲"\nvar n:int = 0\nconst fixed:int = 9\n}\n'
cases = [
    ('var a:Pair = Pair(n=42,b=true)\nvar other:Pair = a\na.b = false\n'
     'if(other.b or a.b){ return other.n } else { return a.n + 1 }', 42),
    ('var a:Pair = Pair()\na.b = 1\nreturn 0', "reject"),
    ('var a:Pair = Pair()\na.n = 42\na.b = true\nvar b:Pair = a\nb.n = 0\nreturn a.n', 42),
    ('var a:Box = Box()\na.s = "*"\nvar b:Box = a.move()\nvar i:int = 0\nreturn b.s[i][0]', 42),
    ('const a:Box = Box()\nvar b:Box = a.move()\nb.n = 42\nreturn b.n', 42),
    ('var a:Box = Box()\nvar b:Box = a.move()\nreturn a.n', "reject"),
    ('var a:Box = Box()\nvar b:Box = a\nreturn b.n', "reject"),
    ('var a:Box = Box()\nvar b:Pair = a.move()\nreturn 0', "reject"),
    ('const a:Box = Box()\na.n = 1\nreturn 0', "reject"),
    ('var a:Box = Box()\na.fixed = 1\nreturn 0', "reject"),
    ('var a:Box = Box()\na.s = true\nreturn 0', "reject"),
    ('var a:Box = Box()\nreturn a.s[1][0]', "runtime"),
]
verify(cli, ((schema + 'fn main()->int{\n' + body + '\n}', value) for body, value in cases))
