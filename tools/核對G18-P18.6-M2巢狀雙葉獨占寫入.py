#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.6 M2：巢狀雙葉真獨占寫入與唯一 BorrowTicket 錯誤退出。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_nested_exclusive_write_tests::"
CASES = (
    "m2_nested_write_動態true只寫19後返回24且false葉不執行",
    "m2_nested_write_動態false只寫23後減30返回負7",
    "m2_nested_write_外層false不觀測內層也不寫任一葉",
    "m2_nested_write_只有true葉寫入時false葉不寫仍需入口獨占",
    "m2_nested_write_只有else葉寫入時true葉不寫返回12",
    "m2_nested_write_寫入完成後加法溢位真Unwind且仍退唯一票",
    "m2_nested_write_竄改兩葉原寫入計畫拒絕且方法故障零假更新",
    "m2_nested_write_非法常值欄位與不可寫receiver由原Checker拒絕",
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
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1400:])
        actual = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if len(expected) != 8 or actual != expected:
            raise ValueError("M2 巢狀雙葉寫入來源漂移：缺 "
                             + str(sorted(expected - actual))
                             + "；多 " + str(sorted(actual - expected)))
        for i, case in enumerate(CASES, 1):
            run = subprocess.run(
                [str(binary), "--exact", PREFIX + case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            out = run.stdout + run.stderr
            if run.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", out,
            ):
                raise ValueError(f"M2 巢狀雙葉寫入 {i}/8 失敗：{case}\n" + out[-3900:])
            print(f"M2 巢狀獨占分支 {i}/8：{case}（通過）")
        print("G18 M2 巢狀雙葉真獨占寫入八項 Gate 完成；M2 整體仍 OpenEngineering。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 M2 巢狀寫入 Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
