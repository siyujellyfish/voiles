#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4/M3：原 Checker 同步方法實參 int 算式內真 helper 八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_method_argument_int_call_arithmetic_tests::"
CASES = (
    "b4_math_call_左常值加原Checker普通函式結果8且函式先退self後借",
    "b4_math_call_左右算術順序互換仍各自從同一真frame求值",
    "b4_math_call_兩個同源不同函式可作左右算術運算元",
    "b4_math_call_多層巢狀helper結果再算術亦保留真進退順序",
    "b4_math_call_先完成左函式後右算術溢位不得提前進方法",
    "b4_math_call_左算術提前溢位禁止右函式虛構Enter",
    "b4_math_call_普通函式本體溢位先退出該函式不得借self",
    "b4_math_call_非法呼叫型別資源或竄改int算式來源必拒絕",
)


def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin",type=Path,required=True)
    args=parser.parse_args()
    try:
        binary=args.test_bin.resolve(strict=True)
        listed=subprocess.run(
            [str(binary),"--list"],capture_output=True,
            text=True,encoding="utf-8",check=False,
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗："+listed.stderr[-1500:])
        actual={line.removesuffix(": test")
            for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")}
        expected={PREFIX+name for name in CASES}
        if len(expected)!=8 or actual!=expected:
            raise ValueError("方法實參 Call 算式清單不符：缺 "
                +str(sorted(expected-actual))+"；多 "+str(sorted(actual-expected)))
        for number,case in enumerate(CASES,1):
            result=subprocess.run(
                [str(binary),"--exact",PREFIX+case,"--test-threads=1"],
                capture_output=True,text=True,encoding="utf-8",check=False)
            output=result.stdout+result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;",output
            ):
                raise ValueError(f"B4/M3 int Call 算式 {number}/8 {case} 失敗\n"+output[-4000:])
            print(f"B4/M3 真 int 算式 helper {number}/8：{case}（通過）")
        print("G18 B4/M3 方法實參原 Checker int+Call 八項通過；B4/M3 未全閉合。")
        return 0
    except (OSError,ValueError) as exc:
        print("G18 B4/M3 int Call 算式 Gate 失敗："+str(exc),file=sys.stderr)
        return 1


if __name__=="__main__":
    raise SystemExit(main())
