"""在相同來源比較公開命令，輸出可重測的程序執行耗時（不含編譯）。"""
import argparse
import os
from pathlib import Path
import platform
from statistics import median
import subprocess
from tempfile import TemporaryDirectory
from time import perf_counter, monotonic


def report(message):
    print(message, flush=True)
    if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
        with Path(summary).open("a", encoding="utf-8") as stream:
            stream.write(f"- {message}\n")


def execute(*args):
    start = perf_counter()
    result = subprocess.run(args, capture_output=True, encoding="utf-8", timeout=60)
    return result, (perf_counter() - start) * 1000


def tree(depth, leaf):
    functions = [f"fn f0()->int{{ return {leaf} }}"]
    for i in range(1, depth + 1):
        functions.append(f"fn f{i}()->int{{ return f{i-1}() + f{i-1}() }}")
    return "\n".join(functions + [f"fn main()->int{{ return f{depth}() }}"])


def compare(cli, root, label, source, expected, runs):
    entry = root / "main.voil"
    entry.write_text(source, encoding="utf-8")
    exe = root / label
    build, _ = execute(cli, "build", entry, "-o", exe)
    assert build.returncode == 0, build
    times = [[], []]
    for _ in range(runs):
        interp, t1 = execute(cli, "run", entry)
        native, t2 = execute(exe)
        if isinstance(expected, int):
            assert (interp.returncode, interp.stdout, interp.stderr) == (0, f"{expected}\n", ""), interp
            assert (native.returncode, native.stdout, native.stderr) == (expected % 256, "", ""), native
        else:
            assert interp.returncode == native.returncode == 1 and expected in interp.stderr, (interp, native)
            assert (interp.stdout, interp.stderr) == (native.stdout, native.stderr), (interp, native)
        times[0].append(t1)
        times[1].append(t2)
    report(f"{label} 來源 {len(source.encode('utf-8'))} bytes，{runs} 次中位數："
          f"直譯 CLI {median(times[0]):.2f} ms，原生 {median(times[1]):.2f} ms")


def soak(cli, root, seconds):
    entry = root / "main.voil"
    source = tree(14, "1")
    compare(cli, root, "持續執行", source, 16384, 1)
    deadline = monotonic() + seconds
    rounds = 0
    while monotonic() < deadline:
        interp, _ = execute(cli, "run", entry)
        native, _ = execute(root / "持續執行")
        assert (interp.returncode, interp.stdout, interp.stderr) == (0, "16384\n", ""), interp
        assert (native.returncode, native.stdout, native.stderr) == (0, "", ""), native
        rounds += 1
    report(f"雙模式持續測試 {seconds} 秒，{rounds} 輪")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cli", type=Path)
    parser.add_argument("--soak-seconds", type=int, default=60)
    args = parser.parse_args()
    cli = args.cli.resolve(strict=True)
    report(f"{platform.platform()} {execute('rustc', '--version')[0].stdout.strip()}")
    with TemporaryDirectory() as tmp:
        root = Path(tmp)
        compare(cli, root, "unicode", 'share msg:str = "' + "中" * 2048 + '"\n'
                + tree(8, "msg[2047][1]"), 184 * 256, 3)
        compare(cli, root, "深度限制", "fn main()->int{ return main() }", "執行步數已耗盡", 2)
        compare(cli, root, "步數限制", tree(16, "1"), "執行步數已耗盡", 3)
        compare(cli, root, "重複呼叫", tree(14, "1"), 2**14, 3)
        compare(cli, root, "索引錯誤", 'share s:str = "a"\nfn main()->int{ return s[2][0] }',
                "運算失敗", 2)

        compare(cli, root, "整串更新",
                'share s:str = "ab"\nfn main()->int{\nvar a:int = s[1][0]\ns = "中b"\nreturn a + s[1][0] }', 196, 2)
        compare(cli, root, "字素覆寫",
                'share s:str = "ab"\nfn main()->int{\nvar a:int = s[1][0]\ns[0] = "中"\nreturn a + s[1][0] }', 196, 2)
        compare(cli, root, "大量來源", " " * (512 * 1024) + "fn main()->int{ return 5 }", 5, 1)
        (root / "model.voil").write_text('struct Box{\nvar n:int = 3\nvar text:str = "甲"\n}', encoding="utf-8")
        (root / "worker.voil").write_text("import Box from model as B\nfn take(b:B)->int{ return b.n }", encoding="utf-8")
        repeated = ('import Box from model as B\nimport worker\nfn value()->int{\n'
                    'var b:B = B()\nreturn worker.take(b.move())\n}\n' + tree(12, "value()"))
        compare(cli, root, "重複資源移交", repeated, 3 * 4096, 2)
        soak(cli, root, args.soak_seconds)


if __name__ == "__main__":
    main()
