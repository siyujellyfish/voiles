#!/usr/bin/env python3
"""P21-A 非公開多檔來源身分、原 Checker 驗證與 TOCTOU 防偽十項 Rust 回歸 Gate。"""

from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import sys

案例 = (
    "p21_a_登錄身分與來源位元組來自同一開啟檔案",
    "p21_a_兩份相同來源內容仍有各自真路徑與獨立_checker_身分",
    "p21_a_同檔路徑別名不得登錄兩份相互冒充的來源權威",
    "p21_a_第二檔不存在時不得回傳已驗證第一檔的部分登錄",
    "p21_a_第二檔非法_utf8_保留原請求路徑與詞彙診斷",
    "p21_a_來源檢查通過後真檔案更換位元組不得沿用已驗證計畫",
    "p21_a_未知跨檔_import_語法必須保留原單檔_checker_拒絕",
    "p21_a_符號連結改指其他同內容來源仍不可沿用原檔案身分",
    "p21_a_unix_硬連結不同正規路徑仍須判定同一實體來源",
    "p21_a_unix_檔案被相同位元組的另一_inode_取代仍拒絕舊證據",
)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    args = parser.parse_args()
    for case in 案例:
        name = "execution::registry::tests::" + case
        result = subprocess.run(
            [str(args.test_bin.resolve()), "--exact", name],
            capture_output=True, encoding="utf-8", check=False,
        )
        if result.returncode or f"test {name} ... ok" not in result.stdout.splitlines():
            print(f"P21-A 失敗 {name}\n{result.stdout}{result.stderr}", file=sys.stderr)
            return 1
    print(f"P21-A 非公開來源登錄：{len(案例)}/{len(案例)} 真 Rust 正反案例通過。")
    print("不包含 import 語意、跨檔求值或模組資源交接，G21 尚未正式驗收。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
