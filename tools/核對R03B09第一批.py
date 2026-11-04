#!/usr/bin/env python3
"""R03 B09/F12 第一批：八條 P16 真實受限橋接逐 ResourceId 事件閉合。"""
from __future__ import annotations
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    "f12_b09_真實資源ID允許多次轉交但只可釋放一次",
    "f12_b09_建立之前轉移或釋放後返回一律拒絕",
    "f12_b09_重複建立重複釋放及漏清理不得通過",
    "f12_b09_多資源交錯轉移與逆序清理各自保持獨立資源ID",
    "f12_b09_真正私有字串資源移交橋接會輸出已驗證事件",
)

def run(command: list[str]) -> str:
    result = subprocess.run(command, capture_output=True, text=True,
        encoding="utf-8", errors="replace", check=False)
    if result.returncode:
        raise AssertionError(f"真 Rust --exact 失敗（{result.returncode}）: {' '.join(command)}\n"
            + result.stdout[-2000:] + result.stderr[-2000:])
    return result.stdout + result.stderr

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("必須指定 --check")
    owned = (ROOT / "src/owned_str_runtime.rs").read_text(encoding="utf-8")
    bridge = (ROOT / "src/struct_stage/resource_trace.rs").read_text(encoding="utf-8")
    if owned.count("pub(crate) fn event_trace_balanced(") != 1:
        raise AssertionError("真 ResourceId 事件同一權威不存在")
    if owned.count("event_trace_balanced(&self.events)") != 1:
        raise AssertionError("原 OwnedStrRuntime 沒有保留相同逐事件驗證")
    if bridge.count("fn closed_bridge_events(") != 1 or bridge.count("fn closed_bridge_result<") != 1:
        raise AssertionError("八橋接共同事件核對入口缺失")
    # 八條受限 trace，成功與失敗路徑合計 12 個真返回位置。
    if bridge.count("closed_bridge_result(") != 8 or bridge.count("closed_bridge_events(") != 6:
        raise AssertionError("既有受限成功／失敗橋接沒有全量消耗唯一事件閉合入口")
    listed = run([args.test_bin, "--list"])
    available = [line[:-6] for line in listed.splitlines() if line.endswith(": test")]
    chosen = set()
    for suffix in CASES:
        found = [name for name in available if name.endswith("::" + suffix)]
        if len(found) != 1 or found[0] in chosen:
            raise AssertionError(f"Rust 真測試非空與唯一性不成立：{suffix}")
        name = found[0]
        chosen.add(name)
        outcome = run([args.test_bin, "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in outcome:
            raise AssertionError(f"--exact 未實際成功：{name}")
    print(f"R03 B09/F12 第一批：{len(chosen)} 筆真 Rust --exact，八受限來源實際事件全對帳")
    print("本批不是完整 F12／G5／G6 正式結案；原 G0 15 路徑／57 真事件另行凍結驗收")
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError) as error:
        print(f"F12 第一批失敗：{error}", file=sys.stderr)
        sys.exit(1)
