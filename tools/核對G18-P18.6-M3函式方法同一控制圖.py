#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.6-M3 原 Checker→真 Runtime 函式方法同一控制圖：8 項正反。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

CASES = (
    "struct_stage::g18_m2_m3_obligation_tests::m3_求參與方法回傳共用迴圈中的真框架",
    "struct_stage::g18_m2_m3_obligation_tests::m3_中途故障逆序離域及子呼叫來源重驗",
    "struct_stage::g18_scalar_self_cfg_tests::持票子函式迴圈反向邊與故障還票",
    "struct_stage::g18_scalar_self_cfg_tests::持票子函式多語句選葉與局部清理",
    "struct_stage::g18_scalar_self_cfg_tests::原方法可轉交普通函式的新字串",
    "struct_stage::g18_scalar_self_cfg_tests::普通函式迴圈內方法持票執行輔助函式並保留故障框架",
    "struct_stage::g18_scalar_self_cfg_tests::布林方法結果可在普通函式分支及實參中使用",
    "struct_stage::g18_scalar_self_cfg_tests::普通函式的迴圈與提前返回依動態次序借票",
)
EXACT_MODULE_PREFIXES = ("struct_stage::g18_m2_m3_obligation_tests::m3_",)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], capture_output=True, text=True,
            encoding="utf-8", check=False,
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1500:])
        available = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.endswith(": test")
        }
        expected = set(CASES)
        if len(expected) != 8:
            raise ValueError("本契約測試名稱出現重複，不得重複採計")
        missing = sorted(expected - available)
        if missing:
            raise ValueError("缺少原 Checker 真 Rust 案例：" + str(missing))
        for prefix in EXACT_MODULE_PREFIXES:
            actual = {name for name in available if name.startswith(prefix)}
            expected_module = {name for name in expected if name.startswith(prefix)}
            if actual != expected_module:
                raise ValueError(
                    f"模組 {prefix} 正反矩陣漂移：缺 {sorted(expected_module-actual)} "
                    f"多 {sorted(actual-expected_module)}"
                )
        for index, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"M3 第 {index}/8 條真來源失敗：{case}\n" + output[-3000:]
                )
            print(f"M3 原始義務 {index}/8：{case}（通過）")
        print("M3：原 Checker 真來源正反／方法工作圖獨立驗收通過；不代表 P18.6 全 Passed。")
        return 0
    except (OSError, ValueError) as error:
        print("M3 獨立 Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
