#!/usr/bin/env python3
"""P16.6-R03 G4 正式獨立安全計畫驗收：F10/F11 原 CFG、逐控制邊及退出清理。
正式 Passed 只可由最新 main 同 GITHUB_SHA 真執行產生，PR 為 PassedCandidate。
本 Gate 是原已核准的受限私有靜態 P13～P16；不簽發執行期資源事件。
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
GATES = {
    "F10": (
        "核對R03B07第一批.py",
        "核對G4控制轉移第一批.py",
        "核對G4出口狀態第三批.py",
    ),
    "F11": (
        "核對R03B08第一批.py",
        "核對G4方法安全第二批.py",
        "核對G4清理義務第四批.py",
    ),
}
EXTRA = (
    "while_分支內_break_不能被當作函式_return",
    "巢狀_for_的_break_只離開最近一層迴圈",
    "單次迴圈_break_後仍須檢查移動來源的後續使用",
    "單次迴圈_if_的_break_與_continue_均不能跳過後繼",
    "真正的_函式_return_仍可在單次迴圈內成立",
    "break_與初始化分支混合不能假裝每條出口都已初始化",
    "continue_與資源移動分支混合不能捏造唯一出口狀態",
    "break_和正常分支都未改外層資源時仍安全",
    "break_之後另一分支的後續寫入不能復活未初始化資源",
    "內層迴圈的_break_不會錯當外層單次迴圈出口",
    "真正來源的零次迴圈內假想引用不可封鎖所有者讀取",
    "正式私有檢查不能因內層遮蔽而提早釋放外層仍會使用的借用",
    "直線_return_必然離開序列_後續借用名字不可延長舊票據",
    "if_兩側均確定返回_分支後面的名字沒有可達_use_def",
    "迴圈_break_continue_只停止本體_迴圈外的後繼仍有效",
    "兩個正常不可達分支應與正式_checker_合流返回同一_flowstate",
    "巢狀混合退出仍不擅自判定為全部_return",
    "f10_同種與混合的迴圈跳轉必須保留_break_continue_各自可達類別",
    "方案_a_依欄位寫入推導方法共享或獨占借用",
    "任一分支修改_self_即推導獨占借用",
    "方法分支提前返回已核准_未知接收者寫入仍拒絕",
    "s15_方法_if_內消耗_self_精準拒絕",
    "g4_同一return的兩個獨立str資源各自有必需清理身分",
    "g4_移交其中一個資源時原Owner不清理且剩餘Owner仍須清理",
    "g4_不同分支各自局部str只能出現在自己的靜態return責任",
)

def run(args: list[str]) -> str:
    p = subprocess.run(args, cwd=ROOT, text=True, encoding="utf-8",
        errors="replace", capture_output=True, check=False)
    if p.returncode:
        raise AssertionError(
            f"真實 G4 依賴或 Rust 執行失敗 {p.returncode}: {' '.join(args)}\n"
            + p.stdout[-2200:] + p.stderr[-2200:]
        )
    return p.stdout + p.stderr

def source_cases(script: Path) -> tuple[str, ...]:
    parsed = ast.parse(script.read_text(encoding="utf-8"), filename=str(script))
    for item in parsed.body:
        if isinstance(item, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id == "CASES"
            for target in item.targets
        ):
            value = ast.literal_eval(item.value)
            if not isinstance(value, tuple) or not value or not all(
                isinstance(x, str) and x for x in value
            ):
                raise AssertionError(f"G4 子工具沒有真正固定正反 Rust 清單：{script.name}")
            return value
    raise AssertionError(f"G4 子工具缺少互異 Rust 案例：{script.name}")

def check_static_authority() -> None:
    core = (ROOT / "src/struct_stage.rs").read_text(encoding="utf-8")
    checker = (ROOT / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    required = (
        (core, "struct VerifiedFunctionExit {"),
        (core, "struct VerifiedControlEdge {"),
        (core, "struct VerifiedMethodSafety {"),
        (core, "struct VerifiedExitSnapshot {"),
        (core, "enum StaticCleanupNeed {"),
        (core, "fn control_edge_matches_ast("),
        (core, "fn control_edges_are_consistent("),
        (core, "fn method_safety_is_consistent("),
        (core, "fn exit_snapshots_are_consistent("),
        (core, "fn cleanup_from_bindings("),
        (core, "fn from_checked_scopes("),
        (core, "plan.control_edges = checker.recorded_control_edges.into_inner()"),
        (core, "plan.method_safety = checker.recorded_method_safety.into_inner()"),
        (core, "plan.exit_snapshots = checker.recorded_exit_snapshots.into_inner()"),
        (checker, "fn checked_outer_state_stability("),
        (checker, "fn record_checked_control_edge("),
        (checker, "fn record_checked_control_snapshot("),
        (checker, "fn method_writes("),
        (checker, "fn check_method("),
    )
    for source, marker in required:
        if source.count(marker) != 1:
            raise AssertionError(f"G4 單一 Checker 安全計畫事實缺失／重複：{marker}")
    for forbidden in (
        "original.value =", "original.pending_fields =", "fn statements_move_name(",
        "fn term_moves_name(", "fn block_moves_source(",
    ):
        if forbidden in checker:
            raise AssertionError(f"正式 Checker 殘留另一份可寫靜態權威：{forbidden}")
    if checker.count("Self::checked_outer_state_stability(") != 3:
        raise AssertionError("for／while／break continue 未使用同一外層靜態不變量")
    if checker.count("self.record_checked_control_snapshot(") != 2:
        raise AssertionError("return 和 break／continue 的完整 Scopes 證據未直接交付")

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("正式 G4 必須 --check")
    testbin = (ROOT / args.test_bin).resolve()
    if not testbin.is_file():
        raise AssertionError("缺少已編譯 Rust 測試執行檔")
    check_static_authority()
    g3_text = run([sys.executable, str(ROOT / "tools/核對G3正式驗收.py"),
        "--test-bin", str(testbin), "--check"])
    g3 = json.loads(g3_text.strip().splitlines()[-1])
    if g3.get("gate") != "P16.6-R03-G3" or g3.get("result") not in (
        "Passed", "PassedCandidate"
    ) or g3.get("unique_rust_tests") != 50 or g3.get("outstanding") != []:
        raise AssertionError("G4 相依的 G3 未有完整獨立真 Rust 安全證據")
    listing = run([str(testbin), "--list"])
    names = [line[:-6] for line in listing.splitlines() if line.endswith(": test")]
    used: set[str] = set()
    grouped: dict[str, int] = {}
    for group, scripts in GATES.items():
        count = 0
        for file in scripts:
            script = ROOT / "tools" / file
            cases = source_cases(script)
            for suffix in cases:
                matched = [name for name in names if name.endswith("::" + suffix)]
                if len(matched) != 1 or matched[0] in used:
                    raise AssertionError(f"{group} 真 Rust 測試缺失或重複：{suffix}")
                used.add(matched[0])
                count += 1
            # 不以子工具的文字聲稱代替真正執行；它們各自 --list/--exact。
            run([sys.executable, str(script), "--test-bin", str(testbin), "--check"])
        if count != 15:
            raise AssertionError(f"{group} 個別 G4 正反應各恰有 15 項，現為 {count}")
        grouped[group] = count
    for suffix in EXTRA:
        found = [name for name in names if name.endswith("::" + suffix)]
        if len(found) != 1 or found[0] in used:
            raise AssertionError(f"G4 CFG／方法／多資源真案例缺失或重複：{suffix}")
        used.add(found[0])
        output = run([str(testbin), "--exact", found[0], "--test-threads=1"])
        if "1 passed; 0 failed" not in output:
            raise AssertionError(f"G4 逐路控制流 --exact 未通過：{suffix}")
    if len(used) != 55:
        raise AssertionError(f"G4 須 55 個獨立真 Rust 正反，實際為 {len(used)}")
    head = run(["git", "rev-parse", "HEAD"]).strip()
    ghsha = os.environ.get("GITHUB_SHA", "")
    ref = os.environ.get("GITHUB_REF", "")
    if ghsha and ghsha != head:
        raise AssertionError("G4 不接受 checkout/GITHUB_SHA 不一致")
    main_verified = ref == "refs/heads/main" and ghsha == head
    record = {
        "gate": "P16.6-R03-G4",
        "github_sha": head,
        "github_ref": ref,
        "main_verified": main_verified,
        "result": "Passed" if main_verified else "PassedCandidate",
        "executed": ["F10", "F11", "G3"],
        "groups": grouped,
        "extra_tests": len(EXTRA),
        "unique_rust_tests": len(used),
        "outstanding": [],
        "g5_g6_in_scope": False,
    }
    print(json.dumps(record, ensure_ascii=False, sort_keys=True))
    return 0

if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, UnicodeError, ValueError, SyntaxError, KeyError) as error:
        print(f"G4 正式驗收拒絕：{error}", file=sys.stderr)
        sys.exit(1)
