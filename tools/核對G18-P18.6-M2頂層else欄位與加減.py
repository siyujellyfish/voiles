#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.6 M2：頂層 else 直接 self.int 欄位、加減、溢位及同票據清理。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_direct_else_self_return_tests::"
CASES = (
    "m2_direct_else_self_動態false原Checker欄位加法返回10",
    "m2_direct_else_self_動態true只計算原Checker真葉加法12",
    "m2_direct_else_self_動態false直接返回同張票據讀取的欄位7",
    "m2_direct_else_self_動態false原Checker減法返回負13",
    "m2_direct_else_self_真正抵達else溢位必Unwind不假Returned仍退票",
    "m2_direct_else_self_動態true完全跳過else必溢位算式",
    "m2_direct_else_self_已編else計算篡改拒絕且入口故障無假返回",
    "m2_direct_else_self_錯型別與錯欄位由原Checker拒絕",
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
            raise ValueError("M2 頂層 else 來源漂移：缺 "
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
                raise ValueError(f"M2 頂層 else {i}/8 失敗：{case}\n" + out[-3900:])
            print(f"M2 頂層 else 真返回 {i}/8：{case}（通過）")
        print("G18 M2 頂層 else 直接返回真執行八項 Gate 完成；M2 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 M2 頂層 else Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
