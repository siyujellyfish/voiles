#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.1-F4：原 Checker 真可達圖／不可達零效果／來源變造拒絕十三類二十六來源。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

CONTRACT = {
    "F4-C01": (
        "struct_stage::g18_private_scalar_call_tests::g18_a_前向兩層帶參數呼叫每層建立獨立框架並按返回順序退出",
        "struct_stage::g18_private_scalar_call_tests::g18_a_三層具參數函式圖真正使用前一層返回值而非來源常值",
    ),
    "F4-C02": (
        "struct_stage::g18_private_scalar_call_tests::g18_r01_未使用的合法bool輔助函式不得阻斷真int主呼叫",
        "struct_stage::g18_private_scalar_call_tests::g18_r01_主函式到二層真葉函式仍收錄且跳過不相干bool宣告",
    ),
    "F4-C03": (
        "struct_stage::g18_private_scalar_call_tests::g18_r01_主函式兄弟分支只執行選中函式而收錄兩者並略過無關bool",
        "struct_stage::g18_private_scalar_call_tests::g18_r03_main_if_false未選多層函式不造資源真else只借用一次",
    ),
    "F4-C04": (
        "struct_stage::g18_private_scalar_call_tests::g18_r01_無關int結構宣告不應阻斷main純函式執行",
        "struct_stage::g18_private_scalar_call_tests::g18_r01_未使用結構的合法self方法不可冒充真函式進入",
    ),
    "F4-C05": (
        "struct_stage::g18_private_scalar_call_tests::g18_r01_無關結構的utf8預設欄位不得於main執行時建立資源",
        "struct_stage::g18_private_scalar_call_tests::g18_r01_原v0可達預設str結構函式真返回",
    ),
    "F4-C06": (
        "struct_stage::g18_private_scalar_call_tests::g18_r01_非法重複結構欄位仍由原checker拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_r01_來源已驗證的結構清單遭竄改必在執行前拒絕",
    ),
    "F4-C07": (
        "struct_stage::g18_private_scalar_call_tests::g18_r01_可達圖身分遭竄改及bool誤用int回傳都須拒絕",
        "struct_stage::g18_private_scalar_call_tests::g18_r01_普通函式不可達閉包計畫被增添目標須在執行前拒絕",
    ),
    "F4-C08": (
        "struct_stage::g18_private_scalar_call_tests::g18_r01_main全分支提前返回不應編譯尾端不支援的結構函式",
        "struct_stage::g18_private_scalar_call_tests::g18_r03_main_if_else假分支不進且不可達末端return零事件",
    ),
    "F4-C09": (
        "struct_stage::g18_private_scalar_call_tests::g18_r01_不含真函式呼叫的main常值仍不得偽造函式證據",
        "struct_stage::g18_private_scalar_call_tests::g18_a_未被選取的合法遞迴函式不得造成其他正向函式拒絕",
    ),
    "F4-C10": (
        "struct_stage::g18_private_scalar_call_tests::g18_r05_偽造另一份平衡_resourceid_不得繞過真事件映射",
        "struct_stage::g18_private_scalar_call_tests::g18_r05_偽造提前函式退出再釋放真_resourceid_必被拒絕",
    ),
    "F4-C11": (
        "struct_stage::g18_private_scalar_call_tests::g18_v1_f4_未選控制葉具名函式零執行零結構",
        "struct_stage::g18_private_scalar_call_tests::g18_v1_f4_具名函式來源計畫遭竄改不得偷偷替換回傳",
    ),
    "F4-C12": (
        "struct_stage::g18_private_scalar_call_tests::g18_v1_f4_同名Copyable結構跨兩次函式呼叫不錯用欄位值",
        "struct_stage::g18_private_scalar_call_tests::g18_v1_f1_f3_父子同名結構幀中讀欄位恢復父值不串位",
    ),
    "F4-C13": (
        "struct_stage::g18_private_scalar_call_tests::g18_a_合法前向六層以上_copyable_int_函式真逐層返回",
        "struct_stage::g18_private_scalar_call_tests::g18_a_巢狀呼叫錯誤型別及未知函式仍由原_checker_拒絕",
    ),
}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    parser.add_argument("--check", action="store_true", required=True)
    args = parser.parse_args()
    try:
        keys = {f"F4-C{i:02d}" for i in range(1, 14)}
        if set(CONTRACT) != keys or any(len(cases) != 2 for cases in CONTRACT.values()):
            raise ValueError("F4 必須恰有十三個等價類，各兩筆真來源")
        cases = [name for names in CONTRACT.values() for name in names]
        if len(set(cases)) != 26:
            raise ValueError("F4 不允許重複測試名稱冒充獨立來源")
        executable = args.test_bin.resolve(strict=True)
        listing = subprocess.run([str(executable), "--list"], text=True,
                                 encoding="utf-8", capture_output=True, check=False)
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1600:])
        available = {ln.removesuffix(": test") for ln in listing.stdout.splitlines()
                     if ln.endswith(": test")}
        missing = sorted(set(cases)-available)
        if missing:
            raise ValueError("F4 真來源名稱缺少：" + str(missing))
        for group, names in CONTRACT.items():
            for name in names:
                out = subprocess.run([str(executable), "--exact", name, "--test-threads=1"],
                    text=True, encoding="utf-8", capture_output=True, check=False)
                printed = out.stdout + out.stderr
                if out.returncode or not re.search(
                    r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", printed):
                    raise ValueError(f"{group} 真 Rust 驗收失敗：{name}\n" + printed[-2500:])
            print(f"G18 F4 {group} 原 Checker／Runtime 動態圖正反 2/2 Passed")
        print("G18 F4 原義務 13/13、26/26 來源全部成功；P18.1 仍須四義務全體 Gate")
        return 0
    except (ValueError, OSError) as error:
        print("G18 F4 原可達圖閉合拒絕：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
