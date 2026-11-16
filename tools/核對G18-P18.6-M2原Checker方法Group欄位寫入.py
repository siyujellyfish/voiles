#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6-M2：真原Checker self Group欄位寫入十三項。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_grouped_field_write_tests::"
CASES = (
    "m2_write_group_直接括號包覆原Checker方法int參數真寫欄位",
    "m2_write_group_直接多層括號整數常值只讀真Lexer值葉",
    "m2_write_group_直接多層括號包覆原Checker方法加減算式",
    "m2_write_group_直接各自括號包覆加減左右方法純值",
    "m2_write_group_直接單層合法整數零值保留真自借票",
    "m2_write_group_原Checker同步純helper在括號內仍用真CallExit",
    "m2_write_group_真if分支括號參數算術寫入只在選中true發生",
    "m2_write_group_真else分支多層括號常值寫入不執行未選true",
    "m2_write_group_true與else皆使用合法Group時兩條真路徑不混寫",
    "m2_write_group_直接非同步方法直接await也必取得唯一真self獨占票",
    "m2_write_group_直接括號算術溢位只報Runtime錯誤並歸唯一self票",
    "m2_write_group_靜態錯型別或消耗self不得因括號被放行",
    "m2_write_group_已驗證直接欄位括號Token被竄改執行前必拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        test_bin = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(test_bin), "--list"], capture_output=True, check=False,
            text=True, encoding="utf-8",
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1800:])
        expected = {PREFIX + name for name in CASES}
        actual = {
            line.removesuffix(": test")
            for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 13 or actual != expected:
            raise ValueError("M2 Group欄位寫入缺=" + str(sorted(expected - actual))
                             + "；多=" + str(sorted(actual - expected)))
        for index, name in enumerate(CASES, 1):
            result = subprocess.run(
                [str(test_bin), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, check=False,
                text=True, encoding="utf-8",
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"M2 Group寫入 {index}/13 {name} 失敗\n" + output[-4200:]
                )
            print(f"M2 Group欄位寫入 {index}/13：{name}（通過）")
        print("M2 原Checker Group欄位寫入十三項Gate通過；P18未提前Passed。")
        return 0
    except (ValueError, OSError) as error:
        print("M2 原Checker Group欄位寫入Gate失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
