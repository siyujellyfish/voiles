#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.1-F2 已核准 bool-return 混合 str/int/bool 真借用／Move 八項安全門。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f2_bool_str_tests::"
CASES = (
    "f2_bool_str_共享借用bool返回後真歸票且持有者最後釋放",
    "f2_bool_str_明確Move只轉交一次真ResourceId且子框架負責釋放",
    "f2_bool_str_同一布林返回呼叫可混合兩str獨立借用與int_bool",
    "f2_bool_str_借用後續int求值溢位不可錯造函式成功或遺失借用",
    "f2_bool_str_移動後續int失敗不能提前轉交ResourceId",
    "f2_bool_str_不同來源Move與共享借用同批必維持各自責任",
    "f2_bool_str_使用str作bool型別實參仍由原Checker拒絕",
    "f2_bool_str_篡改來源ResourceId或BorrowTicket身分即使筆數平衡也拒絕",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run([str(binary), "--list"], text=True,
            encoding="utf-8", capture_output=True, check=False)
        if listing.returncode:
            raise ValueError("真 Rust --list 失敗：" + listing.stderr[-1500:])
        found = {line.removesuffix(": test") for line in listing.stdout.splitlines()
                 if line.startswith(PREFIX) and line.endswith(": test")}
        expected = {PREFIX + case for case in CASES}
        if found != expected:
            raise ValueError("F2 bool-str Rust 測試來源不一致；缺少 "
                             + str(sorted(expected-found))
                             + "；額外 " + str(sorted(found-expected)))
        for n, case in enumerate(CASES, 1):
            proc = subprocess.run(
                [str(binary), "--exact", PREFIX+case, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            output = proc.stdout + proc.stderr
            if proc.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                raise ValueError(f"F2 bool-str {n}/{len(CASES)} 失敗：{case}\n" + output[-2400:])
            print(f"F2 bool-str {n}/{len(CASES)}：{case}（真 Rust 通過）")
        print("F2 bool-str 八項全通過；未驗收其餘 F2 原義務不得提前 Closed。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 F2 bool-str Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
