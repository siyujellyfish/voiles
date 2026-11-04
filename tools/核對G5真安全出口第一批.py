#!/usr/bin/env python3
"""G5 第一批：八 P16 受限真橋接不得在 Checker 控制返回／清理責任缺失下執行。"""
from __future__ import annotations
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "g5_真Checker的return與持有者責任核對後才容許資源橋接",
    "g5_沒有真控制邊或錯誤種類不得憑AST簽發資源事件",
    "g5_清理義務遺失或作用域筆數被偽造就不能進入動態橋接",
    "g5_借用尚有效時不能假定資源可在執行期立即釋放",
    "g5_非return的宣告或消失的快照不可冒充真實完成事件",
)

def run(cmd: list[str]) -> str:
    p = subprocess.run(cmd, cwd=ROOT, text=True, encoding="utf-8",
        errors="replace", capture_output=True, check=False)
    if p.returncode:
        raise AssertionError(f"Rust 真實執行失敗（{p.returncode}）: {' '.join(cmd)}\n"
            + p.stdout[-1900:] + p.stderr[-1900:])
    return p.stdout + p.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("必須指定 --check")
    bridge = (ROOT / "src/struct_stage/resource_trace.rs").read_text(encoding="utf-8")
    if bridge.count("fn require_checked_bridge_return(") != 1:
        raise AssertionError("無唯一真正 Checker 靜態 return 橋接入口")
    if bridge.count("require_checked_bridge_return(&control_edges, &exit_snapshots,") != 10:
        raise AssertionError("八條橋接及跨函式兩個 return 尚未全部接線")
    if bridge.count("closed_bridge_events(") != 6 or bridge.count("closed_bridge_result(") != 8:
        raise AssertionError("真 ResourceId 事件核帳路徑退步")
    if bridge.count("plan: VerifiedSafetyPlan { declarations, scalar_hir") != 8:
        raise AssertionError("八條受限橋接 Checker 原安全來源不再唯一")
    listing = run([args.test_bin, "--list"])
    available = [line[:-6] for line in listing.splitlines() if line.endswith(": test")]
    seen: set[str] = set()
    for suffix in CASES:
        matches = [name for name in available if name.endswith("::" + suffix)]
        if len(matches) != 1 or matches[0] in seen:
            raise AssertionError(f"G5 真 Rust 測試名稱缺失／重複：{suffix}")
        name = matches[0]
        seen.add(name)
        result = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in result:
            raise AssertionError(f"真 Rust --exact 失敗：{name}")
    print(f"G5 第一批：{len(seen)} 個互異真 Rust --exact Checker 靜態出口接通及偽證拒絕成功")
    print("本批不冒稱 F12/G5 正式通過；八橋接 15 路徑／57 真事件由原 G0 完整凍結另行核驗")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as error:
        print(f"G5 第一批拒絕：{error}", file=sys.stderr)
        sys.exit(1)
