#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6-M3／P18.4-B4：方法 Return 純 int helper 算術真退出十項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_method_return_helper_arithmetic_tests::"
CASES = (
    "m3_return_arith_直接零參數seed先真返回14再外層加2等於16",
    "m3_return_arith_先求Lexer常值再進零參數helper等於16",
    "m3_return_arith_兩個普通helper在一個方法Return左右依序退出",
    "m3_return_arith_外層加法溢位不得變造已真完成的seed退出",
    "m3_return_arith_seed本體溢位必先退出子框架才歸唯一self票",
    "m3_return_arith_原if_true只在選中葉進seed返回16",
    "m3_return_arith_原else選中時才執行seed加法正常返回",
    "m3_return_arith_巢狀bool只選中一份原seed算術工作圖",
    "m3_return_arith_未選分支的seed本體溢位不得預先執行",
    "m3_return_arith_錯型別未知函式及封存工作圖篡改須先拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], capture_output=True,
            text=True, encoding="utf-8", check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1600:])
        found = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 10 or found != expected:
            raise ValueError("M3/B4 Return 算術 Gate 清單缺 "
                + str(sorted(expected-found)) + "；多 "
                + str(sorted(found-expected)))
        for index, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True,
                encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"M3/B4 Return 算術 {index}/10 {case} 失敗\n" + output[-4500:]
                )
            print(f"M3/B4 Return 真動態 {index}/10：{case}（通過）")
        print("M3/B4 原 Checker Return helper 算術十項 Gate 通過；原義務尚未完整閉合。")
        return 0
    except (OSError, ValueError) as exc:
        print("M3/B4 Return helper 算術 Gate 失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
