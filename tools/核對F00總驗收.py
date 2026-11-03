"""G0／F00：檢查四份凍結證據之間的完整性、一致性與覆蓋界線。

資料均由其他 CI 工作實際執行後封存；本腳本不能以 JSON 自我核對
替代真正的 Rust 來源／資源事件重新執行。檔案文字採 UTF-8。
"""
from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "tests" / "fixtures"
BASE = FIXTURES / "清單.json"
PUBLIC = FIXTURES / "F00-正式診斷快照.json"
PHASES = FIXTURES / "F00-原始24項測試證據快照.json"
PRIVATE = FIXTURES / "F00-私有來源診斷快照.json"
BRIDGES = FIXTURES / "F00-八橋接資源事件快照.json"
MATRIX = ROOT / "AI-build" / "P13-P16-原始24項正式驗收矩陣.md"
EXPECTED_NO_PRIVATE = {
    "P14.1", "P16.1", "P16.2", "P16.4", "P16.5", "P16.6",
}
EXPECTED_P01 = {
    *(f"P01-正{i:02d}" for i in range(1, 20)),
    *(f"P01-反{i:02d}" for i in range(1, 31)),
}
EXPECTED_PHASES = {
    f"P{phase}.{item}" for phase in range(13, 17) for item in range(1, 7)
}
EXPECTED_BRIDGES = {f"{num:02d}" for num in range(1, 9)}


def read(path: Path) -> dict:
    data = json.loads(path.read_text(encoding="utf-8"))
    if data.get("format_version") != 1:
        raise ValueError(f"{path}：未知快照版本")
    return data


def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def require(ok: bool, reason: str) -> None:
    if not ok:
        raise ValueError("F00／G0：" + reason)


def check_span(value: object) -> None:
    if value is None:
        return
    require(isinstance(value, dict)
            and set(value) == {"start", "end"},
            "診斷位置欄位異常")
    for point in ("start", "end"):
        pair = value[point]
        require(isinstance(pair, list) and len(pair) == 2
                and all(isinstance(n, int) and n >= 1 for n in pair),
                "來源位置須使用一基行列的精確起訖")


def check_diagnostic(record: dict, optional: bool) -> None:
    status = record["status"]
    require(status in {"Passed", "Failed"}, "來源狀態無效")
    if optional:
        diagnostic = record["diagnostic"]
        require((diagnostic is None) == (status == "Passed"),
                "私有成功／失敗及診斷矛盾")
        entries = [] if diagnostic is None else [diagnostic]
    else:
        entries = record["diagnostics"]
        require(isinstance(entries, list), "公開診斷格式必須是列表")
        require((not entries) == (status == "Passed"),
                "公開成功／失敗及診斷矛盾")
    for diag in entries:
        require(diag["kind"] in {"Lexical", "Syntax", "Static"}
                and isinstance(diag["message"], str) and diag["message"],
                "診斷文字或類別缺失")
        check_span(diag["span"])


def inspect_public() -> tuple[int, int]:
    manifest = read(BASE)
    frozen = read(PUBLIC)
    declared = manifest["cases"]
    actual = frozen["cases"]
    expected = {item["id"]: item for item in declared}
    require(len(declared) == len(expected) == len(actual) == 49
            and set(expected) == EXPECTED_P01,
            "P01 必須是 19 正＋30 反共 49 份真實來源")
    require(len(manifest["source_vectors"]) == 11, "P01 11 個 UTF-8 原始向量缺失")
    seen = set()
    failed = 0
    for item in actual:
        name = item["id"]
        require(name in expected and name not in seen, "來源編號缺失／重複")
        seen.add(name)
        declared_path = "tests/fixtures/" + expected[name]["path"]
        require(item["path"] == declared_path, "來源檔案路徑與核准契約不同")
        path = (ROOT / declared_path).resolve()
        require(FIXTURES.resolve() in path.parents and path.is_file(),
                "來源檔不存在或越出素材目錄")
        raw = path.read_bytes()
        raw.decode("utf-8", errors="strict")
        require(item["source_sha256"] == sha(raw), "P01 來源實際位元組漂移")
        check_diagnostic(item, optional=False)
        status = {"accept": "Passed", "reject": "Failed"}[expected[name]["expect"]]
        require(item["status"] == status, "P01 已核准接受集合漂移")
        failed += status == "Failed"
    require(failed == 30 and len(seen) == 49, "P01 正反來源數目異常")
    return len(seen), sum(len(item["diagnostics"]) for item in actual)


