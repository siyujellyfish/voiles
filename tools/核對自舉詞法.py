"""從四個公開入口比對自舉來源的每個記號與原始位元組範圍。"""
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory

root = Path(__file__).resolve().parents[1]
cli = Path(sys.argv[1]).resolve(strict=True)
entry = root / "bootstrap/lexer/main.voil"


def run(*args):
    result = subprocess.run(args, capture_output=True, timeout=90)
    if result.returncode:
        raise AssertionError((args, result.returncode, result.stderr.decode("utf-8", errors="replace")))
    return result.stdout


with TemporaryDirectory() as directory:
    output = Path(directory)
    oracle = output / "oracle"
    run("rustc", "--edition=2021", root / "tests/rust/bootstrap_tokens.rs",
        "--extern", f"voiles_core={cli.parent / 'libvoiles_core.rlib'}", "-o", oracle)
    run(cli, "check", entry)
    programs = []
    for mode in ("build", "pack"):
        exe = output / mode
        run(cli, mode, entry, "-o", exe)
        programs.append(exe)
    corpus = [root / "bootstrap/compiler.voil"]
    for index, data in enumerate((b"", b"fn abc()->int{ return 42 }",
                                 'a\r\n(\nb\r)\r甲'.encode("utf-8"),
                                 b'\xffa\xc0\xaf', b'"a\\\\" "b\\"c"')):
        path = output / str(index)
        path.write_bytes(data)
        corpus.append(path)
    for path in corpus:
        expected = run(oracle, path)
        arguments = [path]
        if path == corpus[0]:
            expected = b"".join(row for row in expected.splitlines(keepends=True)
                                if not row.startswith(b"7 "))
            arguments.append("--compact")
        command = "bootstrap-run" if path == corpus[0] else "run"
        assert run(cli, command, entry, "--", *arguments) == expected + b"0\n", path
        if path != corpus[0]:
            for exe in programs:
                assert run(exe, *arguments) == expected, (exe, path)
