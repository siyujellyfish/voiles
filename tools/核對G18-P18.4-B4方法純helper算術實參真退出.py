#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／B4/M3：原Checker純int方法helper算術實參與R05真退出十四項Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_helper_arithmetic_argument_tests::"
CASES = (
    "m3_helper_arg_arith_真Checker頂層Call保存子Call加法TypedHIR",
    "m3_helper_arg_arith_先子helper返回再父helper入框架與歸self票",
    "m3_helper_arg_arith_真雙層Group括號不重做inner呼叫",
    "m3_helper_arg_arith_減法先常值後子Call返回9",
    "m3_helper_arg_arith_巢狀加法由原AST左至右各計算一次",
    "m3_helper_arg_arith_多形參父helper的兩個算術實參各有真子退出",
    "m3_helper_arg_arith_兩層父子helper加算術再套上層helper",
    "m3_helper_arg_arith_內層算術溢位只完成子helper不可假進父helper",
    "m3_helper_arg_arith_子helper本體故障父helper不能入框架",
    "m3_helper_arg_arith_父helper故障時必先正常退出子helper",
    "m3_helper_arg_arith_if選葉真執行與false旁路零副作用",
    "m3_helper_arg_arith_async_self直接await相同純值子圖與延遲借票",
    "m3_helper_arg_arith_錯型別async子函式與self欄位不得冒充純helper",
    "m3_helper_arg_arith_篡改真inner返回計畫或父Call算術來源必拒絕",
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
            raise ValueError("Rust --list 錯誤：" + listing.stderr[-2500:])
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 14 or actual != expected:
            raise ValueError(
                "真測試集合與十四項原Checker Gate不一致；缺少 "
                + str(sorted(expected-actual)) + "，多出 "
                + str(sorted(actual-expected))
            )
        for index, case in enumerate(CASES, start=1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"B4/M3巢狀算術實參 {index}/14 {case} 失敗：\n"
                    + output[-6500:]
                )
            print(f"B4/M3 原Checker真方法算術實參 {index}/14：{case}（通過）")
        print("十四項真Lexer/Checker→Helper框架→R05自借票驗證通過；B4/M3仍待其餘原義務。")
        return 0
    except (OSError, ValueError) as error:
        print("B4/M3 方法helper算術實參 Gate失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
