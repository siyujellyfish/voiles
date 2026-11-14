#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4：原 Checker 單 int 形參 helper 真值框架與退出八項。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_single_int_helper_parameter_tests::"
CASES = (
    "b4_arg_主程式已初始化具名int加減求參後真進helper返回10",
    "b4_arg_方法int形參加一先求值再呼叫helper且真歸票",
    "b4_arg_方法var重賦使用目前真局部作helper實參並原子提交",
    "b4_arg_同一helper第一次初始化第二次重賦使用第一次返回槽",
    "b4_arg_main實參先溢位沒有任何helper進入或self借用",
    "b4_arg_方法實參先溢位只得方法退出不應假造內層helper",
    "b4_arg_單形參成功綁定後helper本體溢位先退出子框架再還票",
    "b4_arg_原Checker拒絕多形參非法型別及篡改Helper身分",
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
                "B4 單形參 Rust 真來源名稱錯位：缺 "
                + str(sorted(expected - actual)) + "；多 "
                + str(sorted(actual - expected))
            )
        for number, name in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"B4 原 Checker int 參數真框架 {number}/8 失敗：{name}\n"
                    + output[-4000:]
                )
            print(f"B4 原 Checker int 形參動態框架 {number}/8：{name}（通過）")
        print("G18 B4 原 Checker 單 int 形參八項完成；B4 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 B4 單 int 形參 Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
