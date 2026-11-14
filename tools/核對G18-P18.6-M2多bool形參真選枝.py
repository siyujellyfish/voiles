#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6-M2/M3：原 Checker 多 bool 形參與混合 int 真選枝八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_m3_multiple_bool_checker_frames_tests::"
CASES = (
    "m2_bool_兩個獨立真bool都true只選中內層true_helper返回6",
    "m2_bool_外層true但第二個bool_false只選中else_helper返回2",
    "m2_bool_外層false不論內層真假都不進入任何普通helper",
    "m2_bool_多個bool與int倒置形參及第三個獨立bool均保持DeclId",
    "m2_bool_兩個不同bool選中巢狀獨占欄位更新仍只借還唯一票",
    "m2_bool_外層false時內層引數及函式本體溢位均不得提前執行",
    "m2_bool_真選中內層引數故障零Enter而本體故障內先外後",
    "m2_bool_原Checker錯型別缺bool及篡改多個bool形參身分一律拒絕",
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
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError(
                "M2/M3 多 bool Rust 測試漂移：缺 "
                + str(sorted(expected-actual)) + "；多 " + str(sorted(actual-expected))
            )
        for i, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"M2/M3 多 bool {i}/8 失敗：{case}\n" + output[-4000:])
            print(f"M2/M3 原 Checker 多 bool 值槽 {i}/8：{case}（通過）")
        print("M2/M3 原 Checker 多 bool／int 混合八項通過；M2/M3 原義務仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as e:
        print("G18 M2/M3 多 bool Gate 失敗：" + str(e), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
