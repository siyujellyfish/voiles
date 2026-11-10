#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 F2 bool-return 完整多語句與真提前分支返回的九項正反 Rust Gate。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f2_bool_body_tests::"
CASES = (
    "f2_bool_body_真布林函式局部宣告後返回當前值",
    "f2_bool_body_真布林局部可寫更新以最後值作返回",
    "f2_bool_body_真函式前置int與str局部必在bool返回後逆序清理",
    "f2_bool_body_提前if返回true與尾端false按真動態分支求值",
    "f2_bool_body_完整if_else兩葉已返回的尾端不可達不造假執行",
    "f2_bool_body_提前返回前建立真utf8不管選哪側都只釋放一次",
    "f2_bool_body_前置bool子函式真返回更新局部後由另一bool返回",
    "f2_bool_body_提前布林返回內混入整數必由原checker拒絕",
    "f2_bool_body_本體int運算溢位不得假BoolReturned且真清理str",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        proc = subprocess.run([str(binary), "--list"], text=True,
            encoding="utf-8", capture_output=True, check=False)
        if proc.returncode:
            raise ValueError("Rust --list 失敗：" + proc.stderr[-1400:])
        found = {line.removesuffix(": test") for line in proc.stdout.splitlines()
                 if line.startswith(PREFIX) and line.endswith(": test")}
        expected = {PREFIX + name for name in CASES}
        if found != expected:
            raise ValueError("bool-return 本體原測試身分不符；缺少 "
                + str(sorted(expected-found)) + "；額外 " + str(sorted(found-expected)))
        for n, name in enumerate(CASES, 1):
            r = subprocess.run([str(binary), "--exact", PREFIX+name, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False)
            output = r.stdout + r.stderr
            if r.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                raise ValueError(f"F2 bool 多語句真測試 {n}/{len(CASES)} 未通過：{name}\n"
                    + output[-2400:])
            print(f"F2 bool 多語句 {n}/{len(CASES)}：{name}（真 Rust 通過）")
        print("F2 bool-return 多語句／提前返回九項已真正反驗收；其它原義務不得提前 Closed")
        return 0
    except (OSError, ValueError) as error:
        print("F2 bool-return 多語句 Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
