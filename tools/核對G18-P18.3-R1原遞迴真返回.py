#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P18.3 R1：原 Checker→真顯式遞迴框架、UTF-8 與票據動態證據。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

CASES = (
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_真直接遞迴每層_utf8_所有者按呼叫及分支逆序清理",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_真互相遞迴各層和分支資源跨框架清理不重覆",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_直接遞迴_str_真票據與_copyable_int_逐框架傳遞並由_hir_算術返回",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_互相遞迴_參數順序不同仍保留原_checker_int_與_str_借用身份",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_無_str_實參的_bool_int_遞迴仍真建立每框架_utf8_並返回動態參數",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_子遞迴_int_常值覆寫父框架值_底層不得錯讀父參數",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_直接遞迴兩個_copyable_int_互換父子參數槽並保護真_str_票據",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_單張_str_borrowticket_和三個_int_跨遞迴位置交換實際返回",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_相同_copyable_int_父實參可給兩個子參數但不可複製真_str",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_五個互相呼叫函式與六層混合資源_不得被舊兩函式上限拒絕",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_d_兩個獨立_var_str_加雙_int_跨遞迴真票據全序列及逆序清理",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_d_兩個_str_與_int_不同比例順序之互相遞迴仍真借用並返回",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_r05_真直接遞迴_utf8_借用與返回框架交付前全數一致",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_r05_真互相遞迴多_resourceid_跨四框架逆序退出必一致",
    "struct_stage::g18_private_scalar_call_tests::g18_c_合法直接_int_遞迴不得因圖成環被拒_且無_rust_堆疊溢位",
    "struct_stage::g18_private_scalar_call_tests::g18_c_函式圖合法互相遞迴須受控預算耗盡並逐動態框架退出",
)


def main() -> int:
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--test-bin", type=Path, required=True)
    cli.add_argument("--check", action="store_true", required=True)
    args = cli.parse_args()
    try:
        if len(CASES) != 16 or len(set(CASES)) != 16:
            raise ValueError("R1 真來源不得重複與缺漏")
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run([str(binary), "--list"], text=True,
            capture_output=True, encoding="utf-8", check=False)
        if listing.returncode:
            raise ValueError("原 Rust --list 失敗：" + listing.stderr[-1800:])
        available = {line.removesuffix(": test") for line in listing.stdout.splitlines()
                     if line.endswith(": test")}
        missing = sorted(set(CASES)-available)
        if missing:
            raise ValueError("R1 已列真來源不存在：" + str(missing))
        for number, name in enumerate(CASES, 1):
            proc = subprocess.run([str(binary), "--exact", name, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False)
            output = proc.stdout+proc.stderr
            if proc.returncode or not re.search(
                    r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                raise ValueError("R1 第 "+str(number)+" 筆真執行失敗："+
                                 name+"\n"+output[-2600:])
            print(f"P18.3 R1 原遞迴動態來源 {number}/16 Passed")
        print("P18.3 R1 正反 16/16 真 Rust 來源通過")
        return 0
    except (OSError, ValueError) as exc:
        print("P18.3 R1 正式 Gate 失敗："+str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
