#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4：原 Checker int helper 加減與跨 self 框架真 Unwind 八項。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_helper_arithmetic_unwind_tests::"
CASES = (
    "b4_helper_math_普通函式在main先真加減回傳11然後方法回傳12",
    "b4_helper_math_方法持有self時真執行純整數巢狀算式並返回14",
    "b4_helper_math_同一真helper在main與方法各執行一次且同源進退對帳",
    "b4_helper_math_main普通函式加法溢位不能借用self且必真退出失敗框架",
    "b4_helper_math_方法內普通函式溢位先退內層後退self且零Returned",
    "b4_helper_math_先初始化局部後普通函式溢位仍逆序清理已達前綴",
    "b4_helper_math_改寫原Checker加減計畫及偽造已進入計數均拒絕",
    "b4_helper_math_原Checker非法型別與未核准helper呼叫不得進Runtime",
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
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1400:])
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError(
                "B4 helper 純算術來源漂移：缺 "
                + str(sorted(expected - actual)) + "；多 "
                + str(sorted(actual - expected))
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
                    f"B4 helper int 真動態 {index}/8 未通過：{case}\n" + output[-3900:]
                )
            print(f"B4 真 helper 整數算術／Unwind {index}/8：{case}（通過）")
        print("G18 B4 原 Checker 真 int helper 八項完成；B4 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 B4 helper Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
