#!/usr/bin/env python3
"""G18／P18.1、18.4 原 Checker 函式實參借用→動態 BorrowTicket 正反 Gate。"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_d_具名_const_int_與_str_借用同框架按原_checker_引用身份返回",
    "g18_d_具名_var_int_在前_var_str_在後_真獨占借用及複製求值",
    "g18_c_具名_copyable_與_str_借用並存故障仍清空參數及歸還票據",
    "g18_d_偽造具名_copyable_所有者_decl_id_或初始值須零資源拒絕",
    "g18_d_具名_int_未初始化或實參型別不符由原_checker_先拒絕",
    "g18_d_混合_const_str_與_copyable_int_同框架真借用及_hir_加法返回",
    "g18_d_混合_int_在前_var_str_在後_仍以真獨占票據與參數身分執行",
    "g18_d_混合函式重複呼叫不消耗_str_持有者也不重用上次_int_參數框架",
    "g18_c_混合借用期間_int_加法溢位須歸還票據及真_utf8_資源",
    "g18_c_混合函式真借用與_int_參數已綁定後故障仍原子歸還清理",
    "g18_d_篡改混合函式_int_值與_DeclarationId_或_hir_返回必須配置前拒絕",
    "g18_d_混合函式刪除原_checker_真借用表不得使用舊參數票據",
    "g18_d_混合呼叫錯誤型別個數與多餘未執行宣告必保守拒絕",
    "g18_d_正式_checker_const_str_參數共享借用真票據與返回值",
    "g18_d_函式借用讀取先驗證動態票據_歸還後禁止重用",
    "g18_d_函式實參的真借用票據不能跨動態_tracker_存取",
    "g18_d_同追蹤器另一持有者的有效票據不得假扮此函式參數",
    "g18_d_函式共享借用不提升獨占權限且不複製_str_資源",
    "g18_d_正式_checker_var_str_參數獨占借用不複製源_resource_id",
    "g18_c_被呼叫函式真借用存活時故障必先歸還票據再釋放擁有者",
    "g18_d_錯誤實參型別及缺少參數在正式_checker_先拒絕",
    "g18_d_來源借用別名再作函式實參須由_checker_保守拒絕",
    "g18_d_偽造函式借用的_source_或_Kind_在資源建立前拒絕",
    "g18_d_刪除原_checker_已簽發的_call_borrows_不可沿用舊票據",
    "g18_d_函式本體返回值或來源_utf8_被改寫必在資源前拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    result = subprocess.run(
        [str(args.test_bin.resolve()), "--list"],
        text=True, encoding="utf-8", capture_output=True, check=False,
    )
    if result.returncode:
        print("G18 函式借用真票據 Rust 清單取得失敗：" + result.stderr, file=sys.stderr)
        return 1
    names = {
        line.split(": test", 1)[0]
        for line in result.stdout.splitlines()
        if line.endswith(": test")
    }
    expected = {
        "struct_stage::g18_private_call_borrow_tests::" + name for name in 案例
    }
    missing = sorted(expected - names)
    if missing:
        print("G18 函式參數借用 Rust 正反缺漏：" + "、".join(missing), file=sys.stderr)
        return 1
    run = subprocess.run(
        [str(args.test_bin.resolve()), "--test-threads=2",
         "struct_stage::g18_private_call_borrow_tests::"],
        text=True, encoding="utf-8", capture_output=True, check=False,
    )
    if run.returncode or f"{len(expected)} passed; 0 failed;" not in run.stdout:
        print("G18 函式真借用與 Copyable int 混合 Gate 未全過：" +
              run.stdout + run.stderr, file=sys.stderr)
        return 1
    print(f"G18 真函式 str BorrowTicket／Copyable int 同框架："
          f"{len(expected)}/{len(expected)} 已真正重跑，仍非 G18 六原項 Passed。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
