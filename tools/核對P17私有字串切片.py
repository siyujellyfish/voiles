#!/usr/bin/env python3
"""核對二十八項 P17-C／P18-B 私有真 Checker→OwnedStrRuntime 來源執行案例存在且已由全量 Rust 執行。"""

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
    "p17_c_真私有_checker_整筆_move_覆寫現存_str_舊資源立即釋放",
    "p17_c_移動後重新初始化原持有者_按真實完成次序逆序清理",
    "p17_c_賦值目標不可變與自身移動在_checker_先拒絕",
    "p17_c_已移動來源再次轉交必須在建立資源之前靜態拒絕",
    "p17_c_真資源整筆重綁前若來源改寫不得產生任何實際事件",
    "p18_b_真私有_成立分支建立兩筆_str_並在外層返回前逆序清理",
    "p18_b_不成立分支不可建立資源_真_else_才會配置與離域清理",
    "p18_b_條件_false_且無_else_不產生未走到的資源事件",
    "p18_b_局部同名遮蔽在離開真資源作用域後不破壞外層持有者",
    "p18_b_分支局部所有者跨作用域外洩必須在_checker_拒絕",
    "p18_b_分支內返回或巢狀資源控制尚未驗證必須保守拒絕",
    "p18_b_具名_bool_條件由真宣告身分決定實際資源分支",
    "p18_b_具名_bool_false_未選分支永不產生_utf8_資源",
    "p18_b_來源為_bool_但是由字串同名冒充條件須事前拒絕",
    "p18_b_未初始化_bool_或未知名稱不得開始資源執行",
    "p18_b_非純值_bool_初始式即便_checker_通過也不偷渡新求值能力",
    "p18_b_巢狀兩層_true_的真資源按執行路徑逆序釋放",
    "p18_b_巢狀_false_只建立內層_else_實際資源",
    "p18_b_最外層_false_不得執行任何巢狀真資源",
    "p18_b_兩層同名局部遮蔽維持獨立持有者與逆序釋放",
    "p18_b_巢狀分支局部變數逃逸由真_checker_提前拒絕",
    "p18_b_巢狀分支內的_return_動態清理尚未接通必須拒絕",
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
