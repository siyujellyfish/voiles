#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.6-M2：原 Checker 巢狀 if 真選葉、未選中零效果和唯一 BorrowTicket 歸還。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_nested_self_if_tests::"
CASES = (
    "m2_nested_外層true內層true返回原Checker加法值且真歸還self",
    "m2_nested_外層true內層false返回原self欄位而不執行true加法",
    "m2_nested_外層false不進內層if且只求值最後return",
    "m2_nested_外層true內層true真溢位受控Unwind並真還票",
    "m2_nested_內層false跳過不可達溢位且外層false也不進內層",
    "m2_nested_方法入口故障不得假造巢狀條件或正常Returned",
    "m2_nested_竄改內層bool條件或返回來源跨度必在真資源前拒絕",
    "m2_nested_內層條件錯型別與欄位不存在仍由原Checker拒絕",
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
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError("M2 巢狀 if 真來源漂移：缺 "
                + str(sorted(expected-actual)) + " 多 "
                + str(sorted(actual-expected)))
        for index, case in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX+case, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"M2 巢狀 if 第 {index}/8 筆失敗：{case}\n" + output[-3900:]
                )
            print(f"M2 真巢狀 if {index}/8：{case}（通過）")
        print("G18 M2 原 Checker 巢狀 if 真分支八項完成；完整 M2 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as err:
        print("G18 M2 巢狀控制 Gate 失敗：" + str(err), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
