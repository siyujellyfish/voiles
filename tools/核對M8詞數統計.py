"""從公開入口及原生產物驗證詞數報表與專案自己的測試。"""
import os
from pathlib import Path
from shutil import copy2
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

    expect(run(cli, "test", project), 0, "")
    local = subprocess.run([cli, "test"], cwd=project, capture_output=True, encoding="utf-8", timeout=60)
    expect(local, 0, "")
    entry = root / "test.voil"
    for value in (1, 256, -1):
        expression = str(value) if value >= 0 else "0 - 1"
        entry.write_text(f"fn main()->int{{ return {expression} }}", encoding="utf-8")
        result = run(cli, "test", root)
        assert (result.returncode, result.stdout) == (1, "") and f"回傳 {value}" in result.stderr, result
    entry.write_text("fn main()->int{ return false }", encoding="utf-8")
    invalid = run(cli, "test", root)
    assert invalid.returncode == 1 and "test.voil:" in invalid.stderr, invalid
    entry.unlink()
    missing = run(cli, "test", root)
    assert missing.returncode == 1 and "test.voil" in missing.stderr, missing
    assert run(cli, "test", root, root).returncode == 2

    isolated = root / "隔離根目錄"
    isolated.mkdir()
    copy2(root / "main-pack", isolated / "word-count")
    data = isolated / "輸入.txt"
    data.write_text("甲乙 🪼 é", encoding="utf-8")
    if os.environ.get("GITHUB_ACTIONS"):
        isolate = ["sudo", "-n", "chroot", isolated]
    else:
        isolate = ["bwrap", "--ro-bind", isolated, "/", "--chdir", "/", "--"]
    expect(run(*isolate, "/word-count", "/輸入.txt"), 0, "3\n")
    for path in ("/缺檔", "/輸入.txt"):
        data.write_bytes(bytes([255]))
        result = run(*isolate, "/word-count", path)
        assert (result.returncode, result.stdout) == (1, "") and "main.voil:" in result.stderr, result
