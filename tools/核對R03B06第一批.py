#!/usr/bin/env python3
"""R03 B06/F07 第一批：正式 Checker 後繼借用有效期原子驗證。"""
from __future__ import annotations
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "f07_b06_衝突的全域借用不得在最後使用點局部解鎖成假合法",
    "f07_b06_重複別名宣告身分不可靠提早歸還洗去衝突",
    "f07_b06_合法雙共享別名在最後使用點同次歸還且來源COW不變",
    "f07_b06_參數自身借用永遠不會因無後繼提早解除",
    "f07_b06_真正後繼引用借用別名不得讓入口安全狀態提前釋放",
)

def run(command: list[str]) -> str:
    result = subprocess.run(command, text=True, encoding="utf-8",
        errors="replace", capture_output=True, check=False)
    if result.returncode:
        raise AssertionError(f"真實 Rust --exact 回傳 {result.returncode}: {' '.join(command)}\n"
            + result.stdout[-1800:] + result.stderr[-1800:])
    return result.stdout + result.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("正式驗收必須附帶 --check")
    source = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    part = source.split("fn release_last_uses(", 1)
    if len(part) != 2:
        raise AssertionError("正式私有 Checker 沒有唯一最後使用點分析")
    method = part[1].split("/// 這裡僅核准完全唯讀", 1)[0]
    for marker in (
        "Self::active_loan_snapshot(vars).validate().is_err()",
        "let mut proposed = vars.clone();",
        "for (depth, scope) in proposed.iter_mut().enumerate()",
        "Self::active_loan_snapshot(&proposed).validate().is_ok()",
        "*vars = proposed",
        "loan.fact.source == declaration_start",
    ):
        if marker not in method:
            raise AssertionError(f"Checker 原子有效期權威缺失：{marker}")
    if "for (depth, scope) in vars.iter_mut().enumerate()" in method:
        raise AssertionError("最後使用點仍然在驗證前逐來源修改原狀態")
    listing = run([args.test_bin, "--list"])
    testnames = [line[:-6] for line in listing.splitlines() if line.endswith(": test")]
    seen = set()
    for suffix in CASES:
        found = [name for name in testnames if name.endswith("::" + suffix)]
        if len(found) != 1 or found[0] in seen:
            raise AssertionError(f"真 Rust 案例不唯一、為空或重複：{suffix}")
        name = found[0]
        seen.add(name)
        result = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in result:
            raise AssertionError(f"真 Rust --exact 未成功：{name}")
    print(f"R03 B06/F07 第一批：{len(seen)} 筆互異 Rust --exact 與有效期原子提交通過")
    print("F07 全路徑 use-def／F09 全操作／G3 仍待整包正式驗收")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as exc:
        print(f"F07 第一批失敗：{exc}", file=sys.stderr)
        sys.exit(1)
