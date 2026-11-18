#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18／M2：原Checker直接self.bool、唯一ExclusiveWrite票與真P16十四項Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_m2_self_bool_direct_write_tests::"
CASES = (
    "m2_bool_write_原Lexer_bool_false直接真原子覆寫",
    "m2_bool_write_原Lexer_bool_true覆寫原false預設",
    "m2_bool_write_方法bool形參原DeclId真值入同一self欄位",
    "m2_bool_write_main具名bool真值於借self前傳入方法",
    "m2_bool_write_同一self備用bool欄位真讀再覆寫主bool",
    "m2_bool_write_main先寫backup後獨占方法再從真P16讀來源",
    "m2_bool_write_原Lexer多重Group_self欄位仍保留唯一欄位身分",
    "m2_bool_write_原Checker同欄位自讀覆寫不產生多張借票",
    "m2_bool_write_第一個int欄位為const仍可獨占寫另一個var_bool",
    "m2_bool_write_方法入口注入故障不提交bool且歸還唯一借票",
    "m2_bool_write_真欄位已提交但返回溢位仍須Unwind及還票",
    "m2_bool_write_準備期篡改目標來源與預期值均須事前SourceChanged",
    "m2_bool_write_原Checker靜態拒絕const或int冒充bool與未知self欄位",
    "m2_bool_write_async_self直接await延遲喚醒後才真寫bool並還票",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run(
            [str(binary), "--list"], capture_output=True, check=False,
            text=True, encoding="utf-8",
        )
        if listed.returncode:
            raise ValueError("Rust --list 執行失敗：" + listed.stderr[-3000:])
        required = {PREFIX + name for name in CASES}
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        if len(required) != 14 or actual != required:
            raise ValueError(
                "self.bool直接寫入Gate缺失=" + str(sorted(required - actual))
                + "；多出=" + str(sorted(actual - required))
            )
        for i, name in enumerate(CASES, 1):
            proc = subprocess.run(
                [str(binary), "--exact", PREFIX + name, "--test-threads=1"],
                capture_output=True, check=False,
                text=True, encoding="utf-8",
            )
            output = proc.stdout + proc.stderr
            if proc.returncode or not re.search(
                r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", output
            ):
                raise ValueError(
                    f"原Checker self.bool獨占寫入 {i}/14 {name} 失敗\n" + output[-9000:]
                )
            print(f"G18 M2 真ExclusiveWrite／P16 bool {i}/14：{name} 通過")
        print("十四項原Checker self.bool真原子寫入、安全歸票與V7直接await Gate全部通過；"
              "B4/M2/M3/A1/A2仍OpenEngineering。")
        return 0
    except (OSError, ValueError) as exc:
        print("G18 self.bool真獨占寫入Gate失敗：" + str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
