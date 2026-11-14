#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4／M3：方法局部直接 return 純 helper 的真動態退出。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_method_return_helper_exit_tests::"
CASES = (
    "b4_exit_方法直接return已核准雙int_helper結果7且真還self票",
    "b4_exit_方法已核准局部加減作多形參return_helper仍保留原位置",
    "b4_exit_方法var先重賦合法值然後真正return_helper用目前局部",
    "b4_exit_先helper初始化局部再return同helper應為兩次不同真框架",
    "b4_exit_return實參溢位不進普通helper但清理已初始化局部與self票",
    "b4_exit_helper本體溢位要先退出子框架再逆序離域及還票",
    "b4_exit_直接return零參數純helper亦需真呼叫及退出",
    "b4_exit_原Checker禁止return中隱藏函式呼叫型別錯配與計畫篡改",
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
                "B4 方法 return-helper 測試漂移：缺 "
                + str(sorted(expected - actual))
                + "；多 " + str(sorted(actual - expected))
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
                    f"B4 方法 return-helper {index}/8 失敗：{name}\n"
                    + output[-4000:]
                )
            print(f"B4 原 Checker 方法 Return 純 helper {index}/8：{name}（通過）")
        print("G18 P18.4-B4/M3 方法 Return 純 int helper 八項完成；B4/M3 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 B4 方法 Return helper Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
