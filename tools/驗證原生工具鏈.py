"""透過公開入口比對直譯結果、原生產物與拒絕案例。"""
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory

cli = Path(sys.argv[1]).resolve(strict=True)


def run(*args):
    return subprocess.run(args, capture_output=True, encoding="utf-8", timeout=45)


with TemporaryDirectory() as tmp:
    base = Path(tmp)
    main = base / "入口.voil"
    dep = base / "dep.voil"
    dep.write_text("fn value()->int{ return 40 }", encoding="utf-8")
    cases = [
        ("import dep\nfn main()->int{ return dep.value() + 2 }", 42),
        ("fn main()->int{ var x:int = 5\nx = x - 9\nreturn x }", -4),
        ("fn main()->int{ if (false) { return 9223372036854775807 + 1 } else { return 1 } }", 1),
        ("fn main()->int{ return 9223372036854775807 + 1 }", None),
    ]
    for index, (source, expected) in enumerate(cases):
        main.write_text(source, encoding="utf-8")
        exe = base / f"程式 {index}"
        interpreted = run(cli, "run", main)
        build = run(cli, "build", main, "-o", exe)
        assert build.returncode == 0, build
        actual = run(exe)
        if expected is None:
            assert (actual.returncode, actual.stderr) == (1, interpreted.stderr) and actual.stderr, actual
        else:
            assert (interpreted.returncode, interpreted.stdout, interpreted.stderr) == (0, f"{expected}\n", ""), interpreted
            assert (actual.returncode, actual.stdout, actual.stderr) == (expected % 256, "", ""), actual
    main.write_text("fn main()->int{ return true }", encoding="utf-8")
    exe = base / "無效"
    bad = run(cli, "build", main, "-o", exe)
    assert bad.returncode and bad.stderr and not exe.exists(), bad
