#!/usr/bin/env python3
"""量測 voiles 公開 MVP-A CLI 的單檔檢查延遲；不作為硬性效能門檻。"""

from __future__ import annotations

import argparse
import json
import statistics
import subprocess
import tempfile
import time
from pathlib import Path


def 產生合法來源(宣告數: int) -> str:
    內容 = ["fn main()->int{"]
    內容.extend(f"const value_{索引}:int = 1" for 索引 in range(宣告數))
    內容.extend(["return 0", "}"])
    return "\n".join(內容) + "\n"


def 產生合法分支來源(分支數: int) -> str:
    """每段 if/else 都讀寫同一外層變數，檢查分支狀態複製與安全合流。"""
    內容 = ["fn main()->int{", "var value:int = 0"]
    for _ in range(分支數):
        內容.extend(["if (true){", "value = 1", "}else{", "value = 2", "}"])
    內容.extend(["return value", "}"])
    return "\n".join(內容) + "\n"


def 產生非法位元組來源(錯誤數: int) -> bytes:
    return b"fn main()->int{\n" + b"\xff\n" * 錯誤數 + b"return 0\n}\n"


def 測量(命令: Path, 來源: Path, 預期退出碼: int, 重複: int) -> dict[str, float | int]:
    耗時 = []
    for _ in range(重複 + 1):
        起點 = time.perf_counter_ns()
        結果 = subprocess.run(
            [str(命令), str(來源)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        經過 = (time.perf_counter_ns() - 起點) / 1_000_000
        if 結果.returncode != 預期退出碼:
            raise RuntimeError(f"{來源.name}：預期退出 {預期退出碼}，實際 {結果.returncode}")
        耗時.append(經過)
    樣本 = 耗時[1:]  # 首次啟動作為暖機，不納入統計
    return {
        "中位數毫秒": round(statistics.median(樣本), 3),
        "最小毫秒": round(min(樣本), 3),
        "最大毫秒": round(max(樣本), 3),
        "重複次數": 重複,
    }


def 主程式() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("命令", type=Path, help="已編譯的 voiles CLI 路徑")
    parser.add_argument("--repeat", type=int, default=7, help="每種樣本的計時次數")
    parser.add_argument("--sizes", type=int, nargs="+", default=[128, 512, 2048])
    args = parser.parse_args()
    if args.repeat < 3 or any(數量 < 1 for 數量 in args.sizes):
        parser.error("--repeat 需 >= 3，--sizes 需為正整數")
    cli = args.命令.resolve(strict=True)
    報告 = {"命令": str(cli), "注意": "包含程序啟動、檔案 IO、診斷輸出；非純語法分析 CPU 時間", "樣本": []}
    with tempfile.TemporaryDirectory(prefix="voiles-perf-") as 目錄:
        root = Path(目錄)
        for 類型 in ("合法宣告", "合法分支", "非法UTF8診斷"):
            for 數量 in args.sizes:
                path = root / f"{類型}-{數量}.voil"
                if 類型 == "合法宣告":
                    path.write_text(產生合法來源(數量), encoding="utf-8")
                    exit_code = 0
                elif 類型 == "合法分支":
                    path.write_text(產生合法分支來源(數量), encoding="utf-8")
                    exit_code = 0
                else:
                    path.write_bytes(產生非法位元組來源(數量))
                    exit_code = 1
                數據 = 測量(cli, path, exit_code, args.repeat)
                報告["樣本"].append({
                    "類型": 類型,
                    "數量": 數量,
                    "來源位元組": path.stat().st_size,
                    **數據,
                })
    print(json.dumps(報告, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(主程式())
