#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4／P18.6-M3：main Copyable bool 真前向局部複製九項。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_main_bool_copy_chain_tests::"
CASES = (
    "b4_bool_chain_原Checker真const_bool前向複製到方法旗標",
    "b4_bool_chain_var_false經多層Group名稱複製不進入任何helper",
    "b4_bool_chain_三層Copyable來源不同DeclId但同一真值",
    "b4_bool_chain_兩個bool分支參數各從真前向局部讀值",
    "b4_bool_chain_main純int_helper與bool別名在self借票前各守真動態順序",
    "b4_bool_chain_async_self原Checker直接await不擴張延遲佇列",
    "b4_bool_chain_準備後別名來源DeclId偽造不得提前進self框架",
    "b4_bool_chain_準備後別名值變造不得以相同bool型別冒充原值",
    "b4_bool_chain_非法前向未初始化自引或int來源原Checker仍拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        test_bin = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(test_bin), "--list"], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-4000:])
        expected = {PREFIX + name for name in CASES}
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 9 or expected != actual:
            raise ValueError("main bool複製鏈缺=" + str(sorted(expected-actual))
                             + "；多=" + str(sorted(actual-expected)))
        for index, name in enumerate(CASES, 1):
            run = subprocess.run(
                [str(test_bin), "--exact", PREFIX + name, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"bool複製鏈 {index}/9 {name} 失敗\n" + output[-7500:])
            print(f"main bool複製鏈 {index}/9：{name}（通過）")
        print("九項真Copyable bool局部跨方法值槽正反；B4/M3仍OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 bool複製鏈Gate失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
