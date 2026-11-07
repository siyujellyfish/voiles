#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／R06：核對三十九項原 Checker 真執行、資源／票據退出交叉矩陣；並非 G18 全項 Passed。"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_r06_原_checker_VB14_借用別名缺乏重借證明不得進真子呼叫",
    "g18_r06_原_checker_准許真持有者_move_給子函式不受_VB14_錯誤擴張",
    "g18_r05_r06_存活借用票據期間偽造同源跨框架_move_仍須拒絕",
    "g18_r06_三層真函式同一_resourceid_連續跨框架_move_僅一次清理",
    "g18_r06_函式_for_每輪新_str_跨函式_move_返回後即時釋放",
    "g18_r06_同次呼叫先借再_move_不同真來源_各有正確出口",
    "g18_r06_移動實參已準備但_callbudget_耗盡不得假轉交",
    "g18_r05_r06_偽造跨框架_move_目標宣告身分須拒絕",
    "g18_r06_str_覆寫後再借新_resourceid_票據須對應最新擁有者",
    "g18_r06_真_move_覆寫後借目標仍指同一原始_resourceid",
    "g18_r05_r06_偽造_str_覆寫後仍由舊資源認領必拒絕",
    "g18_r06_str_覆寫後整數溢位仍釋放目前唯一擁有者",
    "g18_r05_r06_每張真借用票據必連至實際_utf8_resourceid",
    "g18_r05_r06_同框架兩個存活_resourceid_借用不得錯綁他人",
    "g18_r05_r06_偽造借用未歸還便先釋放真來源資源必拒絕",
    "g18_r05_r06_票據資源身分偽造重複綁定或缺失均拒絕",
    "g18_r06_同一_while_重借外層_const_str_逐次歸還才進下一輪",
    "g18_r06_真借用已取得但子呼叫框架預算耗盡仍全數原子歸還",
    "g18_r06_借用後第二_int_實參溢位_真票據歸還先於_resourceid_釋放",
    "g18_r05_r06_重複歸還同一票據必由同序帳簿拒絕",
    "g18_r06_同一函式圖兩張_const_str_共享票據來源相同但序號不同",
    "g18_r06_兩個獨立_var_str_同一次真呼叫各有獨占票據",
    "g18_r06_同一真函式堆疊先_move_後借另一_source_資源不混淆",
    "g18_r06_for_重複同一外層_str_真借用逐輪歸還",
    "g18_r06_未選中的_if_真借用不簽發任何動態票據",
    "g18_r05_r06_偽造借用函式已退出後才歸還票據必拒絕",
    "g18_r06_真_while_for_局部_算術與_str_借用雙執行器結果皆須平衡",
    "g18_r06_巢狀同名遮蔽與同步_self_不同資源帳簿均有真退出",
    "g18_r06_迴圈_break_先釋放局部與遞迴資源耗盡各守真出口",
    "g18_r06_區塊算術溢位與_str_借用中途故障均禁止虛構返回",
    "g18_r06_純值區塊離域偽造及_move_borrow_資源偽造皆失敗",
    "g18_r06_合法迴圈中提前返回清理_另一條資源模型仍歸還票據",
    "g18_r06_巢狀_while_預算耗盡與_self_故障無假成功返回",
    "g18_r06_靜態逃逸拒絕不得取代_true_runtime_安全交叉驗證",
    "g18_r06_混合布林整數與真借用成功返回各守門須同時成立",
    "g18_r06_同_call_整筆移動借用和同步_self_不混淆資源責任",
    "g18_r06_合法遞迴真資源與普通借用在預算耗盡時皆不得假返回",
    "g18_r06_不同執行器的正常失敗不得丟失已建資源或複製假返回",
    "g18_r06_跨模型偽造離域資源票據任一項必在交付稽核失敗",
)
前綴 = "struct_stage::g18_r06_cross_runtime_tests::"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    args = parser.parse_args()
    executable = str(args.test_bin.resolve())
    listing = subprocess.run(
        [executable, "--list"], text=True, encoding="utf-8",
        capture_output=True, check=False,
    )
    if listing.returncode:
        print("R06 Rust 真來源清單無法取得：" + listing.stderr, file=sys.stderr)
        return 1
    available = {
        line.split(": test", 1)[0]
        for line in listing.stdout.splitlines() if line.endswith(": test")
    }
    missing = sorted({前綴 + name for name in 案例} - available)
    if missing:
        print("R06 真來源案例缺失：" + "、".join(missing), file=sys.stderr)
        return 1
    run = subprocess.run(
        [executable, "--test-threads=2", 前綴], text=True,
        encoding="utf-8", capture_output=True, check=False,
    )
    match = re.search(
        r"test result: ok\.\s+(\d+) passed;\s+0 failed;\s+0 ignored;",
        run.stdout + run.stderr,
    )
    if run.returncode or not match or int(match.group(1)) != len(案例):
        print("R06 跨真執行器資源退出矩陣未全過："
              + run.stdout + run.stderr, file=sys.stderr)
        return 1
    print(f"G18 R06 真跨執行器正反：{len(案例)}/{len(案例)} 通過；"
          "這不代表原 P18.1～P18.6 已完整 Passed。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
