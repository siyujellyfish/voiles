#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""F2 str 直接轉發：逐步數故障、多 ResourceId、來源重播十二項真 Gate。"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f2_str_forward_budget_tests::"
CASES = (
    "f2_forward_budget_多層跨框架每個步數截斷均真資源清理不丟失",
    "f2_forward_budget_先返回後經中層參數再轉發同resourceid",
    "f2_forward_budget_兩個獨立字串參數其中一筆直接返回且另一筆先釋放",
    "f2_forward_budget_返回函式外層括號必保留真checker及唯一id",
    "f2_forward_budget_未命中if不得提前建立或呼叫原str函式",
    "f2_forward_budget_未進入真正子函式前的暫存字面值加溢位逐框架清理",
    "f2_forward_budget_重播第二次真returned事件必遭來源雙帳防偽拒絕",
    "f2_forward_budget_竄改第二次真strreturned的resourceid必遭R05拒絕",
    "f2_forward_budget_雙字串已返回第三實參溢位逆序回收而不入被呼叫者",
    "f2_forward_budget_雙字串前置實參逐步數截斷均無遺漏與重複釋放",
    "f2_forward_budget_雙實參將第二個轉交目標竄改成第一個即使id正確仍拒絕",
    "f2_forward_budget_雙來源第二個str返回偽冒第一個id必遭稽核拒絕",
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    try:
        exe = args.test_bin.resolve(strict=True)
        listed = subprocess.run([str(exe), "--list"], text=True, encoding="utf-8",
                                capture_output=True, check=False)
        if listed.returncode:
            raise ValueError("Rust --list 失敗：" + listed.stderr[-2200:])
        actual = {
            line.removesuffix(": test") for line in listed.stdout.splitlines()
            if line.startswith(PREFIX) and line.endswith(": test")
        }
        expected = {PREFIX + case for case in CASES}
        if actual != expected:
            raise ValueError(f"測試名稱與數量不符：缺少 {sorted(expected-actual)}，"
                             f"多餘 {sorted(actual-expected)}")
        for i, case in enumerate(CASES, 1):
            result = subprocess.run([str(exe), "--exact", PREFIX + case, "--test-threads=1"],
                                    text=True, encoding="utf-8", capture_output=True, check=False)
            out = result.stdout + result.stderr
            if result.returncode or not re.search(
                    r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", out):
                raise ValueError(f"F2 步數／多資源 {i}/{len(CASES)}：{case}\n" + out[-3500:])
            print(f"F2 str 直接轉發真安全 {i}/{len(CASES)}：{case}（通過）")
        print("F2 十二項步數與多資源安全 Gate 真通過；F2 整體仍 OpenEngineering")
        return 0
    except (OSError, ValueError) as error:
        print("G18 F2 真轉發安全 Gate 失敗：" + str(error), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
