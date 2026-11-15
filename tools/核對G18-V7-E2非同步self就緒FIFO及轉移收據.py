#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／V7-E2：真正 async self 就緒 FIFO、獨立收據與安全取消十五項。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_v7_async_self_fifo_receipts_tests::"
CASES = (
    "v7_self_fifo_尚未suspend的真Checked工作不能提前驅動或簽票",
    "v7_self_fifo_就緒依真suspend順序不是ID大小",
    "v7_self_fifo_連續驅動三個真self借票互不重疊且逐次歸還",
    "v7_self_fifo_失敗工作先Unwind還票不妨礙後面就緒工作",
    "v7_self_fifo_取消首筆Ready只能移除該筆其餘依次驅動",
    "v7_self_fifo_明確非FIFO手動wake只能移除指定ID不竄改餘下順序",
    "v7_self_fifo_整批關閉先驗證後依逆登錄次序取消未啟動工作",
    "v7_self_fifo_終結工作保留真退出結果而關閉只取消剩餘等待者",
    "v7_self_fifo_手動插入幽靈Ready在實際wake前即拒絕且零提交",
    "v7_self_fifo_交換兩張真Ready順序即使ID狀態都合法仍不可驅動",
    "v7_self_fifo_假造重複就緒收據無法通過全狀態重播",
    "v7_self_fifo_已成功的真方法不能改寫成Failed或失去原退出證據",
    "v7_self_fifo_取消歷史若被刪除不能與空Ready佯稱從未取消",
    "v7_self_fifo_一筆終態已變造不可先取消另一合法工作才報錯",
    "v7_self_fifo_原Checked工作來源受損不得消耗Ready也不得假完成",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        found = subprocess.run(
            [str(binary), "--list"], check=False, capture_output=True,
            text=True, encoding="utf-8",
        )
        if found.returncode:
            raise ValueError("Rust --list 失敗：" + found.stderr[-1800:])
        expected = {PREFIX + case for case in CASES}
        actual = {
            line.removesuffix(": test") for line in found.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(expected) != 15 or actual != expected:
            raise ValueError(
                f"V7 E2 async self Gate 缺失={sorted(expected - actual)}；"
                f"多餘={sorted(actual - expected)}"
            )
        for index, name in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                check=False, capture_output=True, text=True, encoding="utf-8",
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"E2 FIFO {index}/15 {name} 失敗：\n" + output[-4000:]
                )
            print(f"V7 async self FIFO {index}/15：{name}（通過）")
        print("V7/E2 原Checker async self 十五項真FIFO/取消/收據 Gate 通過；P18.6 尚未 Passed。")
        return 0
    except (ValueError, OSError) as error:
        print("V7 E2 async self Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
