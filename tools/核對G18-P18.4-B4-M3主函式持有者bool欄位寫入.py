#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4 B4／P18.6 M3：原Checker main owner.bool真FieldWrite十一項Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_main_owner_bool_field_write_tests::"
CASES = (
    "b4_bool_field_write_原Checker_true欄位於main真寫false後零helper",
    "b4_bool_field_write_連續寫false再true以最後動態值選入方法",
    "b4_bool_field_write_原Checker具名Copyable_bool經DeclId寫入欄位",
    "b4_bool_field_write_原Lexer多重Group欄位自讀保留真Checker身分",
    "b4_bool_field_write_前序欄位false再由main局部真讀false傳方法",
    "b4_bool_field_write_先複製bool真快照後修改owner不污染舊局部",
    "b4_bool_field_write_與int欄位先後交錯真執行後方法值一致",
    "b4_bool_field_write_方法注入故障仍只借還同一張self票",
    "b4_bool_field_write_同型不同DeclId的bool欄位只依原Checker來源讀值",
    "b4_bool_field_write_已封存原Checker來源遭改寫必先SourceChanged",
    "b4_bool_field_write_原Checker拒絕const欄位與錯誤型別及錯誤欄名",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], capture_output=True,
            check=False, text=True, encoding="utf-8",
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-4000:])
        required = {PREFIX + name for name in CASES}
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(required) != 11 or required != actual:
            raise ValueError(
                "main bool FieldWrite Gate缺失=" + str(sorted(required-actual))
                + "；多出=" + str(sorted(actual-required))
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
                    f"真owner.bool FieldWrite {index}/11 {name}失敗\n" + output[-9000:]
                )
            print(f"G18 B4/M3 owner.bool真寫入 {index}/11：{name} 通過")
        print("十一項原Checker bool FieldWriteEvidence／P16／HC14真Gate通過；B4/M3仍Open。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 main owner.bool FieldWrite Gate失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
