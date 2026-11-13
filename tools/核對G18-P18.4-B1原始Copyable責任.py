#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.4-B1：原 Checker Copyable 參數不消耗來源 11 項真 Rust 正反 Gate。"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

PREFIX = "struct_stage::g18_b1_obligation_tests::"
CASES = (
    "b1_原值重複傳入兩次普通函式仍可在主函式讀回",
    "b1_同一Copyable整數填入三個不同參數不移動",
    "b1_布林局部更新後兩次傳參各讀當前Copyable值",
    "b1_混型別形參依書寫順序且不消耗主函式整數",
    "b1_兄弟巢狀函式共用Copyable來源但框架互不污染",
    "b1_for逐輪Copyable來源與迭代純值不消耗",
    "b1_共享str借用與Copyable來源雙次呼叫先還票後離域",
    "b1_str整筆Move與Copyable來源不會連帶移走整數",
    "b1_子函式算術溢位不冒稱正常返回或留下參數槽",
    "b1_未初始化與錯型別原Checker必拒絕",
    "b1_真參數身分重播即使數值一致也不得交付",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], capture_output=True, text=True,
            encoding="utf-8", check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1500:])
        actual = {
            line.removesuffix(": test")
            for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if actual != expected:
            raise ValueError(
                "B1 來源清單不符，缺少：" + str(sorted(expected - actual))
                + "；未登錄：" + str(sorted(actual - expected))
            )
        for index, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"B1 第 {index}/{len(CASES)} 項真執行失敗：{case}\n"
                    + output[-3000:]
                )
            print(f"B1 Copyable 原始義務 {index}/{len(CASES)}：{case}（通過）")
        print("P18.4-B1 原 Checker→Runtime 十一項獨立正反 Gate 真通過。")
        print("B2～B5 及 P18.4 原 Phase 不因 B1 子項 Gate 成功提前 Passed。")
        return 0
    except (OSError, ValueError) as error:
        print(f"G18 B1 真驗收失敗：{error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
