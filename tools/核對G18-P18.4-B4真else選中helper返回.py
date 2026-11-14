#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.4-B4/M3：原 Checker 真 if/else 選中 else helper 退出與未選葉零事件。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_if_else_helper_return_tests::"
CASES = (
    "b4_else_動態bool_false選中else真helper返回5並還唯一票",
    "b4_else_動態bool_true只return_self欄位不提前呼叫else_helper",
    "b4_else_未選else即便helper本體溢位也必須返回原self7",
    "b4_else_選中else_helper本體溢位先退內層再歸還self票",
    "b4_else_選中else第二實參先溢位不得假進普通helper",
    "b4_else_未選else第二實參即使溢位亦不可偽造錯誤",
    "b4_else_獨占else先寫self後return_helper仍只進退一張票",
    "b4_else_非法混型遞迴與竄改else真Return計畫皆拒絕",
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
                "B4/M3 else helper 真來源漂移：缺 "
                + str(sorted(expected - actual)) + "；多 "
                + str(sorted(actual - expected))
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
                    f"B4 真 else helper {index}/8 失敗：{name}\n"
                    + output[-4000:]
                )
            print(f"B4 真 else 選中 helper Return {index}/8：{name}（通過）")
        print("G18 B4/M3 原 Checker if/else 真選中 helper 八項完成；B4/M3 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 B4 真 else helper Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
