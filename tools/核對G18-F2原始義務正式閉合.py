#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.1-F2 原始參數與返回義務：十五等價類、三十筆真 Checker→Runtime 正反。

此關卡只關閉 F2，不使 F3/F4 或 P18.1 取得 Passed。
F2 正常／失敗所有權退出仍受其他既有 F2 Gate 與 F00/G6/G17 嚴格回歸約束。
"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

# 唯一正式關閉證據，避免不同測試在數個等價類中重複計分。
CONTRACT = {
    "F2-C01": (
        "struct_stage::g18_f2_obligation_tests::f2_純值布林整數同一函式按來源順序求值並重複讀取",
        "struct_stage::g18_f2_obligation_tests::f2_純值布林選擇不得改變不同參數來源身分",
    ),
    "F2-C02": (
        "struct_stage::g18_f2_obligation_tests::f2_字串共享借用布林與整數雙次傳入均各自歸票",
        "struct_stage::g18_f2_str_multi_resource_tests::f2_str_multi_雙獨立唯讀字串借用再接收第三個唯一返回資源",
    ),
    "F2-C03": (
        "struct_stage::g18_f2_obligation_tests::f2_同一呼叫字串整筆移動加另一字串借用與布林整數不混帳",
        "struct_stage::g18_f2_str_multi_resource_tests::f2_str_multi_另一個資源借用時真move並返回同一id沒有重配",
    ),
    "F2-C04": (
        "struct_stage::g18_f2_str_return_args_tests::f2_str_args_整數布林混型別正確求值才交付真utf8",
        "struct_stage::g18_f2_str_return_args_tests::f2_str_args_整數子函式先真返回後作為str返回實參",
    ),
    "F2-C05": (
        "struct_stage::g18_f2_str_forward_budget_tests::f2_forward_budget_先返回後經中層參數再轉發同resourceid",
        "struct_stage::g18_f2_str_forward_budget_tests::f2_forward_budget_兩個獨立字串參數其中一筆直接返回且另一筆先釋放",
    ),
    "F2-C06": (
        "struct_stage::g18_f2_str_return_assign_tests::f2_str_assign_未初始化var接管str返回同一id並於退出唯一釋放",
        "struct_stage::g18_f2_str_return_assign_tests::f2_str_assign_已初始化var函式覆寫及重複覆寫仍由原checker拒絕",
    ),
    "F2-C07": (
        "struct_stage::g18_f2_str_forward_budget_tests::f2_forward_budget_雙字串已返回第三實參溢位逆序回收而不入被呼叫者",
        "struct_stage::g18_f2_str_forward_budget_tests::f2_forward_budget_雙字串前置實參逐步數截斷均無遺漏與重複釋放",
    ),
    "F2-C08": (
        "struct_stage::g18_f2_obligation_tests::f2_借用後續整數實參溢位由呼叫者真還票且不進函式",
        "struct_stage::g18_f2_obligation_tests::f2_成功移入子框架後溢位由目前所有者完成完整資源清理",
    ),
    "F2-C09": (
        "struct_stage::g18_f2_obligation_tests::f2_跨形參錯型別與借移同源衝突由原checker拒絕",
        "struct_stage::g18_f2_str_return_assign_tests::f2_str_assign_const與錯誤型別仍由原checker拒絕不得建立資源",
    ),
    "F2-C10": (
        "struct_stage::g18_f2_str_forward_budget_tests::f2_forward_budget_雙實參將第二個轉交目標竄改成第一個即使id正確仍拒絕",
        "struct_stage::g18_f2_str_forward_budget_tests::f2_forward_budget_雙來源第二個str返回偽冒第一個id必遭稽核拒絕",
    ),
    "F2-C11": (
        "struct_stage::g18_f2_obligation_tests::f2_跨資源借用票據事件偽造即使計數不變也不得交付",
        "struct_stage::g18_f2_str_return_assign_tests::f2_str_assign_偽造首次接收resourceid即使釋放數平衡也須拒絕",
    ),
    "F2-C12": (
        "struct_stage::g18_f2_str_forward_budget_tests::f2_forward_budget_未命中if不得提前建立或呼叫原str函式",
        "struct_stage::g18_f2_str_forward_budget_tests::f2_forward_budget_返回函式外層括號必保留真checker及唯一id",
    ),
    "F2-C13": (
        "struct_stage::g18_f2_str_multi_resource_tests::f2_str_multi_函式進入後溢位必歸還外票再釋放各自擁有者",
        "struct_stage::g18_f2_str_multi_resource_tests::f2_str_multi_兩張borrowticket後續溢位以逆序還票且零callee",
    ),
    "F2-C14": (
        "struct_stage::g18_f2_obligation_tests::f2_移動後續整數溢位不能提前轉交也不能重複釋放",
        "struct_stage::g18_f2_str_return_args_tests::f2_str_args_移動意圖後續溢位不得提前提交資源轉交",
    ),
    "F2-C15": (
        "struct_stage::g18_f2_str_return_args_tests::f2_str_args_先真借用外部字串後再接收返回資源且兩者各自釋放",
        "struct_stage::g18_f2_str_multi_resource_tests::f2_str_multi_同一旁觀資源跨兩次返回借用票據不殘留",
    ),
}


def main() -> int:
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--test-bin", type=Path, required=True)
    cli.add_argument("--check", action="store_true", required=True,
                     help="必須逐一真正執行所有 F2 正反來源")
    opts = cli.parse_args()
    try:
        expected_keys = {f"F2-C{i:02d}" for i in range(1, 16)}
        if set(CONTRACT) != expected_keys or any(len(group) != 2 for group in CONTRACT.values()):
            raise ValueError("F2 必須保持完整十五類各兩份獨立正反來源")
        all_names = [name for group in CONTRACT.values() for name in group]
        if len(set(all_names)) != 30:
            raise ValueError("F2 測試來源重複計分或缺漏")
        binary = opts.test_bin.resolve(strict=True)
        listing = subprocess.run([str(binary), "--list"], text=True,
                                 encoding="utf-8", capture_output=True, check=False)
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-1500:])
        available = {line.removesuffix(": test") for line in listing.stdout.splitlines()
                     if line.endswith(": test")}
        absent = sorted(set(all_names) - available)
        if absent:
            raise ValueError("F2 真 Rust 測試未存在：" + str(absent))
        for contract_id, group in CONTRACT.items():
            for case in group:
                proc = subprocess.run([str(binary), "--exact", case, "--test-threads=1"],
                                      text=True, encoding="utf-8", capture_output=True,
                                      check=False)
                output = proc.stdout + proc.stderr
                if proc.returncode or not re.search(
                        r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output):
                    raise ValueError(f"{contract_id} 真來源失敗：{case}\n" + output[-2500:])
            print(f"G18 F2 正式子契約 {contract_id}：2/2 真來源正反通過")
        print("G18 F2 原始義務 15/15 等價類、30/30 真正反已通過；"
              "其他 F3/F4、P18 原六項仍未完成。")
        return 0
    except (OSError, ValueError) as error:
        print("G18 F2 原始義務正式閉合失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
