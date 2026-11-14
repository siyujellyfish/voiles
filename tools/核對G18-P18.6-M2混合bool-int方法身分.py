#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6-M2／P18.4-B4-M3：混合 bool/int 方法真 Checker 形參 Gate。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_m3_mixed_bool_int_method_args_tests::"
CASES = (
    "m2_mix_bool在前int在後_真flag_true調用純helper結果6",
    "m2_mix_動態false只走原elsehelper且使用真int形參結果2",
    "m2_mix_方法bool置後仍依原Parser型別位置綁定而非按名稱猜測",
    "m2_mix_多個int夾bool且動態選else取尾int真DeclarationId",
    "m2_mix_巢狀if真bool形參與int參數同時保留唯一身份",
    "m2_mix_未選false條件之helper實參溢位不得提前求值或假Enter",
    "m2_mix_真選中helper本體溢位先內層失敗再歸還self唯一票",
    "m2_mix_錯型別多bool未知形參與竄改不同型別DeclId都被拒絕",
)

def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin",type=Path,required=True)
    args=parser.parse_args()
    try:
        binary=args.test_bin.resolve(strict=True)
        listing=subprocess.run(
            [str(binary),"--list"],capture_output=True,
            text=True,encoding="utf-8",check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗："+listing.stderr[-1500:])
        actual={
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected={PREFIX+name for name in CASES}
        if len(expected)!=8 or actual!=expected:
            raise ValueError("混合 bool/int 方法 Rust 測試漂移：缺 "+str(sorted(expected-actual))
                +"；多 "+str(sorted(actual-expected)))
        for i,name in enumerate(CASES,1):
            completed=subprocess.run(
                [str(binary),"--exact",PREFIX+name,"--test-threads=1"],
                capture_output=True,text=True,encoding="utf-8",check=False,
            )
            output=completed.stdout+completed.stderr
            if completed.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;",output
            ):
                raise ValueError(f"M2/M3 混合 bool/int {i}/8 失敗：{name}\n"+output[-4000:])
            print(f"M2/M3 真混合 bool/int {i}/8：{name}（通過）")
        print("M2/M3 真混合 bool/int 方法八項完成；原 M2/M3/B4 尚 OpenEngineering。")
        return 0
    except (OSError,ValueError) as exc:
        print("M2/M3 真混合 bool/int Gate 失敗："+str(exc),file=sys.stderr)
        return 1

if __name__=="__main__":
    raise SystemExit(main())
