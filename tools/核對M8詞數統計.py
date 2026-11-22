"""從公開入口及原生產物驗證詞數報表與專案自己的測試。"""
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory

cli = Path(sys.argv[1]).resolve(strict=True)
project = Path(__file__).resolve().parents[1] / "examples" / "詞數統計"


def run(*args):
    return subprocess.run(args, capture_output=True, encoding="utf-8", timeout=60)


def expect(result, code, output):
    assert (result.returncode, result.stdout, result.stderr) == (code, output, ""), result


with TemporaryDirectory() as temp:
    root = Path(temp)
    for name in ("main", "test"):
        source = project / (name + ".voil")
        expect(run(cli, "check", source), 0, "")
        executables = []
        for mode in ("build", "pack"):
            exe = root / (name + "-" + mode)
            expect(run(cli, mode, source, "-o", exe), 0, "")
            executables.append(exe)
        if name == "test":
            expect(run(cli, "run", source), 0, "0\n")
            for exe in executables:
                expect(run(exe), 0, "")
            continue
        inputs = [("", 0), (" \t\r\n", 0), ("甲乙 🪼 é", 3), ("字 " * 300, 300)]
        data = root / "輸入.txt"
        for content, count in inputs:
            data.write_text(content, encoding="utf-8")
            code = 0
            report = f"{count}\n"
            expect(run(cli, "run", source, "--", data), 0, report + f"{code}\n")
            for exe in executables:
                expect(run(exe, data), code, report)
        missing = root / "缺檔"
        failure = run(cli, "run", source, "--", missing)
        assert failure.returncode == 1 and not failure.stdout and "main.voil:" in failure.stderr, failure
        for exe in executables:
            actual = run(exe, missing)
            assert (actual.returncode, actual.stdout, actual.stderr) == (1, "", failure.stderr), actual
