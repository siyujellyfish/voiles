"""透過公開入口比對直譯結果、原生產物與拒絕案例。"""
import ctypes.util
import os
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
    dep.write_text('struct Box{\nvar value:int = 5\nvar label:str = "甲"\n}', encoding="utf-8")
    receiver = base / "receiver.voil"
    owner = "import Box from dep as B\n"
    main.write_text(owner + "import receiver\nfn main()->int{\nvar a:B = B()\nreturn receiver.take(a.move())\n}", encoding="utf-8")
    for index, expr in enumerate(["a.value", "a.value + 9223372036854775807"]):
        receiver.write_text(owner + f"fn take(a:B)->int{{ return {expr} }}", encoding="utf-8")
        exe = base / f"owner{index}"
        checked = run(cli, "run", main)
        built = run(cli, "build", main, "-o", exe)
        assert built.returncode == 0, built
        result = run(exe)
        assert result.returncode == (5 if index == 0 else 1) and result.stderr == checked.stderr, result
        sanitizer = ctypes.util.find_library("lsan")
        if os.environ.get("GITHUB_ACTIONS"):
            assert sanitizer, "CI 需要 LeakSanitizer"
        if sanitizer:
            memory = subprocess.run([exe], capture_output=True, encoding="utf-8",
                                    env={**os.environ, "LD_PRELOAD": sanitizer,
                                         "LSAN_OPTIONS": "exitcode=77"}, timeout=45)
            traced = "LeakSanitizer does not work under ptrace" in memory.stderr
            if traced and not os.environ.get("GITHUB_ACTIONS"):
                print("本機環境限制 ptrace，交由 CI 執行 LeakSanitizer")
            else:
                assert (memory.returncode, memory.stderr) == (result.returncode, result.stderr), memory
    invalid_move = main.read_text(encoding="utf-8").replace("take(a.move())", "take(a.move()) + a.value")
    main.write_text(invalid_move, encoding="utf-8")
    bad = run(cli, "build", main, "-o", base / "invalid-move")
    assert bad.returncode and not (base / "invalid-move").exists(), bad
    dep.write_text('share msg:str = "A💻"\n', encoding="utf-8")
    main.write_text('import dep\nfn main()->int{ return dep.msg[1][0] }', encoding="utf-8")
    exe = base / "unicode"
    built = run(cli, "build", main, "-o", exe)
    assert built.returncode == 0, built
    assert run(exe).returncode == int(run(cli, "run", main).stdout) == 240

    # 將唯一封裝產物移入空根目錄。測試必須實際跨越檔案系統邊界。
    clean_root = base / "root"
    clean_root.mkdir()
    packed = clean_root / "app"
    packed_result = run(cli, "pack", main, "-o", packed)
    assert packed_result.returncode == 0, packed_result
    elf = run("readelf", "-lWd", packed)
    assert elf.returncode == 0 and "INTERP" not in elf.stdout and "(NEEDED)" not in elf.stdout, elf
    assert list(clean_root.iterdir()) == [packed]
    if os.environ.get("GITHUB_ACTIONS"):
        isolated = run("sudo", "-n", "chroot", clean_root, "/app")
    else:
        isolated = run("bwrap", "--ro-bind", clean_root, "/", "--chdir", "/", "--", "/app")
    assert (isolated.returncode, isolated.stdout, isolated.stderr) == (240, "", ""), isolated

    original = packed.read_bytes()
    collision = run(cli, "pack", main, "-o", packed)
    assert collision.returncode and packed.read_bytes() == original, collision
    dep.unlink()
    missing = run(cli, "pack", main, "-o", base / "missing")
    assert missing.returncode and "匯入失敗" in missing.stderr and not (base / "missing").exists(), missing
    main.write_text("fn main()->int{ return 5 }", encoding="utf-8")
    # shell 由 exec 替換成 CLI，兩者 PID 相同，可重現先前中斷留下的同名目錄。
    retry = run("sh", "-c",
                'mkdir "$1/.voiles-build-$$" && exec "$2" build "$3" -o "$1/retry"',
                "sh", base, cli, main)
    assert retry.returncode == 0 and run(base / "retry").returncode == 5, retry
    leftovers = list(base.glob(".voiles-build-*"))
    assert len(leftovers) == 1 and leftovers[0].is_dir(), leftovers
    no_host = subprocess.run([cli, "pack", main, "-o", base / "no-host"], capture_output=True,
                             encoding="utf-8", env={**os.environ, "PATH": str(clean_root)})
    assert no_host.returncode and "rustc" in no_host.stderr and not (base / "no-host").exists(), no_host
