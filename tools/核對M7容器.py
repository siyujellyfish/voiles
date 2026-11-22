"""核對公開命令的持有者移動、唯讀元素及執行錯誤。"""
from pathlib import Path
from m7_public_cases import verify
import sys

cli = Path(sys.argv[1]).resolve(strict=True)
cases = [
    ('var a:list[Optional[int]] = [42]\nwith a[0] as x { const flag:bool = x == none\n'
     'if(not flag){ return x } }\nreturn 0', 42),
    ('var a:list[Optional[int]] = [42]\nwith a[0] as x { if(not (x == none)){ return x } }\nreturn 0', 42),
    ('var a:list[Optional[int]] = [42]\nwith a[0] as x { var flag:bool = x != none\nflag = true\n'
     'if(flag){ return x } }\nreturn 0', "reject"),
    ('var a:list[Optional[int]] = [42]\nwith a[0] as x { const flag:bool = x != none\n'
     'const copy:bool = flag\nif(copy){ return x } }\nreturn 0', "reject"),
    ('var a:list[Optional[int]] = [42]\nwith a[0] as x { const flag:bool = x != none\n'
     'var k:int = len(a)\nif(flag){ return x } }\nreturn 0', "reject"),
    ('var a:list[Optional[int]] = [42]\nwith a[0] as x { if(x != none and x != 0){ return 42 } }\nreturn 0', "reject"),
    ('var a:list[Optional[int]] = [42]\nwith a[0] as x { while(x != none){ return x } }\nreturn 0', "reject"),
    ('var a:list[int] = [1]\na.append(2) + 3\nreturn 0', "reject"),
    ('var a:list[int] = [42]\nfor i in 0..2 { with a[0] as x { if(i != 1){ continue } '
     'else { break } } }\na.append(0)\nwith a[0] as x { return x }', 42),
    ('var a:list[int] = []\nfor i in 0..3 { a.append(i + 40) }\nreturn a.pop()', 42),
    ('var d:dict{str,int} = {}\nd.set("x",1)\nd.set("x",42)\nwith d["x"] as x { return x }', 42),
    ('var a:list[Optional[int,str]] = []\na.append(none)\na.append(42)\n'
     'with a[1] as x { if(x != none){ if(x is int){ return x } } }\nreturn 0', 42),
    ('var a:list[Optional[str]] = ["*"]\nwith a[0] as x { if(x == none){ return 0 } '
     'else { return x[0][0] } }', 42),
    ('var a:list[list[int]] = [[42]]\nwith a[0] as row { with row[0] as n { return n } }', 42),
    ('var a:list[Optional[int]] = [42]\nwith a[0] as x { var b:list[Optional[int,bool]] = [x]\n'
     'with b[0] as y { if(y != none){ if(y is int){ return y } } } }\nreturn 0', 42),
    ('var a:list[Optional[int,str]] = [42]\nwith a[0] as x { var b:list[Optional[int,str]] = [x] }\nreturn 0', "reject"),
    ('var a:list[int] = [1]\nwith a[0] as x { a.append(2) }\nreturn 0', "reject"),
    ('const a:list[int] = []\na.append(2)\nreturn 0', "reject"),
    ('var a:list[int] = []\nreturn a.pop()', "runtime"),
    ('var a:list[int] = []\na.append(true)\nreturn 0', "reject"),
    ('var d:dict{int,int} = {}\nd.set(true,42)\nreturn 0', "reject"),
    ('var d:dict{str,Optional[str,bool]} = {"甲":""}\nwith d["甲"] as x { '
     'if(x == none){ return 1 } else { if(x is str){ return 42 } } }\nreturn 0', 42),
    ('var a:list[Optional[int]] = [1]\nwith a[0] as x { if(x != none){ '
     'var n:int = 1\nif(x is int){ return n } } }\nreturn 0', "reject"),
    ('var a:list[Optional[int,bool,str]] = [0,false,"",none]\nvar n:int = 0\n'
     'for i in 0..4 { with a[i] as x { if(x != none){ if(x is bool){ n = n + 40 } '
     'else if(x is int){ n = n + 2 } } } }\nreturn n', 42),
    ('var a:list[Optional[int]] = [1]\nwith a[0] as x { if(x is int){ return 1 } }\nreturn 0', "reject"),
    ('var a:list[Optional[int]] = [1]\nwith a[0] as x { if(x != none){ '
     'if(x is str){ return 1 } } }\nreturn 0', "reject"),
    ("var a:list[Optional[int,bool]] = [none,0,false]\nvar n:int = 0\n"
     "for i in 0..3 { with a[i] as x { if(x != none){ n = n + 21 } } }\nreturn n", 42),
    ('var d:dict{str,Optional[str]} = {"甲":none}\nwith d["甲"] as x { '
     'if(x == none){ return 42 } else { return 0 } }', 42),
    ("var a:list[int] = [42]\nwith a[0] as x { if(x != none){ return 1 } }\nreturn 0", "reject"),
    ("var a:list[Optional[int]] = [42]\nwith a[0] as x { return x }", "reject"),
    ("var a:list[Optional[int]] = []\nwith a[0] as x { if(x == none){ return 0 } }\nreturn 42", "runtime"),
    ("var a:list[int] = [42]\nif(true){\nvar a:list[int] = [0,1]\n"
     "var b:list[int] = a.move()\nif(len(b) != 2){ return 0 }\n}\n"
     "with a[0] as item { return item }", 42),
    ("var n:int = 0\nfor i in 0..3 {\nvar a:list[int] = [10,12,20]\n"
     "var b:list[int] = a.move()\nwith b[i] as item { n = n + item }\n}\nreturn n", 42),
    ("if(true){ var a:list[int] = [0] }\nreturn len(a)", "reject"),
    ("if(true){\nvar a:list[int] = [0]\nvar b:list[int] = a.move()\n"
     "return len(a)\n}\nreturn 42", "reject"),
    ("var a:list[int] = [10,32]\nvar b:list[int] = a.move()\n"
     "var n:int = 10\nwith b[len(b) - 1] as item { n = n + item }\nreturn n", 42),
    ('var d:dict{str,int} = {"甲":42}\nwith d["甲"] as item { return item }', 42),
    ("const a:list[Optional[int]] = [none,7]\nreturn len(a)", 2),
    ("var d:dict{bool,bool} = {true:false}\nwith d[true] as item {\n"
     "if(item){ return 0 } else { return 42 }\n}", 42),
    ("var a:list[int] = []\nwith a[0] as item { return item }", "runtime"),
    ('var d:dict{str,int} = {}\nwith d["甲"] as item { return item }', "runtime"),
    ("var a:list[int] = [1]\nvar b:list[int] = a.move()\nreturn len(a)", "reject"),
    ("var a:list[int] = [1]\nwith a[0] as item { item = 2 }\nreturn 0", "reject"),
    ("var a:list[int] = [1]\nwith a[0] as item { var b:list[int] = a.move() }\nreturn 0", "reject"),
    ("var a:list[int] = [true]\nreturn len(a)", "reject"),
]
verify(cli, (("fn main()->int{\n" + body + "\n}", expected) for body, expected in cases))

