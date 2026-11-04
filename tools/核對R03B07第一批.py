#!/usr/bin/env python3
"""R03 B07/F10 第一批：for／while／break／continue 唯一外層安全不變量。"""
from __future__ import annotations
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "f10_b07_全部外層安全欄位同源時共通反向邊核對成功",
    "f10_b07_反向邊變動ValueSlot必定被同一契約拒絕",
    "f10_b07_反向邊變動借用有效期必定拒絕且不污染入口",
    "f10_b07_跳轉遺失原外層宣告應保留獨立失效分類",
    "f10_b07_真正私有for迴圈中Copyable賦值可安全保留原安全狀態",
)

def run(command: list[str]) -> str:
    process = subprocess.run(command, encoding="utf-8", errors="replace",
        text=True, capture_output=True, check=False)
    if process.returncode:
        raise AssertionError(f"Rust 真測試失敗 {process.returncode}: {' '.join(command)}\n"
            + process.stdout[-2400:] + process.stderr[-2400:])
    return process.stdout + process.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("正式驗收必須使用 --check")
    core = (ROOT / "src/struct_stage.rs").read_text(encoding="utf-8")
    checker = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    for marker in ("enum OuterSafetyDiff {", "MissingBinding,", "ChangedState,"):
        if core.count(marker) != 1:
            raise AssertionError(f"迴圈結果種類權威不是唯一：{marker}")
    if checker.count("fn checked_outer_state_stability(") != 1:
        raise AssertionError("缺少唯一反向邊狀態核對器")
    if checker.count("Self::checked_outer_state_stability(") != 3:
        raise AssertionError("break／continue、for、while 三條真 Checker 路徑未共用同一核對器")
    for marker in (
        "OuterSafetyDiff::MissingBinding",
        "OuterSafetyDiff::ChangedState",
        "original.same_safety_state(candidate)",
    ):
        if marker not in checker:
            raise AssertionError(f"缺少原有安全／診斷契約：{marker}")
    listing = run([args.test_bin, "--list"])
    names = [line[:-6] for line in listing.splitlines() if line.endswith(": test")]
    used = set()
    for suffix in CASES:
        matches = [name for name in names if name.endswith("::" + suffix)]
        if len(matches) != 1 or matches[0] in used:
            raise AssertionError(f"Rust 真測試非空、唯一或重複核對失敗：{suffix}")
        name = matches[0]
        used.add(name)
        result = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in result:
            raise AssertionError(f"--exact 未真正成功：{name}")
    print(f"R03 B07/F10 第一批：{len(used)} 個互異真 Rust --exact 成功；三種轉移共用唯一核對器")
    print("F10 全 CFG／不動點與 G4/G5/G6 尚待獨立正式驗收")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as error:
        print(f"F10 第一批驗收失敗：{error}", file=sys.stderr)
        sys.exit(1)
