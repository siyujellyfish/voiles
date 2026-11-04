#!/usr/bin/env python3
"""G5 第二批：八個 P16 受限真橋接的 DeclarationId 對來源靜態 Owner 關係。"""
from __future__ import annotations
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "g5_真資源執行者DeclarationId須命中同一Checker退出清理Owner",
    "g5_假持有者即使同型別或同名字串也不得冒充真DeclarationId",
    "g5_已移走的來源不得假稱仍保有原ResourceId清理責任",
    "g5_純Copyable持有者無須任何真字串清理事件不能偽造Required",
    "g5_真持有者若遺失Checker清理義務絕不可只靠runtime事件平衡放行",
)

def run(cmd: list[str]) -> str:
    proc = subprocess.run(cmd, cwd=ROOT, encoding="utf-8", text=True,
        errors="replace", capture_output=True, check=False)
    if proc.returncode:
        raise AssertionError(f"真 Rust 執行失敗 {proc.returncode}：{' '.join(cmd)}\n"
            + proc.stdout[-1900:] + proc.stderr[-1900:])
    return proc.stdout + proc.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("須指定 --check")
    bridge = (ROOT / "src/struct_stage/resource_trace.rs").read_text(encoding="utf-8")
    if bridge.count("fn require_checked_bridge_owner(") != 1:
        raise AssertionError("真 Checker 持有者靜態責任來源不是唯一")
    # 八橋接各一次＋早退真內外持有者兩次＋函式定義一次。
    if bridge.count("require_checked_bridge_owner(") != 11:
        raise AssertionError("八橋接及實際已執行的早退內外 Owner 沒有完整來源核對")
    if bridge.count("require_checked_bridge_return(&control_edges, &exit_snapshots,") != 10:
        raise AssertionError("第一批 G5 真 return 前置證據不可回退")
    listed = run([args.test_bin, "--list"])
    available = [line[:-6] for line in listed.splitlines() if line.endswith(": test")]
    used: set[str] = set()
    for suffix in CASES:
        matches = [name for name in available if name.endswith("::" + suffix)]
        if len(matches) != 1 or matches[0] in used:
            raise AssertionError(f"真 Rust 名稱不存在／重複：{suffix}")
        name = matches[0]
        used.add(name)
        output = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in output:
            raise AssertionError(f"真 Rust --exact 失敗：{name}")
    print(f"G5 第二批：{len(used)} 筆互異真 Rust --exact 所有者來源身分及清理責任成功")
    print("仍非 G5/F12/F13 整包正式驗收；真 ResourceId 事件對帳與效能根因待完成")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as error:
        print(f"G5 第二批拒絕：{error}", file=sys.stderr)
        sys.exit(1)