verify(cli, [
    ('fn id(n:int)->int{ return n }\nfn main()->int{ var a:list[Optional[int]] = [42]\n'
     'with a[0] as x { if(x != none){ var k:int = id(0)\nreturn x } }\nreturn 0 }', "reject"),
    ('fn id(n:int)->int{ return n }\nfn main()->int{ var a:list[Optional[int]] = [42]\n'
     'with a[0] as x { if(x != none){ var k:int = id(0)\nif(x != none){ return x } } }\nreturn 0 }', 42),
    ('fn main()->int{ var a:list[Optional[int]] = [42]\nwith a[0] as x { const flag:bool = x != none\n'
     'if(true){ var n:int = 1 }\nif(flag){ return x } }\nreturn 0 }', "reject"),
])

verify(cli, (("fn main()->int{\n" + body + "\n}", expected) for body,expected in [
    ("var x:Optional[int] = none\nx = 42\nif(x != none){ return x }\nreturn 0",42),
    ("var x:Optional[int] = 42\nconst y:Optional[int,bool] = x\nx = none\n"
     "if(y != none){ if(y is int){ return y } }\nreturn 0",42),
    ('const x:Optional[str,int] = "*"\nif(x != none){ if(x is str){ return x[0][0] } }\nreturn 0',42),
    ("var x:Optional[float] = 0.1 + 0.2\nif(x != none){ if(x != 0.3){ return 1 } else { return 42 } }\nreturn 0",42),
    ("var x:Optional[int] = 42\nconst flag:bool = x != none\nx = none\nif(flag){ return x }\nreturn 0","reject"),
    ("const x:Optional[int] = none\nx = 42\nreturn 0","reject"),
    ("var x:Optional[int] = false\nreturn 0","reject"),
]))

factory = """fn make()->list[int]{
var a:list[int] = [40,2]
return a.move()
}
fn pass(a:list[int])->list[int]{ return a.move() }
fn read(a:list[int],n:int)->int{
with a[0] as x { return x+n }
}
"""
verify(cli, ((factory + 'fn main()->int{\n' + body + '\n}', expected) for body,expected in [
    ("var a:list[int] = make()\nvar b:list[int] = pass(a.move())\nreturn read(n=2,a=b.move())",42),
    ("var a:list[int] = make()\nreturn read(a,2)","reject"),
    ("var a:list[int] = make()\nvar b:list[int] = pass(a.move())\nreturn len(a)","reject"),
    ("var a:list[bool] = [true]\nreturn read(a.move(),2)","reject"),
    ("var a:list[int] = []\nreturn read(a.move(),2)","runtime"),
]))

optional = """fn make(n:int)->Optional[int]{ return n }
fn use(x:Optional[int])->int{
if(x != none){ return x }
return 0
}
"""
verify(cli, [
    (optional+"fn main()->int{ var x:Optional[int] = make(42)\nreturn use(x) }",42),
    (optional+"fn main()->int{ return use(42) }",42),
    (optional+"fn main()->int{ return use(none) }",0),
    (optional+"fn main()->int{ return use(true) }","reject"),
    ('fn relay(v:Optional[str])->Optional[str]{ return v.move() }\n'
     'fn main()->int{ var x:Optional[str] = "*"\nvar y:Optional[str] = relay(x.move())\n'
     'if(y != none){ return y[0][0] }\nreturn 0 }',42),
])

verify(cli, [('fn main()->int{ var d:dict{str,int} = {}\nvar key:str = "甲"\n'
              'd.set(key,42)\nkey = "乙"\nd.set(key,2)\n'
              'with d["甲"] as a { with d[key] as b { return a + b - 2 } } }',42)])
