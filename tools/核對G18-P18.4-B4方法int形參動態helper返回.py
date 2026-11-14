#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4／M3：方法 int 原 DeclId 動態實參 Return helper Gate。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_dynamic_int_return_helper_tests::"
CASES = (
    "b4_param_單int形參在末尾Return當下真求helper結果6",
    "b4_param_同來源兩次不同main實參改變方法真返回結果",
    "b4_param_兩個不同方法int形參綁定原DeclId及其宣告順序",
    "b4_param_方法形參組括號與左右複合純int算術保留同一真HIR",
    "b4_param_if_true選中提前Return以目前int形參動態進helper",
    "b4_param_已綁方法真形參後在helper返回算術溢位內後外退出",
    "b4_param_方法真形參本身加法溢位在進helper前回滾票",
    "b4_param_原Checker禁止錯型別未知方法形參巢狀Call與竄改原身份",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary=args.test_bin.resolve(strict=True)
        listing=subprocess.run(
            [str(binary),"--list"],capture_output=True,
            text=True,encoding="utf-8",check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1500:])
        actual={
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected={PREFIX+name for name in CASES}
        if len(expected)!=8 or actual!=expected:
            raise ValueError("動態 int helper 名稱漂移：缺 "+str(sorted(expected-actual))
                +"；多 "+str(sorted(actual-expected)))
        for i,name in enumerate(CASES,1):
            checked=subprocess.run(
                [str(binary),"--exact",PREFIX+name,"--test-threads=1"],
                capture_output=True,text=True,encoding="utf-8",check=False,
            )
            output=checked.stdout+checked.stderr
            if checked.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;",output
            ):
                raise ValueError(f"動態 int helper {i}/8 失敗：{name}\n"+output[-4000:])
            print(f"B4/M3 動態 int 形參真 Return {i}/8：{name}（通過）")
        print("G18 B4/M3 方法動態 int 形參八項成功；B4/M3 尚 OpenEngineering。")
        return 0
    except (OSError,ValueError) as exc:
        print("B4/M3 動態形參 Return Gate 失敗："+str(exc),file=sys.stderr)
        return 1

if __name__=="__main__":
    raise SystemExit(main())
