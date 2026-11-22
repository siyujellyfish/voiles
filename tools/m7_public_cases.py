"""以四個公開入口驗證完整來源，並執行原生及封裝產物。"""
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory


def verify(cli, cases):
    with TemporaryDirectory() as temporary:
        root = Path(temporary)
        for i, (program, expected) in enumerate(cases):
            source = root / "main.voil"
            source.write_text(program, encoding="utf-8")
            reference = None
            for mode in ["check", "run", "build", "pack"]:
                exe = root / f"{mode}{i}"
                args = [cli, mode, source] + (["-o", exe] if mode in ["build", "pack"] else [])
                result = subprocess.run(args, capture_output=True, encoding="utf-8", timeout=60)
                if expected == "reject":
                    assert result.returncode == 1 and result.stderr and not exe.exists(), (program, result)
                    continue
                if mode == "run" and expected == "runtime":
                    assert result.returncode == 1 and "main.voil:" in result.stderr, (program, result)
                    reference = result.stderr
                    continue
                assert result.returncode == 0, (program, result)
                if mode == "run":
                    assert result.stdout == f"{expected}\n", result
                if mode in ["build", "pack"]:
                    process = subprocess.run([exe], capture_output=True, encoding="utf-8", timeout=30)
                    assert (process.returncode, process.stderr) == (
                        (1, reference) if expected == "runtime" else (expected, "")), (program, process)
