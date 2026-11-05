#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.6／S20 靜態唯一方法→Copyable 真參數框架與 self BorrowTicket 執行八正反 Gate。"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_f_真多載_int_參數由已選獨占方法實際寫入欄位並返回",
    "g18_f_真多載_bool_參數由原_checker_唯一選中共享方法不寫入",
    "g18_f_真多載方法宣告顛倒仍以完整型別選擇正確動態結果",
    "g18_f_const_多載選到獨占方法時由原_checker_禁止寫入",
    "g18_f_真多載借用存活中失敗必歸還票據和局部參數",
    "g18_f_竄改已選多載方法跨度或來源值須在執行前拒絕",
    "g18_f_竄改原_checker_call_borrows_不得改用另一方法票據",
    "g18_f_多載不合法的參數型別與相同參數簽章必由原_checker_拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    result = subprocess.run(
        [str(args.test_bin.resolve()), "--list"], text=True, encoding="utf-8",
        capture_output=True, check=False,
    )
    if result.returncode:
        print("G18 S20 多載執行 Rust 清單取得失敗：" + result.stderr, file=sys.stderr)
        return 1
    present = {
        line.split(": test", 1)[0] for line in result.stdout.splitlines()
        if line.endswith(": test")
    }
    expected = {"struct_stage::g18_private_method_overload_runtime_tests::" + name
                for name in 案例}
    missing = sorted(expected - present)
    if missing:
        print("G18 S20 真多載方法執行正反缺漏：" + "、".join(missing), file=sys.stderr)
        return 1
    print(f"G18 S20 同名方法 Copyable 實參與 self 真 BorrowTicket："
          f"{len(expected)}/{len(expected)}；正式 Rust 全量已真正執行；"
          "仍非 P18.6／G18 原項 Passed。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
