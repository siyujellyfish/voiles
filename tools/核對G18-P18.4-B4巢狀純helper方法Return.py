#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4/M3：同步 self Return 有限層純 int helper 八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_nested_pure_helper_method_return_tests::"
CASES = (
    "b4_return_nested_借self後內層sub先返回再進外層add最後還票",
    "b4_return_nested_反向函式身份與回傳計畫不能互換",
    "b4_return_nested_三層不同函式工作圖只取得一次self票",
    "b4_return_nested_條件true與false分別只執行選中巢狀Return",
    "b4_return_nested_內層本體溢位先Unwind內函式再歸還外方法票",
    "b4_return_nested_外層本體溢位保留已完成子函式真退出",
    "b4_return_nested_未選中溢位分支完全不進helper也不釋放假資源",
    "b4_return_nested_未證算術包裝資源與竄改任一子計畫先拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1500:])
        actual = {
            x.removesuffix(": test") for x in listing.stdout.splitlines()
            if x.startswith(PREFIX) and x.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError("B4/M3 巢狀 Return 清單不一致：缺 "
                + str(sorted(expected - actual)) + "；多 "
                + str(sorted(actual - expected)))
        for i,case in enumerate(CASES,1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX+case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"方法 Return 巢狀 {i}/8 {case} 失敗：\n"+output[-4000:])
            print(f"B4/M3 方法 Return 真退出 {i}/8：{case}（通過）")
        print("G18 B4/M3 原 Checker 巢狀方法 Return 八項真來源通過；B4/M3 仍未全閉合。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 方法 Return 巢狀 Gate 失敗："+str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
