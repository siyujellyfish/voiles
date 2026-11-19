#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／B4/M2：原Checker多int/bool真P16同物件與自借十一項獨立Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m2_mixed_copyable_fields_tests::"
CASES = (
    "b4_mixed_copy_原Checker第二var_bool_true真建立與原欄位方法寫入",
    "b4_mixed_copy_原Checker額外const_bool_false保留immutable來源",
    "b4_mixed_copy_原Checker混合int_bool_int依宣告順序建立真P16",
    "b4_mixed_copy_相等bool值不同原Checker欄位身分不得去重",
    "b4_mixed_copy_main先寫真int欄位再進方法仍保留bool欄位",
    "b4_mixed_copy_方法內受控故障只歸還一張self票不創造bool資源",
    "b4_mixed_copy_改寫封存bool值必在任何真借票前SourceChanged",
    "b4_mixed_copy_把bool封存型別偽裝int不可建構",
    "b4_mixed_copy_修改封存const可寫性必SourceChanged",
    "b4_mixed_copy_真正str在故障歸票後釋放且不可傳入整數形參",
    "b4_mixed_copy_沒有預設值的附加欄位保守拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], capture_output=True,
            check=False, text=True, encoding="utf-8",
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-4000:])
        expected = {PREFIX + case for case in CASES}
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 11 or actual != expected:
            raise ValueError(
                "混合Copyable欄位Gate缺失=" + str(sorted(expected - actual))
                + "；意外項目=" + str(sorted(actual - expected))
            )
        for number, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, check=False, text=True, encoding="utf-8",
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"混合Copyable欄位 {number}/11 {case} 失敗\n" + output[-9000:]
                )
            print(f"原Checker int／bool 真P16 {number}/11：{case} 通過")
        print("十一項原Checker int/bool 純值同物件與自借安全Gate全數通過；B4/M2仍OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 B4 混合欄位真來源Gate失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
