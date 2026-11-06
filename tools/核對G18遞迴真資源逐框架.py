#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.3：原私有 Checker bool 參數遞迴與 UTF-8 真 ResourceId 逐框架清理。"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
    "g18_d_兩個獨立_var_str_加雙_int_跨遞迴真票據全序列及逆序清理",
    "g18_d_同函式兩個_const_str_共享借用同一來源須用兩張真票據歸還",
    "g18_d_兩個_str_與_int_不同比例順序之互相遞迴仍真借用並返回",
    "g18_c_兩張真_str_票據存活時框架預算耗盡仍全部原子歸還",
    "g18_c_兩張真_str_票據已簽發後動態故障不留任何資源",
    "g18_d_兩張借用中第二張_source_或_kind_被竄改必在配置前拒絕",
    "g18_d_同個可寫_str_來源同次傳兩個獨占參數須原_checker_拒絕",
    "g18_d_雙_str_呼叫漏參數及錯誤型別仍由原_checker_拒絕",
    "g18_c_直接遞迴兩個_copyable_int_互換父子參數槽並保護真_str_票據",
    "g18_c_單張_str_borrowticket_和三個_int_跨遞迴位置交換實際返回",
    "g18_c_相同_copyable_int_父實參可給兩個子參數但不可複製真_str",
    "g18_c_五個互相呼叫函式與六層混合資源_不得被舊兩函式上限拒絕",
    "g18_c_多函式混合資源框架預算耗盡須逆序釋放每個已配置_resourceid",
    "g18_c_混合多個_int_底層溢位不得略過真借用歸還及_utf8_釋放",
    "g18_c_混合遞迴第二_int_及多函式來源篡改必在建立資源前拒絕",
    "g18_c_混合多_int_錯誤參數型別與個數須由原_checker_拒絕",
    "g18_c_直接遞迴_str_真票據與_copyable_int_逐框架傳遞並由_hir_算術返回",
    "g18_c_互相遞迴_參數順序不同仍保留原_checker_int_與_str_借用身份",
    "g18_c_無_str_實參的_bool_int_遞迴仍真建立每框架_utf8_並返回動態參數",
    "g18_c_子遞迴_int_常值覆寫父框架值_底層不得錯讀父參數",
    "g18_c_混合_int_底層算術溢位須失敗並先歸還借用票據再逐層釋放",
    "g18_c_混合_str_int_跨框架呼叫預算耗盡仍保留實際參數值與全部清理",
    "g18_c_混合遞迴第二層_int_已綁定後故障仍清理所有_utf8_與票據",
    "g18_c_竄改遞迴_int_實參來源或返回_hir_必須在真資源建立前拒絕",
    "g18_c_混合遞迴錯誤_int_型別及漏參數必先由原_checker_拒絕",
    "g18_c_深層重入同一宣告來源須各有動態_borrow_tracker_且受控耗盡",
    "g18_c_跨框架相同原宣告位置的唯讀和獨占票據不能誤互斥",
    "g18_c_深層重入步數耗盡各框架票據與所有者全部逆序歸還",
    "g18_c_重入框架內故障不以共用靜態宣告身份錯誤解鎖",
    "g18_c_直接遞迴真_str_參數兩層_borrowticket_先歸還再清理_utf8",
    "g18_c_互相遞迴每層真_str_借用票據不重覆也不洩漏",
    "g18_c_跨框架借用存活時預算耗盡須先還真票據再釋放來源",
    "g18_c_第二層借用來源完成配置後注入錯誤仍歸還全部票據",
    "g18_c_竄改已核准呼叫_borrowfact_來源與_kind_在配置前拒絕",
    "g18_c_原_checker_缺失呼叫借用票據或非法_str_來源不可真執行",
    "g18_c_真直接遞迴每層_utf8_所有者按呼叫及分支逆序清理",
    "g18_c_真互相遞迴各層和分支資源跨框架清理不重覆",
    "g18_c_遞迴框架預算耗盡須釋放已建立_utf8_且不得假成功",
    "g18_c_遞迴步數預算耗盡即使分支開啟仍逐層逆序清理",
    "g18_c_真第二層局部_utf8_建立後注入故障逐層回滾",
    "g18_c_源碼或預備的巢狀資源被篡改須在零配置前拒絕",
    "g18_c_非法遞迴引數及非_copyable_複製由原_checker_先拒絕",
    "g18_c_空預算不進入任一框架且動態事件為空",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    exe = str(args.test_bin.resolve())
    listing = subprocess.run([exe, "--list"], capture_output=True, text=True,
                             encoding="utf-8", check=False)
    if listing.returncode:
        print("G18 遞迴真資源 Rust 清單失敗：" + listing.stderr, file=sys.stderr)
        return 1
    actual = {line.split(": test", 1)[0]
              for line in listing.stdout.splitlines() if line.endswith(": test")}
    required = {"struct_stage::g18_private_recursive_str_frames_tests::" + name
                for name in 案例}
    missing = sorted(required - actual)
    if missing:
        print("G18 原 Checker 遞迴真資源測試缺漏：" + "、".join(missing),
              file=sys.stderr)
        return 1
    run = subprocess.run([exe, "--test-threads=2",
                          "struct_stage::g18_private_recursive_str_frames_tests::"],
                         capture_output=True, text=True,
                         encoding="utf-8", check=False)
    if run.returncode or "test result: ok." not in run.stdout or \
            f"{len(案例)} passed; 0 failed;" not in run.stdout:
        print("G18 遞迴真資源執行未全過：" + run.stdout + run.stderr,
              file=sys.stderr)
        return 1
    print(f"G18 原 Checker bool／str／int 混合遞迴真 UTF-8 ResourceId："
          f"{len(required)}/{len(required)} 已真正重跑；"
          "本批已接通受限跨框架 str 借用；一般遞迴與借用完整矩陣尚未正式 Passed。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
