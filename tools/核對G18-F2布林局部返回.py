#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 F2 bool-return 局部初始化與更新真執行七項正反 Gate。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f2_bool_local_tests::"
CASES = (
    "f2_bool_local_真bool返回初始化const局部再作條件選擇",
    "f2_bool_local_真bool返回更新可寫局部並讀取最新值",
    "f2_bool_local_未初始化var布林由真正子函式返回後才首次綁定",
    "f2_bool_local_雙層巢狀bool函式先返回才真建立局部值",
    "f2_bool_local_布林函式可直接返回另一個已核准的布林子函式",
    "f2_bool_local_資源持有者與bool局部順序清理不混淆",
    "f2_bool_local_整數冒充bool初始值由原checker拒絕",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run([str(binary), "--list"], capture_output=True,
            text=True, encoding="utf-8", check=False)
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1600:])
        actual = {line.removesuffix(": test") for line in listed.stdout.splitlines()
                  if line.startswith(PREFIX) and line.endswith(": test")}
        expected = {PREFIX + case for case in CASES}
        if actual != expected:
            raise ValueError("七項案例不同源，缺少：" + str(sorted(expected - actual))
                + "；多餘：" + str(sorted(actual - expected)))
        for n, case in enumerate(CASES, 1):
            r = subprocess.run([str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False)
            output = r.stdout + r.stderr
            if r.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                raise ValueError(f"F2 bool local {n}/{len(CASES)} 失敗：{case}\n" + output[-2400:])
            print(f"F2 bool 局部 {n}/{len(CASES)}：{case}（通過）")
        print("F2 bool 返回局部七項實證完成；F2 整項尚不可宣告 Closed")
        return 0
    except (OSError, ValueError) as error:
        print("G18 F2 bool 局部 Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
