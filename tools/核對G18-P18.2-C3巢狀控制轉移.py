#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P18.2 原始 C3 義務真 Parser／Checker／TypedHIR／動態區塊與資源事件驗收。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

CASES = (
    "struct_stage::g18_private_scalar_call_tests::g18_r03_普通函式_for_break_只結束最近一輪並先離域",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_普通函式_for_continue_每輪清除同一宣告身份",
    "struct_stage::g18_private_scalar_call_tests::g18_r02_普通函式_巢狀_for_同名迭代器用不同宣告身份",
    "struct_stage::g18_private_scalar_call_tests::g18_r02_while_逐輪純值局部宣告配合_continue_逆序清除",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_main_while內for_break只清理內層move借用且外層仍繼續",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_main_for內提前return完成真move借用且不進後續迭代",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_main_for_continue每輪真move與借用先還票再走回邊",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_main_if提前return真move借用且跳過後續資源",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_main_while_break前真move借用只建立清理首輪資源",
    "struct_stage::g18_private_scalar_call_tests::g18_r04_同函式_if_move_提前返回後不得遺失原_ResourceId",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_main_if_else假分支不進且不可達末端return零事件",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_普通函式_while_真控制計畫篡改須執行前拒絕",
)


def main() -> int:
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--test-bin", type=Path, required=True)
    cli.add_argument("--check", action="store_true", required=True)
    opts = cli.parse_args()
    try:
        if len(CASES) != 12 or len(set(CASES)) != 12:
            raise ValueError("C3 不可重複或減少原始正反來源")
        exe = opts.test_bin.resolve(strict=True)
        result = subprocess.run([str(exe), "--list"], text=True,
                                encoding="utf-8", capture_output=True, check=False)
        if result.returncode:
            raise ValueError("真 Rust --list 失敗：" + result.stderr[-1600:])
        available = {line.removesuffix(": test") for line in result.stdout.splitlines()
                     if line.endswith(": test")}
        missing = sorted(set(CASES)-available)
        if missing:
            raise ValueError("C3 原 Checker 真來源未存在：" + str(missing))
        for number, name in enumerate(CASES, 1):
            run = subprocess.run([str(exe), "--exact", name, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False)
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                raise ValueError("C3 第 " + str(number) + " 筆失敗：" + name +
                                 "\n" + output[-2600:])
            print(f"P18.2 C3 真控制來源 {number}/12 Passed")
        print("P18.2 C3 正式義務真 Checker／Runtime 12/12 達標")
        return 0
    except (ValueError, OSError) as err:
        print("P18.2 C3 正式義務拒絕：" + str(err), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
