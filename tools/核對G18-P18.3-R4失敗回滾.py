#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P18.3 R4：原 Checker→真顯式遞迴框架、UTF-8 與票據動態證據。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

CASES = (
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_真第二層局部_utf8_建立後注入故障逐層回滾",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_源碼或預備的巢狀資源被篡改須在零配置前拒絕",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_非法遞迴引數及非_copyable_複製由原_checker_先拒絕",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_第二層借用來源完成配置後注入錯誤仍歸還全部票據",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_重入框架內故障不以共用靜態宣告身份錯誤解鎖",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_混合_int_底層算術溢位須失敗並先歸還借用票據再逐層釋放",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_混合遞迴第二層_int_已綁定後故障仍清理所有_utf8_與票據",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_混合多個_int_底層溢位不得略過真借用歸還及_utf8_釋放",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_兩張真_str_票據已簽發後動態故障不留任何資源",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_r05_動態資源返回後偽造退出名字與成功標記必拒絕",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_r05_真資源釋放或票據空集合遭竄改必拒絕交付",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_r05_動態框架重複進入與漏退出不得因資源表平衡而放行",
    "struct_stage::g18_private_scalar_call_tests::g18_c_巢狀葉函式溢位不得虛構子層或父層成功返回",
    "struct_stage::g18_private_scalar_call_tests::g18_c_真正參數框架中途故障必清空且不冒充成功返回",
    "struct_stage::g18_private_scalar_call_tests::g18_r05_外層待呼叫借用與內層函式溢位須先還內票再還外票",
    "struct_stage::g18_private_scalar_call_tests::g18_r05_巢狀回滾交換內外票據歸還順序須拒絕",
)


def main() -> int:
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--test-bin", type=Path, required=True)
    cli.add_argument("--check", action="store_true", required=True)
    args = cli.parse_args()
    try:
        if len(CASES) != 16 or len(set(CASES)) != 16:
            raise ValueError("R4 真來源不得重複與缺漏")
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run([str(binary), "--list"], text=True,
            capture_output=True, encoding="utf-8", check=False)
        if listing.returncode:
            raise ValueError("原 Rust --list 失敗：" + listing.stderr[-1800:])
        available = {line.removesuffix(": test") for line in listing.stdout.splitlines()
                     if line.endswith(": test")}
        missing = sorted(set(CASES)-available)
        if missing:
            raise ValueError("R4 已列真來源不存在：" + str(missing))
        for number, name in enumerate(CASES, 1):
            proc = subprocess.run([str(binary), "--exact", name, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False)
            output = proc.stdout+proc.stderr
            if proc.returncode or not re.search(
                    r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                raise ValueError("R4 第 "+str(number)+" 筆真執行失敗："+
                                 name+"\n"+output[-2600:])
            print(f"P18.3 R4 原遞迴動態來源 {number}/16 Passed")
        print("P18.3 R4 正反 16/16 真 Rust 來源通過")
        return 0
    except (OSError, ValueError) as exc:
        print("P18.3 R4 正式 Gate 失敗："+str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
