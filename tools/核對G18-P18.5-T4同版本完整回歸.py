"""在同一 checkout 重跑原六階段相依驗收與所有已編譯的 Rust 案例。"""
import argparse
import os
from pathlib import Path
import re
import subprocess
import sys
from tempfile import TemporaryDirectory

ROOT = Path(__file__).resolve().parents[1]


def command(args):
    result = subprocess.run(args, cwd=ROOT, capture_output=True, encoding="utf-8")
    if result.returncode:
        raise SystemExit(f"{args} 驗收失敗\n{(result.stdout + result.stderr)[-4000:]}")
    return result.stdout


def main():
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--test-bin", type=Path, required=True)
    cli.add_argument("--probe", type=Path, default=ROOT/"target/p02/f00_probe")
    args = cli.parse_args()
    binary = str(args.test_bin.resolve(strict=True))
    probe = str(args.probe.resolve(strict=True))
    head = command(["git", "rev-parse", "HEAD"]).strip()
    if os.getenv("GITHUB_ACTIONS") == "true" and os.getenv("GITHUB_SHA") != head:
        raise SystemExit("本次 CI 與 checkout 提交不同")
    listed = command([binary, "--list"])
    names = {line.removesuffix(": test") for line in listed.splitlines() if line.endswith(": test")}
    full = command([binary, "--format=pretty"])
    passed = re.findall(r"^test ([^\r\n]+) \.\.\. ok$", full, flags=re.M)
    if (not names or set(passed) != names or len(passed) != len(names)
            or f"test result: ok. {len(names)} passed; 0 failed; 0 ignored;" not in full):
        raise SystemExit("完整 Rust 測試名稱、通過數或忽略數不符")
    print(f"完整 Rust 測試 {len(names)}/{len(names)}，零忽略；checkout={head}")
    def gate(name, *options):
        command([sys.executable, "-X", "utf8", "tools/"+name+".py", *options])
        print(f"{name} 通過")
    test = ("--test-bin", binary)
    gate("鎖定F00基準", "--probe", probe, "--check")
    for name in ("鎖定F00八橋接","鎖定F00原始24項","鎖定F00私有診斷"):
        gate(name, *test, "--check")
    gate("核對F00總驗收")
    with TemporaryDirectory() as output:
        for script in ("核對-P13-驗收證據.sh","核對-P14-P16-工程證據.sh"):
            command(["bash", "tools/"+script, binary, output])
            print(f"{script} 通過")
    gate("核對G6正式總驗收", *test, "--check")
    gate("核對G17正式總驗收", *test)
    for phase, count in ((1,"四"),(2,"四"),(3,"四"),(4,"五"),(6,"五")):
        gate(f"核對G18-P18.{phase}原始{count}義務正式驗收", *test, "--check")
    gate("核對G18-R06跨執行器正反矩陣", *test)
    gate("核對G18-R07正式狀態一致性", *test)
    gate("核對G18-V0原六項義務來源", *test)
    gate("核對G18-P18原義務同源狀態", "--check")
    if command(["git","rev-parse","HEAD"]).strip() != head:
        raise SystemExit("驗收期間 checkout 已變動")


if __name__ == "__main__":
    main()
