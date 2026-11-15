#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／V7-T4／E3：真 async self 終態退休及完整歷史收據十五項。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_v7_async_self_terminal_retirement_tests::"
CASES = (
    "v7_self_retire_正常完成後僅交付一次真方法Outcome不再持有工作幀",
    "v7_self_retire_獨占self方法真動態結果退休後仍可由呼叫者查驗",
    "v7_self_retire_受控溢位先Unwind與還票然後以Failed收據退休",
    "v7_self_retire_Prepared未啟動安全取消後只有Cancelled沒有假借票",
    "v7_self_retire_Suspended取消退出Ready後也能釋放整筆工作",
    "v7_self_retire_尚Prepared或Suspended均不能假裝已完成移除",
    "v7_self_retire_同一真ID只能交付一次且不得再偽造喚醒",
    "v7_self_retire_交錯取消與已完成退休後仍可註冊新的唯一工作ID",
    "v7_self_retire_已退休的原Wake全域歷史不可事後整筆抹除",
    "v7_self_retire_同時抹除原執行歷史與就緒帳也不能騙過已退休收據",
    "v7_self_retire_退休收據的原全域事件位置遭篡改即拒絕新增工作",
    "v7_self_retire_退休成功結果不能竄改終態為Failed或取消",
    "v7_self_retire_退休取消收據遭刪後Ready空也不代表未取消",
    "v7_self_retire_某份退休收據變造不可導致另一份Pending部分取消",
    "v7_self_retire_整批取消後退休全部工作仍保留完整取消順序",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], check=False, capture_output=True,
            text=True, encoding="utf-8",
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1200:])
        expected = {PREFIX + case for case in CASES}
        actual = {
            line.removesuffix(": test")
            for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 15 or actual != expected:
            raise ValueError(
                "T4/E3 退休 Gate 缺失="
                + str(sorted(expected-actual))
                + "，多餘=" + str(sorted(actual-expected))
            )
        for index, case in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                check=False, capture_output=True, text=True, encoding="utf-8",
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"T4/E3 async self 退休 {index}/15 {case} 失敗:\n"
                    + output[-4000:]
                )
            print(f"T4/E3 async self 退休 {index}/15：{case}（通過）")
        print("T4/E3 原Checker async self 真退休與歷史收據十五項通過；P18.6仍未Passed。")
        return 0
    except (OSError, ValueError) as error:
        print("T4/E3 async self 退休 Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
