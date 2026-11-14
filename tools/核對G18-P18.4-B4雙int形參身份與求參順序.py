#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4：原 Checker 雙 int 形參真 DeclarationId／求參順序 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_two_int_helper_argument_order_tests::"
CASES = (
    "b4_two_args_主程式依實參順序求已初始化seed及整數結果8",
    "b4_two_args_方法動態形參計算兩個不同實參後在self借用期間求差9",
    "b4_two_args_兩個不同DeclarationId作實參不可交換順序或把值當身份",
    "b4_two_args_方法var重賦以同一真局部作兩個Copyable引數原子提交",
    "b4_two_args_先初始化再使用第一次返回值作下一次雙參數Helper",
    "b4_two_args_第一或第二實參溢位均不得假造已進helper框架",
    "b4_two_args_兩參數都已綁定後返回溢位先退內層後還self票",
    "b4_two_args_三參數非法型別未知名稱與篡改雙DeclarationId必拒絕",
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
                "B4 雙參數 Rust 來源漂移：缺 "
                + str(sorted(expected - actual)) + "；多 " + str(sorted(actual - expected))
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
                    f"B4 雙 int 形參真求值 {index}/8 失敗：{name}\n" + output[-4000:]
                )
            print(f"B4 雙 int 形參真 DeclId 與順序 {index}/8：{name}（通過）")
        print("G18 B4 雙 int 形參八項完成；B4 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 B4 雙 int 形參 Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
