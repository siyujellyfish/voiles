#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 V7 E3：原私有 Checker 已執行真動態幀安全取消五項正反。"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

前綴 = "struct_stage::g18_v7_real_graph_cancel_tests::"
案例 = (
    "g18_v7_e3_真父子函式已建兩份UTF8資源後取消須逆序真釋放",
    "g18_v7_e3_第一次真await已完成第二次暫停取消不得重執行第一個",
    "g18_v7_e3_尚未執行與已完成的真工作不能冒稱安全點取消",
    "g18_v7_e3_暫停後意外新增真BorrowTicket時取消拒絕並保留原框架",
    "g18_v7_e3_暫停取消只留下真退場事件不釋放未啟動async子函式資源",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    binary = str(args.test_bin.resolve())
    listed = subprocess.run([binary, "--list"], text=True,
                            encoding="utf-8", capture_output=True, check=False)
    if listed.returncode:
        print("G18 V7 E3 真 Rust 測試清單失敗："
              + listed.stdout + listed.stderr, file=sys.stderr)
        return 1
    present = {line.split(": test", 1)[0]
               for line in listed.stdout.splitlines()
               if line.endswith(": test")}
    names = [前綴 + case for case in 案例]
    missing = [name for name in names if name not in present]
    if missing:
        print("G18 V7 E3 真來源測試缺失：" + "、".join(missing),
              file=sys.stderr)
        return 1
    for name in names:
        run = subprocess.run([binary, "--exact", name], text=True,
                             encoding="utf-8", capture_output=True, check=False)
        if run.returncode or "1 passed; 0 failed;" not in run.stdout:
            print("G18 V7 E3 真框架取消驗收失敗：" + name + "\n"
                  + run.stdout + run.stderr, file=sys.stderr)
            return 1
    print(f"G18 V7 E3 真父子動態幀、P16／HC14 安全點取消："
          f"{len(names)}/{len(names)} 精確 Rust 驗收成功；"
          "async self 與統一工作佇列尚未完成。")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
