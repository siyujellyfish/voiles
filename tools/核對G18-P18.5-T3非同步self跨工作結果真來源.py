#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T3：async self 原Checker工作與Outcome來源綁定十三項。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_t3_async_self_outcome_origin_binding_tests::"
CASES = (
    "t3_async_origin_原Checker真方法在動態結果封存相同完整來源",
    "t3_async_origin_合法Read與Write兩份Outcome雖都平衡也不得交換工作ID",
    "t3_async_origin_相同Loan與方法跨度的不同原始碼結果交換也必拒絕",
    "t3_async_origin_未啟動工作封存ownerDeclarationId被改不得發Ready",
    "t3_async_origin_未啟動工作呼叫Span變造不得簽發等待憑據",
    "t3_async_origin_未啟動完整UTF8封存被改不能只靠同長及票據通過",
    "t3_async_origin_準備後偷換真Checker原計畫票據即使帳單獨合法也拒",
    "t3_async_origin_真Outcome的來源位元組遭變造即使退出平衡也禁止退休",
    "t3_async_origin_真Outcome方法Span與計畫不同不得冒領原Checked框架",
    "t3_async_origin_受控溢位Failed也須保存原來源而非借別的Outcome",
    "t3_async_origin_取消尚未執行工作也應先稽核原Checker封存來源",
    "t3_async_origin_另一份終態來源錯置不得先取消合法等待者",
    "t3_async_origin_終態退休後交付的真結果仍攜帶可核驗來源",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        found = subprocess.run(
            [str(binary), "--list"], capture_output=True, check=False,
            text=True, encoding="utf-8",
        )
        if found.returncode:
            raise ValueError("Rust --list 失敗：" + found.stderr[-1500:])
        expected = {PREFIX + name for name in CASES}
        actual = {
            line.removesuffix(": test")
            for line in found.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 13 or actual != expected:
            raise ValueError(
                "T3 async self 來源 Gate 缺=" + str(sorted(expected - actual))
                + "，多=" + str(sorted(actual - expected))
            )
        for index, name in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, check=False,
                text=True, encoding="utf-8",
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"T3 async self {index}/13 {name} 失敗:\n" + output[-4200:]
                )
            print(f"T3 async self Outcome來源 {index}/13：{name}（通過）")
        print("T3 原Checker async self 工作/Outcome 十三項完整來源 Gate 通過；P18.5仍未Passed。")
        return 0
    except (OSError, ValueError) as error:
        print("T3 async self 來源 Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
