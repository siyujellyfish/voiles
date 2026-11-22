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

pending = 'struct Later{\nvar n:int\nvar s:str\nvar active:bool = true\n}\n'
verify(cli, ((pending + 'fn main()->int{\n' + body + '\n}', value) for body, value in [
    ('var v:Later = Later()\nv.s = "*"\nv.n = 0\nvar done:Later = v.move()\nreturn done.s[0][0]',42),
    ('var v:Later = Later(n=40)\nv.s = "乙"\nv.n = v.n + 2\nreturn v.n',42),
    ('var v:Later = Later()\nv.n = 42\nreturn v.n',"reject"),
    ('var v:Later = Later()\nv.n = 1\nv.n = 42\nreturn 0',"reject"),
    ('var v:Later = Later()\nv.s = "*"\nvar bad:Later = v.move()\nreturn 0',"reject"),
    ('var v:Later = Later()\nv.s = 42\nreturn 0',"reject"),
    ('var v:Later = Later()\nv.s = "*"\nv.n = 9223372036854775807+1\nreturn 0',"runtime"),
]))

def factory(body, result="Box"):
    return schema + "fn make()->" + result + "{\n" + body + "\n}\n"


receive = "fn main()->int{ var value:Box = make()\nreturn value.s[0][0] }"
verify(cli, [
    (factory('var a:Box = Box(s="*")\nreturn a.move()') + receive,42),
    (factory('var a:Box = Box(s="*")\nreturn a') + receive,"reject"),
    (factory('var a:Pair = Pair()\nreturn a.move()') + receive,"reject"),
    (factory('var a:Box = Box(s="*")\nvar b:Box = a.move()\nreturn a.move()') + receive,"reject"),
    (factory('var a:Box = Box(s="*")\nvar n:int = 9223372036854775807+1\nreturn a.move()') + receive,"runtime"),
])

counter = """struct Counter{
var n:int = 0
fn add(by:int)->int{
self.n = self.n + by
return self.n
}
fn twice()->int{ return self.n + self.n }
}
"""
verify(cli, ((counter + 'fn main()->int{\n' + body + '\n}', expected) for body,expected in [
    ("var a:Counter = Counter()\nconst done:int = a.add(21)\nreturn a.twice()",42),
    ("const a:Counter = Counter(n=21)\nreturn a.twice()",42),
    ("const a:Counter = Counter()\nreturn a.add(42)","reject"),
    ("var a:Counter = Counter()\nreturn a.add(true)","reject"),
    ("var a:Counter = Counter(n=9223372036854775807)\nreturn a.add(1)","runtime"),
]))
