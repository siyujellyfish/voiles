#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／B4/M3：main var int/bool 未初始化真Checker首次賦值十二項驗收。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_main_uninitialized_scalar_tests::"
CASES = (
    "b4_uninit_main_原Checker_var_int先宣告後常值首次提交",
    "b4_uninit_main_原Checker_var_bool先宣告後true首次提交",
    "b4_uninit_main_原Checker_bool先宣告後false不假預設",
    "b4_uninit_main_原Checker初次具名int與多層Group再算術",
    "b4_uninit_main_原Checker_bool初次由已初始化名稱Copyable傳遞",
    "b4_uninit_main_兩個未初始化var交錯賦值後才可複製",
    "b4_uninit_main_未初始化int先經真helper完成後才簽self借票",
    "b4_uninit_main_首次helper溢位不可填值亦不得進方法或後續bool",
    "b4_uninit_main_未被讀取的空var不得偽造零值或退出錯誤",
    "b4_uninit_main_原Checker已核准async直接await仍不放寬E1延遲佇列",
    "b4_uninit_main_來源步驟Empty變成有值或重播必先SourceChanged",
    "b4_uninit_main_原Checker拒絕未初始化讀取自指常值型別混用",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], check=False, capture_output=True,
            text=True, encoding="utf-8",
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-3500:])
        expected = {PREFIX + name for name in CASES}
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 12 or expected != actual:
            raise ValueError(
                "未初始化值槽清冊缺失=" + str(sorted(expected - actual))
                + "；多出=" + str(sorted(actual - expected))
            )
        for index, name in enumerate(CASES, 1):
            proc = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, check=False, text=True, encoding="utf-8",
            )
            output = proc.stdout + proc.stderr
            if proc.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"main 空值槽驗收 {index}/12 {name} 失敗\n" + output[-8000:]
                )
            print(f"原Checker空值槽 {index}/12：{name}（通過）")
        print("十二項主函式已宣告／未初始化／首次賦值來源與HC14安全；B4/M3仍Open。")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 main 未初始化真值槽必要Gate失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
