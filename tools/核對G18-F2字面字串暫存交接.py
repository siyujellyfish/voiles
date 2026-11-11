#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P18.1-F2：原 Lexer str 常值直接作實參的真資源雙帳八項 Gate。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f2_literal_str_temporary_tests::"
CASES = (
    "f2_literal_arg_直接utf8字面作str形參原id移交並真正返回",
    "f2_literal_arg_int返回接收字面並在子框架清理",
    "f2_literal_arg_bool返回接收字面沒有整數偽裝",
    "f2_literal_arg_兩個不同字面資源按實參原序移交且只各釋放一次",
    "f2_literal_arg_完成字面後序溢位零子框架零轉交但必釋放",
    "f2_literal_arg_被呼叫端溢位仍從真子框架單次清理",
    "f2_literal_arg_字面先交內層返回再交外層始終同一resourceid",
    "f2_literal_arg_偽造常值暫存id及移除來源事件必遭真稽核拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        exe = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(exe), "--list"], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-2000:])
        actual = {
            line.removesuffix(": test")
            for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if actual != expected:
            raise ValueError(
                f"測試身分不符；缺少 {sorted(expected - actual)}，"
                f"多餘 {sorted(actual - expected)}"
            )
        for i, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(exe), "--exact", PREFIX + case, "--test-threads=1"],
                text=True, encoding="utf-8",
                capture_output=True, check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;",
                output,
            ):
                raise ValueError(f"F2 字面交接 {i}/{len(CASES)}：{case}\n"
                                 + output[-3500:])
            print(f"F2 字面 str 真入參 {i}/{len(CASES)}：{case}（通過）")
        print("F2 原 Lexer str 字面交接八項 Gate 通過；F2 仍 OpenEngineering")
        return 0
    except (OSError, ValueError) as error:
        print("G18-F2 字面 str Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
