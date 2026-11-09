#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.1-F2 原始已核准 int/bool/str 混合參數九項正反來源真實執行。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f2_obligation_tests::"
CASES = (
    "f2_純值布林整數同一函式按來源順序求值並重複讀取",
    "f2_字串共享借用布林與整數雙次傳入均各自歸票",
    "f2_同一呼叫字串整筆移動加另一字串借用與布林整數不混帳",
    "f2_純值布林選擇不得改變不同參數來源身分",
    "f2_借用後續整數實參溢位由呼叫者真還票且不進函式",
    "f2_移動後續整數溢位不能提前轉交也不能重複釋放",
    "f2_成功移入子框架後溢位由目前所有者完成完整資源清理",
    "f2_跨形參錯型別與借移同源衝突由原checker拒絕",
    "f2_跨資源借用票據事件偽造即使計數不變也不得交付",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run([str(binary), "--list"], capture_output=True,
            text=True, encoding="utf-8", check=False)
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1500:])
        present = {line.removesuffix(": test") for line in listing.stdout.splitlines()
                   if line.startswith(PREFIX) and line.endswith(": test")}
        expected = {PREFIX + name for name in CASES}
        if expected != present:
            raise ValueError("F2 來源不完整，缺少 " + str(sorted(expected - present))
                             + "；額外 " + str(sorted(present - expected)))
        for index, name in enumerate(CASES, 1):
            process = subprocess.run([str(binary), "--exact", PREFIX + name,
                "--test-threads=1"], capture_output=True, text=True,
                encoding="utf-8", check=False)
            output = process.stdout + process.stderr
            if process.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                raise ValueError(f"F2 真測試 {index}/{len(CASES)} 未通過：{name}\n" + output[-2200:])
            print(f"F2 真執行 {index}/{len(CASES)}：{name}（通過）")
        print("F2 九項真 Checked Source→Runtime 正反已通過；其他義務不可藉此提前 Passed。")
        return 0
    except (OSError, ValueError) as error:
        print(f"G18 F2 閘門失敗：{error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
