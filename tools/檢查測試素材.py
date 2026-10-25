"""驗證 P01 測試素材一致性；不解析或執行 voiles 程式。"""

from __future__ import annotations

import json
from pathlib import Path


根目錄 = Path(__file__).resolve().parent.parent
素材根目錄 = 根目錄 / "tests" / "fixtures"
清單路徑 = 素材根目錄 / "清單.json"
預期正向編號 = {f"P01-正{序號:02d}" for 序號 in range(1, 19)}
預期反向編號 = {f"P01-反{序號:02d}" for 序號 in range(1, 31)}
預期向量位元組 = {
    "P01-源01": b"",
    "P01-源02": b"A\r\nB",
    "P01-源03": b"\r\n",
    "P01-源04": b"\xff\x80",
    "P01-源05": "\ufffd".encode("utf-8"),
    "P01-源06": b"A\r\nB",
    "P01-源07": "中\nB".encode("utf-8"),
    "P01-源08": b"A\rB",
    "P01-源09": b"A\nB",
    "P01-源10": b"A\tB",
    "P01-源11": b"\xffA\x80",
}


def 驗證() -> None:
    清單 = json.loads(清單路徑.read_text(encoding="utf-8"))
    if 清單.get("format_version") != 1:
        raise ValueError("測試素材清單版本不是 1")

    案例 = 清單["cases"]
    編號集合: set[str] = set()
    已列路徑: set[Path] = set()
    for 項目 in 案例:
        編號 = 項目["id"]
        if 編號 in 編號集合:
            raise ValueError(f"測試素材編號重複：{編號}")
        編號集合.add(編號)

        預期 = ("accept" if 編號 in 預期正向編號 else
               "reject" if 編號 in 預期反向編號 else None)
        if 預期 is None or 項目["expect"] != 預期:
            raise ValueError(f"不受支援的案例編號或預期：{編號}")

        來源路徑 = 項目["path"]
        if 來源路徑 != f"voiles/{編號}.voil":
            raise ValueError(f"案例路徑與編號不一致：{編號}")
        完整路徑 = (素材根目錄 / 來源路徑).resolve()
        if 素材根目錄.resolve() not in 完整路徑.parents:
            raise ValueError(f"案例路徑超出素材目錄：{編號}")
        if 完整路徑 in 已列路徑 or not 完整路徑.is_file():
            raise ValueError(f"重複或缺少案例檔：{來源路徑}")
        已列路徑.add(完整路徑)

        原始資料 = 完整路徑.read_bytes()
        原始內容 = 原始資料.decode("utf-8", errors="strict")
        if not 原始內容.endswith("\n") or "\r" in 原始內容:
            raise ValueError(f"案例應以 LF 換行且以 LF 結尾：{編號}")
        if not 原始內容.strip() or "\ufeff" in 原始內容:
            raise ValueError(f"空白或含 BOM 的案例：{編號}")
        if not 項目.get("rules") or not all(
            isinstance(代號, str) and 代號 for 代號 in 項目["rules"]
        ):
            raise ValueError(f"案例缺少可追溯決策代號：{編號}")

    if 編號集合 != 預期正向編號 | 預期反向編號:
        raise ValueError("正反案例編號未完整對應 P01-正01～18 與反01～30")
    實際路徑 = set((素材根目錄 / "voiles").glob("*.voil"))
    if 已列路徑 != {檔案.resolve() for 檔案 in 實際路徑}:
        raise ValueError("存在未列於清單的 voiles 案例檔案")

    向量 = 清單["source_vectors"]
    向量編號: set[str] = set()
    for 項目 in 向量:
        編號 = 項目["id"]
        if 編號 in 向量編號 or 編號 not in 預期向量位元組:
            raise ValueError(f"重複或未知來源向量：{編號}")
        向量編號.add(編號)
        位元組 = bytes.fromhex(項目["hex"])
        if 位元組 != 預期向量位元組[編號]:
            raise ValueError(f"來源向量位元組與已核准 P01 判例不符：{編號}")
        if not 項目.get("rules") or not 項目.get("observations"):
            raise ValueError(f"來源向量未附決策依據與驗收觀察：{編號}")
    if 向量編號 != set(預期向量位元組):
        raise ValueError("來源向量不完整")


if __name__ == "__main__":
    驗證()
    print("測試素材檢查通過：48 個 UTF-8 原始案例、11 個原始位元組向量。")
