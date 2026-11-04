#!/usr/bin/env python3
"""G5 第三批：實際選取 if 提前 return 與真正 Checker 靜態義務一致。"""
from __future__ import annotations
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "g5_實際提早return同一Checker靜態內外兩個Owner各自負清理責任",
    "g5_提前return的內層清理不可從函式最後return靜態責任假借",
    "g5_未執行的提前return不創造資源事件且失敗注入不改變逆序清理",
)

def run(cmd: list[str]) -> str:
    p = subprocess.run(cmd, cwd=ROOT, text=True, encoding="utf-8",
        errors="replace", capture_output=True, check=False)
    if p.returncode:
        raise AssertionError(f"真 Rust 批次回傳 {p.returncode}: {' '.join(cmd)}\n"
            + p.stdout[-2500:] + p.stderr[-2500:])
    return p.stdout + p.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("須 --check")
    bridge = (ROOT / "src/struct_stage/resource_trace.rs").read_text(encoding="utf-8")
    if bridge.count("actual_return @ Stmt {kind:StmtKind::Return(term),..}") != 1:
        raise AssertionError("動態實際選中早退語句的 Checker 來源缺失")
    if bridge.count("&exit_snapshots, actual_return") != 3:
        raise AssertionError("早退自己的控制邊／內外 Owner 沒有完整獨立核對")
    if bridge.count("return closed_bridge_events(events);") != 1:
        raise AssertionError("真提前退出不再使用唯一事件核帳入口")
    listed = run([args.test_bin, "--list"])
    available = [line[:-6] for line in listed.splitlines() if line.endswith(": test")]
    unique: set[str] = set()
    for suffix in CASES:
        matched = [name for name in available if name.endswith("::" + suffix)]
        if len(matched) != 1 or matched[0] in unique:
            raise AssertionError(f"真正 Rust 早退正反缺失或重複：{suffix}")
        unique.add(matched[0])
        result = run([args.test_bin, "--exact", matched[0], "--test-threads=1"])
        if "1 passed; 0 failed" not in result:
            raise AssertionError(f"真 Rust 早退 --exact 失敗：{suffix}")
    print(f"G5 真早退第三批：{len(unique)} 項互異 Rust --exact 來源／事件順序成功")
    print("只能表示已核准 P16 單層可達動態路徑；其它通用 CFG 執行仍不在 G5 範圍")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as error:
        print(f"G5 真早退拒絕：{error}", file=sys.stderr)
        sys.exit(1)
