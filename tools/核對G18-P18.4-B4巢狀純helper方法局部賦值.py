#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18/P18.4-B4/M3：真 Checker 巢狀純 int 方法局部與 var 更新八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_nested_pure_helper_method_local_tests::"
CASES = (
    "b4_local_nested_方法局部先內sub後外add完成才記初始化",
    "b4_local_nested_先完成局部後以兩層helper更新原var且返回8",
    "b4_local_nested_局部更新完成後Return再執行兩層仍共用唯一票據",
    "b4_local_nested_三層局部與兩個兄弟helper保留真原參數順序",
    "b4_local_nested_第一子helper完成後第二引數溢位不得初始化局部",
    "b4_local_nested_局部內層本體溢位外helper未進並歸還self票",
    "b4_local_nested_更新RHS失敗不提交LocalAssigned只逆序清理已建立局部",
    "b4_local_nested_非純值巢狀算術包裝非法函式與竄改DeclId仍拒絕",
)


def main() -> int:
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
        actual={
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected={PREFIX+name for name in CASES}
        if len(expected)!=8 or actual!=expected:
            raise ValueError("方法局部 Gate 清單不符：缺 "
                +str(sorted(expected-actual))+"；多 "+str(sorted(actual-expected)))
        for index,case in enumerate(CASES,1):
            result=subprocess.run(
                [str(binary),"--exact",PREFIX+case,"--test-threads=1"],
                capture_output=True,text=True,encoding="utf-8",check=False,
            )
            output=result.stdout+result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;",output
            ):
                raise ValueError(f"B4/M3 {index}/8 {case} 失敗：\n"+output[-4000:])
            print(f"B4/M3 巢狀局部／更新 {index}/8：{case}（通過）")
        print("B4/M3 真 Checked Source 方法局部與賦值八項通過；B4/M3 尚未全閉合。")
        return 0
    except (OSError,ValueError) as error:
        print("B4/M3 巢狀局部 Gate 失敗："+str(error),file=sys.stderr)
        return 1


if __name__=="__main__":
    raise SystemExit(main())
