#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.4-B2 原 Checker→真 Runtime 獨立安全義務：20 項正反。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

CASES = (
    "struct_stage::g18_private_owned_move_call_tests::g18_d_原_checker_明確_move_普通函式實參轉交同一真_utf8_不複製",
    "struct_stage::g18_private_owned_move_call_tests::g18_d_被呼叫者真的再次_move_參數至局部唯一所有者退出才釋放一次",
    "struct_stage::g18_private_owned_move_call_tests::g18_c_多函式逐層_move_同一_resourceid_末端真正釋放且返回逐框架退出",
    "struct_stage::g18_private_owned_move_call_tests::g18_c_合法自我遞迴每層轉交唯一_utf8_受框架預算安全耗盡",
    "struct_stage::g18_private_owned_move_call_tests::g18_c_跨函式_move_步數預算耗盡不得假成功或雙重釋放",
    "struct_stage::g18_private_owned_move_call_tests::g18_c_真移動之前及連續轉交之後注入故障都由目前所有者清理一次",
    "struct_stage::g18_private_owned_move_call_tests::g18_c_零函式框架或零步數預算在配置真_utf8_前受控拒絕",
    "struct_stage::g18_private_owned_move_call_tests::g18_d_移動後原來源再次使用與漏掉明確_move_必由原_checker_拒絕",
    "struct_stage::g18_private_owned_move_call_tests::g18_d_非法借用別名不能以_move_變成真正所有者",
    "struct_stage::g18_private_owned_move_call_tests::g18_d_更改已核准轉交函式或字串原值必須零執行前拒絕",
    "struct_stage::g18_private_mixed_ownership_call_tests::g18_d_同一真函式先_move_後共享借用_兩資源身分分離且各釋放一次",
    "struct_stage::g18_private_mixed_ownership_call_tests::g18_d_同一真函式先_shared_borrow_後_move_不交換原_parser_求值次序",
    "struct_stage::g18_private_mixed_ownership_call_tests::g18_d_獨占_var_str_借用與另一獨立來源整筆_move_可同次合法存在",
    "struct_stage::g18_private_mixed_ownership_call_tests::g18_c_混合借用與_move_在每個原引數及函式內故障點仍清兩資源",
    "struct_stage::g18_private_mixed_ownership_call_tests::g18_c_混合轉交函式未再移動內部參數仍由被呼叫者離域釋放",
    "struct_stage::g18_private_mixed_ownership_call_tests::g18_d_同一來源先借用再移動或先移動再借用必由原_checker_拒絕",
    "struct_stage::g18_private_mixed_ownership_call_tests::g18_d_借用別名不能冒充所有者移動_錯型別及漏參數仍拒絕",
    "struct_stage::g18_private_mixed_ownership_call_tests::g18_d_竄改借用_kind_或_move_來源參數身份都必須真資源建立前拒絕",
    "struct_stage::g18_private_mixed_ownership_call_tests::g18_r05_同次真_move_與_borrow_兩資源及票據共同平衡",
    "struct_stage::g18_private_mixed_ownership_call_tests::g18_r05_混合所有權偽造資源數量或借用未歸不能通過聯合稽核",
)
EXACT_MODULE_PREFIXES = (
    "struct_stage::g18_private_owned_move_call_tests::",
    "struct_stage::g18_private_mixed_ownership_call_tests::",
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
        if len(expected) != 20:
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
                    f"B2 第 {index}/20 條真來源失敗：{case}\n" + output[-3000:]
                )
            print(f"B2 原始義務 {index}/20：{case}（通過）")
        print("B2：原 Checker 真來源正反／資源票據身分獨立驗收通過；不代表 P18.4 全 Passed。")
        return 0
    except (OSError, ValueError) as error:
        print("B2 獨立 Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
