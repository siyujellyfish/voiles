#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T4：真 Checker 協作工作 wake token 與 FIFO poll 次數八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_t4_wake_token_ledger_tests::"
CASES = (
    "t4_wake_token_Prepared憑空新增wake歷史不得開始執行",
    "t4_wake_token_真Suspended被偷塞ready不可跳過wake守門",
    "t4_wake_token_偽wake歷史沒有ready時不可再簽第二張",
    "t4_wake_token_重複偽wake紀錄不得授予新執行權",
    "t4_wake_token_完成後刪除真wake歷史不得安全退休",
    "t4_wake_token_真wake後尚未poll合法取消並完整歸還資源",
    "t4_wake_token_已取消工作多出一筆偽wake不得退休",
    "t4_wake_token_整批關閉遇假wake須全部保留後續正常逆序取消",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], capture_output=True,
            text=True, encoding="utf-8", check=False,
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1500:])
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError(
                "T4 wake 授權測試名稱不一致：缺 "
                + str(sorted(expected - actual)) + "；多 "
                + str(sorted(actual - expected))
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
                    f"T4 wake 授權 {index}/8 失敗：{case}\n" + output[-4000:]
                )
            print(f"T4 wake 真授權 {index}/8：{case}（通過）")
        print("G18 T4/V7-E2 原 Checker wake 授權與 poll 次數八項通過；T4 尚未閉合。")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 T4 wake token Gate 失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
