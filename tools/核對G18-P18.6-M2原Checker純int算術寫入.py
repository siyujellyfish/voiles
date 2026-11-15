#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6-M2：原 Checker self 純 int 算術獨占寫入八項。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX="struct_stage::g18_m2_self_checked_int_write_tests::"
CASES=(
    "m2_checked_write_真int參數加常值後只在持獨占票時寫入12",
    "m2_checked_write_兩int真DeclId彼此加法後寫入16",
    "m2_checked_write_嵌套左右減法仍依原Checker有號i64求值",
    "m2_checked_write_main先真普通helper求參再寫入self欄位",
    "m2_checked_write_真int加法溢位不得假寫self並先退出真借票",
    "m2_checked_write_真int減法下溢受控退票可重複執行合法原計畫",
    "m2_checked_write_未核准來源不得繞過Checker與借用安全",
    "m2_checked_write_同值造假算式或夾帶動態寫入來源必於執行前拒絕",
)


def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin",type=Path,required=True)
    args=parser.parse_args()
    try:
        exe=args.test_bin.resolve(strict=True)
        listed=subprocess.run([str(exe),"--list"],
            capture_output=True,text=True,encoding="utf-8",check=False)
        if listed.returncode:
            raise ValueError("Rust --list 失敗："+listed.stderr[-1800:])
        seen={line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")}
        expected={PREFIX+name for name in CASES}
        if len(expected)!=8 or expected!=seen:
            raise ValueError("M2 真算術寫入 Gate 名稱不一致，缺 "
                +str(sorted(expected-seen))+"；多 "+str(sorted(seen-expected)))
        for number,name in enumerate(CASES,1):
            process=subprocess.run([str(exe),"--exact",PREFIX+name,
                "--test-threads=1"],capture_output=True,text=True,
                encoding="utf-8",check=False)
            output=process.stdout+process.stderr
            if process.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;",output):
                raise ValueError(f"M2 真算術寫入 {number}/8 {name} 失敗\n"+output[-4500:])
            print(f"M2 真算術獨占寫入 {number}/8：{name}（通過）")
        print("M2 真 Checker int 算術寫入八項通過；M2／P18.6 尚未全閉合。")
        return 0
    except (OSError,ValueError) as error:
        print("M2 純 int 算術寫入 Gate 失敗："+str(error),file=sys.stderr)
        return 1


if __name__=="__main__":
    raise SystemExit(main())
