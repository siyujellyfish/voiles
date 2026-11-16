#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／B4／M3：真Checker main var bool直線重賦十項驗收Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_main_bool_assignment_tests::"
CASES = (
    "b4_bool_assign_原Checker_var_true更新false後方法零helper",
    "b4_bool_assign_原Checker_var_false更新true後方法只跑真選葉",
    "b4_bool_assign_原Lexer合法Group常值與具名bool RHS可真求值",
    "b4_bool_assign_同一變數多次直線更新以最後真值傳方法",
    "b4_bool_assign_同一真DeclId自我複製只讀舊值再提交",
    "b4_bool_assign_交錯兩個可變bool正確保持逐筆原來源時序",
    "b4_bool_assign_原Checker合法async直接await執行成功但E1佇列拒絕前置",
    "b4_bool_assign_原Checker const賦值與未知名稱靜態拒絕",
    "b4_bool_assign_準備後更改目標或RHS來源必在借self前拒絕",
    "b4_bool_assign_不同int局部真函式交錯仍保守拒絕未核准工作圖",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], capture_output=True, check=False,
            text=True, encoding="utf-8",
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-3000:])
        expected = {PREFIX + item for item in CASES}
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 10 or actual != expected:
            raise ValueError("bool重賦Gate缺失="+str(sorted(expected-actual))
                             +"；多出="+str(sorted(actual-expected)))
        for index, name in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                check=False, capture_output=True, text=True, encoding="utf-8",
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"main var bool重賦 {index}/10 {name} 失敗\n" + output[-7000:]
                )
            print(f"main var bool重賦 {index}/10：{name}（通過）")
        print("十項原Checker真bool重賦與未核准工作圖保守拒絕；B4/M3仍Open。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 main bool重賦必要Gate失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
