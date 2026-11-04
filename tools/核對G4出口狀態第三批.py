#!/usr/bin/env python3
"""R03 G4 第三批：Checker 逐可達控制點保存所有權及借用靜態狀態。"""
from __future__ import annotations
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "g4_原Checker已驗證return會封存來源移動後的完整靜態狀態",
    "g4_兩條合法return分支各有獨立來源狀態而非假動態事件",
    "g4_刪除已驗證控制點任一持有者不得以空帳簿假裝退出已閉合",
    "g4_來源相同但型別偽造不許通過真Checker出口安全計畫",
    "g4_重複同一路徑狀態與無來源快照均不能冒充不同可達分支",
)

def run(argv: list[str]) -> str:
    proc = subprocess.run(argv, cwd=ROOT, encoding="utf-8", text=True,
        capture_output=True, errors="replace", check=False)
    if proc.returncode:
        raise AssertionError(f"Rust 真實執行失敗（{proc.returncode}）：{' '.join(argv)}\n"
            + proc.stdout[-2200:] + proc.stderr[-2200:])
    return proc.stdout + proc.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("需要 --check")
    core = (ROOT / "src/struct_stage.rs").read_text(encoding="utf-8")
    checker = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    for marker in (
        "struct VerifiedExitBinding {",
        "struct VerifiedExitSnapshot {",
        "exit_snapshots: BTreeMap<usize, Vec<VerifiedExitSnapshot>>",
        "fn exit_snapshots_are_consistent(",
        "scope_sizes: Vec<usize>",
        "&& self.exit_snapshots_are_consistent()",
        "plan.exit_snapshots = checker.recorded_exit_snapshots.into_inner()",
    ):
        if core.count(marker) != 1:
            raise AssertionError(f"真 Checker 靜態出口責任不存在或重複：{marker}")
    if checker.count("fn record_checked_control_snapshot(") != 1:
        raise AssertionError("獨立 Checker 出口快照來源不唯一")
    if checker.count("self.record_checked_control_snapshot(") != 2:
        raise AssertionError("Return 和 Break/Continue 原位控制出口快照未完整接線")
    listing = run([args.test_bin, "--list"])
    names = [line[:-6] for line in listing.splitlines() if line.endswith(": test")]
    seen: set[str] = set()
    for suffix in CASES:
        hits = [name for name in names if name.endswith("::" + suffix)]
        if len(hits) != 1 or hits[0] in seen:
            raise AssertionError(f"真 Rust --exact 測試無法唯一匹配：{suffix}")
        name = hits[0]
        seen.add(name)
        result = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in result:
            raise AssertionError(f"真 Rust 正反未通過：{name}")
    print(f"G4 出口安全狀態第三批：{len(seen)} 筆互異 Rust --exact 真正成功")
    print("G4 各可達返回的完整清理計畫與 G5 真資源對帳尚未整包簽核")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as error:
        print(f"G4 出口靜態狀態失敗：{error}", file=sys.stderr)
        sys.exit(1)
