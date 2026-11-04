#!/usr/bin/env python3
"""R03 B10/F13：Checker 分支三維安全狀態僅經單一 SafetyState Transition。"""
from __future__ import annotations
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "f13_b10_缺少借用來源時合流整筆失敗且值與欄位完全不變",
    "f13_b10_偽造借用活性不可在無LoanBinding的持有者上寫入",
    "f13_b10_合法分支合流同次更換值欄位與借用有效期",
    "f13_b10_沒有借用的分支合流保持唯一靜態狀態儲存",
)

def run(command: list[str]) -> str:
    result = subprocess.run(command, text=True, encoding="utf-8",
        errors="replace", capture_output=True, check=False)
    if result.returncode:
        raise AssertionError(f"真 Rust --exact 失敗：{' '.join(command)}\n"
            + result.stdout[-2000:] + result.stderr[-2000:])
    return result.stdout + result.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("正式驗收必須指定 --check")
    core = (ROOT / "src/struct_stage.rs").read_text(encoding="utf-8")
    checker = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    for marker in (
        "StaticTransition::JoinReachable {",
        "if loan_active.is_some() != self.loan.is_some()",
        "self.pending_fields = pending_fields;",
        "fn commit_reachable_join(",
        "self.safety.transition(StaticTransition::JoinReachable {",
    ):
        # 單一正式變體在 match 的實作處與 Binding 的呼叫處各出現一次；
        # 兩者不是兩個獨立的可寫權威。
        expected = 2 if marker == "StaticTransition::JoinReachable {" else 1
        if core.count(marker) != expected:
            raise AssertionError(f"F13 CFG 三維安全轉移實作／消費數量異常：{marker}")
    if checker.count("original.commit_reachable_join(") != 1:
        raise AssertionError("真 Checker 尚未接線唯一 JoinReachable")
    for old in ("original.value =", "original.pending_fields =", "loan.active = true_active || false_active;"):
        if old in checker:
            raise AssertionError(f"CFG 合流仍保留可獨立改變安全結果的舊直接寫入：{old}")
    listing = run([args.test_bin, "--list"])
    names = [line[:-6] for line in listing.splitlines() if line.endswith(": test")]
    seen = set()
    for suffix in CASES:
        matched = [name for name in names if name.endswith("::" + suffix)]
        if len(matched) != 1 or matched[0] in seen:
            raise AssertionError(f"真 Rust 案例缺失、重複或未唯一：{suffix}")
        name = matched[0]
        seen.add(name)
        result = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in result:
            raise AssertionError(f"未真正通過 --exact：{name}")
    print(f"R03 B10/F13 第一批：{len(seen)} 個獨立真 Rust --exact 及分支安全權威唯一性通過")
    print("F13 雙權威根因／F06～F12 完整義務與 G5/G6 仍須後續正式驗收")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as error:
        print(f"F13 第一批失敗：{error}", file=sys.stderr)
        sys.exit(1)
