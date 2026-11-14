#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.4-B4/M3：原 Checker 真 if 選中 helper 提前返回與未選葉零事件。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_if_selected_helper_return_tests::"
CASES = (
    "b4_selected_動態bool_true真進helper並提前Return5再還票",
    "b4_selected_動態bool_false跳過未選helper且返回self原欄位7",
    "b4_selected_未選true葉即便helper返回會溢位仍不得提前求值",
    "b4_selected_真選中helper返回溢位時內層先失敗再歸還self票",
    "b4_selected_真選中實參先溢位不得假造helper進入",
    "b4_selected_未選實參即使會溢位仍返回欄位且不進helper",
    "b4_selected_先獨占寫入self後選中helper返回與借用仍只一張票",
    "b4_selected_錯型別巢狀普通呼叫與竄改真選葉必拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], capture_output=True,
            text=True, encoding="utf-8", check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1500:])
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError(
                "B4 真 if helper Return 測試漂移：缺 "
                + str(sorted(expected - actual)) + "；多 "
                + str(sorted(actual - expected))
            )
        for index, name in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"B4 真 if helper 退出 {index}/8 失敗：{name}\n"
                    + output[-4000:]
                )
            print(f"B4 真選中葉純 helper Return {index}/8：{name}（通過）")
        print("G18 B4/M3 原 Checker if 真選中 helper 八項完成；B4/M3 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 B4 真 if helper Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
