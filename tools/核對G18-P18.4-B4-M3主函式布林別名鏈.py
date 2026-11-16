#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4-B4／P18.6-M3：main bool 前序複製別名來源十項真Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_main_bool_alias_chain_tests::"
CASES = (
    "b4_bool_alias_真Checker_const_true直接具名複製方法選中內層",
    "b4_bool_alias_真Checker_var_false具名複製保留false與零helper",
    "b4_bool_alias_原Lexer多層Group包覆具名bool及其方法實參",
    "b4_bool_alias_三層具名傳遞各有唯一原CheckerDeclId",
    "b4_bool_alias_兩條獨立鏈不能錯混原Checker形參來源",
    "b4_bool_alias_與前置main純helper交錯仍在借self前真返回",
    "b4_bool_alias_已核准async直接await成功但E1延遲佇列保守拒前置別名",
    "b4_bool_alias_準備後更改別名來源索引必零借票拒絕",
    "b4_bool_alias_同真值不同宣告身分不得交換別名來源",
    "b4_bool_alias_原Checker拒絕未知型別前置自指與未初始化別名",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], check=False, capture_output=True,
            text=True, encoding="utf-8",
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-3500:])
        expected = {PREFIX + name for name in CASES}
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 10 or expected != actual:
            raise ValueError("bool 別名清冊缺少 " + str(sorted(expected - actual))
                             + "；多出 " + str(sorted(actual - expected)))
        for index, name in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                check=False, capture_output=True, text=True, encoding="utf-8",
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"bool 真別名 {index}/10 {name} 失敗\n" + output[-7000:])
            print(f"main bool 別名 {index}/10：{name}（通過）")
        print("十項真來源複製、跨框架、E1保守拒絕與異常防偽；B4/M3仍Open。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 main bool別名必要Gate失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
