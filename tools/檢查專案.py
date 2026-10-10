"""以 Python 標準函式庫驗證 voiles 儲存庫的基本管理規範。"""

from __future__ import annotations

import os
import subprocess
from pathlib import Path


根目錄 = Path(__file__).resolve().parent.parent
必要文件 = [
    "README.md",
    "AI-build/README.md",
    "AI-build/專案規則.md",
    "AI-build/語言規格-v0.2.md",
    "AI-build/待確認事項.md",
    "AI-build/工作紀錄.md",
]
文字副檔名 = {".md", ".py", ".yml", ".yaml", ".json", ".toml"}


def 執行_git(*參數: str) -> list[str]:
    結果 = subprocess.run(
        ["git", *參數],
        cwd=根目錄,
        check=True,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    return [資料 for 資料 in 結果.stdout.splitlines() if 資料]


def 檢查必要文件() -> list[str]:
    錯誤 = []
    for 路徑 in 必要文件:
        檔案 = 根目錄 / 路徑
        if not 檔案.is_file():
            錯誤.append(f"缺少必要文件：{路徑}")
        elif 檔案.stat().st_size == 0:
            錯誤.append(f"文件不能為空：{路徑}")
    return 錯誤


def 檢查文字編碼() -> list[str]:
    錯誤 = []
    for 路徑 in 執行_git("ls-files"):
        檔案 = 根目錄 / 路徑
        if not 檔案.is_file() or 檔案.suffix.lower() not in 文字副檔名:
            continue
        try:
            內容 = 檔案.read_text(encoding="utf-8")
        except UnicodeError:
            錯誤.append(f"檔案不是有效的 UTF-8：{路徑}")
            continue
        if 路徑.endswith(".md") and not 內容.strip():
            錯誤.append(f"文件沒有內容：{路徑}")
        if 路徑.endswith(".md") and "Violte" in 內容:
            錯誤.append(f"文件仍含舊語言名稱：{路徑}")
    return 錯誤


def 檢查工作紀錄更新() -> list[str]:
    # 在合併請求的暫存合併提交中，第一父提交為基底，第二父提交為工作分支。
    if os.environ.get("VOILES_PULL_REQUEST") != "true":
        return []
    try:
        變更檔案 = 執行_git(
            "diff",
            "--name-only",
            "--diff-filter=ACMR",
            "HEAD^1",
            "HEAD^2",
        )
    except subprocess.CalledProcessError:
        return ["無法取得合併請求的差異，請檢查分支擷取深度。"]
    if not 變更檔案:
        return []
    有外部變更 = any(not 路徑.startswith("AI-build/") for 路徑 in 變更檔案)
    有內部更新 = any(路徑.startswith("AI-build/") for 路徑 in 變更檔案)
    if 有外部變更 and not 有內部更新:
        return ["此次變更未同步更新 AI-build/；每次實作或規劃更新都必須同步修改管理文件。"]
    return []


def 主程式() -> int:
    錯誤 = 檢查必要文件()
    錯誤.extend(檢查文字編碼())
    錯誤.extend(檢查工作紀錄更新())
    if 錯誤:
        for 訊息 in 錯誤:
            print(f"檢查失敗：{訊息}")
        return 1
    print("檢查通過：必要文件、文字編碼及工作紀錄同步規範。")
    return 0


if __name__ == "__main__":
    raise SystemExit(主程式())
