"""驗證共享字串與借用別名的兩層動態索引。"""
from pathlib import Path
import sys
from m7_public_cases import verify

cli = Path(sys.argv[1]).resolve(strict=True)
prefix = 'share s:str = "甲éZ"\nshare t:str = ""\nfn main()->int{\n'
cases = [
    ('var a:str = "甲乙"\nvar i:int = 0 - 1\nvar b:str = "*"\na[i] = b\nreturn a[i][0]', 42),
    ('var a:str = "甲乙"\nconst b:str = a[1] = "*"\na = "別"\nreturn b[0][0]', 42),
    ('var a:str = "甲"\nvar b:str = ""\na[0] = b\nreturn 0', "runtime"),
    ('var a:str = "甲"\nconst b:str = a.move()\na[0] = "*"\nreturn 0', "reject"),
    ('const a:str = "甲"\na[0] = "*"\nreturn 0', "reject"),
    ('var a:str = "甲*"\nvar b:str = a.move()\na = b[1]\nreturn a[0][0]', 42),
    ('var a:str = "é*"\nvar i:int = 0 - 1\nconst b:str = a[i]\na = "甲"\nreturn b[0][0]', 42),
    ('var a:str = "甲"\nvar b:str = "*"\na = b.move()\nreturn a[0][0]', 42),
    ('const a:str = "甲"\na = "*"\nreturn 0', "reject"),
    ('var a:str = "*"\nvar b:str = a.move()\nreturn a[0][0]', "reject"),
    ('var a:str = "*"\nvar b:str = a\nreturn 0', "reject"),
    ('var a:str = "*"\nvar i:int = 1\nconst b:str = a[i]\nreturn 0', "runtime"),
    ('const a:str = "甲"\nreturn a[0][9]', "runtime"),
    ('t = "*"\nvar i:int = 0\ns[i] = t\nreturn s[i][0]', 42),
    ('t = "ab"\ns[0] = t\nreturn 0', "runtime"),
    ('s[0] = t\nreturn 0', "runtime"),
    ('var n:int = 1\ns[0] = n\nreturn 0', "reject"),
    ("var i:int = 1\nvar j:int = 0 - 1\nreturn s[i][j]", 129),
    ("var i:int = 1\nt = s[i]\ns[i] = \"*\"\nreturn t[0][0] - s[i][0]", 59),
    ("var i:int = 0 - 1\nvar b:str = s.borrow()\nb[i] = \"*\"\nreturn b[i][0]", 42),
    ("var i:int = 3\nreturn s[i][0]", "runtime"),
    ("var j:int = 8\nreturn s[0][j]", "runtime"),
    ("var i:int = 3\ns[i] = \"*\"\nreturn 0", "runtime"),
    ("var i:bool = true\nreturn s[i][0]", "reject"),
    ("return s[unknown][0]", "reject"),
    ("var i:int = 0\nconst b:str = s.borrow()\nb[i] = \"*\"\nreturn 0", "reject"),
]
verify(cli, ((prefix + body + "\n}", result) for body, result in cases))

consume = 'fn read(value:str)->int{ return value[0][0] }\n'
forward = 'fn forward(value:str)->int{ return read(value.move()) }\n'
verify(cli, ((consume + forward + "fn main()->int{\n" + body + "\n}", result) for body, result in [
    ('var value:str = "*"\nreturn forward(value=value.move())', 42),
    ('var s:str = ""\nreturn forward(s.move())', "runtime"),
    ('var s:str = "*"\nvar n:int = read(s.move())\nreturn s[0][0]', "reject"),
    ('var s:str = "*"\nreturn read(s)', "reject"),
    ('var s:str = "*"\nreturn read(s.move()) + read(s.move())', "reject"),
]))
