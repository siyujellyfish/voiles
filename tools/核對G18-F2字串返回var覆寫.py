#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／F2：原 Checker str-return 覆寫 var 與 P16 接收原語八項真正反 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

GROUPS = {
    "struct_stage::g18_f2_str_return_assign_tests::": (
        "f2_str_assign_既有var原子接管真返回先釋放舊資源且不二次配置",
        "f2_str_assign_未初始化var接管str返回同一id並於退出唯一釋放",
        "f2_str_assign_連續兩次返回覆寫各id只釋放一次",
        "f2_str_assign_借用實參後溢位不進callee不交付且舊資源正常清理",
        "f2_str_assign_const與錯誤型別仍由原checker拒絕不得建立資源",
        "f2_str_assign_偽造覆寫接受resourceid即使釋放數平衡也須拒絕",
    ),
    "owned_str_runtime::g18_f2_str_return_reassign_runtime_tests::": (
        "f2_return_replace_原子接收相同utf8且只釋放舊資源",
        "f2_return_replace_不可寫拒絕歸還完整資源並保留原值",
    ),
}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        exe = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(exe), "--list"], text=True, encoding="utf-8",
            capture_output=True, check=False,
        )
        if listed.returncode:
            raise ValueError("原 Rust --list 失敗：" + listed.stderr[-1800:])
        known = {
            line.removesuffix(": test")
            for line in listed.stdout.splitlines()
            if line.endswith(": test")
        }
        all_cases = [prefix + case for prefix, cases in GROUPS.items() for case in cases]
        for prefix, cases in GROUPS.items():
            observed = {name for name in known if name.startswith(prefix)}
            expected = {prefix + case for case in cases}
            if observed != expected:
                raise ValueError(
                    f"來源 {prefix} 不符：缺少 {sorted(expected - observed)}；"
                    f"多餘 {sorted(observed - expected)}"
                )
        for index, name in enumerate(all_cases, 1):
            proc = subprocess.run(
                [str(exe), "--exact", name, "--test-threads=1"],
                text=True, encoding="utf-8", capture_output=True, check=False,
            )
            output = proc.stdout + proc.stderr
            if proc.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"真 Rust {index}/{len(all_cases)} 失敗：{name}\n" + output[-2500:]
                )
            print(f"F2 str-return var 覆寫 {index}/{len(all_cases)}：{name}（通過）")
        print("F2 var str-return 八項真驗收通過；F2 原義務仍待全矩陣閉合")
        return 0
    except (OSError, ValueError) as error:
        print("G18 F2 str-return var Gate 失敗：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
