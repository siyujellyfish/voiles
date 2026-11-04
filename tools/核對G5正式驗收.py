#!/usr/bin/env python3
"""P16.6-R03 G5：F12/F13 八橋接真事件及 Checker 靜態責任獨立正式驗收。

執行原始凍結 15 路徑／57 ResourceEvent 的真正 Rust 採集比對；
以同 HEAD G4 真 Gate、六個 F12/F13 子 Gate 與額外 26 個資源事件
正反 --exact 核對，不從靜態 CFG 或 AST 假造執行期身分。
PR 僅 PassedCandidate；同一 main SHA 真 push 才能 Passed。
"""
from __future__ import annotations
import argparse
import ast
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
FROZEN = ROOT / "tests" / "fixtures" / "F00-八橋接資源事件快照.json"
GATES = {
    "F12": (
        "核對R03B09第一批.py",
        "核對G5真安全出口第一批.py",
        "核對G5真持有者第二批.py",
        "核對G5真早退第三批.py",
    ),
    "F13": (
        "核對R03B10第一批.py",
        "核對G5同環境效能.py",
    ),
}
EXTRA = (
    "真實具名字串返回擁有者須接收同一資源且只清理一次",
    "返回與移動事件不互換且來源身分及清理都不可偽造",
    "單層已知布林分支內層必須先於外層逆序清理",
    "未執行的分支不得創造內層所有者或資源事件",
    "作用域釋放順序錯置_移動事件_雙重資源身分都必須拒絕",
    "混合結構僅對真正_str_建立移交釋放_而純值複製不造假事件",
    "混合欄位失敗只清理當時已完成之_str_不把純值當成資源",
    "欄位原子覆寫舊資源先釋放而新資源由持有者退出清理",
    "未初始化欄位第一次覆寫不得清理不存在的舊值",
    "欄位覆寫拒絕先釋放舊值_雙重清理_混入移動及相同資源身分",
    "多欄位整筆移交遵守建立順序與嚴格逆序清理",
    "多資源跨函式返回以相同宣告身分帳簿審核並逆序清理",
    "結構返回不得混入移動_交錯事件_偽造身分或錯誤清理順序",
    "建構失敗只能逆序清理已建立資源且不移交",
    "多資源失序_重複身分_造假轉交或清理不得被帳簿接受",
    "真正唯一資源建立移交釋放序列同時通過兩張責任表",
    "預檢拒絕同一宣告身分且不建立任何擁有者",
    "不同資源身分與重複或缺失清理都必須保守拒絕",
    "計數相等仍不得掩蓋同一身分重複釋放",
    "未建立的身分不可移動或回傳",
    "建立和釋放身分不一致即使數量相同仍要拒絕",
    "釋放後仍有轉移事件不得被視為有效所有權",
    "真正_voil_混合結構具名優先求值只有字串建立移動清理事件",
    "真正_voil_if_分支內提前_return_離開函式按逆序清理",
    "真正_voil_if_分支提前返回且故障時不得遺失外層",
    "實際_voil_跨函式整筆返回原作用域不釋放_最終呼叫端逆序清理",
)

def run(cmd: list[str]) -> str:
    p = subprocess.run(cmd, cwd=ROOT, text=True, encoding="utf-8",
        capture_output=True, errors="replace", check=False)
    if p.returncode:
        raise AssertionError(
            f"G5 真實資源測試失敗（{p.returncode}）：{' '.join(cmd)}\n"
            + p.stdout[-2600:] + p.stderr[-2600:]
        )
    return p.stdout + p.stderr

