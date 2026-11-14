#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T4：原 Checker 真 Prepared／Suspended 取消及 shutdown 唯一證據。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_t4_pending_cancel_evidence_tests::"
CASES = (
    "t4_pending_從未啟動Prepared可取消但絕不憑空產生函式或資源事件",
    "t4_pending_真正Suspended在安全點取消保留R05與P16完整反序退出",
    "t4_pending_完成工作偽造Prepared不能消耗真結果也不可再取消",
    "t4_pending_已經取消的Prepared若偽造回待命不得重新取消",
    "t4_pending_從未啟動Prepared卻已有Unwind事件不能藉取消洗掉證據",
    "t4_pending_假Suspended但無原poll和checkpoint不得擬作安全點",
    "t4_pending_真Completed偽裝Suspended不可以重用已完成的身份",
    "t4_pending_真wake後Suspended仍可取消並按逆登錄順序shutdown",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], capture_output=True,
            text=True, encoding="utf-8", check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1500:])
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError(
                "T4 待取消真工作測試來源漂移：缺 "
                + str(sorted(expected - actual))
                + "；多 " + str(sorted(actual - expected))
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
                    f"T4 準備／暫停真取消 {index}/8 失敗：{case}\n"
                    + output[-4000:]
                )
            print(f"T4 真待取消工作 {index}/8：{case}（通過）")
        print("G18 T4/V7-E3 真 Prepared／Suspended 取消八項完成；T4 仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 T4 取消工作真證據 Gate 失敗：" + str(exc), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
