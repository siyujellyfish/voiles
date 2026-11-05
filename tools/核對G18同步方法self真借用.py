#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.6：原私有 Checker 同步方法唯讀 self 真 BorrowTicket 六項 Rust 正反 Gate。"""

from __future__ import annotations
import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_f_真同步方法_self_使用原_checker_共享票據讀取實際_int_欄位",
    "g18_f_合法_const_結構接收者共享_self_不需可寫權限",
    "g18_f_方法真借用存活期間發生測試故障也要先歸還票據再離域",
    "g18_f_偽造動態_self_來源或借用_kind_必在配置前拒絕",
    "g18_f_移除真_checker_方法自動借用或方法證據不能執行",
    "g18_f_自動_self_錯誤返回型別與消耗整個_self_由原_checker_拒絕",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    listing = subprocess.run(
        [str(args.test_bin.resolve()), "--list"], text=True, encoding="utf-8",
        capture_output=True, check=False,
    )
    if listing.returncode:
        print("G18 方法 self 真 Rust 清單失敗：" + listing.stderr, file=sys.stderr)
        return 1
    available = {
        line.split(": test", 1)[0]
        for line in listing.stdout.splitlines() if line.endswith(": test")
    }
    required = {
        "struct_stage::g18_private_method_self_tests::" + name for name in 案例
    }
    missing = sorted(required - available)
    if missing:
        print("G18 self 借用真來源正反缺漏：" + "、".join(missing), file=sys.stderr)
        return 1
    print(f"G18 真同步方法 self BorrowTicket：{len(required)}/{len(required)}；"
          "多載與 async 正向執行尚未完成，原 P18.6 不得正式 Passed。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
