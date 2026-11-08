#!/usr/bin/env python3
"""以 GitHub 真實 HEAD 與必要 CI 決定排程狀態；失去證據一律不進入施工。"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

必要檢查 = {"Rust 前端建置", "Rust 獨立架構測試", "文件與工作紀錄"}
自動分支 = "auto/voiles-p18-progress"
自動標記 = "<!-- voiles-auto-progress-v1 -->"


def 取得(path: str) -> object:
    權杖 = os.environ["GH_TOKEN"]
    請求 = Request(
        f"https://api.github.com/repos/{os.environ['GITHUB_REPOSITORY']}/{path}",
        headers={
            "Authorization": f"Bearer {權杖}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "voiles-auto-progress",
        },
    )
    with urlopen(請求, timeout=20) as 回應:
        return json.load(回應)


def 結果(狀態: str, 說明: str, **欄位: str) -> None:
    print(json.dumps({"狀態": 狀態, "說明": 說明, **欄位}, ensure_ascii=False))
    輸出檔 = os.environ.get("GITHUB_OUTPUT")
    if 輸出檔:
        with Path(輸出檔).open("a", encoding="utf-8") as 檔:
            for 名稱, 值 in {"mode": 狀態, "reason": 說明, **欄位}.items():
                檔.write(f"{名稱}={值}\n")


def 檢查成功(sha: str) -> tuple[bool, str]:
    資料 = 取得(f"commits/{sha}/check-runs?per_page=100")
    if not isinstance(資料, dict) or 資料.get("total_count", 0) > 100:
        return False, "檢查資料超出上限或格式不符"
    執行結果: dict[str, list[dict]] = {}
    for 項目 in 資料.get("check_runs", []):
        if 項目.get("name") in 必要檢查:
            執行結果.setdefault(項目["name"], []).append(項目)
    不合格 = []
    for 名稱 in sorted(必要檢查):
        項目 = 執行結果.get(名稱, [])
        if not 項目 or len(項目) != 1:
            不合格.append(f"{名稱}: 缺少或重複")
        elif 項目[0].get("status") != "completed" or 項目[0].get("conclusion") != "success":
            不合格.append(f"{名稱}: {項目[0].get('status')}/{項目[0].get('conclusion')}")
    return not 不合格, "；".join(不合格) if 不合格 else "必要三項 completed/success"


def main() -> None:
    if not os.environ.get("GH_TOKEN") or not os.environ.get("GITHUB_REPOSITORY"):
        結果("blocked", "缺少 GitHub 唯讀授權")
        return
    資料 = 取得("git/ref/heads/main")
    sha = 資料["object"]["sha"]
    合格, 診斷 = 檢查成功(sha)
    if not 合格:
        結果("blocked", f"最新 main CI 未全綠：{診斷}", main_sha=sha)
        return
    請求項 = 取得("pulls?state=open&per_page=100")
    if not isinstance(請求項, list) or len(請求項) == 100:
        結果("blocked", "PR 清單超過可稽核上限", main_sha=sha)
        return
    自動項 = [p for p in 請求項 if p.get("head", {}).get("ref") == 自動分支]
    if len(自動項) > 1:
        結果("blocked", "自動 PR 不唯一", main_sha=sha)
        return
    可寫 = os.environ.get("HAS_OPENAI_KEY") == "true" and os.environ.get("HAS_GITHUB_APP") == "true"
    if 自動項:
        p = 自動項[0]
        pr = str(p["number"])
        head_sha = p["head"]["sha"]
        if (
            自動標記 not in (p.get("body") or "")
            or not (p.get("user", {}).get("login") or "").endswith("[bot]")
            or p.get("base", {}).get("ref") != "main"
        ):
            結果("blocked", "存在未通過身分／契約驗證的自動分支 PR", main_sha=sha, pr=pr)
            return
        合格, 診斷 = 檢查成功(head_sha)
        if not 合格:
            結果("blocked", f"自動 PR 最新 HEAD CI 未全綠：{診斷}", main_sha=sha, pr=pr, head_sha=head_sha)
            return
        詳情 = 取得(f"pulls/{pr}")
        if 詳情.get("mergeable") is not True or 詳情.get("mergeable_state") != "clean" or 詳情.get("base", {}).get("sha") != sha:
            結果("blocked", "PR 合併狀態未可證安全或 base 已變更", main_sha=sha, pr=pr, head_sha=head_sha)
            return
        if not 可寫:
            結果("blocked", "尚未配置 Codex 與專用 GitHub App Secrets", main_sha=sha, pr=pr)
            return
        結果("merge", "自動 PR 最新 HEAD 三項 CI 全綠且可 Squash merge", main_sha=sha, pr=pr, head_sha=head_sha)
        return
    重疊 = [
        f"#{p['number']}"
        for p in 請求項
        if not p.get("draft")
        and p.get("base", {}).get("ref") == "main"
        and (
            p.get("head", {}).get("ref", "").startswith(("feat/g18", "work/p18", "auto/"))
            or "P18" in (p.get("title") or "")
            or "G18" in (p.get("title") or "")
        )
    ]
    if 重疊:
        結果("blocked", f"尚有待裁定／整合的 P18 重疊 PR：{', '.join(重疊[:12])}", main_sha=sha)
        return
    if not 可寫:
        結果("blocked", "缺少 OPENAI_API_KEY 或專用 GitHub App Secrets，維持唯讀巡檢", main_sha=sha)
        return
    結果("develop", "最新主線驗證通過，允許開始單一已核准施工子批", main_sha=sha)


if __name__ == "__main__":
    try:
        main()
    except (HTTPError, URLError, KeyError, ValueError, TypeError, OSError) as 錯誤:
        print(f"自動巡檢安全中止：{錯誤}", file=sys.stderr)
        結果("blocked", "GitHub 檢查失敗；不允許動作")
