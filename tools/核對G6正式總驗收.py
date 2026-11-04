#!/usr/bin/env python3
"""P16.6-R G6/F14：同一真正 main SHA 的 G0～G5 和 R02/R03 最終獨立驗收。

本工具每次從本次 checkout 的 Rust 二進位測試檔重新執行所有子 Gate；
PR 虛擬合併只可 PassedCandidate，不能簽發正式 6／6。
三項 GitHub CI 必須全部 completed/success 才能於主線確認結案。
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
REQUIRED_DOCUMENTS = (
    "AI-build/P16.6-R03-G3-F06-F09-正式驗收結果-20261104.md",
    "AI-build/P16.6-R03-G4-F10-F11-正式驗收結果-20261104.md",
    "AI-build/P16.6-R02-A6-同主線SHA正式總驗收-20261104.md",
)
EXPECTED_GATES = {
    "P16.6-R02-A6": {
        "executed": ["F01", "F02", "F03", "F04", "F05", "G0", "G1", "G2"],
    },
    "P16.6-R03-G3": {
        "executed": ["F06", "F07", "F08", "F09"],
        "unique_rust_tests": 50,
    },
    "P16.6-R03-G4": {
        "executed": ["F10", "F11", "G3"],
        "unique_rust_tests": 55,
    },
    "P16.6-R03-G5": {
        "executed": ["F12", "F13", "G4"],
        "unique_rust_tests": 50,
        "real_bridge_paths": 15,
        "real_resource_events": 57,
    },
}


def run(command: list[str], label: str) -> str:
    proc = subprocess.run(command, cwd=ROOT, check=False, capture_output=True,
        text=True, encoding="utf-8", errors="replace")
    if proc.returncode:
        raise AssertionError(
            f"{label} 真命令失敗（{proc.returncode}）: {' '.join(command)}\n"
            + proc.stdout[-3500:] + "\n" + proc.stderr[-3500:]
        )
    return proc.stdout + proc.stderr


def require_git_identity() -> tuple[str, str, bool]:
    head = run(["git", "rev-parse", "HEAD"], "git HEAD").strip()
    if not re.fullmatch(r"[0-9a-f]{40}", head):
        raise AssertionError("G6 HEAD 不是可追溯的完整 Git SHA")
    if os.environ.get("GITHUB_ACTIONS") == "true":
        env_sha = os.environ.get("GITHUB_SHA", "")
        ref = os.environ.get("GITHUB_REF", "")
        if head != env_sha or not re.fullmatch(r"[0-9a-f]{40}", env_sha):
            raise AssertionError("G6 git HEAD 與 GitHub 真工作 SHA 不一致")
        if not (ref == "refs/heads/main"
                or (ref.startswith("refs/pull/") and ref.endswith("/merge"))):
            raise AssertionError("G6 拒絕非 main 或 PR merge 來源冒充驗收")
        return head, ref, ref == "refs/heads/main"
    return head, "local", False


def run_json_gate(name: str, script: str, binary: str, expected_main: bool,
                  current_head: str, current_ref: str) -> dict:
    output = run([sys.executable, str(ROOT / script),
        "--test-bin", binary, "--check"], name)
    reports: list[dict] = []
    for line in output.splitlines():
        try:
            candidate = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(candidate, dict) and candidate.get("gate") == name:
            reports.append(candidate)
    if len(reports) != 1:
        raise AssertionError(f"{name} 缺少唯一真獨立驗收 JSON")
    report = reports[0]
    expected = "Passed" if expected_main else "PassedCandidate"
    if report.get("result") != expected or report.get("main_verified") is not expected_main:
        raise AssertionError(f"{name} 主線身分或 PassedCandidate 被冒充：{report}")
    actual_sha = report.get("github_sha", report.get("git_head"))
    if actual_sha != current_head or report.get("github_ref") != current_ref:
        raise AssertionError(f"{name} 使用的 SHA/REF 不是 G6 當前真 checkout")
    expected_fields = EXPECTED_GATES[name]
    for key, value in expected_fields.items():
        if report.get(key) != value:
            raise AssertionError(f"{name} 驗收條件 {key} 漂移：{report.get(key)!r}")
    if name == "P16.6-R02-A6":
        if report.get("outstanding") != ([] if expected_main
                else ["同一最新 main SHA push CI"]):
            raise AssertionError("A6 總驗收仍有不明已知缺口")
    elif report.get("outstanding") != []:
        raise AssertionError(f"{name} 正式工作包仍有未驗收事項")
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("G6 必須 --check 真執行；文件和歷史記錄不等於驗收")
    head, ref, verified_main = require_git_identity()
    binary = str((ROOT / args.test_bin).resolve())
    if not Path(binary).is_file():
        raise AssertionError("G6 缺少本次 HEAD 真正重新編譯的 Rust 測試執行檔")

    for path in REQUIRED_DOCUMENTS:
        target = ROOT / path
        if not target.is_file() or not target.read_text(encoding="utf-8").strip():
            raise AssertionError(f"先前獨立根因簽核文件不存在：{path}")

    # G0 另從本次來源確認凍結四組；後續 A6 會真正重跑 F01～F05
    # 及 G0，G5 會重新採集同本次 Rust 檔的 15/57 真實 P16 事件。
    g0 = run([sys.executable, str(ROOT / "tools/核對F00總驗收.py")], "G0")
    frozen = (
        "P01 49 份／30 筆診斷",
        "Phase 24 項／157 個具名選取紀錄",
        "私有 113 筆來源選取／63 筆拒絕",
        "八橋接 15 路徑／57 筆實際事件",
    )
    if not all(fragment in g0 for fragment in frozen):
        raise AssertionError("G6 禁止變動 G0 四組已核准 UTF-8/事件凍結")

    reports = {}
    for name, path in (
        ("P16.6-R02-A6", "tools/核對A6主線總驗收.py"),
        ("P16.6-R03-G3", "tools/核對G3正式驗收.py"),
        ("P16.6-R03-G4", "tools/核對G4正式驗收.py"),
        ("P16.6-R03-G5", "tools/核對G5正式驗收.py"),
    ):
        reports[name] = run_json_gate(
            name, path, binary, verified_main, head, ref)
        print(f"G6/{name}：以本次 SHA 重跑真 Rust 並核對獨立來源成功")

    all_work_packages = [f"F{i:02d}" for i in range(15)]
    audited = ["F00", *EXPECTED_GATES["P16.6-R02-A6"]["executed"][:5],
        *EXPECTED_GATES["P16.6-R03-G3"]["executed"],
        *EXPECTED_GATES["P16.6-R03-G4"]["executed"][:2],
        *EXPECTED_GATES["P16.6-R03-G5"]["executed"][:2], "F14"]
    if audited != all_work_packages:
        raise AssertionError(f"G6 15 個 F 工作包存在缺口或重複：{audited}")

    # 原已核准 24 個 Phase 不改變；沒有獨立 P17 執行器。
    output = {
        "gate": "P16.6-R03-G6",
        "result": "Passed" if verified_main else "PassedCandidate",
        "github_sha": head,
        "github_ref": ref,
        "main_verified": verified_main,
        "executed": ["G0", "G1", "G2", "G3", "G4", "G5", "G6"],
        "r02_status": "Passed",
        "r03_status": "Passed" if verified_main else "PassedCandidate",
        "f_work_packages": audited,
        "f_complete": 15 if verified_main else 14,
        "f_total": 15,
        "p16_6_r_complete": 6 if verified_main else 5,
        "p16_6_r_total": 6,
        "frozen_bridge_paths": 15,
        "frozen_resource_events": 57,
        "g3_unique_rust_tests": 50,
        "g4_unique_rust_tests": 55,
        "g5_unique_rust_tests": 50,
        "outstanding": [] if verified_main else
            ["最終 Squash merge 後新 main 同 SHA 真 CI 與正式 G6 重驗"],
        "p17_in_scope": False,
    }
    print(json.dumps(output, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError, ValueError, KeyError) as exc:
        print(f"G6 同 SHA 全程總驗收拒絕：{exc}", file=sys.stderr)
        sys.exit(1)
