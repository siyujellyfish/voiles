#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.5-T4：原 Checker 真 FIFO 遺失待執行憑據二十五項 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_t4_fifo_missing_ready_conservation_tests::"
CASES = (
    "t4_missing_單一Prepared被偷刪ready不可假報空佇列",
    "t4_missing_後一Prepared被刪仍不可先執行佇列頭",
    "t4_missing_真wake後刪除唯一ready不得使Suspended假空",
    "t4_missing_兩個真wake刪掉尾端也不得執行合法頭",
    "t4_missing_整批關閉遇Prepared中間憑據遺失全員不可先取消",
    "t4_missing_合法Suspended尚未wake應真回報無待驅動工作",
    "t4_missing_合法wake後安全取消會消耗憑據並允許真空佇列",
    "t4_missing_合法完成退休後歷史仍存但不能假生新ready",
    "t4_missing_退休後最後Submitted歷史被抹除不得假回空",
    "t4_missing_雙工作退休後裁掉尾筆提交仍拒絕新工作",
    "t4_missing_退休後next_id大於真提交歷史不得續發",
    "t4_missing_全空佇列偽造發號上限不可冒充零工作",
    "t4_missing_退休完成後同步刪除Woken與wake收據也不可假報空",
    "t4_missing_退休完成後篡改wake時間不得跳過原poll時序",
    "t4_missing_退休後插入不存在工作身分的幽靈poll必拒絕",
    "t4_missing_退休完成後偽造Completed再Suspended不可重啟已交付工作",
    "t4_missing_另一Prepared仍存活時退休wake竄改須整批零取消",
    "t4_missing_退休後偽造未知工作取消歷史不得延用空佇列",
    "t4_missing_真退休完成後整段pollwake與授權被同步抹除不能假空",
    "t4_missing_真退休取消後同步清空poll取消歷史仍不能假空",
    "t4_missing_已退休唯一終態收據被刪除不可續發同一工作身份",
    "t4_missing_兩個退休取消歷史交換次序即使數量相同亦拒絕",
    "t4_missing_仍有Prepared時退休收據缺失不得先部分shutdown",
    "t4_missing_兩個退休真poll位置對換不能冒充同數量歷史",
    "t4_missing_退休Cancelled收據篡改為Succeeded不能掩蓋未完成工作",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run(
            [str(binary), "--list"], capture_output=True, text=True,
            encoding="utf-8", check=False,
        )
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1500:])
        found = {
            line.removesuffix(": test")
            for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if len(expected) != 25 or found != expected:
            raise ValueError(
                "T4 ready 遺失測試清單不一致，缺 "
                + str(sorted(expected - found)) + "；多 "
                + str(sorted(found - expected))
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
                    f"T4 遺失憑據 {index}/25 失敗：{case}\n" + output[-4000:]
                )
            print(f"T4 真 ready 守恆 {index}/25：{case}（通過）")
        print("G18 T4/E2 真 FIFO 未消耗憑據雙向守恆二十五項成功；原 T4 未全項閉合。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 T4 FIFO ready 守恆 Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
