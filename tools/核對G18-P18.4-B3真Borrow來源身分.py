#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.4-B3 原 Checker→真 Runtime 獨立安全義務：42 項正反。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

CASES = (
    "struct_stage::g18_private_call_borrow_tests::g18_d_正式_checker_const_str_參數共享借用真票據與返回值",
    "struct_stage::g18_private_call_borrow_tests::g18_d_正式_checker_var_str_參數獨占借用不複製源_resource_id",
    "struct_stage::g18_private_call_borrow_tests::g18_c_被呼叫函式真借用存活時故障必先歸還票據再釋放擁有者",
    "struct_stage::g18_private_call_borrow_tests::g18_d_錯誤實參型別及缺少參數在正式_checker_先拒絕",
    "struct_stage::g18_private_call_borrow_tests::g18_d_來源借用別名再作函式實參須由_checker_保守拒絕",
    "struct_stage::g18_private_call_borrow_tests::g18_d_偽造函式借用的_source_或_Kind_在資源建立前拒絕",
    "struct_stage::g18_private_call_borrow_tests::g18_d_刪除原_checker_已簽發的_call_borrows_不可沿用舊票據",
    "struct_stage::g18_private_call_borrow_tests::g18_d_函式本體返回值或來源_utf8_被改寫必在資源前拒絕",
    "struct_stage::g18_private_call_borrow_tests::g18_d_函式借用讀取先驗證動態票據_歸還後禁止重用",
    "struct_stage::g18_private_call_borrow_tests::g18_d_函式實參的真借用票據不能跨動態_tracker_存取",
    "struct_stage::g18_private_call_borrow_tests::g18_d_同追蹤器另一持有者的有效票據不得假扮此函式參數",
    "struct_stage::g18_private_call_borrow_tests::g18_d_函式共享借用不提升獨占權限且不複製_str_資源",
    "struct_stage::g18_private_call_borrow_tests::g18_d_混合_const_str_與_copyable_int_同框架真借用及_hir_加法返回",
    "struct_stage::g18_private_call_borrow_tests::g18_d_混合_int_在前_var_str_在後_仍以真獨占票據與參數身分執行",
    "struct_stage::g18_private_call_borrow_tests::g18_d_混合函式重複呼叫不消耗_str_持有者也不重用上次_int_參數框架",
    "struct_stage::g18_private_call_borrow_tests::g18_c_混合借用期間_int_加法溢位須歸還票據及真_utf8_資源",
    "struct_stage::g18_private_call_borrow_tests::g18_c_混合函式真借用與_int_參數已綁定後故障仍原子歸還清理",
    "struct_stage::g18_private_call_borrow_tests::g18_d_篡改混合函式_int_值與_DeclarationId_或_hir_返回必須配置前拒絕",
    "struct_stage::g18_private_call_borrow_tests::g18_d_混合函式刪除原_checker_真借用表不得使用舊參數票據",
    "struct_stage::g18_private_call_borrow_tests::g18_d_混合呼叫錯誤型別個數與多餘未執行宣告必保守拒絕",
    "struct_stage::g18_private_call_borrow_tests::g18_d_具名_const_int_與_str_借用同框架按原_checker_引用身份返回",
    "struct_stage::g18_private_call_borrow_tests::g18_d_具名_var_int_在前_var_str_在後_真獨占借用及複製求值",
    "struct_stage::g18_private_call_borrow_tests::g18_c_具名_copyable_與_str_借用並存故障仍清空參數及歸還票據",
    "struct_stage::g18_private_call_borrow_tests::g18_d_偽造具名_copyable_所有者_decl_id_或初始值須零資源拒絕",
    "struct_stage::g18_private_call_borrow_tests::g18_d_具名_int_未初始化或實參型別不符由原_checker_先拒絕",
    "struct_stage::g18_private_call_borrow_tests::g18_d_唯一_str_借用與四個_copyable_int_同框架真算術返回",
    "struct_stage::g18_private_call_borrow_tests::g18_d_參數宣告順序為_int_str_int_int_仍以原_checker_身分真借用",
    "struct_stage::g18_private_call_borrow_tests::g18_d_同一具名_copyable_int_重複使用不消耗_str_借用持有者",
    "struct_stage::g18_private_call_borrow_tests::g18_c_多個_int_算術溢位須於真_str_borrowticket_歸還前中止成功返回",
    "struct_stage::g18_private_call_borrow_tests::g18_c_混合四個_int_與_str_中途故障不得殘留任一動態參數或票據",
    "struct_stage::g18_private_call_borrow_tests::g18_d_多個_copyable_int_後續參數被篡改不得建立任何_utf8_資源",
    "struct_stage::g18_private_call_borrow_tests::g18_d_多參數型別錯誤與未使用具名_int_保持原_checker_拒絕",
    "struct_stage::g18_private_call_borrow_tests::g18_r05_共享與獨占真借用返回或故障皆須資源票據聯合平衡",
    "struct_stage::g18_private_call_borrow_tests::g18_r05_真借用結果偽造已退出未歸票據或遺失資源一律拒絕",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_d_兩個獨立_var_str_加雙_int_跨遞迴真票據全序列及逆序清理",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_d_同函式兩個_const_str_共享借用同一來源須用兩張真票據歸還",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_d_兩個_str_與_int_不同比例順序之互相遞迴仍真借用並返回",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_兩張真_str_票據存活時框架預算耗盡仍全部原子歸還",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_c_兩張真_str_票據已簽發後動態故障不留任何資源",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_d_兩張借用中第二張_source_或_kind_被竄改必在配置前拒絕",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_d_同個可寫_str_來源同次傳兩個獨占參數須原_checker_拒絕",
    "struct_stage::g18_private_recursive_str_frames_tests::g18_d_雙_str_呼叫漏參數及錯誤型別仍由原_checker_拒絕",
)
EXACT_MODULE_PREFIXES = (
    "struct_stage::g18_private_call_borrow_tests::",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], capture_output=True, text=True,
            encoding="utf-8", check=False,
        )
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-1500:])
        available = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.endswith(": test")
        }
        expected = set(CASES)
        if len(expected) != 42:
            raise ValueError("本契約測試名稱出現重複，不得重複採計")
        missing = sorted(expected - available)
        if missing:
            raise ValueError("缺少原 Checker 真 Rust 案例：" + str(missing))
        for prefix in EXACT_MODULE_PREFIXES:
            actual = {name for name in available if name.startswith(prefix)}
            expected_module = {name for name in expected if name.startswith(prefix)}
            if actual != expected_module:
                raise ValueError(
                    f"模組 {prefix} 正反矩陣漂移：缺 {sorted(expected_module-actual)} "
                    f"多 {sorted(actual-expected_module)}"
                )
        for index, case in enumerate(CASES, 1):
            result = subprocess.run(
                [str(binary), "--exact", case, "--test-threads=1"],
                capture_output=True, text=True, encoding="utf-8", check=False,
            )
            output = result.stdout + result.stderr
            if result.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"B3 第 {index}/42 條真來源失敗：{case}\n" + output[-3000:]
                )
            print(f"B3 原始義務 {index}/42：{case}（通過）")
        print("B3：原 Checker 真來源正反／資源票據身分獨立驗收通過；不代表 P18.4 全 Passed。")
        return 0
    except (OSError, ValueError) as error:
        print("B3 獨立 Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
