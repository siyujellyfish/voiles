#!/usr/bin/env python3
"""R03 B05／F06 第一批：靜態別名來源唯一性與真正私有 Checker 正反回歸。"""
from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "f06_b05_不同別名共享同來源保留重數並對帳真票據",
    "f06_b05_重複靜態別名不允許冒充兩張合法共享票據",
    "f06_b05_失效別名不被算作活躍借用或虛構票據",
    "f06_b05_遮蔽別名與參數自身借用使用真宣告身分",
    "f06_b05_真正私有來源的同名遮蔽不應延長外層借用",
    "f06_b05_真正私有來源仍引用外層借用不得提前解除封鎖",
)


def run(command: list[str]) -> str:
    result = subprocess.run(
        command, encoding="utf-8", errors="replace", text=True,
        capture_output=True, check=False,
    )
    if result.returncode:
        raise AssertionError(
            f"實際 Rust 測試失敗（{result.returncode}）：{' '.join(command)}\n"
            + result.stdout[-2000:] + result.stderr[-2000:]
        )
    return result.stdout + result.stderr


def check_authority() -> None:
    snapshot = (ROOT / "src/borrow_safety.rs").read_text(encoding="utf-8")
    checker = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    for marker in (
        "pub(crate) fn from_aliases(",
        "if !seen.insert(alias) { duplicate_alias = true; }",
        "if self.duplicate_alias { return Err(BorrowSafetyError::SnapshotMismatch); }",
        "tracker.verify_static_snapshot(self.facts.iter().copied())",
    ):
        if snapshot.count(marker) != 1:
            raise AssertionError(f"靜態／真票據來源權威未唯一：{marker}")
    if checker.count("StaticBorrowSnapshot::from_aliases(vars.iter()") != 1:
        raise AssertionError("正式 Checker 沒有由真正詞彙 Binding 直接建立唯一別名快照")
    if "StaticBorrowSnapshot::from_active(vars.iter()" in checker:
        raise AssertionError("Checker 仍使用不帶別名來源身分的舊可寫權威")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("必須指定 --check 才可檢驗正式真 Rust 來源")
    check_authority()
    output = run([args.test_bin, "--list"])
    listed = [line[:-6] for line in output.splitlines() if line.endswith(": test")]
    actual = []
    for suffix in CASES:
        matching = [name for name in listed if name.endswith("::" + suffix)]
        if len(matching) != 1:
            raise AssertionError(f"真 Rust 測試缺失或重複：{suffix}（{len(matching)}）")
        name = matching[0]
        if name in actual:
            raise AssertionError(f"同一案例重複採計：{name}")
        actual.append(name)
        test = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in test:
            raise AssertionError(f"真 Rust --exact 沒有成功：{name}")
    print(f"R03 B05/F06 第一批：{len(actual)} 個互異真 Rust --exact 通過，來源別名唯一與票據隔離成立")
    print("F06/F08/G3 整包仍待獨立驗收；不得以這一批宣告 R03 結案")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as exc:
        print(f"R03 B05/F06 第一批失敗：{exc}", file=sys.stderr)
        sys.exit(1)
