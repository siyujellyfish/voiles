#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／B4/M2：main owner真FieldWriteEvidence／MixedStructRuntime十三項Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m2_main_owner_int_field_write_tests::"
CASES = (
    "b4_owner_write_原Checker單次var_int欄位常值真寫入再由self讀取",
    "b4_owner_write_兩次主函式欄位覆寫以最後真動態值返回",
    "b4_owner_write_具名int經原CheckerDeclId寫入owner欄位",
    "b4_owner_write_原Lexer多層Group與純int加法真動態寫入",
    "b4_owner_write_普通helper須真退出後才寫欄位及借入self",
    "b4_owner_write_未初始化int首次賦值與owner欄位真交接",
    "b4_owner_write_夾bool重賦與helper保持唯一原Parser語句順序",
    "b4_owner_write_先寫持有者欄位後方法選中helper仍不多簽票",
    "b4_owner_write_同步純helper外層溢位不得寫入owner或借方法票",
    "b4_owner_write_與Grouped方法Return保持內葉唯一真借票",
    "b4_owner_write_來源清冊被改或步驟重播必先SourceChanged",
    "b4_owner_write_原Checker拒絕const持有者不存在欄位與錯型別",
    "b4_owner_write_未接通的欄位讀取作RHS仍保守拒絕第二套ScalarHIR",
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
            raise ValueError("Rust --list 失敗：" + listed.stderr[-3500:])
        expected = {PREFIX + name for name in CASES}
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 13 or actual != expected:
            raise ValueError(
                "owner欄位來源Gate缺失=" + str(sorted(expected - actual))
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
                    f"owner真FieldWrite {index}/13 {name} 失敗\n" + output[-9000:]
                )
            print(f"原Checker owner欄位 {index}/13：{name}（通過）")
        print("十三項原Checker FieldWriteEvidence／SharedCallExit／HC14與P16真門檻；B4/M2仍Open。")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 main持有者欄位寫入必要Gate失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
