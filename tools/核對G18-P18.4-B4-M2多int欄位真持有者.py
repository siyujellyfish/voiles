#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4 B4、P18.6 M2：原Checker多個預設int欄位及P16真self八項Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m2_multiple_int_fields_self_tests::"
CASES = (
    "b4_multi_fields_兩個var_int預設依原Checker順序同物件建構",
    "b4_multi_fields_保留原Checker_const_int額外欄位不可當可寫",
    "b4_multi_fields_三個int欄位依原Lexer宣告次序分別真預設",
    "b4_multi_fields_main真欄位更新後方法獨占寫入仍只更新指定count",
    "b4_multi_fields_方法入口故障仍唯一還self票並逐作用域離開",
    "b4_multi_fields_額外真Checker欄位封存預設遭改寫須先SourceChanged",
    "b4_multi_fields_未開放額外str資源欄位不得偽裝Copyable_int",
    "b4_multi_fields_未開放額外bool欄位不得冒充int預設",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], capture_output=True, check=False,
            text=True, encoding="utf-8",
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-3000:])
        required = {PREFIX + item for item in CASES}
        listed = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(required) != 8 or listed != required:
            raise ValueError(
                "多int欄位Gate缺失=" + str(sorted(required - listed))
                + "；多出=" + str(sorted(listed - required))
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
                raise ValueError(
                    f"原Checker多int持有者 {i}/8 {case} 失敗\n" + output[-8000:]
                )
            print(f"原Checker多int欄位真P16 {i}/8：{case} 通過")
        print("B4/M2 多int欄位獨立Gate成功；原B4/M2仍OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 多int欄位同物件Gate失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
