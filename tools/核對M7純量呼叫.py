from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory

cli = Path(sys.argv[1]).resolve(strict=True)


def invoke(*args):
    return subprocess.run(args, capture_output=True, encoding="utf-8", timeout=45)


with TemporaryDirectory() as temp:
    root = Path(temp)
    dep = root / "math.voil"
    dep.write_text("fn diff(left:int,right:int)->int{ return left - right }\n"
                   "fn overflow()->int{ return 9223372036854775807 + 1 }", encoding="utf-8")
    entry = root / "main.voil"
    for idx, (expr, value) in enumerate([
            ("diff(50,8)", 42), ("diff(right=8,left=50)", 42),
            ("diff(50,diff(right=2,left=10))", 42), ("diff(1,overflow())", None)]):
        entry.write_text(f"import diff from math as diff\nimport overflow from math as overflow\n"
                         f"fn main()->int{{ return {expr} }}", encoding="utf-8")
        assert invoke(cli, "check", entry).returncode == 0
        baseline = invoke(cli, "run", entry)
        if value is None:
            assert baseline.returncode == 1 and "math.voil:2:" in baseline.stderr, baseline
        else:
            assert (baseline.returncode, baseline.stdout) == (0, f"{value}\n"), baseline
        for mode in ["build", "pack"]:
            exe = root / f"{mode}{idx}"
            built = invoke(cli, mode, entry, "-o", exe)
            assert built.returncode == 0, built
            result = invoke(exe)
            assert (result.returncode, result.stderr) == (1 if value is None else value, baseline.stderr), result
    for expr in ["diff(7)", "diff(true,2)", "diff(nope=1,right=2)", "diff()"]:
        entry.write_text(f"import diff from math as diff\nfn main()->int{{ return {expr} }}", encoding="utf-8")
        for mode in ["check", "run", "build", "pack"]:
            exe = root / "invalid"
            result = invoke(cli, mode, entry, *(["-o", exe] if mode in ["build", "pack"] else []))
            assert result.returncode == 1 and result.stderr and not exe.exists(), result
