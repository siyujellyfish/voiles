#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T3／R05：真 HC14 BorrowTicket frame 生命期與不可復用八項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_r05_t3_ticket_frame_lifetime_tests::"
CASES = (
    "t3_frame_真兩個兄弟框架各借還獨立票再退出可平衡",
    "t3_frame_第一兄弟已退出卻讓第二兄弟代還原BorrowTicket必拒",
    "t3_frame_已退票的HC14唯一身分在下一兄弟重新借入必拒",
    "t3_frame_外層真票可跨多個已完成普通子函式再歸還",
    "t3_frame_原Caller先借唯一self票由方法正常Return內還票合法",
    "t3_frame_真三層失敗Unwind先內借票歸還再外借票歸還",
    "t3_frame_同一合法函式不同共享來源仍可不依LIFO還票",
    "t3_frame_局部ScopeEnded已清空也不可帶未還票跨已退出frame",

)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run([str(binary), "--list"], capture_output=True,
                                text=True, encoding="utf-8", check=False)
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1500:])
        actual = {line.removesuffix(": test") for line in listed.stdout.splitlines()
                  if line.startswith(PREFIX) and line.endswith(": test")}
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError("T3 真票據 frame 八測試漂移：缺 "
                             + str(sorted(expected - actual)) + "；多 "
                             + str(sorted(actual - expected)))
        for n, name in enumerate(CASES, 1):
            result = subprocess.run([str(binary), "--exact", PREFIX + name,
                                     "--test-threads=1"], capture_output=True,
                                    text=True, encoding="utf-8", check=False)
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(f"T3／R05 frame {n}/8 失敗：{name}\n" + output[-5000:])
            print(f"T3／R05 HC14 借票生命期 {n}/8：{name}（通過）")
        print("T3 真票據 frame 生命期八項已驗證；P18.5 仍 SubsetVerified。")
        return 0
    except (OSError, ValueError) as exc:
        print("T3 BorrowTicket frame Gate 失敗：" + str(exc), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
