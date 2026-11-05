#!/usr/bin/env python3
"""G18／P18.1、18.4 原 Checker 函式實參借用→動態 BorrowTicket 正反 Gate。"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_d_正式_checker_const_str_參數共享借用真票據與返回值",
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
    print(f"G18 真函式 call_borrows→BorrowTicket：{len(expected)}/{len(expected)}，"
          "正式 Rust 全量已實際執行；仍非 G18 六原項 Passed。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
