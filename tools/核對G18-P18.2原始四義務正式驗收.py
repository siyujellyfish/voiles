#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P18.2 四條 C1～C4 原始義務需各自 Closed、各自真正重跑 Gate 才能 Passed。"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
REGISTER = ROOT / "AI-build/G18-P18-V0-原六項義務清冊-20261108.json"
GATES = (
    ("C1", "tools/核對G18-P18.2-C1分支選葉.py"),
    ("C2", "tools/核對G18-P18.2-C2迴圈回邊.py"),
    ("C3", "tools/核對G18-P18.2-C3巢狀控制轉移.py"),
    ("C4", "tools/核對G18-P18.2-C4局部資源離域.py"),
)


def main() -> int:
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--test-bin", type=Path, required=True)
    cli.add_argument("--check", action="store_true", required=True)
    args = cli.parse_args()
    try:
        data = json.loads(REGISTER.read_text(encoding="utf-8"))
        if data["current_status"]["P18.2"] != "Passed":
            raise ValueError("P18.2 尚未依正式原四義務 Passed")
        selected = [x for x in data["obligations"] if x["phase"] == "P18.2"]
        if [x["id"] for x in selected] != [x[0] for x in GATES]:
            raise ValueError("P18.2 C1～C4 契約不完整或順序改寫")
        for item, (name, script) in zip(selected, GATES):
            if item["status"] != "Closed" or item.get("closure_proof", {}).get("gate") != script:
                raise ValueError(f"{name} 尚未有獨立 Closed 證明")
            proc = subprocess.run([sys.executable, str(ROOT / script),
                                   "--test-bin", str(args.test_bin), "--check"],
                                  text=True, encoding="utf-8", capture_output=True, check=False)
            if proc.returncode:
                raise ValueError(f"{name} 重新真執行失敗：\n" + (proc.stdout+proc.stderr)[-3800:])
            print(f"P18.2 {name} 獨立 Gate 再執行 Passed")
        print("P18.2 C1～C4 原始四義務全閉合、44 筆真正控制來源重新驗收 Passed")
        return 0
    except (ValueError, OSError, KeyError, TypeError) as err:
        print("P18.2 正式原四義務驗收失敗：" + str(err), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
