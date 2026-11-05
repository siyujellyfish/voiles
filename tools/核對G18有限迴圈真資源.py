#!/usr/bin/env python3
"""G18／P18.2～18.3 有限 for／while 真迭代與 UTF-8 資源清理正反測試門檻。"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_b_真_for_三次每迭代重新建立唯一_utf8_並離域釋放",
    "g18_b_半開空區間不建立任何假迭代資源",
    "g18_b_真_continue_每次先離域釋放再開始下一次",
    "g18_b_真_break_只執行首輪且離域釋放",
    "g18_b_真_while_false_不執行未走過的字串建構",
    "g18_b_while_true_只在直接已驗證_break_出口下執行一次",
    "g18_c_真_for_內_return_提前退出並按動態完成順序清理",
    "g18_c_第二次迭代真資源故障必回滾當次及外層持有者",
    "g18_b_巢狀_for_各迭代作用域獨立釋放且順序正確",
    "g18_b_非法隱含_str_複製由原_checker_在第一筆資源前拒絕",
    "g18_b_超出私有已證明有限上界只保守拒絕不新增語言錯誤",
    "g18_b_擅改_for_區間或_control_必須在配置任何資源前拒絕",
    "g18_b_刪除原_checker_控制轉移或改寫來源不可沿用舊執行許可",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    check = subprocess.run(
        [str(args.test_bin.resolve()), "--list"],
        text=True, encoding="utf-8", capture_output=True, check=False,
    )
    if check.returncode:
        print("G18 真迴圈：無法讀取原 Rust 測試列表", file=sys.stderr)
        print(check.stderr, file=sys.stderr)
        return 1
    names = {
        line.split(": test", 1)[0]
        for line in check.stdout.splitlines()
        if line.endswith(": test")
    }
    expected = {
        "struct_stage::g18_private_source_loop_tests::" + item for item in 案例
    }
    missing = sorted(expected - names)
    if missing:
        print("G18 有限迴圈真來源正反缺失：" + "、".join(missing), file=sys.stderr)
        return 1
    print(f"G18／P18.2～18.3 原 Checker 有限迴圈真資源："
          f"{len(expected)}/{len(expected)} 測試存在，原 CI Rust 全量已真正執行。")
    print("注意：這是已核准的受限內部執行子批，不代表 P18.1～P18.6 G18 Passed。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
