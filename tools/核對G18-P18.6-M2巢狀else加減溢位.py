#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.6 M2：內層 else 真加減、Unwind、唯一 BorrowTicket 與未選葉零求值。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_nested_else_arithmetic_tests::"
CASES = (
    "m2_nested_else_math_動態false才計算巢狀else加法並返回10",
    "m2_nested_else_math_動態true只走內層true返回12不執行else",
    "m2_nested_else_math_動態false以原Checker減法回傳負值",
    "m2_nested_else_math_動態false已到溢位必真Unwind不假Returned並歸票",
    "m2_nested_else_math_內層true完全不執行未選else的溢位算式",
    "m2_nested_else_math_外層false根本不到內層else即使其必溢位",
    "m2_nested_else_math_竄改else計算被SourceChanged拒絕且故障零假選葉",
    "m2_nested_else_math_非法bool加法及不存在欄位仍需原Checker拒絕",
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
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError("M2 巢狀 else 算術來源漂移：缺 "
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
                raise ValueError(f"M2 巢狀 else {i}/8 失敗：{case}\n" + out[-3900:])
            print(f"M2 巢狀 else 返回 {i}/8：{case}（通過）")
        print("G18 M2 巢狀 else 真返回八項安全 Gate 完成；M2 整體仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 M2 巢狀 else Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
