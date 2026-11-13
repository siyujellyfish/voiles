#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P18.3 R2：原 Checker→真顯式遞迴框架、UTF-8 與票據動態證據。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

CASES = (
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_遞迴框架預算耗盡須釋放已建立_utf8_且不得假成功",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_遞迴步數預算耗盡即使分支開啟仍逐層逆序清理",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_空預算不進入任一框架且動態事件為空",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_跨框架借用存活時預算耗盡須先還真票據再釋放來源",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_深層重入步數耗盡各框架票據與所有者全部逆序歸還",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_混合_str_int_跨框架呼叫預算耗盡仍保留實際參數值與全部清理",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_多函式混合資源框架預算耗盡須逆序釋放每個已配置_resourceid",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_兩張真_str_票據存活時框架預算耗盡仍全部原子歸還",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_r05_真資源遞迴預算耗盡與局部建立故障均須完整退出",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_r05_真零預算尚未進入主框架不允許假資源事件",
    "struct_stage::g18_private_scalar_call_tests::g18_c_純值遞迴步數預算受控耗盡且深層框架逆序退出",
    "struct_stage::g18_private_scalar_call_tests::g18_c_零框架或零步數預算不得偽造任何真函式進入",
    "struct_stage::g18_private_scalar_call_tests::g18_c_main_零參數根函式直接遞迴預算耗盡及逐框架退出",
    "struct_stage::g18_private_scalar_call_tests::g18_c_main_真正被呼叫者本體巢狀超單框架預算不可虛構葉函式進入",
)


def main() -> int:
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--test-bin", type=Path, required=True)
    cli.add_argument("--check", action="store_true", required=True)
    args = cli.parse_args()
    try:
        if len(CASES) != 14 or len(set(CASES)) != 14:
            raise ValueError("R2 真來源不得重複與缺漏")
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run([str(binary), "--list"], text=True,
            capture_output=True, encoding="utf-8", check=False)
        if listing.returncode:
            raise ValueError("原 Rust --list 失敗：" + listing.stderr[-1800:])
        available = {line.removesuffix(": test") for line in listing.stdout.splitlines()
                     if line.endswith(": test")}
        missing = sorted(set(CASES)-available)
        if missing:
            raise ValueError("R2 已列真來源不存在：" + str(missing))
        for number, name in enumerate(CASES, 1):
            proc = subprocess.run([str(binary), "--exact", name, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False)
            output = proc.stdout+proc.stderr
            if proc.returncode or not re.search(
                    r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                raise ValueError("R2 第 "+str(number)+" 筆真執行失敗："+
                                 name+"\n"+output[-2600:])
            print(f"P18.3 R2 原遞迴動態來源 {number}/14 Passed")
        print("P18.3 R2 正反 14/14 真 Rust 來源通過")
        return 0
    except (OSError, ValueError) as exc:
        print("P18.3 R2 正式 Gate 失敗："+str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
