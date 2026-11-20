#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.4-B4：原 Checker 同步方法原借票責任 7 項真 Rust 正反 Gate。"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

PREFIX = "struct_stage::g18_b4_obligation_tests::"
CASES = (
    "b4_01_兩輪獨占寫入及跨持有者共享讀取",
    "b4_02_方法參數借票與逐輪移交使用實際資源",
    "b4_03_同一來源借後移動與不可寫接收者均在靜態拒絕",
    "b4_04_第二輪子框架溢位截斷後繼共享呼叫",
    "b4_05_共用預算在借票前與持票後均安全退出",
    "b4_06_方法與參數還票次序交換須拒絕",
    "b4_07_原始來源及不同接收者呼叫位置不可互換",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], capture_output=True, text=True,
            encoding="utf-8", check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1500:])
        actual = {
            line.removesuffix(": test")
            for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if actual != expected:
            raise ValueError(
                "B4 來源清單不符，缺少：" + str(sorted(expected - actual))
                + "；未登錄：" + str(sorted(actual - expected))
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
                    f"B4 第 {index}/{len(CASES)} 項真執行失敗：{case}\n"
                    + output[-3000:]
                )
            print(f"B4 同步方法借票義務 {index}/{len(CASES)}：{case}（通過）")
        print("P18.4-B4 原 Checker→Runtime 七項獨立正反 Gate 真通過。")
        return 0
    except (OSError, ValueError) as error:
        print(f"G18 B4 真驗收失敗：{error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
