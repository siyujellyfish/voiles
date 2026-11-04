#!/usr/bin/env python3
"""R03 G4 第四批：真 Checker 逐出口靜態清理義務，非執行期事件。"""
from __future__ import annotations
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "g4_未移交的真實str持有者必須出現在靜態必需清理責任中",
    "g4_已明確move返回的來源不能重複列為靜態清理責任",
    "g4_可能有資源的未知合流狀態必須維持條件清理而非假必需",
    "g4_資源清理義務被刪除或靜態借用歸還清單遭偽造皆拒絕",
    "g4_非擁有借用別名只留下靜態歸還義務不被誤當資源owner",
)

def run(cmd: list[str]) -> str:
    p = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True,
        encoding="utf-8", errors="replace", check=False)
    if p.returncode:
        raise AssertionError(f"Rust 真實執行失敗（{p.returncode}）: {' '.join(cmd)}\n"
            + p.stdout[-1800:] + p.stderr[-1800:])
    return p.stdout + p.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("必須指定 --check")
    core = (ROOT / "src/struct_stage.rs").read_text(encoding="utf-8")
    checker = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    slot = (ROOT / "src/ownership_model.rs").read_text(encoding="utf-8")
    for marker in (
        "enum StaticCleanupNeed {",
        "struct StaticCleanupObligation {",
        "fn cleanup_from_bindings(",
        "fn active_borrows_from_bindings(",
        "fn from_checked_scopes(",
        "cleanup_owners: Vec<StaticCleanupObligation>",
        "active_borrows: Vec<(DeclarationId, BorrowFact)>",
        "&& obligations",
    ):
        if core.count(marker) != 1:
            raise AssertionError(f"G4 靜態清理義務核對器缺失或不唯一：{marker}")
    if checker.count("VerifiedExitSnapshot::from_checked_scopes(") != 1:
        raise AssertionError("真 Checker 沒有使用唯一靜態 Scopes 義務正規化")
    if slot.count("pub(crate) fn copy_permission(self) -> CopyPermission") != 1:
        raise AssertionError("不能繞過既有 ValueSlot 能力來源")
    listed = run([args.test_bin, "--list"])
    names = [line[:-6] for line in listed.splitlines() if line.endswith(": test")]
    used: set[str] = set()
    for suffix in CASES:
        hits = [name for name in names if name.endswith("::" + suffix)]
        if len(hits) != 1 or hits[0] in used:
            raise AssertionError(f"G4 清理正反 Rust 案例缺少或重複：{suffix}")
        name = hits[0]
        used.add(name)
        result = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in result:
            raise AssertionError(f"Rust --exact 真測試未成功：{name}")
    print(f"G4 靜態清理責任第四批：{len(used)} 筆互異真 Rust --exact 成功")
    print("不因靜態義務而產生任何真 ResourceId／BorrowTicket／Released")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as error:
        print(f"G4 靜態清理義務拒絕：{error}", file=sys.stderr)
        sys.exit(1)
