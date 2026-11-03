#!/usr/bin/env python3
"""R03 B05／F08 第二批：真正私有 Checker 的唯讀安全狀態與原子分支合流。"""
from __future__ import annotations
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "f08_b05_同一安全視圖比較所有權借用與宣告能力且不複製帳簿",
    "f08_b05_非法借用來源合流拒絕且不修改入口作用域任何欄位",
    "f08_b05_合法分支候選完成全部驗證後才一次提交",
    "f08_b05_無效獨占共享合流不得污染已驗證的其它宣告",
)

def run(command: list[str]) -> str:
    result = subprocess.run(
        command, capture_output=True, text=True, encoding="utf-8",
        errors="replace", check=False,
    )
    if result.returncode:
        raise AssertionError(
            f"真 Rust 測試錯誤：{' '.join(command)}\n"
            + result.stdout[-2500:] + result.stderr[-2500:]
        )
    return result.stdout + result.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("正式真 Rust 核對必須指定 --check")
    core = (ROOT / "src/struct_stage.rs").read_text(encoding="utf-8")
    checker = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    for marker in (
        "struct SafetyStateView<",
        "fn safety_state(&self) -> SafetyStateView<",
        "self.safety_state() == other.safety_state()",
    ):
        if core.count(marker) != 1:
            raise AssertionError(f"F08 安全狀態唯讀單一比較權威缺失：{marker}")
    for marker in (
        "fn commit_reachable_safety_join(",
        "let mut proposed = snapshot.clone();",
        "Self::commit_reachable_safety_join(vars, proposed)",
    ):
        if checker.count(marker) != 1:
            raise AssertionError(f"F08 正式 Checker 原子合流權威缺失：{marker}")
    if "for (depth, scope) in vars.iter_mut().enumerate()" in checker:
        raise AssertionError("正式 Checker 仍可能在失敗前直接修改原 CFG 合流作用域")
    listing = run([args.test_bin, "--list"])
    names = [line[:-6] for line in listing.splitlines() if line.endswith(": test")]
    seen: set[str] = set()
    for suffix in CASES:
        found = [name for name in names if name.endswith("::" + suffix)]
        if len(found) != 1 or found[0] in seen:
            raise AssertionError(f"Rust 案例非空、非唯一或重複：{suffix}／{len(found)}")
        name = found[0]
        seen.add(name)
        result = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in result:
            raise AssertionError(f"沒有真正完成 --exact：{name}")
    print(f"R03 B05/F08 第二批：{len(seen)} 個互異真 Rust --exact 通過；分支合流全狀態原子提交")
    print("F08 尚須統一全部操作的唯一可寫狀態；G3/R03 不提前簽核")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as exc:
        print(f"F08 第二批驗收失敗：{exc}", file=sys.stderr)
        sys.exit(1)
