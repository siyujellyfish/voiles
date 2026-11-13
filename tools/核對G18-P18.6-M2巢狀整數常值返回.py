#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.6 M2：巢狀 true／else 真 int 常值返回與 BorrowTicket 安全來源。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_nested_constant_return_tests::"
CASES = (
    "m2_nested_constant_動態true只返回原Lexer常值41",
    "m2_nested_constant_動態false只返回原Lexer常值23",
    "m2_nested_constant_外層false未執行內層任何常值返回而落到尾端",
    "m2_nested_constant_true葉常值不執行else必溢位算式",
    "m2_nested_constant_false葉常值不執行true必溢位算式",
    "m2_nested_constant_內層原Lexer布林常值也只選真正返回葉",
    "m2_nested_constant_常值計畫篡改拒絕且注入故障零假返回",
    "m2_nested_constant_錯型別與不存在欄位仍由原Checker拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], capture_output=True,
            text=True, encoding="utf-8", check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1400:])
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError("M2 巢狀常值返回來源漂移：缺 "
                             + str(sorted(expected - actual))
                             + "；多 " + str(sorted(actual - expected)))
        for i, case in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            out = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", out,
            ):
                raise ValueError(f"M2 巢狀常值 {i}/8 失敗：{case}\n" + out[-3900:])
            print(f"M2 巢狀常值返回 {i}/8：{case}（通過）")
        print("G18 M2 巢狀雙葉 int 常值八項真安全 Gate 完成；M2 整體仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 M2 巢狀常值 Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
