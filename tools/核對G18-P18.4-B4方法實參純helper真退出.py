#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4／M3：原 Checker 方法位置直接純 helper 真求值與歸票 Gate。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_helper_method_argument_tests::"
CASES = (
    "b4_method_arg_直接純helper先進退再借self返回7",
    "b4_method_arg_兩個直接helper真求值完才取得唯一self票",
    "b4_method_arg_具名main局部及原Copyable算術實參真交付helper",
    "b4_method_arg_bool及int混合方法先普通helper再真選中條件葉",
    "b4_method_arg_呼叫者第二純int實參溢位不進helper亦不借self",
    "b4_method_arg_前一helper真完成後下一實參失敗不假進第二函式",
    "b4_method_arg_純helper本體溢位先退失敗函式且尚未借self",
    "b4_method_arg_巢狀或非int普通函式拒絕及竄改實參計畫防偽",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], capture_output=True,
            text=True, encoding="utf-8", check=False,
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1500:])
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError(
                "B4 方法實參 helper 測試漂移：缺 "
                + str(sorted(expected-actual)) + "；多 " + str(sorted(actual-expected))
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
                    f"B4 方法實參真 helper {index}/8 失敗：{name}\n" + output[-4000:]
                )
            print(f"B4/M3 真方法實參先普通 helper {index}/8：{name}（通過）")
        print("G18 B4/M3 方法位置純 helper 八項完成；B4/M3 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 B4/M3 方法 helper 實參 Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
