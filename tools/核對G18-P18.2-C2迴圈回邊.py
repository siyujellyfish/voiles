#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P18.2 原始 C2 義務真 Parser／Checker／TypedHIR／動態區塊與資源事件驗收。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

CASES = (
    "struct_stage::g18_private_scalar_call_tests::g18_r01_普通函式_while_false_零迭代仍真正檢查且返回",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_普通函式_while_逐輪重讀_bool_並僅更新到達的_int",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_普通函式_while_步數預算耗盡不產生假返回",
    "struct_stage::g18_private_scalar_call_tests::g18_r01_普通函式_for_迭代器真值逐輪參與累加",
    "struct_stage::g18_private_scalar_call_tests::g18_r01_普通函式_for_空半開區間不虛構本體或迭代器",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_普通函式_巨大合法_for_受真步數預算控制",
    "struct_stage::g18_private_scalar_call_tests::g18_r04_真_for_每輪_utf8_各有唯一_resourceid_並先釋放",
    "struct_stage::g18_private_scalar_call_tests::g18_r04_真_while_continue_離域_utf8_不重用舊資源",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_main_while逐輪重讀條件內的真函式來源",
    "struct_stage::g18_private_scalar_call_tests::g18_r01_main_for迭代值在真函式參數中按半開區間逐輪傳入",
)


def main() -> int:
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--test-bin", type=Path, required=True)
    cli.add_argument("--check", action="store_true", required=True)
    opts = cli.parse_args()
    try:
        if len(CASES) != 10 or len(set(CASES)) != 10:
            raise ValueError("C2 不可重複或減少原始正反來源")
        exe = opts.test_bin.resolve(strict=True)
        result = subprocess.run([str(exe), "--list"], text=True,
                                encoding="utf-8", capture_output=True, check=False)
        if result.returncode:
            raise ValueError("真 Rust --list 失敗：" + result.stderr[-1600:])
        available = {line.removesuffix(": test") for line in result.stdout.splitlines()
                     if line.endswith(": test")}
        missing = sorted(set(CASES)-available)
        if missing:
            raise ValueError("C2 原 Checker 真來源未存在：" + str(missing))
        for number, name in enumerate(CASES, 1):
            run = subprocess.run([str(exe), "--exact", name, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False)
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                raise ValueError("C2 第 " + str(number) + " 筆失敗：" + name +
                                 "\n" + output[-2600:])
            print(f"P18.2 C2 真控制來源 {number}/10 Passed")
        print("P18.2 C2 正式義務真 Checker／Runtime 10/10 達標")
        return 0
    except (ValueError, OSError) as err:
        print("P18.2 C2 正式義務拒絕：" + str(err), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
