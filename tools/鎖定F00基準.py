"""F00：擷取／比對 49 份正式 P01 來源的原始位元組及完整診斷快照。

快照必須先由凍結前的真正 check_file 探針取得、核對來源，
再獨立提交。--check 只會比較，不會悄悄重寫期望值。
所有文字檔採 UTF-8；來源原始位元組另以 SHA-256 鎖定。
"""
from __future__ import annotations

import argparse
import difflib
import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "tests" / "fixtures"
MANIFEST = FIXTURES / "清單.json"
FROZEN = FIXTURES / "F00-正式診斷快照.json"
BASE_HEAD = "57d0fc7b011165f14eaa5c2c1537a2b6cc5fe302"


def 執行來源(probe: Path, relative_path: str) -> tuple[str, list[dict[str, object]]]:
   finished = subprocess.run(
       [str(probe), relative_path],
       cwd=ROOT, capture_output=True, text=True, encoding="utf-8", check=False,
   )
   if finished.returncode != 0 or finished.stderr:
       raise RuntimeError(f"{relative_path}：來源探針異常：{finished.stderr!r}")
   lines = finished.stdout.splitlines()
   if not lines:
       raise ValueError(f"{relative_path}：探針無任何可驗證輸出")
   header = lines[0].split("\t")
   if len(header) != 3 or header[1] != "None":
       raise ValueError(f"{relative_path}：探針狀態或 IO 失敗：{header!r}")
   status, _, count_text = header
   if status not in {"Passed", "Failed"}:
       raise ValueError(f"{relative_path}：未知檢查狀態 {status}")
   count = int(count_text)
   if len(lines) - 1 != count:
       raise ValueError(f"{relative_path}：診斷數量與探針宣告不符")
   details = []
   for line in lines[1:]:
       cols = line.split("\t")
       if len(cols) != 6 or cols[0] not in {"Lexical", "Syntax", "Static"}:
           raise ValueError(f"{relative_path}：診斷欄位格式不正確：{cols!r}")
       positions = cols[1:5]
       if all(value == "-" for value in positions):
           span = None
       elif all(value.isdigit() and int(value) >= 1 for value in positions):
           span = {"start": [int(positions[0]), int(positions[1])],
                   "end": [int(positions[2]), int(positions[3])]}
       else:
           raise ValueError(f"{relative_path}：診斷位置不能部分缺失")
       details.append({"kind": cols[0],
                       "message": bytes.fromhex(cols[5]).decode("utf-8", errors="strict"),
                       "span": span})
   return status, details


def 取得快照(probe: Path) -> dict[str, object]:
   cases = json.loads(MANIFEST.read_text(encoding="utf-8"))["cases"]
   expected = {f"P01-正{i:02d}" for i in range(1, 20)} | {
       f"P01-反{i:02d}" for i in range(1, 31)}
   }
   actual = {item["id"] for item in cases}
   if len(cases) != 49 or actual != expected:
       raise ValueError("F00 契約只接受現存 P01 19 正＋30 反；不可憑空補第 50 份來源")
   collected: list[dict[str, object]] = []
   for item in sorted(cases, key=lambda entry: entry["id"]):
       name = item["id"]
       relative = "tests/fixtures/" + item["path"]
       resolved = (ROOT / relative).resolve()
       if FIXTURES.resolve() not in resolved.parents or not resolved.is_file():
           raise ValueError(f"{name}：來源不在正式測試素材目錄內")
       raw = resolved.read_bytes()
       raw.decode("utf-8", errors="strict")
       status, details = 執行來源(probe, relative)
       expected_status = {"accept": "Passed", "reject": "Failed"}[item["expect"]]
       if status != expected_status or (status == "Failed" and not details):
           raise AssertionError(f"{name}：接受／拒絕結果與 P01 契約不符")
       collected.append({
           "id": name,
           "path": relative,
           "source_sha256": hashlib.sha256(raw).hexdigest(),
           "status": status,
           "diagnostics": details,
       })
   return {
       "format_version": 1,
       "source_baseline_head": BASE_HEAD,
       "case_definition": "P01 19 正＋30 反＝49 原始來源；另有 1 項素材路徑測試",
       "cases": collected,
   }


def 主程式() -> None:
   parser = argparse.ArgumentParser(description=__doc__)
   parser.add_argument("--probe", required=True, type=Path)
   mode = parser.add_mutually_exclusive_group(required=True)
   mode.add_argument("--print", action="store_true", help="僅輸出供初次人工核對的候選快照")
   mode.add_argument("--check", action="store_true", help="嚴格核對已凍結快照")
   args = parser.parse_args()
   probe = args.probe.resolve()
   if not probe.is_file():
       parser.error(f"來源探針不存在：{probe}")
   current = 取得快照(probe)
   if args.print:
       print("F00_SNAPSHOT_JSON=" + json.dumps(
           current, ensure_ascii=False, sort_keys=True, separators=(",", ":")
       ))
       return
   frozen = json.loads(FROZEN.read_text(encoding="utf-8"))
   if frozen != current:
       earlier = json.dumps(frozen, ensure_ascii=False, indent=2, sort_keys=True).splitlines()
       latest = json.dumps(current, ensure_ascii=False, indent=2, sort_keys=True).splitlines()
       print("\n".join(difflib.unified_diff(
           earlier, latest, fromfile="F00 已凍結基準",
           tofile="目前正式 check_file", lineterm="",
       )))
       raise SystemExit("F00：來源位元組、狀態或完整診斷發生非核准回歸")
   print(f"F00 精確來源／診斷快照通過：{len(current['cases'])} 份原始來源。")


if __name__ == "__main__":
   主程式()
