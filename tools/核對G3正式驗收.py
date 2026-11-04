#!/usr/bin/env python3
"""P16.6-R03 G3 正式整包驗收：F06～F09 真所有權／借用／原子安全關卡。
僅在當前 checkout 同一 SHA 的 main push 中輸出 Passed；PR 僅 PassedCandidate。
G4/G5/G6 以及未核准的 P17 仍不在本 Gate 範圍。
"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
EXTRA = (
    "hc14_不同追蹤器的借用票據即使宣告位置與序號相同也不能互換",
    "hc14_批次中混入跨追蹤器票據不得部分解除其餘借用",
    "hc14_錯誤或重複票據不可提前解除真實借用限制",
    "一般別名最後使用點可單獨歸還而方法_self_須等作用域結束",
    "方法作用域退出失敗不可提前解鎖接收者",
    "p14_獨占借用期間不可再借_移動_重新初始化",
    "f09_同一批次所有持有者必須通過兩側預檢才可原子清理",
    "f09_責任帳簿分歧不能提前清理另一個有效持有者",
    "原子宣告_責任帳簿重複身分時所有權不得提前宣告",
    "原子移交_資源帳簿缺失時不能先移走所有權",
    "原子複製_資源表目標衝突時不得產生所有權目標",
    "原子清理_資源責任缺失時不可提前結束來源所有權",
    "未明確完成所有資源責任不得宣稱安全退出",
)

def run(cmd: list[str]) -> str:
    result = subprocess.run(cmd, cwd=ROOT, capture_output=True,
        text=True, encoding="utf-8", errors="replace", check=False)
    if result.returncode:
        raise AssertionError(
            f"G3 真實執行失敗（{result.returncode}）: {' '.join(cmd)}\n"
            + result.stdout[-2200:] + result.stderr[-2200:]
        )
    return result.stdout + result.stderr

def check_authority() -> None:
    safety = (ROOT / "src/borrow_safety.rs").read_text(encoding="utf-8")
    struct = (ROOT / "src/struct_stage.rs").read_text(encoding="utf-8")
    checker = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    pipeline = (ROOT / "src/resource_borrow_pipeline.rs").read_text(encoding="utf-8")
    required = (
        ("單一正式靜態安全資料儲存", struct, "struct SafetyState {"),
        ("唯一靜態安全操作轉移", struct, "fn transition(&mut self, action: StaticTransition<"),
        ("分支三維原子轉移", struct, "StaticTransition::JoinReachable {"),
        ("真借用票據身分來源", safety, "pub(crate) struct BorrowTicket {"),
        ("真票據完整歸還", safety, "fn finish_scope_verified("),
        ("真詞彙 HIR use-def", checker, "fn scan_resolved_block_uses("),
        ("真 Parser 別名來源", checker, "StaticBorrowSnapshot::from_aliases(vars.iter()"),
        ("真雙帳簿宣告預檢", pipeline, "self.resources.check_create_binding(id)"),
        ("真雙帳簿複製預檢", pipeline, "self.resources.check_copy_into(from, to)"),
        ("真雙帳簿移交預檢", pipeline, "self.resources.check_move_into(from, to)"),
        ("真雙帳簿清理預檢", pipeline, "self.resources.check_release_batch(owners)"),
    )
    for label, source, marker in required:
        if marker not in source:
            raise AssertionError(f"G3 來源完整性缺失：{label}")
    if "fn term_moves_name(" in checker or "fn block_moves_source(" in checker:
        raise AssertionError("仍存在可繞過 DeclId use-def 的平行名稱掃描權威")
    if "original.value =" in checker or "original.pending_fields =" in checker:
        raise AssertionError("CFG 分支仍可直接繞過 SafetyState 的原子 Transition")
    if "pub(crate) struct SafetyState" in pipeline:
        raise AssertionError("靜態模型不准在執行期雙帳簿建立另一份同名可寫權威")

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("正式 G3 必須使用 --check")
    testbin = (ROOT / args.test_bin).resolve()
    if not testbin.is_file():
        raise AssertionError("G3 必須使用真正已編譯的 Rust 測試執行檔")
    check_authority()
    partial = run([
        sys.executable, str(ROOT / "tools" / "核對G3獨立驗收矩陣.py"),
        "--test-bin", str(testbin), "--check",
    ])
    lines = partial.strip().splitlines()
    if not lines:
        raise AssertionError("獨立 G3 原子正反矩陣沒有真實回傳")
    evidence = json.loads(lines[-1])
    if evidence.get("gate") != "P16.6-R03-G3-SUBSET" or evidence.get("result") != "VerifiedPartial":
        raise AssertionError("前置 G3 子矩陣沒有交付真實來源")
    if evidence.get("groups") != {"F06": 12, "F07": 13, "F08": 7, "F09": 5}:
        raise AssertionError("F06～F09 任一來源正反數量遭到改寫或漏案")
    if evidence.get("unique_rust_tests") != 37:
        raise AssertionError("G3 已驗證來源不能空過濾或重複採計")
    listing = run([str(testbin), "--list"])
    names = [line[:-6] for line in listing.splitlines() if line.endswith(": test")]
    selected: set[str] = set()
    for suffix in EXTRA:
        matched = [name for name in names if name.endswith("::" + suffix)]
        if len(matched) != 1 or matched[0] in selected:
            raise AssertionError(f"G3 真票據／動靜原子補充案例不唯一：{suffix}")
        name = matched[0]
        selected.add(name)
        output = run([str(testbin), "--exact", name, "--test-threads=1"])
        if "1 passed; 0 failed" not in output:
            raise AssertionError(f"G3 真票據／清理原子補充案例未成功：{name}")
    head = run(["git", "rev-parse", "HEAD"]).strip()
    ghsha = os.environ.get("GITHUB_SHA", "")
    ref = os.environ.get("GITHUB_REF", "")
    if ghsha and head != ghsha:
        raise AssertionError("Git checkout SHA 與 GitHub SHA 不一致，不得簽 G3")
    main_verified = ref == "refs/heads/main" and ghsha == head
    output = {
        "gate": "P16.6-R03-G3",
        "result": "Passed" if main_verified else "PassedCandidate",
        "github_sha": head,
        "github_ref": ref,
        "main_verified": main_verified,
        "executed": ["F06", "F07", "F08", "F09"],
        "subset_tests": 37,
        "extra_tests": len(selected),
        "unique_rust_tests": 37 + len(selected),
        "outstanding": [],
        "g4_g5_g6_in_scope": False,
    }
    print(json.dumps(output, ensure_ascii=False, sort_keys=True))
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError, ValueError, KeyError) as exc:
        print(f"G3 正式安全驗收拒絕：{exc}", file=sys.stderr)
        sys.exit(1)
