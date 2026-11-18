#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／B4/M2：主函式同一owner Copyable int真欄位讀取十四項Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m2_main_field_copy_local_tests::"
CASES = (
    "b4_field_copy_原Checker預設owner_int直接建立const局部再傳方法",
    "b4_field_copy_先寫十一再真正讀取owner欄位可傳方法",
    "b4_field_copy_先Copy七再寫十一局部保留原值身份",
    "b4_field_copy_兩個具名讀取各綁不同原CheckerDeclId與真欄位時序",
    "b4_field_copy_合法多層Group包覆owner欄位讀取仍是內葉ScalarHIR",
    "b4_field_copy_欄位先複製到var再純整數重賦保留真正次序",
    "b4_field_copy_普通helper先寫owner欄位再讀取進main值槽",
    "b4_field_copy_含bool局部與int欄位更新的呼叫前置真圖",
    "b4_field_copy_原Checker_const_owner可複製唯讀欄位且self共享借票",
    "b4_field_copy_先寫入helper溢位不建立後續偽局部或self借票",
    "b4_field_copy_原Checker合法async直接await但前置欄位讀取不開背景工作",
    "b4_field_copy_變造FieldRead原Span或工作步驟必SourceChanged",
    "b4_field_copy_原Checker拒絕不存在欄位與不合法型別複製",
    "b4_field_copy_複合欄位算術尚無同一型別工作圖仍保守拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], capture_output=True, check=False,
            text=True, encoding="utf-8",
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-3500:])
        expected = {PREFIX + name for name in CASES}
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 14 or actual != expected:
            raise ValueError(
                "main欄位複製來源清冊缺失=" + str(sorted(expected - actual))
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
                    f"main owner Copyable FieldRead {index}/14 {name} 失敗\n"
                    + output[-9000:]
                )
            print(f"原Checker Copyable FieldRead {index}/14：{name}（通過）")
        print("十四項真MixedStructRuntime欄位Copy來源／HC14與溢位門檻；B4/M2仍Open。")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 main owner欄位複製必要Gate失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
