#!/usr/bin/env python3
"""G5/F13：同 HEAD 同一測試程序的 Checker 與完整受限真事件橋接成本。

本工具觀測而非猜測效能基準：同一測試、UTF-8 來源、重複次數，
交替執行兩條真路徑。不存在既有核准門檻時不得宣稱加速／達標。
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "g5_f13_同環境具名字串源驗證對真資源事件橋接量測",
    "g5_f13_同環境多字串結構Checker對原生逆序清理橋接量測",
)
PATTERN = re.compile(
    r"G5_PERFORMANCE name=(\w+) iterations=(\d+) checker_ns=(\d+) full_bridge_ns=(\d+)"
)

def run(cmd: list[str]) -> str:
    p = subprocess.run(cmd, cwd=ROOT, text=True, encoding="utf-8",
        errors="replace", capture_output=True, check=False)
    if p.returncode:
        raise AssertionError(f"真 Rust 同環境量測失敗（{p.returncode}）: {' '.join(cmd)}\n"
            + p.stdout[-2500:] + p.stderr[-2500:])
    return p.stdout + p.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("須指定 --check")
    checker = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    core = (ROOT / "src/struct_stage.rs").read_text(encoding="utf-8")
    pipeline = (ROOT / "src/struct_stage/resource_trace.rs").read_text(encoding="utf-8")
    for source, marker in (
        (core, "struct SafetyState {"),
        (core, "fn transition(&mut self, action: StaticTransition<"),
        (core, "fn exit_snapshots_are_consistent("),
        (checker, "fn record_checked_control_snapshot("),
        (pipeline, "fn require_checked_bridge_owner("),
        (pipeline, "fn require_checked_bridge_return("),
        (pipeline, "fn closed_bridge_events("),
    ):
        if source.count(marker) != 1:
            raise AssertionError(f"G5/F13 共通安全資料與資源事件權威不存在或重複：{marker}")
    listed = run([args.test_bin, "--list"])
    names = [line[:-6] for line in listed.splitlines() if line.endswith(": test")]
    selected: set[str] = set()
    stats: dict[str, dict[str, int | float]] = {}
    for suffix in CASES:
        hits = [name for name in names if name.endswith("::" + suffix)]
        if len(hits) != 1 or hits[0] in selected:
            raise AssertionError(f"效能真 Rust 測試缺失／重複：{suffix}")
        name = hits[0]
        selected.add(name)
        output = run([args.test_bin, "--exact", name, "--nocapture", "--test-threads=1"])
        if "1 passed; 0 failed" not in output:
            raise AssertionError(f"效能真執行未成功：{suffix}")
        records = PATTERN.findall(output)
        if len(records) != 1:
            raise AssertionError(f"效能測量未產生唯一同環境數值：{suffix}")
        workload, iterations, checker_ns, full_ns = records[0]
        a, b, n = int(checker_ns), int(full_ns), int(iterations)
        if n != 48 or a <= 0 or b <= 0 or workload in stats:
            raise AssertionError("同環境效能樣本重複、缺失或無效")
        stats[workload] = {
            "iterations": n,
            "checker_ns": a,
            "full_bridge_ns": b,
            "observed_ratio": round(b / a, 3),
        }
    if set(stats) != {"named_str", "struct_three_str"}:
        raise AssertionError("效能樣本缺少具名或多字串結構")
    print(json.dumps({
        "gate": "P16.6-R03-G5-PERF",
        "result": "MeasuredOnly",
        "workloads": stats,
        "same_process": True,
        "historical_baseline_claimed": False,
        "speedup_or_threshold_claimed": False,
    }, ensure_ascii=False, sort_keys=True))
    print("本項只是同環境 Checker 與含真資源橋接成本量測，無核准門檻故不宣稱效能提升；G5 仍待整包")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as error:
        print(f"G5/F13 真效能及來源拒絕：{error}", file=sys.stderr)
        sys.exit(1)
