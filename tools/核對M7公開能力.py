"""依版本矩陣實跑舊式檢查及四個公開命令，並啟動原生產物。"""
import json
import subprocess
import sys
from pathlib import Path
from tempfile import TemporaryDirectory

ROOT = Path(__file__).resolve().parents[1]
cli = Path(sys.argv[1]).resolve(strict=True)
matrix = json.loads((ROOT / "tests/m7/preview1.json").read_text(encoding="utf-8"))


def invoke(*args):
    return subprocess.run(args, encoding="utf-8", capture_output=True, timeout=40)


with TemporaryDirectory() as directory:
    root = Path(directory)
    for case in matrix:
        path = root / "main.voil"
        path.write_text(case["source"], encoding="utf-8")
        for mode in ["legacy", "check", "run", "build", "pack"]:
            exe = root / mode
            args = [cli] + ([] if mode == "legacy" else [mode]) + [path]
            if mode in ["build", "pack"]:
                args += ["-o", exe]
            result = invoke(*args)
            expected = case.get(mode, case["module"])
            if not expected:
                assert result.returncode == 1 and result.stderr and not result.stdout, (case["id"], mode, result)
                assert not exe.exists()
            else:
                assert result.returncode == 0 and not result.stderr, (case["id"], mode, result)
                assert result.stdout == (f"{case['value']}\n" if mode == "run" else "")
                if mode in ["build", "pack"]:
                    native = invoke(exe)
                    assert (native.returncode, native.stdout, native.stderr) == (case["value"] % 256, "", "")
                    exe.unlink()
        print(case["id"], "通過")
