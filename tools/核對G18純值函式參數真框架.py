#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.1、P18.4 正式私有 Checker Copyable int 參數真框架的十六項 Rust 正反 Gate。"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_a_前向函式真_copyable_int_參數直接返回並保留動態事件順序",
    "g18_a_真函式_copyable_int_參數參與加法且回傳動態值",
    "g18_a_真函式_copyable_int_參數多次引用每次都取同一實際值",
    "g18_a_真函式_群組與左右結合加減_依原_checker_來源_hir_計算",
    "g18_a_具名_copyable_int_實參傳入函式後參與真減法",
    "g18_c_函式參數真加法溢位不能假造_return_且確實離域",
    "g18_c_函式參數真減法溢位亦不可留下框架或偽造成功事件",
    "g18_a_竄改編譯後_true_arithmetic_運算樹必在執行前拒絕",
    "g18_a_非法_bool_參與_int_加法與字面值不足均由原_checker_拒絕",
    "g18_a_具名_const_int_實參以原_checker_引用身分綁定函式參數",
    "g18_a_具名_var_int_合法_copyable_不因函式返回消耗持有者",
    "g18_c_真正參數框架中途故障必清空且不冒充成功返回",
    "g18_a_錯誤_copyable_int_實參型別及引數個數由原_checker_拒絕",
    "g18_a_具名未初始化_int_無法假裝有函式實參值",
    "g18_a_竄改已驗證參數_declarationid_和返回值必須零執行拒絕",
    "g18_a_竄改主函式呼叫跨度或原來源位元組不得建立任何參數框架",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    result = subprocess.run(
        [str(args.test_bin.resolve()), "--list"],
        capture_output=True, text=True, encoding="utf-8", check=False,
    )
    if result.returncode:
        print("G18 純值函式框架無法取得 Rust 測試清單：" + result.stderr,
              file=sys.stderr)
        return 1
    present = {
        line.split(": test", 1)[0]
        for line in result.stdout.splitlines() if line.endswith(": test")
    }
    required = {
        "struct_stage::g18_private_scalar_call_tests::" + name for name in 案例
    }
    missing = sorted(required - present)
    if missing:
        print("G18 Copyable int 函式真來源正反缺漏：" + "、".join(missing),
              file=sys.stderr)
        return 1
    print(f"G18 真 Copyable int 函式參數與框架：{len(required)}/{len(required)}；"
          "Rust 全量已真正執行，G18 六原項仍非 Passed。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
