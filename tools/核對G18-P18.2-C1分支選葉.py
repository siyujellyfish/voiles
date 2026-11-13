#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P18.2 原始 C1 義務真 Parser／Checker／TypedHIR／動態區塊與資源事件驗收。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

CASES = (
    "struct_stage::g18_private_scalar_call_tests::g18_r03_已核准_var_bool_更新後真_if_重讀而非使用宣告舊值",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_else_if_依真_checker_條件順序選擇唯一提前返回",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_if_false_未選中分支不得寫入而仍由原值返回",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_else_if_具名_bool_分支更新與順序後續函式呼叫",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_真巢狀_if_葉提前返回保留雙層條件順序",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_外層_false_路徑內的_else_巢狀_if_只執行選中的葉",
    "struct_stage::g18_private_scalar_call_tests::g18_r04_真_if_區塊_utf8_只在選中分支建立且退域釋放",
    "struct_stage::g18_private_scalar_call_tests::g18_r04_選中_if_字串覆寫原子提交_未選分支零資源事件",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_main_if_else全分支真返回且末端不可達常值不需假hir",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_main_if_false未選多層函式不造資源真else只借用一次",
)


def main() -> int:
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--test-bin", type=Path, required=True)
    cli.add_argument("--check", action="store_true", required=True)
    opts = cli.parse_args()
    try:
        if len(CASES) != 10 or len(set(CASES)) != 10:
            raise ValueError("C1 不可重複或減少原始正反來源")
        exe = opts.test_bin.resolve(strict=True)
        result = subprocess.run([str(exe), "--list"], text=True,
                                encoding="utf-8", capture_output=True, check=False)
        if result.returncode:
            raise ValueError("真 Rust --list 失敗：" + result.stderr[-1600:])
        available = {line.removesuffix(": test") for line in result.stdout.splitlines()
                     if line.endswith(": test")}
        missing = sorted(set(CASES)-available)
        if missing:
            raise ValueError("C1 原 Checker 真來源未存在：" + str(missing))
        for number, name in enumerate(CASES, 1):
            run = subprocess.run([str(exe), "--exact", name, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False)
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                raise ValueError("C1 第 " + str(number) + " 筆失敗：" + name +
                                 "\n" + output[-2600:])
            print(f"P18.2 C1 真控制來源 {number}/10 Passed")
        print("P18.2 C1 正式義務真 Checker／Runtime 10/10 達標")
        return 0
    except (ValueError, OSError) as err:
        print("P18.2 C1 正式義務拒絕：" + str(err), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
