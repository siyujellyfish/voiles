#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4／M3：原 Checker 巢狀 if 雙葉 helper 共通退出 Gate。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_nested_if_helper_return_tests::"
CASES = (
    "b4_nested_外層真內層bool_true唯一helper返回7再歸還票",
    "b4_nested_內層bool_false僅選else_helper返回負4",
    "b4_nested_外層bool_false完全跳過內層兩個helper",
    "b4_nested_選中true時未選else的實參即使溢位亦零事件",
    "b4_nested_選中else時未選true的實參溢位也不可提前求值",
    "b4_nested_選中else第二實參溢位不能進helper且唯一Unwind",
    "b4_nested_雙葉可獨占寫入但僅被選一葉且真helper先退出再還票",
    "b4_nested_選中helper溢位內後外退出且篡改else來源必拒絕",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        executable = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(executable), "--list"], capture_output=True,
            text=True, encoding="utf-8", check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1500:])
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError(
                "巢狀 if helper Rust 測試漂移：缺 "
                + str(sorted(expected - actual)) + "；多 " + str(sorted(actual - expected))
            )
        for i, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(executable), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"巢狀雙葉 helper {i}/8 失敗：{case}\n" + output[-4000:]
                )
            print(f"B4／M3 巢狀選中 Return {i}/8：{case}（通過）")
        print("B4／M3 巢狀雙葉 helper 八項通過；B4/M3 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 巢狀雙葉 helper Gate 失敗：" + str(exc), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
