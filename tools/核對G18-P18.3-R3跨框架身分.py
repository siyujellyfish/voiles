#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P18.3 R3：原 Checker→真顯式遞迴框架、UTF-8 與票據動態證據。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

CASES = (
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_直接遞迴真_str_參數兩層_borrowticket_先歸還再清理_utf8",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_互相遞迴每層真_str_借用票據不重覆也不洩漏",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_竄改已核准呼叫_borrowfact_來源與_kind_在配置前拒絕",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_原_checker_缺失呼叫借用票據或非法_str_來源不可真執行",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_深層重入同一宣告來源須各有動態_borrow_tracker_且受控耗盡",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_跨框架相同原宣告位置的唯讀和獨占票據不能誤互斥",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_竄改遞迴_int_實參來源或返回_hir_必須在真資源建立前拒絕",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_混合遞迴錯誤_int_型別及漏參數必先由原_checker_拒絕",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_混合遞迴第二_int_及多函式來源篡改必在建立資源前拒絕",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_混合多_int_錯誤參數型別與個數須由原_checker_拒絕",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_d_同函式兩個_const_str_共享借用同一來源須用兩張真票據歸還",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_d_兩張借用中第二張_source_或_kind_被竄改必在配置前拒絕",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_d_同個可寫_str_來源同次傳兩個獨占參數須原_checker_拒絕",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_d_雙_str_呼叫漏參數及錯誤型別仍由原_checker_拒絕",
    "struct_stage::g18_private_scalar_call_tests::g18_r05_跨框架_move_已完成綁定後故障由子框架釋放",
    "struct_stage::g18_private_scalar_call_tests::g18_r05_第二實參溢位時_move_仍由原呼叫者唯一釋放",
)


def main() -> int:
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--test-bin", type=Path, required=True)
    cli.add_argument("--check", action="store_true", required=True)
    args = cli.parse_args()
    try:
        if len(CASES) != 16 or len(set(CASES)) != 16:
            raise ValueError("R3 真來源不得重複與缺漏")
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run([str(binary), "--list"], text=True,
            capture_output=True, encoding="utf-8", check=False)
        if listing.returncode:
            raise ValueError("原 Rust --list 失敗：" + listing.stderr[-1800:])
        available = {line.removesuffix(": test") for line in listing.stdout.splitlines()
                     if line.endswith(": test")}
        missing = sorted(set(CASES)-available)
        if missing:
            raise ValueError("R3 已列真來源不存在：" + str(missing))
        for number, name in enumerate(CASES, 1):
            proc = subprocess.run([str(binary), "--exact", name, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False)
            output = proc.stdout+proc.stderr
            if proc.returncode or not re.search(
                    r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                raise ValueError("R3 第 "+str(number)+" 筆真執行失敗："+
                                 name+"\n"+output[-2600:])
            print(f"P18.3 R3 原遞迴動態來源 {number}/16 Passed")
        print("P18.3 R3 正反 16/16 真 Rust 來源通過")
        return 0
    except (OSError, ValueError) as exc:
        print("P18.3 R3 正式 Gate 失敗："+str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
