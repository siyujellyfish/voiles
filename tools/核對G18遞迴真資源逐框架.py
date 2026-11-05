#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.3：原私有 Checker bool 參數遞迴與 UTF-8 真 ResourceId 逐框架清理。"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

案例 = (
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
    print(f"G18 原 Checker bool 直接／互相遞迴真 UTF-8 ResourceId："
          f"{len(required)}/{len(required)} 已真正重跑；"
          "本批已接通受限跨框架 str 借用；一般遞迴與借用完整矩陣尚未正式 Passed。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
