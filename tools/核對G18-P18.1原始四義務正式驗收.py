#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P18.1 四義務 F1～F4 全數 Closed 且各真來源 Gate 成功才可正式 Passed。"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
REGISTER = ROOT / "AI-build" / "G18-P18-V0-原六項義務清冊-20261108.json"
GATES = (
    ("F1", "tools/核對G18-F1真函式圖閉合.py", False),
    ("F2", "tools/核對G18-F2原始義務正式閉合.py", True),
    ("F3", "tools/核對G18-F3原始義務正式閉合.py", True),
    ("F4", "tools/核對G18-F4可達函式圖原始義務閉合.py", True),
)


def main() -> int:
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--test-bin", type=Path, required=True)
    cli.add_argument("--check", action="store_true", required=True)
    opts = cli.parse_args()
    try:
        registered = json.loads(REGISTER.read_text(encoding="utf-8"))
        if registered["current_status"]["P18.1"] != "Passed":
            raise ValueError("機讀原六項 P18.1 並未正式 Passed")
        obligations = [v for v in registered["obligations"] if v["phase"] == "P18.1"]
        if [x["id"] for x in obligations] != [g[0] for g in GATES]:
            raise ValueError("P18.1 四個原始義務身分或順序不一致")
        for item, (ident, path, require_check) in zip(obligations, GATES):
            if item["status"] != "Closed" or item.get("closure_proof", {}).get("gate") != path:
                raise ValueError(f"{ident} 缺少 Closed 與獨立 Gate 原始證明")
            script = ROOT / path
            if not script.is_file():
                raise ValueError(f"{ident} 真 Gate 不存在")
            command = [sys.executable, str(script), "--test-bin", str(opts.test_bin)]
            if require_check:
                command.append("--check")
            result = subprocess.run(command, capture_output=True, text=True,
                                    encoding="utf-8", check=False)
            if result.returncode:
                raise ValueError(f"{ident} 原真來源 Gate 失敗：\n" +
                                 (result.stdout + result.stderr)[-3800:])
            print(f"P18.1 {ident} 來源 Gate 重新真執行通過")
        print("P18.1 原四項 F1～F4 已全數 Closed，四獨立 Gate 真重跑 Passed；"
              "其餘 P18.2～P18.6 不提前完成。")
        return 0
    except (ValueError, OSError, KeyError, TypeError) as error:
        print("P18.1 正式原四義務驗收拒絕：" + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
