"""核對公開命令的持有者移動、唯讀元素及執行錯誤。"""
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory

cli = Path(sys.argv[1]).resolve(strict=True)
cases = [
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
with TemporaryDirectory() as temporary:
    root = Path(temporary)
    for i, (body, expected) in enumerate(cases):
        source = root / "main.voil"
        source.write_text("fn main()->int{\n" + body + "\n}", encoding="utf-8")
        reference = None
        for mode in ["check", "run", "build", "pack"]:
            exe = root / f"{mode}{i}"
            args = [cli, mode, source] + (["-o", exe] if mode in ["build", "pack"] else [])
            result = subprocess.run(args, capture_output=True, encoding="utf-8", timeout=60)
            if expected == "reject":
                assert result.returncode == 1 and result.stderr and not exe.exists(), (body, result)
                continue
            if mode == "run" and expected == "runtime":
                assert result.returncode == 1 and "main.voil:" in result.stderr, (body, result)
                reference = result.stderr
                continue
            assert result.returncode == 0, (body, result)
            if mode == "run":
                assert result.stdout == f"{expected}\n", result
            if mode in ["build", "pack"]:
                process = subprocess.run([exe], capture_output=True, encoding="utf-8", timeout=30)
                assert (process.returncode, process.stderr) == (
                    (1, reference) if expected == "runtime" else (expected, "")), (body, process)
