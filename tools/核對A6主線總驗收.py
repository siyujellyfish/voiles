#!/usr/bin/env python3
"""A6：同一正式 main SHA 真正重跑 G0、F01～F05、G1/G2，不拼湊不同 PR 證據。"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
TEST_SCRIPTS = {
    "F01": "tools/核對F01驗收.py",
    "F02": "tools/核對F02驗收.py",
    "F03": "tools/核對F03驗收.py",
    "F04": "tools/核對F04驗收.py",
    "F05": "tools/核對F05驗收.py",
}
G1G2 = "tools/核對G1G2驗收.py"
FROZEN_G0 = "tools/核對F00總驗收.py"


def run(command: list[str], stage: str) -> str:
    result = subprocess.run(
        command, cwd=ROOT, check=False,
        capture_output=True, text=True, encoding="utf-8", errors="replace",
    )
    if result.returncode != 0:
        raise AssertionError(
            f"{stage} 真命令失敗（{result.returncode}）：{' '.join(command)}\n"
            + result.stdout[-3500:] + "\n" + result.stderr[-3500:]
        )
    return result.stdout + result.stderr


def actual_source_identity() -> tuple[str, str, bool]:
    """PR 候選不能冒充實際 main；正式 main 必須跟 checkout 實際 HEAD 完全同一 SHA。"""
    actual = run(["git", "rev-parse", "HEAD"], "git HEAD").strip()
    if not re.fullmatch(r"[0-9a-f]{40}", actual):
        raise AssertionError("git HEAD 不是有效 40 字元 commit SHA")
    if os.environ.get("GITHUB_ACTIONS") != "true":
        return actual, "local", False
    sha = os.environ.get("GITHUB_SHA", "")
    ref = os.environ.get("GITHUB_REF", "")
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise AssertionError("GitHub Actions 缺少完整 GITHUB_SHA")
    if sha != actual:
        raise AssertionError(f"CI 工作目錄 HEAD 與 GitHub SHA 不一致：{actual} != {sha}")
    if ref == "refs/heads/main":
        return actual, ref, True
    if ref.startswith("refs/pull/") and ref.endswith("/merge"):
        return actual, ref, False
    raise AssertionError(f"非正式 main 或 PR 的未知 CI ref，不得被 A6 採認：{ref}")


def check_g1g2(binary: str, sha: str, ref: str, main_verified: bool) -> None:
    output = run([
        sys.executable, str(ROOT / G1G2),
        "--test-bin", binary, "--check",
    ], "G1/G2")
    required = {"P16.6-R02-G1", "P16.6-R02-G2"}
    reports = []
    for line in output.splitlines():
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(record, dict) and record.get("gate") in required:
            reports.append(record)
    if len(reports) != 2 or {r["gate"] for r in reports} != required:
        raise AssertionError("G1/G2 真工具必須各輸出唯一、非空結果；不能靠成功文案推算")
    for record in reports:
        if record.get("result") != "Passed":
            raise AssertionError(f"{record['gate']} 並未真成功")
        if main_verified:
            if record.get("github_sha") != sha or record.get("ref") != ref \
                    or record.get("main_verified") is not True:
                raise AssertionError(f"{record['gate']} 缺乏相同 main SHA 真工作證據")
        else:
            if record.get("main_verified") is True:
                raise AssertionError("PR／本地測試竟宣稱 main_verified，必須拒絕")
    print("G1/G2：兩個真工具各具唯一成功記錄；主線身分檢查："
          + ("同 main SHA" if main_verified else "僅 PR／本地候選"))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("A6 必須 --check：拒絕以文件或過往 CI 狀態代替真實執行")
    sha, ref, main_verified = actual_source_identity()
    binary = str((ROOT / args.test_bin).resolve())
    if not Path(binary).is_file():
        raise AssertionError("未重新編譯真正 Rust 測試執行檔，不得驗收")
    executed: list[str] = []
    for stage, path in TEST_SCRIPTS.items():
        result = run([
            sys.executable, str(ROOT / path), "--test-bin", binary, "--check",
        ], stage)
        if stage not in result:
            raise AssertionError(f"{stage} 出場腳本沒有獨立成功訊息")
        executed.append(stage)
        print(f"A6/{stage}：真正逐子項 --list／--exact 重新執行成功")
    frozen = run([sys.executable, str(ROOT / FROZEN_G0)], "G0")
    if "F00／G0 四組凍結證據一致" not in frozen:
        raise AssertionError("G0 原四凍結未輸出實際一致性證據")
    executed.append("G0")
    print("A6/G0：P01／Phase24／私有113／八橋接57真事件四組重新核對成功")
    check_g1g2(binary, sha, ref, main_verified)
    executed.extend(["G1", "G2"])
    if executed != ["F01", "F02", "F03", "F04", "F05", "G0", "G1", "G2"]:
        raise AssertionError("A6 八個要求實際執行的驗收項目不完整")
    print(json.dumps({
        "gate": "P16.6-R02-A6",
        "result": "Passed" if main_verified else "PassedCandidate",
        "main_verified": main_verified,
        "source": "fresh-checkout Rust --exact + G0 4 frozen checks + G1/G2",
        "git_head": sha,
        "github_ref": ref,
        "executed": executed,
        "outstanding": [] if main_verified else ["同一最新 main SHA push CI"],
        "r03_in_scope": False,
    }, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as exc:
        print(f"A6 最終主線驗收失敗：{exc}", file=sys.stderr)
        sys.exit(1)
