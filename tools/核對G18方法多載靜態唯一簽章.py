#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／S20 方法多載完整型別靜態唯一解析及借用證據 Gate；非動態方法執行證據。"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_f_原_parser_保留同名不同參數簽章全部方法候選順序",
    "g18_f_同名_int_多載只用完整靜態參數型別取得真共享借用",
    "g18_f_同名_bool_多載從原_checker_選獨占_self_不得誤選唯讀",
    "g18_f_同名多載方法宣告顛倒不影響靜態唯一選擇或借用權限",
    "g18_f_同名多載不可憑返回型別或參數名稱視為不同簽章",
    "g18_f_const_結構僅允許與參數型別匹配的共享唯讀多載",
    "g18_f_同名多載參數數量或型別無匹配必靜態拒絕",
    "g18_f_欄位方法同名無論宣告順序仍由原_parser_拒絕",
    "g18_f_原同名唯一方法索引與正常非多載不因新候選表退化",
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
        print("G18 方法多載真 Rust 清單取得失敗：" + result.stderr, file=sys.stderr)
        return 1
    available = {
        line.split(": test", 1)[0]
        for line in result.stdout.splitlines() if line.endswith(": test")
    }
    expected = {"struct_stage::g18_private_method_overload_tests::" + name
                for name in 案例}
    missing = sorted(expected - available)
    if missing:
        print("G18 方法多載靜態來源正反缺漏：" + "、".join(missing), file=sys.stderr)
        return 1
    print(f"G18 S20 同名方法靜態簽章／self 借用 Gate：{len(expected)}/{len(expected)}；"
          "須由 Rust 全量真正執行。此 Gate 不可充當方法多載動態執行或 P18.6 Passed。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
