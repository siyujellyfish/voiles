#!/usr/bin/env python3
"""G17／P17.3：正式 Checker 直線 .borrow() → 真 BorrowTicket／ResourceId 正反 Gate。"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "g17_c_合法_const_str_真共享借用取得和歸還動態票據不複製資源",
    "g17_c_合法_var_str_真獨占票據封鎖持有者直接存取後歸還",
    "g17_c_兩個合法共享別名各有真實票據而同一_utf8_只配置一次",
    "g17_c_非法借用型別或_var_const_權限不符由真_checker_先拒絕",
    "g17_c_偽造借用宣告_id_不能靠相同名稱在真資源分配前過關",
    "g17_c_偽造共享獨占權限或_source_id_在任何資源分配前拒絕",
    "g17_c_刪去原_checker_直線借用事實不可沿用舊票據授權",
    "g17_c_篡改已驗證來源位元組不能開始真字串資源執行",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    proc = subprocess.run(
        [str(args.test_bin.resolve()), "--list"],
        text=True, encoding="utf-8", capture_output=True, check=False,
    )
    if proc.returncode:
        print("G17 真借用票據：Rust 測試列表讀取失敗：" + proc.stderr, file=sys.stderr)
        return 1
    actual = {
        line.split(": test", 1)[0]
        for line in proc.stdout.splitlines()
        if line.endswith(": test")
    }
    required = {"struct_stage::g17_private_source_borrow_tests::" + name for name in 案例}
    missing = sorted(required - actual)
    if missing:
        print("G17 真借用來源／票據測試缺漏：" + "、".join(missing), file=sys.stderr)
        return 1
    print(
        f"G17 真借用來源與動態票據正反：{len(required)}／{len(required)}，"
        "既有 Rust 全量步驟逐項實際執行；此 Gate 不重跑測試。"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
