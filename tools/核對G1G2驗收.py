#!/usr/bin/env python3
"""G1/G2 獨立根因關卡：真正五階 Parser／Checker 驗收，不替代 G0。"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def check_stage(stage: str, command: list[str]) -> None:
    result = subprocess.run(
        command, cwd=ROOT, capture_output=True, text=True, encoding="utf-8",
        errors="replace", check=False,
    )
    if result.returncode:
        raise AssertionError(
            f"{stage} 子驗收失敗（{result.returncode}）\n"
            + result.stdout[-3000:] + "\n" + result.stderr[-3000:]
        )
    if stage not in result.stdout:
        raise AssertionError(f"{stage} 真實成功標記沒有出現在正式 CLI 輸出\n"
                             + result.stdout[-1800:])
    print(f"{stage}：獨立真 Rust／共同權威完整重新執行成功")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("必須指定 --check，拒絕以文件描述取代驗收")
    if os.environ.get("GITHUB_ACTIONS") == "true":
        sha = os.environ.get("GITHUB_SHA", "")
        if not re.fullmatch("[0-9a-f]{40}", sha):
            raise AssertionError("GitHub Actions 未提供完整 SHA")
    else:
        sha = None
    stage_paths = {
        "F01": ROOT / "tools/核對F01驗收.py",
        "F02": ROOT / "tools/核對F02驗收.py",
        "F03": ROOT / "tools/核對F03驗收.py",
        "F04": ROOT / "tools/核對F04驗收.py",
        "F05": ROOT / "tools/核對F05驗收.py",
    }
    for stage, path in stage_paths.items():
        if not path.is_file():
            raise AssertionError(f"缺少不可取代的真階段驗收入口：{path}")
        check_stage(stage, [
            sys.executable, str(path), "--test-bin", args.test_bin, "--check",
        ])
        if stage == "F04":
            print(json.dumps({
                "gate": "P16.6-R02-G1",
                "result": "Passed",
                "source": "F01～F04 真 Parser/HIR --exact 與單一可寫來源",
                "github_sha": sha,
                "ref": os.environ.get("GITHUB_REF", "local"),
                "main_verified": os.environ.get("GITHUB_REF") == "refs/heads/main",
                "requires_g0_separately": True,
            }, ensure_ascii=False, sort_keys=True))
    print(json.dumps({
        "gate": "P16.6-R02-G2",
        "result": "Passed",
        "source": "F05 14 個真 Rust、共通解析唯一及所有模式正反",
        "github_sha": sha,
        "ref": os.environ.get("GITHUB_REF", "local"),
        "main_verified": os.environ.get("GITHUB_REF") == "refs/heads/main",
        "requires_g0_separately": True,
    }, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as exc:
        print(f"G1/G2 正式證據不足：{exc}", file=sys.stderr)
        sys.exit(1)
