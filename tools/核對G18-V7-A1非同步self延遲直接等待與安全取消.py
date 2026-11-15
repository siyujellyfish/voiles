#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／V7-A1/E3：原 Checker async self 直接 await 延遲／安全取消十二項。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_v7_async_self_deferred_session_tests::"
CASES = (
    "v7_async_deferred_真Checker尚未啟動時只有Prepared無動態結果",
    "v7_async_deferred_真正suspend跨呼叫後wake才借入唯讀self並返回",
    "v7_async_deferred_獨占self寫欄位必在真正wake後執行",
    "v7_async_deferred_兩個真Checker工作依明確wake次序才各執行一次",
    "v7_async_deferred_Prepared未啟動安全取消不得造假BorrowTicket",
    "v7_async_deferred_Suspended未啟動取消不得虛構清理事件",
    "v7_async_deferred_真方法內溢位仍須Unwind並唯一還票標記Failed",
    "v7_async_deferred_原Checker靜態拒絕不得配發async工作ID",
    "v7_async_deferred_有前置main局部的工作不能假裝無副作用等待",
    "v7_async_deferred_已封存async語意被更改wake先拒且保留Suspended",
    "v7_async_deferred_封存Lexer的await遭改不得預先啟動self票",
    "v7_async_deferred_工作ID耗盡不回繞且不能消耗新計畫",
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
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + n for n in CASES}
        if len(expected) != 12 or actual != expected:
            raise ValueError("V7 async self 延遲工作 Gate 缺 "
                + str(sorted(expected-actual)) + "；多 "
                + str(sorted(actual-expected)))
        for i, name in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output,
            ):
                raise ValueError(
                    f"V7 async self {i}/12 {name} 失敗\n" + output[-4500:]
                )
            print(f"V7 async self 真延遲 {i}/12：{name}（通過）")
        print("V7 async self 真Checker延遲、首次wake、未啟動安全取消十二項成功；A1/A2仍未全閉合。")
        return 0
    except (OSError, ValueError) as exc:
        print("V7 async self 延遲工作 Gate 失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
