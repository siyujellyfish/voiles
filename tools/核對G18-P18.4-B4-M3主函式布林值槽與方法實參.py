#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4／P18.6-M3：main bool 值槽及方法實參十項真原Checker驗證。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_main_bool_local_method_argument_tests::"
CASES = (
    "b4_main_bool_原Checker_const_true真值槽傳給方法選中內層helper",
    "b4_main_bool_原Checker_var_false傳入方法外層false零helper",
    "b4_main_bool_原Checker多層括號初始化和具名實參真bool來源",
    "b4_main_bool_兩份bool局部維持獨立DeclarationId及真選枝",
    "b4_main_bool_原型別重排int_bool形參不混用main兩種值槽",
    "b4_main_bool_main普通helper先返回再借self且保持選中葉helper",
    "b4_main_bool_async_self直接await執行_延遲佇列前置局部保守拒絕",
    "b4_main_bool_預備後主布林值槽竄改不可進入方法取得BorrowTicket",
    "b4_main_bool_布林實參來源跨DeclId偽裝不得先簽self票",
    "b4_main_bool_原Checker錯型別未知來源或含呼叫布林初始化均不可放行",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        test_bin = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(test_bin), "--list"], capture_output=True, check=False,
            text=True, encoding="utf-8",
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-2500:])
        expected = {PREFIX + name for name in CASES}
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 10 or expected != actual:
            raise ValueError("main bool 值槽Gate缺失=" + str(sorted(expected - actual))
                             + "；多出=" + str(sorted(actual - expected)))
        for index, name in enumerate(CASES, 1):
            run = subprocess.run(
                [str(test_bin), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, check=False, text=True, encoding="utf-8",
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"main bool 方法 {index}/10 {name} 失敗\n" + output[-7000:]
                )
            print(f"main bool 方法 {index}/10：{name}（通過）")
        print("十項 main 真bool/int值槽與同步／async self方法原Checker證據；B4/M3仍Open。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 main bool方法必要Gate失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
