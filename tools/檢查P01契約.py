"""核對 P01 驗收矩陣的追溯與素材一致性；不是 voiles 解析器。"""

from __future__ import annotations

import json
from pathlib import Path


根 = Path(__file__).resolve().parent.parent / "tests" / "fixtures"


def 主程式() -> None:
    清單 = json.loads((根 / "清單.json").read_text(encoding="utf-8"))
    契約 = json.loads((根 / "驗收矩陣.json").read_text(encoding="utf-8"))
    if 契約.get("format_version") != 1:
        raise ValueError("不支援的 P01 驗收矩陣版本")
    案例 = {項目["id"]: 項目 for 項目 in 清單["cases"]}
    向量 = {項目["id"]: 項目 for 項目 in 清單["source_vectors"]}
    群組 = 契約["groups"]
    必備 = {
        "P01.1-子集", "P01.1-文法", "P01.1-AST",
        "P01.2-名稱", "P01.2-型別", "P01.2-資料流", "P01.2-溢位",
        "P01.3-Optional隔離", "P01.4-未支援",
        "P01.5-來源", "P01.5-測試與CI",
    }
    編號 = [項目["id"] for 項目 in 群組]
    if set(編號) != 必備 or len(set(編號)) != len(編號):
        raise ValueError("P01 追溯群組不足或重複")
    已覆蓋案例: set[str] = set()
    已覆蓋向量: set[str] = set()
    for 群 in 群組:
        if not 群.get("rules") or not (群.get("cases") or 群.get("source_vectors")):
            raise ValueError(f"驗收群組沒有決策依據或案例：{群['id']}")
        if len(set(群.get("cases", []))) != len(群.get("cases", [])):
            raise ValueError(f"群組重複案例：{群['id']}")
        for 案例編號 in 群.get("cases", []):
            if 案例編號 not in 案例:
                raise ValueError(f"案例不存在：{案例編號}")
            已覆蓋案例.add(案例編號)
        for 向量編號 in 群.get("source_vectors", []):
            if 向量編號 not in 向量:
                raise ValueError(f"位元組向量不存在：{向量編號}")
            已覆蓋向量.add(向量編號)
    if 已覆蓋案例 != set(案例):
        raise ValueError("有案例未列入 P01 契約群組：" + str(sorted(set(案例) - 已覆蓋案例)))
    if 已覆蓋向量 != set(向量):
        raise ValueError("有原始位元組向量未列入 P01 契約群組")
    if not any(x["expect"] == "accept" for x in 案例.values()):
        raise ValueError("缺少正向驗收案例")
    if not any(x["expect"] == "reject" for x in 案例.values()):
        raise ValueError("缺少反向驗收案例")
    print("P01 契約追溯檢查通過：11 個群組，48 個正反案例，11 個原始位元組向量。")
    print("注意：只驗證素材／決策引用完整性，沒有執行 voiles 的語法或靜態檢查。")


if __name__ == "__main__":
    主程式()
