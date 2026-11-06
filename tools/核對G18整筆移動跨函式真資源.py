#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.3、P18.4：原 Checker str.move() 普通函式與遞迴唯一資源真轉交。"""
from __future__ import annotations
import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_d_原_checker_明確_move_普通函式實參轉交同一真_utf8_不複製",
    "g18_d_被呼叫者真的再次_move_參數至局部唯一所有者退出才釋放一次",
    "g18_c_多函式逐層_move_同一_resourceid_末端真正釋放且返回逐框架退出",
    "g18_c_合法自我遞迴每層轉交唯一_utf8_受框架預算安全耗盡",
    "g18_c_跨函式_move_步數預算耗盡不得假成功或雙重釋放",
    "g18_c_真移動之前及連續轉交之後注入故障都由目前所有者清理一次",
    "g18_c_零函式框架或零步數預算在配置真_utf8_前受控拒絕",
    "g18_d_移動後原來源再次使用與漏掉明確_move_必由原_checker_拒絕",
    "g18_d_非法借用別名不能以_move_變成真正所有者",
    "g18_d_更改已核准轉交函式或字串原值必須零執行前拒絕",
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
        print("G18 真 move 測試清單取得失敗：" + listing.stderr, file=sys.stderr)
        return 1
    actual = {
        line.split(": test", 1)[0] for line in listing.stdout.splitlines()
        if line.endswith(": test")
    }
    expected = {
        "struct_stage::g18_private_owned_move_call_tests::" + name
        for name in 案例
    }
    if expected - actual:
        print("G18 原 Checker 真 move 缺少正反：" +
              "、".join(sorted(expected - actual)), file=sys.stderr)
        return 1
    run = subprocess.run(
        [exe, "--test-threads=2",
         "struct_stage::g18_private_owned_move_call_tests::"],
        text=True, encoding="utf-8", capture_output=True, check=False,
    )
    if run.returncode or f"{len(expected)} passed; 0 failed;" not in run.stdout:
        print("G18 真 move 跨函式唯一 ResourceId Gate 未完成：" +
              run.stdout + run.stderr, file=sys.stderr)
        return 1
    print(f"G18 原 Checker str.move 真跨函式／遞迴唯一資源："
          f"{len(expected)}/{len(expected)} 真動態重跑；"
          "尚缺一般 move／borrow 混合完整矩陣，不等於 P18 Passed。")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
