#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／B4/M3：原 Checker CopyStructBoolField 真方法實參十項獨立 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_owner_bool_method_argument_tests::"
CASES = (
    "b4_bool_method_arg_原Checker_true欄位直接作首個方法bool實參",
    "b4_bool_method_arg_原Checker_false欄位令未選helper零執行",
    "b4_bool_method_arg_原Checker_const_bool欄位真預設且不得混成int",
    "b4_bool_method_arg_原Lexer_Group包裹bool欄位仍是同一Checker值葉",
    "b4_bool_method_arg_欄位bool在第二bool實參依原位置進巢狀if",
    "b4_bool_method_arg_先更新int欄位後同物件bool實參與int實參皆真讀",
    "b4_bool_method_arg_原P16布林實參方法入口故障仍唯一還票",
    "b4_bool_method_arg_偽造封存bool欄位名稱須在BorrowTicket前SourceChanged",
    "b4_bool_method_arg_偽造封存bool欄位DeclId不得借入self",
    "b4_bool_method_arg_原Checker拒絕int欄位冒充bool與不存在欄位",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        proc = subprocess.run(
            [str(binary), "--list"], capture_output=True, check=False,
            text=True, encoding="utf-8",
        )
        if proc.returncode:
            raise ValueError("Rust --list 無法驗證真測試：" + proc.stderr[-3000:])
        expected = {PREFIX + name for name in CASES}
        actual = {
            line.removesuffix(": test") for line in proc.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 10 or actual != expected:
            raise ValueError(
                "真owner bool欄位方法實參缺失=" + str(sorted(expected - actual))
                + "；多出=" + str(sorted(actual - expected))
            )
        for i, case in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, check=False, text=True, encoding="utf-8",
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"bool欄位方法實參 {i}/10 {case} 失敗\n" + output[-9000:])
            print(f"G18 B4/M3 bool欄位真實參 {i}/10：{case} 通過")
        print("原Checker CopyStructBoolField 真 P16 值 → 方法前序求值十項完成；B4/M3仍Open。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 B4/M3 bool欄位真實參Gate失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
