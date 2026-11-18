#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／B4/M3：原Checker main bool宣告與賦值讀真owner欄位十二項Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_main_owner_bool_local_read_tests::"
CASES = (
    "b4_bool_local_原Checker_true欄位初始化const真局部再作方法bool參",
    "b4_bool_local_原Checker_false欄位在main局部傳遞後零helper",
    "b4_bool_local_原Lexer多重Group包覆欄位bool初始化真值槽",
    "b4_bool_local_原Checker未初始化var首次賦值讀真owner_bool",
    "b4_bool_local_var重賦必讀原P16值而不是宣告常值",
    "b4_bool_local_已複製bool局部再賦false不得污染原owner欄位",
    "b4_bool_local_欄位真值先成main別名再傳方法仍唯一DeclId",
    "b4_bool_local_先更新int後main從同一P16讀bool再用最新int實參",
    "b4_bool_local_真bool來源方法中注入故障必唯一歸票",
    "b4_bool_local_封存Initializer欄位身分被改必SourceChanged",
    "b4_bool_local_封存賦值欄位DeclId被改必SourceChanged",
    "b4_bool_local_原Checker拒絕int欄位冒充bool與未知欄位",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], capture_output=True, check=False,
            text=True, encoding="utf-8",
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-3000:])
        required = {PREFIX + name for name in CASES}
        found = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(required) != 12 or found != required:
            raise ValueError(
                "main bool欄位局部Gate缺失=" + str(sorted(required - found))
                + "；多出=" + str(sorted(found - required))
            )
        for n, name in enumerate(CASES, 1):
            proc = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, check=False, text=True, encoding="utf-8",
            )
            output = proc.stdout + proc.stderr
            if proc.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"bool真局部 {n}/12 {name} 失敗\n" + output[-9000:])
            print(f"G18 B4/M3 main bool真P16值槽 {n}/12：{name} 通過")
        print("十二項原Checker bool欄位與main局部真值槽Gate成功；B4/M3仍OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 main bool欄位局部Gate失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
