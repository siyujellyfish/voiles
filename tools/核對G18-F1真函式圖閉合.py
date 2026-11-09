#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.1-F1：由原 Checker 原始來源到真函式圖的七項正反驗收。

嚴格 --exact 真執行；不把本 Gate 成功冒充其他 F2～A2 已閉合。
"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f1_obligation_tests::"
CASES = (
    "f1_前向宣告與直接呼叫真返回",
    "f1_同層兄弟與巢狀實參按來源順序進退",
    "f1_相互呼叫雙向均有真動態到達與停止路徑",
    "f1_巢狀真資源先釋放子框架再釋放父框架",
    "f1_內層溢位不能偽造成功且外層資源完整退出",
    "f1_不存在函式錯型別與未初始化實參由原checker拒絕",
    "f1_來源函式事件遭竄改即使數目平衡也不可交付",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 執行失敗：" + listing.stderr[-1200:])
        actual = {
            line.removesuffix(": test")
            for line in listing.stdout.splitlines() if line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        present = {name for name in actual if name.startswith(PREFIX)}
        if present != expected:
            raise ValueError(
                "F1 測試名稱／數量不符：缺少 "
                + str(sorted(expected - present))
                + "；額外 " + str(sorted(present - expected))
            )
        for index, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;",
                output,
            ):
                raise ValueError(f"第 {index} 筆真 Rust 正反失敗：{case}\n" + output[-1500:])
            print(f"F1 真執行 {index}/{len(CASES)}：{case}（通過）")
        print("G18 P18.1 F1 真函式圖七項正反完成；不代表 F2～F4 或 P18.1 Passed。")
        return 0
    except (OSError, ValueError) as error:
        print(f"G18 F1 真驗收失敗：{error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
