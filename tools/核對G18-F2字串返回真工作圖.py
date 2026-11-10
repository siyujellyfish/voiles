#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""G18 P18.1-F2：str 返回真原 Checker／P16／R05 來源五項 --exact 驗收門。"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_f2_str_return_graph_tests::"
CASES = (
    "f2_str_return_來源直接utf8字面真正跨函式接收與唯一釋放",
    "f2_str_return_原checker局部move後const接收無重建且同resourceid",
    "f2_str_return_同一主函式兩次真正接收必區分資源id與逆序清理",
    "f2_str_return_原checker非法資源拷貝與借用逃逸一律拒絕",
    "f2_str_return_竄改真正接收ResourceId即使釋放筆數平衡亦拒絕",
)

def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--test-bin", required=True, type=Path)
    args = p.parse_args()
    try:
        binary = args.test_bin.resolve(strict=True)
        listed = subprocess.run([str(binary), "--list"], capture_output=True,
            text=True, encoding="utf-8", check=False)
        if listed.returncode:
            raise ValueError("Rust --list 失敗："+listed.stderr[-1500:])
        found = {line.removesuffix(": test") for line in listed.stdout.splitlines()
                 if line.startswith(PREFIX) and line.endswith(": test")}
        expected = {PREFIX + name for name in CASES}
        if found != expected:
            raise ValueError("真來源五項身分不符；缺少："+str(sorted(expected-found))
                             +"；額外："+str(sorted(found-expected)))
        for index,name in enumerate(CASES,1):
            result = subprocess.run([str(binary), "--exact", PREFIX+name,
                "--test-threads=1"], capture_output=True, text=True,
                encoding="utf-8", check=False)
            text = result.stdout + result.stderr
            if result.returncode or not re.search(
                    r"test result: ok\.\s+1 passed;\s+0 failed;\s+0 ignored;", text):
                raise ValueError(f"第 {index}/{len(CASES)} 項真 Rust 未通過：{name}\n"+text[-2600:])
            print(f"F2 str return {index}/{len(CASES)}：{name}（真實通過）")
        print("F2 str-return 來源首批五項成功；F2 尚不得視為全部 Closed")
        return 0
    except (OSError,ValueError) as e:
        print("F2 str-return Gate 失敗："+str(e), file=sys.stderr)
        return 1

if __name__ == "__main__":
    raise SystemExit(main())
