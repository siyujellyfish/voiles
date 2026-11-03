"""F00：真實 P16 八條橋接與部分建構／覆寫失敗的資源事件快照。

僅從已通過私有 Checker 的真實來源與真實事件採集；
先 --print 從 CI 核對，再凍結 JSON，日後 --check 不得自動更新。
所有文字檔明確採 UTF-8。
"""
from __future__ import annotations

import argparse
import difflib
import hashlib
import json
import os
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FROZEN = ROOT / "tests" / "fixtures" / "F00-八橋接資源事件快照.json"
BASE_HEAD = "ce173af5810d008a40bc54f7624989acc6c0bbe2"
TEST_FILTER = "f00_八橋接真正資源事件基準::從真正已驗證來源採集八條橋接及其成功失敗事件"
EXPECTED = [
    "01_建構_成功", "01_建構_中途失敗",
    "02_結構移動_成功", "02_結構移動_建構失敗",
    "03_字串返回_成功",
    "04_欄位覆寫_成功", "04_欄位覆寫_右值失敗",
    "05_混合結構_成功", "05_混合結構_求值失敗", "05_純值結構_無資源",
    "06_作用域_成立分支", "06_作用域_未成立分支",
    "07_結構返回_成功", "07_結構返回_建構失敗",
    "08_具名字串移動_成功",
]
EVENT = re.compile(r"(Created|Moved|Returned|Released)\(([1-9][0-9]*)\)")


def 擷取真實事件(test_bin: Path) -> dict[str, object]:
    env = dict(os.environ)
    env["VOILES_F00_BRIDGE_EXPORT"] = "1"
    result = subprocess.run(
        [str(test_bin), "--nocapture", "--test-threads=1", TEST_FILTER],
        cwd=ROOT, env=env, text=True, encoding="utf-8",
        capture_output=True, check=False,
    )
    if result.returncode != 0 or "test result: ok. 1 passed; 0 failed; 0 ignored" not in result.stdout:
        raise RuntimeError("F00 八橋接真正來源測試未完成單一成功：\n"
                           + result.stdout[-2000:] + "\n" + result.stderr[-2000:])
    prefix = "F00_BRIDGE_RECORD="
    rows = []
    for line in result.stdout.splitlines():
        if prefix not in line:
            continue
        fields = line[line.index(prefix) + len(prefix):].rstrip("\r\n").split("\t")
        if len(fields) != 3:
            raise ValueError(f"F00 事件紀錄欄位數量錯誤：{fields!r}")
        key, source_hex, event_text = fields
        source_raw = bytes.fromhex(source_hex)
        source = source_raw.decode("utf-8", errors="strict")
        if not source.endswith("\n") or not source:
            raise ValueError(f"F00 八橋接來源不合法：{key}")
        events = []
        for raw in event_text.split(",") if event_text else []:
            match = EVENT.fullmatch(raw)
            if match is None:
                raise ValueError(f"{key}：不合法的實際資源事件 {raw!r}")
            events.append({"kind": match.group(1), "resource_id": int(match.group(2))})
        created = [event["resource_id"] for event in events if event["kind"] == "Created"]
        released = [event["resource_id"] for event in events if event["kind"] == "Released"]
        if len(set(created)) != len(created) or sorted(created) != sorted(released):
            raise ValueError(f"{key}：事件不能有重複配置、洩漏或雙重釋放")
        if any(event["resource_id"] not in created for event in events):
            raise ValueError(f"{key}：事件不得引用不存在的 ResourceId")
        rows.append({
            "id": key,
            "source": source,
            "source_sha256": hashlib.sha256(source_raw).hexdigest(),
            "events": events,
        })
    if [row["id"] for row in rows] != EXPECTED:
        raise ValueError("F00 事件採集缺失、重複或順序不一致；不得以空集合通過")
    return {
        "format_version": 1,
        "source_baseline_head": BASE_HEAD,
        "category": "已核准 P16 真正來源之八條受限資源橋接與失敗切點",
        "test_filter": TEST_FILTER,
        "cases": rows,
    }


def 主程式() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--print", action="store_true")
    mode.add_argument("--check", action="store_true")
    args = parser.parse_args()
    test_bin = args.test_bin.resolve()
    if not test_bin.is_file():
        parser.error(f"找不到真正 Rust 核心測試執行檔：{test_bin}")
    current = 擷取真實事件(test_bin)
    if args.print:
        print("F00_BRIDGE_JSON=" + json.dumps(
            current, ensure_ascii=False, sort_keys=True, separators=(",", ":")
        ))
        return
    frozen = json.loads(FROZEN.read_text(encoding="utf-8"))
    if current != frozen:
        before = json.dumps(frozen, ensure_ascii=False, sort_keys=True, indent=2).splitlines()
        after = json.dumps(current, ensure_ascii=False, sort_keys=True, indent=2).splitlines()
        print("\n".join(difflib.unified_diff(
            before, after, fromfile="已凍結八橋接事件",
            tofile="真正 Checker 與執行期事件", lineterm="",
        )))
        raise SystemExit("F00：八橋接的真正 ResourceId 或清理順序與原基準不一致")
    print(f"F00 八橋接實際事件快照通過：{len(current['cases'])} 條成功／失敗來源。")


if __name__ == "__main__":
    主程式()
