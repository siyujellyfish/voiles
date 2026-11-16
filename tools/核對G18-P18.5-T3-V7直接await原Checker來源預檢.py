#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T3／V7：非公開 P17 直接 await 封存來源與 ID 耗盡六項真驗收。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "execution::async_task::tests::"
CASES = (
    "v7_t3_原來源位元組變造取消前須拒絕且恢復後可安全取消",
    "v7_t3_旁觀工作來源污染不得取消或直接執行合法工作",
    "v7_t3_原Checker可達HIR變造不得發新工作身分",
    "v7_t3_待啟動計畫缺失不得假取消且可修復",
    "v7_t3_工作身分耗盡受控拒絕不panic不回繞",
    "v7_t3_已結束工作不得阻礙其他合法工作新增",
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
            raise ValueError("Rust --list 失敗：" + listing.stderr[-2500:])
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX + "v7_t3_") and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 6 or actual != expected:
            raise ValueError(
                "直接 await 來源預檢 Gate 清冊不一致；缺少 "
                + str(sorted(expected - actual)) + "、多出 " + str(sorted(actual - expected))
            )
        for index, name in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"直接 await {index}/6 {name} 失敗\n" + output[-6500:])
            print(f"直接 await 原Checker {index}/6：{name}（通過）")
        print("六項真來源拒絕與無回繞驗收；P18.5 T3/T4、P18.6 A1/A2 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as exc:
        print("直接 await 原Checker來源預檢 Gate 失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
