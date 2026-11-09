#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 V7：原 Checker 真 main 直接函式快路的動態 await 續行／取消四筆來源。"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

前綴 = "struct_stage::g18_v7_fastpath_graph_tests::"
案例 = (
    "g18_v7_e2_直接main呼叫原私有wrapper真UTF8幀可跨呼叫暫停恢復",
    "g18_v7_e2_快路多int實參沿原Checker宣告順序綁定並恢復await",
    "g18_v7_e3_直接main快路父資源暫停取消走原真scope退場",
    "g18_v7_e4_直接main快路await後原運算溢位不生成成功Returned",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    binary = str(args.test_bin.resolve())
    listed = subprocess.run([binary, "--list"], text=True, encoding="utf-8",
                            capture_output=True, check=False)
    if listed.returncode:
        print("V7 main 快路真測試清單失敗：" + listed.stderr,
              file=sys.stderr)
        return 1
    known = {line.split(": test", 1)[0]
             for line in listed.stdout.splitlines() if line.endswith(": test")}
    required = [前綴 + name for name in 案例]
    missing = [name for name in required if name not in known]
    if missing:
        print("V7 main 快路缺少真 Rust 測試：" + "、".join(missing),
              file=sys.stderr)
        return 1
    for name in required:
        run = subprocess.run([binary, "--exact", name], text=True,
                             encoding="utf-8", capture_output=True, check=False)
        if run.returncode or "1 passed; 0 failed;" not in run.stdout:
            print("V7 main 快路真驗收失敗：" + name + "\n"
                  + run.stdout + run.stderr, file=sys.stderr)
            return 1
    print(f"G18 V7 原 Checker 直接 main 快路真續行／取消："
          f"{len(required)}/{len(required)} --exact Rust 驗證成功；"
          "原 P18 六義務尚未正式結案。")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