def cases_for(script: Path) -> tuple[str, ...]:
    source = ast.parse(script.read_text(encoding="utf-8"), filename=str(script))
    for node in source.body:
        if isinstance(node, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id == "CASES" for target in node.targets
        ):
            cases = ast.literal_eval(node.value)
            if isinstance(cases, tuple) and cases and all(isinstance(x, str) and x for x in cases):
                return cases
    raise AssertionError(f"G5 子來源沒有可列名、固定、不重複的 Rust 正反 CASES：{script.name}")

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("G5 正式主線驗收必須 --check")
    binpath = (ROOT / args.test_bin).resolve()
    if not binpath.is_file():
        raise AssertionError("缺少與當前 HEAD 相同編譯來源的真 Rust 測試執行檔")

    head = run(["git", "rev-parse", "HEAD"]).strip()
    github_sha = os.environ.get("GITHUB_SHA", "")
    github_ref = os.environ.get("GITHUB_REF", "")
    if github_sha and github_sha != head:
        raise AssertionError("G5 checkout SHA 和 GITHUB_SHA 不同")
    main_verified = bool(github_sha) and github_ref == "refs/heads/main" and head == github_sha

    # G4 必須重新獨立驗收，而不是讀取先前文字／GitHub 綠燈。
    g4_output = run([sys.executable, str(ROOT / "tools" / "核對G4正式驗收.py"),
        "--test-bin", str(binpath), "--check"])
    g4_rows = [json.loads(line) for line in g4_output.splitlines()
        if line.startswith("{") and '"gate": "P16.6-R03-G4"' in line]
    if len(g4_rows) != 1 or g4_rows[0].get("unique_rust_tests") != 55 \
            or g4_rows[0].get("outstanding") != []:
        raise AssertionError("G5 不能繞過 G4 55 個真 CFG 安全計畫證據")
    if g4_rows[0].get("result") != ("Passed" if main_verified else "PassedCandidate"):
        raise AssertionError("G5 的 G4 前置同 SHA 身分不匹配")

    # 不從靜態義務重建 ResourceEvent：重新執行凍結的真 P16 八橋接來源。
    actual = run([sys.executable, str(ROOT / "tools" / "鎖定F00八橋接.py"),
        "--test-bin", str(binpath), "--check"])
    if "F00 八橋接實際事件快照通過：15 條" not in actual:
        raise AssertionError("沒有真執行 F00 八橋接 15 條成功／失敗路徑")
    frozen = json.loads(FROZEN.read_text(encoding="utf-8"))
    rows = frozen["cases"]
    n_events = sum(len(row["events"]) for row in rows)
    if len(rows) != 15 or n_events != 57:
        raise AssertionError("八橋接來源或真 ResourceId 事件凍結被改動")
    for row in rows:
        events = row["events"]
        ids_created = {e["resource_id"] for e in events if e["kind"] == "Created"}
        ids_released = [e["resource_id"] for e in events if e["kind"] == "Released"]
        if len(ids_released) != len(set(ids_released)) or set(ids_released) != ids_created:
            raise AssertionError("同一資源存在雙重釋放或遺失釋放")

    bridge = (ROOT / "src/struct_stage/resource_trace.rs").read_text(encoding="utf-8")
    core = (ROOT / "src/struct_stage.rs").read_text(encoding="utf-8")
    resource = (ROOT / "src/owned_str_runtime.rs").read_text(encoding="utf-8")
    for source, marker, count in (
        (bridge, "fn require_checked_bridge_return(", 1),
        (bridge, "fn require_checked_bridge_owner(", 1),
        (bridge, "fn closed_bridge_events(", 1),
        (bridge, "fn closed_bridge_result<", 1),
        (bridge, "actual_return @ Stmt {kind:StmtKind::Return(term),..}", 1),
        (core, "fn exit_snapshots_are_consistent(", 1),
        (core, "fn cleanup_from_bindings(", 1),
        (resource, "pub(crate) fn event_trace_balanced(", 1),
    ):
        if source.count(marker) != count:
            raise AssertionError(f"G5 真動靜權威缺失、重複或被繞過：{marker}")
    # 八條主返回、兩條跨函式、單層真早退：11 個實際出口守衛。
    if bridge.count("require_checked_bridge_return(") != 12:
        raise AssertionError("G5 真橋接或跨函式／早退退出證據不完整")
    if bridge.count("require_checked_bridge_owner(") != 11:
        raise AssertionError("G5 真資源執行者或提前返回內外持有者沒有逐 DeclarationId 檢查")

    listed = run([str(binpath), "--list"])
    available = [line[:-6] for line in listed.splitlines() if line.endswith(": test")]
    used: set[str] = set()
    counts: dict[str, int] = {}
    perf: dict[str, object] | None = None
    for group, scripts in GATES.items():
        count = 0
        for name in scripts:
            script = ROOT / "tools" / name
            cases = cases_for(script)
            for suffix in cases:
                matched = [test for test in available if test.endswith("::" + suffix)]
                if len(matched) != 1 or matched[0] in used:
                    raise AssertionError(f"{group} 真 Rust 正反來源缺失／重複：{suffix}")
                used.add(matched[0])
                count += 1
            output = run([sys.executable, str(script),
                "--test-bin", str(binpath), "--check"])
            if name == "核對G5同環境效能.py":
                matches = [json.loads(line) for line in output.splitlines()
                    if line.startswith("{") and '"gate": "P16.6-R03-G5-PERF"' in line]
                if len(matches) != 1 or matches[0].get("result") != "MeasuredOnly":
                    raise AssertionError("F13 沒有同一真 Rust 程序的兩種可核對效能數值")
                perf = matches[0]["workloads"]
        counts[group] = count
    if counts != {"F12": 18, "F13": 6}:
        raise AssertionError(f"G5 逐包真 Rust 非空測試數量漂移：{counts}")

    for suffix in EXTRA:
        matched = [test for test in available if test.endswith("::" + suffix)]
        if len(matched) != 1 or matched[0] in used:
            raise AssertionError(f"G5 真事件／原子性／逆序清理來源缺失或重複：{suffix}")
        used.add(matched[0])
        result = run([str(binpath), "--exact", matched[0], "--test-threads=1"])
        if "1 passed; 0 failed" not in result:
            raise AssertionError(f"G5 補充真 ResourceId Rust 負例未成功：{suffix}")
    if len(used) != 50 or perf is None:
        raise AssertionError("G5 整包至少需 50 個互異 Rust 真正測試與同環境效能")

    print(json.dumps({
        "gate": "P16.6-R03-G5",
        "result": "Passed" if main_verified else "PassedCandidate",
        "main_verified": main_verified,
        "github_sha": head,
        "github_ref": github_ref,
        "executed": ["F12", "F13", "G4"],
        "groups": counts,
        "extra_tests": len(EXTRA),
        "unique_rust_tests": len(used),
        "real_bridge_paths": len(rows),
        "real_resource_events": n_events,
        "performance_workloads": sorted(perf),
        "performance_is_observational": True,
        "outstanding": [],
        "g6_in_scope": False,
    }, ensure_ascii=False, sort_keys=True))
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError, ValueError, SyntaxError, KeyError, TypeError) as error:
        print(f"G5 真資源正式驗收拒絕：{error}", file=sys.stderr)
        sys.exit(1)
