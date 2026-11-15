#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T3／V7：普通async延期工作原Checker完整來源重驗十項。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_t3_v7_ordinary_deferred_preflight_tests::"
CASES = (
    "t3_v7_ordinary_preflight_真完整Checker來源與直接await正常執行共用",
    "t3_v7_ordinary_preflight_Prepared回傳HIR遭變造取消前必拒絕且零移交",
    "t3_v7_ordinary_preflight_Suspended原awaitToken遭變造不得偽造喚醒",
    "t3_v7_ordinary_preflight_旁觀污染計畫不得讓另一合法任務先喚醒",
    "t3_v7_ordinary_preflight_污染旁觀工作不得先取消另一份合法來源",
    "t3_v7_ordinary_preflight_受損計畫不得發行新工作ID且來源可恢復",
    "t3_v7_ordinary_preflight_污染原普通函式簽章不得提前進Suspended",
    "t3_v7_ordinary_preflight_正常資源僅真wake後建立釋放且取消零事件",
    "t3_v7_ordinary_preflight_受控溢位保留原失敗結果不錯拒後續合法工作",
    "t3_v7_ordinary_preflight_工作ID耗盡應保守拒絕不得panic或回繞",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], capture_output=True,
            text=True, encoding="utf-8", check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-2500:])
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 10 or expected != actual:
            raise ValueError(
                "普通async延期來源預檢真清冊不一致；缺少 "
                + str(sorted(expected-actual)) + "、多出 " + str(sorted(actual-expected))
            )
        for index, case in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"普通async延期原Checker {index}/10 {case} 失敗\n" + output[-7000:]
                )
            print(f"普通async延期全來源 {index}/10：{case}（通過）")
        print("十項真原Parser/Checker與資源安全點驗收；T3/T4/A1/A2原義務仍未Closed。")
        return 0
    except (OSError, ValueError) as exc:
        print("普通async延期原Checker全來源Gate失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
