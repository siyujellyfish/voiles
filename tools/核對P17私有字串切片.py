#!/usr/bin/env python3
"""核對十項 P17-C 私有真 Checker→OwnedStrRuntime 來源執行案例存在且已由全量 Rust 執行。"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "p17_c_真私有_checker_來源驅動字串建立移動再初始化並逆序清理",
    "p17_c_真私有_lexer_字串內容含中文及跳脫仍只有一筆真資源",
    "p17_c_非法字串隱含複製在真_checker_階段就拒絕",
    "p17_c_尚未接通的結構或分支在資源建立前保守拒絕",
    "p17_c_原始來源遭到替換不得產生動態資源事件",
    "p17_c_真資源覆寫錯誤注入原子回滾並清理原持有者",
    "p17_c_準備後修改字串值不得偷換已驗證執行計畫",
    "p17_c_準備後偷換整筆移動的來源身分立即拒絕",
    "p17_c_準備後刪除返回或插入多餘操作都不得執行",
    "p17_c_操作未遭篡改時仍保持真實_resourceid_與清理順序",
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
        print("P17-C 無法讀取既有 Rust 全量測試名稱", file=sys.stderr)
        print(result.stderr, file=sys.stderr)
        return 1
    names = {
        line.split(": test", 1)[0]
        for line in result.stdout.splitlines() if line.endswith(": test")
    }
    required = {
        "struct_stage::p17_private_str_execution_tests::" + name
        for name in 案例
    }
    missing = sorted(required - names)
    if missing:
        print("P17-C 缺少真 Rust 來源／資源正反案例：" + "、".join(missing), file=sys.stderr)
        return 1
    print(f"P17-C 非公開私有資源切片：{len(required)}/{len(required)} 項真 Rust 案例存在；"
          "由原全量 Rust 步驟已實際執行。")
    print("注意：此批不是 P17-C 全範圍或 G17 正式驗收。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
