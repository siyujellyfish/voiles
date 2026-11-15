#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6-M2／P18.4-B4：原 Checker 直接純 int helper 寫 self 九項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_direct_helper_self_write_tests::"
CASES = (
    "m2_direct_helper_借self唯一票後真add返回12再提交欄位",
    "m2_direct_helper_兩個正式int形參值必逐一綁定各自DeclId",
    "m2_direct_helper_加減左右兩個真函式依Parser求值後寫18",
    "m2_direct_helper_巢狀原Checker兩層helper先內後外真寫10",
    "m2_direct_helper_實參先算術溢位不准假進add且真還票",
    "m2_direct_helper_真add本體溢位先子函式失敗再歸還借票",
    "m2_direct_helper_外層純加法溢位保留已成功helper的真退出",
    "m2_direct_helper_經原Checker已證同步純值函式可重複執行不遺留票",
    "m2_direct_helper_非法型別資源來源與竄改真函式圖都須拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], capture_output=True, text=True,
            encoding="utf-8", check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1600:])
        found = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 9 or found != expected:
            raise ValueError(
                "M2 直接 helper Gate 清單不一致，缺 "
                + str(sorted(expected - found)) + "；多 "
                + str(sorted(found - expected))
            )
        for index, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"M2 直接 helper {index}/9 失敗：{case}\n"
                    + output[-4500:]
                )
            print(f"M2/B4 直接純 helper {index}/9：{case}（通過）")
        print("M2/B4 原 Checker 直接純 helper RHS 九項 Gate 通過；原 M2/B4 未全閉合。")
        return 0
    except (OSError, ValueError) as error:
        print("M2/B4 直接純 helper Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
