#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／F2：bool 返回作 if／else-if／while 動態條件八項真來源正反。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f2_bool_condition_tests::"
CASES = (
    "f2_bool_cond_if真函式返回true才執行選中分支",
    "f2_bool_cond_if真函式返回false不執行未選分支",
    "f2_bool_cond_else_if按原順序真執行前兩個條件",
    "f2_bool_cond_else_if首分支已選中時不可多執行後續條件",
    "f2_bool_cond_while每輪重呼叫真bool函式而非重用快照",
    "f2_bool_cond_while第一次false不得建立循環本體資源",
    "f2_bool_cond_迴圈預算耗盡仍清理父框架真utf8而不假成功",
    "f2_bool_cond_整數函式不能冒充bool條件由原checker拒絕",
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
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1400:])
        actual = {line.removesuffix(": test") for line in listing.stdout.splitlines()
                  if line.startswith(PREFIX) and line.endswith(": test")}
        expected = {PREFIX+name for name in CASES}
        if actual != expected:
            raise ValueError("八項身分不完整，缺少：" + str(sorted(expected-actual))
                + "；額外：" + str(sorted(actual-expected)))
        for i, name in enumerate(CASES, 1):
            proc = subprocess.run(
                [str(binary), "--exact", PREFIX+name, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            output = proc.stdout + proc.stderr
            if proc.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                raise ValueError(f"F2 條件 {i}/{len(CASES)} 失敗：{name}\n"+output[-2400:])
            print(f"F2 真布林條件 {i}/{len(CASES)}：{name}（通過）")
        print("F2 真 bool 條件八項成功；其它合法型別混合及 F2 完整閉合仍需驗收")
        return 0
    except (OSError, ValueError) as err:
        print(f"G18 F2 真條件 Gate 失敗：{err}", file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
