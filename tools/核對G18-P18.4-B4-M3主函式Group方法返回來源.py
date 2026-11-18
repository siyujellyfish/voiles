#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／B4/M3／T3：main Group方法返回真Checker借票跨度十項Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m3_grouped_main_method_return_tests::"
CASES = (
    "b4_grouped_main_原Checker單層括號方法直接回傳真內葉",
    "b4_grouped_main_原Checker二層括號方法Return仍只一個self票據",
    "b4_grouped_main_多層括號false分支不憑字面值偷選helper",
    "b4_grouped_main_原Checker具名bool與int真來源仍保留",
    "b4_grouped_main_原Checker空宣告後賦值再呼叫Group不錯讀佔位",
    "b4_grouped_main_原Checker非公開async直接await括號仍有真安全點",
    "b4_grouped_main_原Checkerasync直接await有前置主函式局部仍禁止延遲FIFO",
    "b4_grouped_main_原Checker不合法括號與缺失方法靜態拒絕",
    "b4_grouped_main_準備後callspan改成外括號不可冒充借票來源",
    "b4_grouped_main_非方法Return不能靠Group隱瞞錯誤執行器路徑",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], capture_output=True, check=False,
            text=True, encoding="utf-8",
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-3500:])
        expected = {PREFIX + name for name in CASES}
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 10 or actual != expected:
            raise ValueError(
                "Group返回來源Gate缺少=" + str(sorted(expected - actual))
                + "；多出=" + str(sorted(actual - expected))
            )
        for index, name in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, check=False, text=True, encoding="utf-8",
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"Group方法Return {index}/10 {name} 失敗\n" + output[-8500:]
                )
            print(f"原Checker Group方法Return {index}/10：{name}（通過）")
        print("十項真Group內葉call_span／Checker BorrowFact／await防偽；B4/M3仍Open。")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 Group方法返回必要Gate失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
