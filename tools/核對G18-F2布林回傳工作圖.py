#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／F2 bool-return 可達來源的原 Checker→真 Runtime 六項嚴格 Gate。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f2_bool_return_tests::"
CASES = (
    "f2_bool_return_原checker真bool葉函式作為int函式的布林引數",
    "f2_bool_return_false與true真正返回維持獨立型別與求值順序",
    "f2_bool_return_無參數真bool函式與原checker零參數分類",
    "f2_bool_return_兩層bool實參函式由真工作棧逐層退出",
    "f2_bool_return_父函式字串借用在bool子工作結束後安全歸還",
    "f2_bool_return_不得冒用整數返回或竄改bool函式原始計畫",
)

def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--test-bin", required=True, type=Path)
    args = p.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run([str(binary), "--list"], capture_output=True,
            text=True, encoding="utf-8", check=False)
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1500:])
        actual = {line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")}
        expected = {PREFIX + x for x in CASES}
        if actual != expected:
            raise ValueError("bool-return 測試身分不符，缺少 "
                + str(sorted(expected-actual)) + "；額外 " + str(sorted(actual-expected)))
        for n, case in enumerate(CASES, 1):
            r = subprocess.run([str(binary), "--exact", PREFIX+case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False)
            combined = r.stdout + r.stderr
            if r.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", combined
            ):
                raise ValueError(f"bool-return 真測試 {n}/{len(CASES)} 失敗：{case}\n"
                    + combined[-2400:])
            print(f"F2 bool-return {n}/{len(CASES)}：{case}（通過）")
        print("F2 bool-return 六項來源真實通過；F2 全部合法 bool 控制圖／返回仍不得標 Closed")
        return 0
    except (OSError, ValueError) as err:
        print("G18 F2 bool-return Gate 失敗：" + str(err), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
