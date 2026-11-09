#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 V7 D18-01：每份原 Checker 工作跨 await 的累積 D18 預算五項正反 Gate。"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

前綴 = "struct_stage::g18_v7_cumulative_budget_tests::"
案例 = (
    "g18_v7_d1801_原真已執行工作跨await總步數不因wake重置",
    "g18_v7_d1801_兩個真正await的第二個安全點與第三次poll仍共用總步數",
    "g18_v7_d1801_同一原Checker來源兩份FIFO工作各自獨立預算與資源帳",
    "g18_v7_d1801_零環境預算與未知登錄身分不消耗FIFO工作ID",
    "g18_v7_d1801_函式框架環境預算限制在真正await前拒絕且釋放父資源",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    binary = str(args.test_bin.resolve())
    listed = subprocess.run([binary, "--list"], capture_output=True, text=True,
                            encoding="utf-8", check=False)
    if listed.returncode:
        print("G18 V7 D18-01 真 Rust 清單失敗："
              + listed.stdout + listed.stderr, file=sys.stderr)
        return 1
    available = {line.split(": test", 1)[0] for line in listed.stdout.splitlines()
                 if line.endswith(": test")}
    required = [前綴 + case for case in 案例]
    missing = [case for case in required if case not in available]
    if missing:
        print("G18 V7 D18-01 五項真來源缺漏："
              + "、".join(missing), file=sys.stderr)
        return 1
    for name in required:
        run = subprocess.run([binary, "--exact", name], capture_output=True,
                             text=True, encoding="utf-8", check=False)
        if run.returncode or "1 passed; 0 failed;" not in run.stdout:
            print("G18 V7 D18-01 真預算 Gate 失敗：" + name + "\n"
                  + run.stdout + run.stderr, file=sys.stderr)
            return 1
    print(f"G18 V7 D18-01 真 CheckedTask 累積環境預算："
          f"{len(required)}/{len(required)} 逐項 --exact 通過；"
          "P18 原六項正式驗收尚未結案。")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
