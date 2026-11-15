#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／V7-A1：原 Parser/Checker 非同步 self 直接 await 真動態十二項。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_v7_async_self_direct_await_tests::"
CASES = (
    "v7_async_self_原LexerParserChecker識別async方法與await方法專用AST",
    "v7_async_self_直接await才真進唯讀self框架並歸借票",
    "v7_async_self_直接await獨占self方法真寫欄位並回傳",
    "v7_async_self_真main形參純函式必先退出再啟動async方法借票",
    "v7_async_self_真if選中葉內self獨占寫入而未選葉不寫",
    "v7_async_self_方法真溢位先Unwind再歸self票不得成功",
    "v7_async_self_借票前函式溢位不得假啟動async方法",
    "v7_async_self_未await的async方法和await同步方法均不得執行",
    "v7_async_self_方法內再次直接await未證跨票據安全必靜態拒絕",
    "v7_async_self_錯參數非Copyable與未知方法均不能偷入await",
    "v7_async_self_常數接收者不得在await後執行獨占寫入",
    "v7_async_self_原已簽await方法來源身份與真借票入場索引變造拒絕",
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
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1800:])
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 12 or actual != expected:
            raise ValueError("V7 async self Gate 缺 "
                + str(sorted(expected-actual)) + "；多 "
                + str(sorted(actual-expected)))
        for index, case in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"V7 async self {index}/12 {case} 失敗\n" + output[-4500:]
                )
            print(f"V7 原Checker async self {index}/12：{case}（通過）")
        print("V7／A1 async self 直接 await 十二項真來源通過；A1/A2 原義務仍未閉合。")
        return 0
    except (OSError, ValueError) as error:
        print("V7 async self 直接 await Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
