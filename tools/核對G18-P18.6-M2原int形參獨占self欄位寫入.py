#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18/P18.6-M2：原 Checker 同步自借用 int 方法參數真欄位寫入八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX="struct_stage::g18_m2_self_int_parameter_field_write_tests::"
CASES=(
    "m2_param_write_真Checker_int形參動態值9實際寫入self欄位",
    "m2_param_write_main原具名int局部先求值才作方法實參寫欄位",
    "m2_param_write_兩個不同DeclId形參僅允許真正right寫欄位",
    "m2_param_write_原bool與int混合宣告只認int對應的形參欄位來源",
    "m2_param_write_原Checker多次執行每次自動取得新真self票並寫同源值",
    "m2_param_write_原本Lexer_int常值直接寫入路徑未退步",
    "m2_param_write_真方法入口注入故障不得假寫入且須歸還獨占票",
    "m2_param_write_非法型別const接收者錯名與竄改原DeclId一律拒絕",
)


def main()->int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin",type=Path,required=True)
    args=parser.parse_args()
    try:
        binary=args.test_bin.resolve(strict=True)
        listing=subprocess.run(
            [str(binary),"--list"],capture_output=True,text=True,
            encoding="utf-8",check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗："+listing.stderr[-1500:])
        actual={line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")}
        expected={PREFIX+case for case in CASES}
        if len(expected)!=8 or actual!=expected:
            raise ValueError("M2 形參寫欄位 Gate 名稱不符：缺 "
                +str(sorted(expected-actual))+"；多 "+str(sorted(actual-expected)))
        for index,case in enumerate(CASES,1):
            result=subprocess.run(
                [str(binary),"--exact",PREFIX+case,"--test-threads=1"],
                capture_output=True,text=True,encoding="utf-8",check=False)
            output=result.stdout+result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;",output
            ):
                raise ValueError(f"M2 動態形參欄位 {index}/8 {case} 失敗\n"+output[-4000:])
            print(f"M2 真動態形參欄位 {index}/8：{case}（通過）")
        print("G18 M2/B4 原 Checker int 方法參數獨占 self 欄位真寫入八項成功；M2 仍非全閉合。")
        return 0
    except (OSError,ValueError) as error:
        print("G18 M2 方法參數欄位 Gate 失敗："+str(error),file=sys.stderr)
        return 1


if __name__=="__main__":
    raise SystemExit(main())
