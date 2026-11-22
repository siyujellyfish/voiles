"""驗證共享字串與借用別名的兩層動態索引。"""
from pathlib import Path
import sys
from m7_public_cases import verify

cli = Path(sys.argv[1]).resolve(strict=True)
prefix = 'share s:str = "甲éZ"\nshare t:str = ""\nfn main()->int{\n'
cases = [
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