def inspect_phases() -> tuple[dict[str, dict], int]:
    frozen = read(PHASES)
    rows = frozen["cases"]
    by_id = {row["id"]: row for row in rows}
    require(len(rows) == len(by_id) == 24
            and set(by_id) == EXPECTED_PHASES,
            "P13～P16 原始 24 項來源證據不完整")
    original = {}
    for line in MATRIX.read_text(encoding="utf-8").splitlines():
        match = re.match(r"^\|\s*(P(?:13|14|15|16)\.[1-6])\s*\|", line)
        if match:
            cols = [part.strip() for part in line.split("|")[1:-1]]
            require(len(cols) >= 5 and "已驗收" in cols[-1],
                    "原始 24 項權威矩陣缺少既有簽核")
            original[match.group(1)] = cols[1]
    require(set(original) == EXPECTED_PHASES, "權威 24 項條文不完整")
    selections = 0
    for name, row in by_id.items():
        require(original[name] == row["original_requirement"]
                and row["requirement_sha256"] == sha(original[name].encode("utf-8")),
                "原始 Phase 條文不是已核准的同一筆要求")
        require(row["passed"] == len(row["tests"]) > 0
                and row["failed"] == 0 and row["ignored"] == 0,
                "每項必須有非零編譯執行測試及零錯誤／忽略")
        require(len(row["tests"]) == len(set(row["tests"]))
                and all(row["test_filter"] in name for name in row["tests"]),
                "工作包來源測試身分與實際過濾器不一致")
        selections += len(row["tests"])
    return by_id, selections


def inspect_private(phase_map: dict[str, dict]) -> tuple[int, int, set[str]]:
    frozen = read(PRIVATE)
    rows = frozen["cases"]
    require(len(rows) == 24 and
            {row["id"] for row in rows} == EXPECTED_PHASES,
            "私有診斷快照必須對應全部 24 個原始項目")
    no_entry = set()
    selected = 0
    rejected = 0
    for row in rows:
        phase = phase_map[row["id"]]
        require(row["test_filter"] == phase["test_filter"]
                and row["compiled_tests_passed"] == phase["passed"],
                "私有來源與原始已編譯測試過濾器不一致")
        sources = row["sources"]
        require(row["private_source_coverage"] ==
                ("captured" if sources else "no_private_entry_in_selected_tests"),
                "私有來源覆蓋標記與真實觀測值不符")
        if not sources:
            no_entry.add(row["id"])
        require(len(sources) == len({
            json.dumps(item, sort_keys=True, ensure_ascii=False) for item in sources
        }), "同工作包內同一實際診斷事實重複")
        for item in sources:
            raw = item["source"].encode("utf-8")
            require(item["source_sha256"] == sha(raw),
                    "私有 Checker 來源原始 SHA-256 不一致")
            check_diagnostic(item, optional=True)
            selected += 1
            rejected += item["status"] == "Failed"
    require(no_entry == EXPECTED_NO_PRIVATE,
            "純票據／模型測試覆蓋界線漂移，不可捏造其私有來源")
    return selected, rejected, no_entry


def inspect_bridges() -> tuple[int, int]:
    frozen = read(BRIDGES)
    rows = frozen["cases"]
    require(len(rows) == 15 and len({item["id"] for item in rows}) == 15,
            "八條真實 P16 橋接必須保持 15 條原始成功／失敗路徑")
    require({item["id"].split("_")[0] for item in rows} == EXPECTED_BRIDGES,
            "八種已核准資源橋接至少有一條實際來源")
    events = 0
    for item in rows:
        raw = item["source"].encode("utf-8")
        require(raw.endswith(b"\n") and item["source_sha256"] == sha(raw),
                "資源橋接真實來源原始內容或雜湊漂移")
        created = set()
        released = set()
        for event in item["events"]:
            kind, rid = event["kind"], event["resource_id"]
            require(type(rid) is int and rid >= 1,
                    "真實 ResourceId 必須是正整數")
            if kind == "Created":
                require(rid not in created, "重複建立同一個資源 ID")
                created.add(rid)
            elif kind in {"Moved", "Returned"}:
                require(rid in created and rid not in released,
                        "移動／返回不得使用不存在或已清理資源")
            elif kind == "Released":
                require(rid in created and rid not in released,
                        "不可清理未建立或已釋放的資源")
                released.add(rid)
            else:
                raise ValueError("F00／G0：未核准的 ResourceEvent 類別")
            events += 1
        require(created == released, "每條真實路徑結束都須零洩漏")
    require(events == 57, "真實資源事件數量不能偷偷增減")
    return len(rows), events


def main() -> None:
    public_count, diag_count = inspect_public()
    phase_map, selected_tests = inspect_phases()
    private_count, private_failed, model_only = inspect_private(phase_map)
    bridge_count, event_count = inspect_bridges()
    print(
        "F00／G0 四組凍結證據一致："
        f"P01 {public_count} 份／{diag_count} 筆診斷；"
        f"Phase {len(phase_map)} 項／{selected_tests} 個具名選取紀錄；"
        f"私有 {private_count} 筆來源選取／{private_failed} 筆拒絕；"
        f"八橋接 {bridge_count} 路徑／{event_count} 筆實際事件。"
    )
    print("純模型來源界線：" + "、".join(sorted(model_only))
          + "；不會把靜態測試快照當作未執行分支的動態事件。")


if __name__ == "__main__":
    main()
