#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／P18.1-F3：同一原 Checker 圖的呼叫位置與零不可達事件六項真來源驗證。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f3_call_site_tests::"
CASES = (
    "f3_sites_主函式同一圖按初始化賦值分支與返回順序完成四呼叫",
    "f3_sites_未選if葉不呼叫而選中else借用完成先還票後離域",
    "f3_sites_for三輪逐次呼叫並於每輪退出真utf8與借用",
    "f3_sites_while真首輪呼叫後break不得重放返回或第二輪",
    "f3_sites_選中if的被呼叫者溢位後未到達後續函式",
    "f3_sites_改寫已編譯主函式動態返回圖即使框架平衡仍先拒絕",
)


def main() -> int:
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--test-bin", type=Path, required=True)
    args = cli.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listing = subprocess.run([str(binary), "--list"], capture_output=True,
                                 text=True, encoding="utf-8", check=False)
        if listing.returncode:
            raise ValueError("Rust --list 失敗：" + listing.stderr[-2000:])
        present = {
            line.removesuffix(": test") for line in listing.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + name for name in CASES}
        if present != expected:
            raise ValueError(f"F3 來源失配：缺少 {sorted(expected-present)}；"
                             f"多出 {sorted(present-expected)}")
        for n, name in enumerate(CASES, 1):
            cmd = [str(binary), "--exact", PREFIX + name, "--test-threads=1"]
            result = subprocess.run(cmd, capture_output=True, text=True,
                                    encoding="utf-8", check=False)
            stdout = result.stdout + result.stderr
            if result.returncode or not re.search(
                    r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", stdout):
                raise ValueError(f"F3 新來源 {n}/6 失敗：{name}\n" + stdout[-3200:])
            print(f"F3 真呼叫位置 {n}/6：{name}（通過）")
        print("F3 六項合法呼叫位置真動態 Gate 已通過；F3 整體仍 OpenEngineering")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 F3 呼叫位置 Gate 失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
