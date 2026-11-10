#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18/P18.1-F2：多字串 ResourceId、BorrowTicket 真交叉正反八項 Gate。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f2_str_multi_resource_tests::"
CASES = (
    "f2_str_multi_雙獨立唯讀字串借用再接收第三個唯一返回資源",
    "f2_str_multi_另一個資源借用時真move並返回同一id沒有重配",
    "f2_str_multi_前序借用和move意圖後續溢位須全部回滾不提前移交",
    "f2_str_multi_兩張borrowticket後續溢位以逆序還票且零callee",
    "f2_str_multi_同一旁觀資源跨兩次返回借用票據不殘留",
    "f2_str_multi_函式進入後溢位必歸還外票再釋放各自擁有者",
    "f2_str_multi_同源重複獨占借用及隱含拷貝原checker先拒絕",
    "f2_str_multi_偽造另一resourceid的借用連結即使計數不變也拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        exe = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(exe), "--list"], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if listed.returncode:
            raise ValueError("原 Rust --list 失敗：" + listed.stderr[-2000:])
        actual = {
            line.removesuffix(": test")
            for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if actual != expected:
            raise ValueError(
                f"Rust 真來源身分不符；缺少 {sorted(expected - actual)}，"
                f"多餘 {sorted(actual - expected)}"
            )
        for i, case in enumerate(CASES, 1):
            run = subprocess.run(
                [str(exe), "--exact", PREFIX + case, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            output = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"F2 多資源 {i}/{len(CASES)}：{case}\n" + output[-3000:]
                )
            print(f"F2 str 多資源真帳 {i}/{len(CASES)}：{case}（通過）")
        print("F2 多 ResourceId/BorrowTicket 八項已真驗收；F2 尚未正式 Closed")
        return 0
    except (OSError, ValueError) as error:
        print("G18 F2 多資源 Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
