#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6-M2：原 Checker self 寫入後真返回運算與溢位清理八項原來源。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_post_write_overflow_tests::"
CASES = (
    "m2_after_write_真正更新self欄位後才求值true葉加法",
    "m2_after_write_false不執行true葉寫入也不求值其回傳算式",
    "m2_after_write_已寫入後加法真溢位必Unwind且歸還唯一獨占票據",
    "m2_after_write_false不受不可達true葉溢位影響",
    "m2_after_write_減法從真已寫欄位讀值且正確返回負整數",
    "m2_after_write_無寫入的共享self方法早返加法仍正確",
    "m2_after_write_已核准true葉加法計畫遭竄改必執行前拒絕",
    "m2_after_write_原Checker拒絕不存在欄位或非int的return加法",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1600:])
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + c for c in CASES}
        if actual != expected or len(expected) != 8:
            raise ValueError("M2 已寫入後溢位案例清單漂移：缺 "
                + str(sorted(expected-actual)) + " 多 " + str(sorted(actual-expected)))
        for index, case in enumerate(CASES, 1):
            executed = subprocess.run(
                [str(binary), "--exact", PREFIX+case, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            out = executed.stdout + executed.stderr
            if executed.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", out
            ):
                raise ValueError(
                    f"M2 真寫入後算術 {index}/8 失敗：{case}\n" + out[-3800:]
                )
            print(f"M2 寫入後原運算 {index}/8：{case}（真通過）")
        print("G18 M2 已寫入後溢位 Unwind／歸票八項真 Gate 通過；M2 整項仍未 Closed。")
        return 0
    except (OSError, ValueError) as err:
        print("M2 寫入後算術安全 Gate 失敗：" + str(err), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
