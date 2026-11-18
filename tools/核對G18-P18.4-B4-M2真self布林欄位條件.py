#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／B4／M2：原 Checker self.bool 真方法分支、BorrowTicket、P16、V7 十四項驗收。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_b4_m2_self_bool_condition_tests::"
CASES = (
    "b4_self_bool_原Checker外層self_ready_true以唯一共享票選中",
    "b4_self_bool_原Checker外層self_ready_false不進巢狀helper",
    "b4_self_bool_方法內層self_backup_false由相同共享票真讀",
    "b4_self_bool_原Lexer多層Group_內外self欄位各自證實",
    "b4_self_bool_相同bool真值不同欄位不可擅自視為同一身分",
    "b4_self_bool_原Checker_const_bool欄位也可共享自借讀條件",
    "b4_self_bool_前序main寫入bool後方法真讀最新值而非預設",
    "b4_self_bool_前序main寫true讓原false欄位在方法內真選中",
    "b4_self_bool_獨占self寫int的分支可在同一票讀bool條件",
    "b4_self_bool_方法真共享讀取後故障反向還票且可再次執行",
    "b4_self_bool_變造外層或內層欄位封存均先SourceChanged",
    "b4_self_bool_原Checker_async_self直接await真wake後讀bool條件",
    "b4_self_bool_async_安全點取消未執行方法不產生借票或helper",
    "b4_self_bool_原Checker拒絕非bool或不存在的self條件",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        executable = args.test_bin.resolve(strict=True)
        tests = subprocess.run(
            [str(executable), "--list"], capture_output=True, check=False,
            text=True, encoding="utf-8",
        )
        if tests.returncode != 0:
            raise ValueError("Rust --list 執行失敗：" + tests.stderr[-4000:])
        expected = {PREFIX + name for name in CASES}
        found = {
            line.removesuffix(": test") for line in tests.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 14 or found != expected:
            raise ValueError(
                "self.bool 方法條件 Gate 缺失=" + str(sorted(expected - found))
                + "；多出=" + str(sorted(found - expected))
            )
        for number, case in enumerate(CASES, 1):
            run = subprocess.run(
                [str(executable), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, check=False, text=True, encoding="utf-8",
            )
            output = run.stdout + run.stderr
            if run.returncode != 0 or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;",
                output,
            ):
                raise ValueError(
                    f"原 Checker self.bool {number}/14 {case} 失敗\n"
                    + output[-10000:]
                )
            print(f"G18 B4/M2 self.bool 真 P16／V7 {number}/14：{case} 通過")
        print("十四項原 Checker self.bool 真借票、安全退場與直接 await 驗收全部通過；"
              "B4/M2/A1/A2 維持 OpenEngineering。")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 self.bool 真條件安全 Gate 失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
