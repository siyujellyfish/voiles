#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6-M2/B4：原Checker方法Group純值返回真執行十五項。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_grouped_pure_self_return_tests::"
CASES = (
    "m2_grouped_self_單層括號self欄位由原Checker共享票據真返回",
    "m2_grouped_self_多層括號self欄位不能被誤判為未核准MethodCall",
    "m2_grouped_self_括號Copyable方法參數用真DeclarationId取值",
    "m2_grouped_self_多層方法參數回傳不讀取同名owner來源",
    "m2_grouped_self_真Checker常值Return七與多層括號同一動態退出",
    "m2_grouped_self_括號包覆整份self欄位加三保留原算術操作",
    "m2_grouped_self_括號包覆二元左self欄位仍須真HC14借票",
    "m2_grouped_self_右端括號self欄位由原int參數加總",
    "m2_grouped_self_兩個括號方法int參數保持各自真來源",
    "m2_grouped_self_多層括號右整數常值由原Lexer驗證",
    "m2_grouped_self_獨占self寫入後括號讀取必用同張已核准BorrowTicket",
    "m2_grouped_self_非同步self直接await方法也能真返回Group欄位",
    "m2_grouped_self_括號純self整數運算溢位先Unwind再歸唯一票據",
    "m2_grouped_self_錯誤返回型別與禁止消耗self仍須原Checker靜態拒絕",
    "m2_grouped_self_執行前括號原LexerToken被偷改不得沿用舊安全計畫",
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
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1500:])
        expected = {PREFIX + case for case in CASES}
        actual = {
            line.removesuffix(": test")
            for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 15 or actual != expected:
            raise ValueError(
                "M2 方法Group返回 Gate 缺=" + str(sorted(expected - actual))
                + "；多=" + str(sorted(actual - expected))
            )
        for index, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                check=False, capture_output=True, text=True, encoding="utf-8",
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"M2/B4 Group {index}/15 {case} 失敗：\n" + output[-4200:]
                )
            print(f"M2/B4 Group {index}/15：{case}（通過）")
        print("G18 M2/B4 原Checker方法Group純值返回十五項通過；P18未提前Passed。")
        return 0
    except (OSError, ValueError) as error:
        print("M2/B4 原Checker方法Group Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
