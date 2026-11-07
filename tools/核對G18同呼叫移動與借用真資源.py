#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.4：同一原 Checker Call 真 str.move 與 BorrowTicket 混合正反 Gate。"""
from __future__ import annotations
import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_r05_同次真_move_與_borrow_兩資源及票據共同平衡",
    "g18_r05_混合所有權偽造資源數量或借用未歸不能通過聯合稽核",
    "g18_d_同一真函式先_move_後共享借用_兩資源身分分離且各釋放一次",
    "g18_d_同一真函式先_shared_borrow_後_move_不交換原_parser_求值次序",
    "g18_d_獨占_var_str_借用與另一獨立來源整筆_move_可同次合法存在",
    "g18_c_混合借用與_move_在每個原引數及函式內故障點仍清兩資源",
    "g18_c_混合轉交函式未再移動內部參數仍由被呼叫者離域釋放",
    "g18_d_同一來源先借用再移動或先移動再借用必由原_checker_拒絕",
    "g18_d_借用別名不能冒充所有者移動_錯型別及漏參數仍拒絕",
    "g18_d_竄改借用_kind_或_move_來源參數身份都必須真資源建立前拒絕",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    exe = str(args.test_bin.resolve())
    listing = subprocess.run(
        [exe, "--list"], text=True, encoding="utf-8",
        capture_output=True, check=False,
    )
    if listing.returncode:
        print("G18 同呼叫混合 move／borrow 清單失敗：" + listing.stderr,
              file=sys.stderr)
        return 1
    actual = {
        line.split(": test", 1)[0] for line in listing.stdout.splitlines()
        if line.endswith(": test")
    }
    expected = {
        "struct_stage::g18_private_mixed_ownership_call_tests::" + name
        for name in 案例
    }
    missing = expected - actual
    if missing:
        print("G18 真 move／BorrowTicket 測試缺漏：" +
              "、".join(sorted(missing)), file=sys.stderr)
        return 1
    run = subprocess.run(
        [exe, "--test-threads=2",
         "struct_stage::g18_private_mixed_ownership_call_tests::"],
        text=True, encoding="utf-8", capture_output=True, check=False,
    )
    if run.returncode or f"{len(expected)} passed; 0 failed;" not in run.stdout:
        print("G18 同一 Call 真 move／BorrowTicket Gate 未全過：" +
              run.stdout + run.stderr, file=sys.stderr)
        return 1
    print(f"G18 原 Checker 同 Call str.move／str BorrowTicket 正反："
          f"{len(expected)}/{len(expected)} 真重跑；"
          "一般多來源 move／逃逸及 async 仍未正式驗收。")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
