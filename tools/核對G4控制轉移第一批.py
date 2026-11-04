#!/usr/bin/env python3
"""R03 G4 第一批：真 Checker 已核對 return/break/continue 來源，非整包 G4。"""
from __future__ import annotations
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "g4_兩函式真正Checker各簽已核准的return來源",
    "g4_兩可達分支return都有核對但其後不可達return不得假記",
    "g4_合法迴圈break與continue明確區分且零次路徑沒有假出口",
    "g4_偽造錯誤控制分類或UTF8跨度不得冒充Checker退出",
    "g4_刪除唯一已驗證函式return證據必須拒絕整份計畫",
)

def run(cmd: list[str]) -> str:
    p = subprocess.run(cmd, cwd=ROOT, text=True, encoding="utf-8",
        capture_output=True, errors="replace", check=False)
    if p.returncode:
        raise AssertionError(f"真正 Rust 執行失敗 {p.returncode}：{' '.join(cmd)}\n"
            + p.stdout[-1600:] + p.stderr[-1600:])
    return p.stdout + p.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("須指定 --check")
    core = (ROOT / "src/struct_stage.rs").read_text(encoding="utf-8")
    checker = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    for marker in (
        "struct VerifiedControlEdge {",
        "recorded_control_edges: RefCell<BTreeMap<usize, VerifiedControlEdge>>",
        "fn control_edge_matches_ast(",
        "fn control_edges_are_consistent(",
        "plan.control_edges = checker.recorded_control_edges.into_inner()",
        "&& self.control_edges_are_consistent(functions, text)",
    ):
        if core.count(marker) != 1:
            raise AssertionError(f"真 Checker 控制證據權威不可缺失或複製：{marker}")
    if checker.count("fn record_checked_control_edge(") != 1:
        raise AssertionError("缺少唯一真 Checker 控制退出簽出入口")
    if checker.count("self.record_checked_control_edge(") != 2:
        raise AssertionError("Return 與 Break/Continue 兩條真 Checker 路徑未接線")
    listed = run([args.test_bin, "--list"])
    available = [line[:-6] for line in listed.splitlines() if line.endswith(": test")]
    seen: set[str] = set()
    for suffix in CASES:
        hits = [name for name in available if name.endswith("::" + suffix)]
        if len(hits) != 1 or hits[0] in seen:
            raise AssertionError(f"真 Rust 正反非空／唯一性失敗：{suffix}")
        name = hits[0]
        seen.add(name)
        result = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in result:
            raise AssertionError(f"--exact 真來源未成功：{name}")
    print(f"R03 G4 第一批：{len(seen)} 筆互異真 Rust --exact 控制來源與負例成功")
    print("僅證明普通函式 Checker 控制轉移的真來源，不表示 G4 已全部 Passed")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as ex:
        print(f"R03 G4 第一批失敗：{ex}", file=sys.stderr)
        sys.exit(1)
