#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P18.2 原始 C4 義務真 Parser／Checker／TypedHIR／動態區塊與資源事件驗收。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

CASES = (
    "struct_stage::g18_private_scalar_call_tests::g18_r02_真str無初值單純宣告離域零ResourceId",
    "struct_stage::g18_private_scalar_call_tests::g18_r02_無初值str以已核准move首次接收原ResourceId",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_for_逐輪無初值str先賦值每輪唯一ResourceId清理",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_for_逐輪var_int先保留空槽再第一次賦值與continue",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_while_continue_未初始化bool與int只執行選中路徑",
    "struct_stage::g18_private_scalar_call_tests::g18_r05_for_逐輪真_utf8_釋放不得延後至下一輪或函式退出",
    "struct_stage::g18_private_scalar_call_tests::g18_r04_同函式_for_逐輪真_move_每次是新_resourceid",
    "struct_stage::g18_private_scalar_call_tests::g18_r04_同函式_if_內_move_外層所有者成空_內層只釋放一次",
    "struct_stage::g18_private_scalar_call_tests::g18_r02_main_if內真str局部move給第二局部仍僅釋放原resourceid一次",
    "struct_stage::g18_private_scalar_call_tests::g18_r02_r03_main_for外層utf8每輪重借同源須逐次還票",
    "struct_stage::g18_private_scalar_call_tests::g18_r05_偽造未初始化局部冒充已初始化或刪除真離域須拒絕",
    "struct_stage::g18_private_scalar_call_tests::g18_r03_普通函式_for_迭代身份計畫偽造必先拒絕",
)


def main() -> int:
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--test-bin", type=Path, required=True)
    cli.add_argument("--check", action="store_true", required=True)
    opts = cli.parse_args()
    try:
        if len(CASES) != 12 or len(set(CASES)) != 12:
            raise ValueError("C4 不可重複或減少原始正反來源")
        exe = opts.test_bin.resolve(strict=True)
        result = subprocess.run([str(exe), "--list"], text=True,
                                encoding="utf-8", capture_output=True, check=False)
        if result.returncode:
            raise ValueError("真 Rust --list 失敗：" + result.stderr[-1600:])
        available = {line.removesuffix(": test") for line in result.stdout.splitlines()
                     if line.endswith(": test")}
        missing = sorted(set(CASES)-available)
        if missing:
            raise ValueError("C4 原 Checker 真來源未存在：" + str(missing))
        for number, name in enumerate(CASES, 1):
            run = subprocess.run([str(exe), "--exact", name, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False)
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                raise ValueError("C4 第 " + str(number) + " 筆失敗：" + name +
                                 "\n" + output[-2600:])
            print(f"P18.2 C4 真控制來源 {number}/12 Passed")
        print("P18.2 C4 正式義務真 Checker／Runtime 12/12 達標")
        return 0
    except (ValueError, OSError) as err:
        print("P18.2 C4 正式義務拒絕：" + str(err), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
