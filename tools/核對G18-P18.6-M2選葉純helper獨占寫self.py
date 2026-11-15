#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6-M2／P18.4-B4：原 Checker if/else 選葉純 int helper 寫 self 十項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_selected_helper_self_write_tests::"
CASES = (
    "m2_helper_write_原if_true在self獨占借票後執行add並真寫22",
    "m2_helper_write_原if_false即便helper本體會溢位也不得進入",
    "m2_helper_write_真helper本體溢位先退子框架再歸self票",
    "m2_helper_write_真else選葉才啟動add並寫入24",
    "m2_helper_write_未選true仍執行else時僅進一份helper",
    "m2_helper_write_巢狀true及else分別唯一選葉且用不同helper",
    "m2_helper_write_外層false時兩個巢狀helper都不得提前執行",
    "m2_helper_write_原BinaryInt左端helper先真返回再加常值寫24",
    "m2_helper_write_兩層已核准helper依實參次序進退後寫入20",
    "m2_helper_write_非法型別未知函式與篡改封存ScalarHIR必先拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], capture_output=True, text=True,
            encoding="utf-8", check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1600:])
        found = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 10 or found != expected:
            raise ValueError(
                "M2/B4 選葉 helper Gate 清單不一致，缺 "
                + str(sorted(expected - found)) + "；多 "
                + str(sorted(found - expected))
            )
        for index, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"M2/B4 選葉 helper {index}/10 失敗：{case}\n"
                    + output[-4500:]
                )
            print(f"M2/B4 真選葉 helper {index}/10：{case}（通過）")
        print("M2/B4 真選中葉 helper RHS 十項安全 Gate 通過；原 M2/B4 未全項閉合。")
        return 0
    except (OSError, ValueError) as error:
        print("M2/B4 選葉 helper Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
