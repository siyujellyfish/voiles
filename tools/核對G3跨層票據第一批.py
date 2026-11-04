#!/usr/bin/env python3
"""G3 第一批候選：真 BorrowTicket／靜態 BorrowFact／雙帳簿原子交易。
此工具只驗證跨層子關卡，不能簽發整個 G3 Passed。
"""
from __future__ import annotations
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "g3_雙共享借用逐張對帳後完整原子歸還再整筆移交及清理",
    "g3_靜態完整快照卻只帶部分票據時不可解鎖任何所有者",
    "g3_跨追蹤器真票據不可冒充相同宣告來源的正確票據",
    "g3_抽象資源批次清理遇活躍借用時拒絕前後兩帳簿皆不變",
    "g3_重複目標與資源能力衝突不會留下半完成複製或移交",
    "g3_偽造靜態借用事實不會改變實際票據或資源責任",
)

def run(command: list[str]) -> str:
    result = subprocess.run(command, cwd=ROOT, encoding="utf-8", text=True,
        capture_output=True, errors="replace", check=False)
    if result.returncode:
        raise AssertionError(
            f"真實 Rust 測試失敗 {result.returncode}: {' '.join(command)}\n"
            + result.stdout[-2000:] + result.stderr[-2000:]
        )
    return result.stdout + result.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("必須 --check 才可核對真正測試")
    source = (ROOT / "src/resource_borrow_pipeline.rs").read_text(encoding="utf-8")
    borrow = (ROOT / "src/borrow_safety.rs").read_text(encoding="utf-8")
    checker = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    required = (
        ("雙側宣告預檢", "self.resources.check_create_binding(id)"),
        ("雙側移動預檢", "self.resources.check_move_into(from, to)"),
        ("雙側複製預檢", "self.resources.check_copy_into(from, to)"),
        ("批次資源清理預檢", "self.resources.check_release_batch(owners)"),
        ("靜態來源唯讀驗證", "self.ownership.verify_static_borrows(facts.iter().copied())"),
        ("全票據歸還契約", "self.ownership.finish_method_scope_verified(tickets, facts)"),
    )
    for label, marker in required:
        if source.count(marker) != 1:
            raise AssertionError(f"G3 真跨層原子權威缺失或重複：{label}")
    if borrow.count("if tickets.len() != checked.len()") != 1:
        raise AssertionError("G3 缺少完整真票據集合與靜態重數守衛")
    if checker.count("StaticBorrowSnapshot::from_aliases(vars.iter()") != 1:
        raise AssertionError("真正 Checker 沒有直接由詞彙 alias 身分建立借用來源")
    listing = run([args.test_bin, "--list"])
    tests = [line[:-6] for line in listing.splitlines() if line.endswith(": test")]
    seen: set[str] = set()
    for suffix in CASES:
        matched = [name for name in tests if name.endswith("::" + suffix)]
        if len(matched) != 1 or matched[0] in seen:
            raise AssertionError(f"G3 真 Rust 正反案例缺失或不唯一：{suffix}")
        name = matched[0]
        seen.add(name)
        output = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in output:
            raise AssertionError(f"G3 正反來源 --exact 未成功：{name}")
    print(f"G3 第一批跨層真證據：{len(seen)} 個互異 Rust --exact 全數成功")
    print("G3 整包尚欠全已核准 CFG 用例／所有操作及逃逸權威盤點；本工具不能簽發 Passed")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as err:
        print(f"G3 第一批真證據失敗：{err}", file=sys.stderr)
        sys.exit(1)
