#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.6-M2 原 Checker→真 Runtime 同步方法借用與欄位：8 項正反。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

CASES = (
    "struct_stage::g18_m2_m3_obligation_tests::m2_交替持有者的寫入分支與參數資源",
    "struct_stage::g18_m2_m3_obligation_tests::m2_故障退出與變造持有者",
    "struct_stage::g18_scalar_self_cfg_tests::巢狀方法分支依每輪欄位真值寫入與提前返回",
    "struct_stage::g18_scalar_self_cfg_tests::布林實參在方法內決定提前返回的欄位",
    "struct_stage::g18_scalar_self_cfg_tests::方法內算術溢位先還獨占票再清理呼叫者",
    "struct_stage::g18_scalar_self_cfg_tests::方法由分支移出單一字串並逆序清理其餘形參",
    "struct_stage::g18_scalar_self_cfg_tests::方法整筆移動來源與形參交叉同名仍逐筆清理",
    "struct_stage::g18_scalar_self_cfg_tests::方法來源收據與重播票據分別拒絕",
)
EXACT_MODULE_PREFIXES = ("struct_stage::g18_m2_m3_obligation_tests::m2_",)


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
                    f"M2 第 {index}/8 條真來源失敗：{case}\n" + output[-3000:]
                )
            print(f"M2 原始義務 {index}/8：{case}（通過）")
        print("M2：原 Checker 真來源正反／方法工作圖獨立驗收通過；不代表 P18.6 全 Passed。")
        return 0
    except (OSError, ValueError) as error:
        print("M2 獨立 Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
