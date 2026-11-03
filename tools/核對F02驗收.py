#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P16.6-R02/A2 F02 六子項：真 Rust 測試與唯一可寫語法權威稽核（唯讀）。"""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

# 在已編譯的真正 Rust --list 中找唯一全名；不得以空 filter 冒充通過。
GATES: dict[str, tuple[str, ...]] = {
    "F02-01": (
        "公私真正_int_bool_ast_直接持有同一_scalar_literal_無第二次組樹",
        "無後綴名稱公私使用同一半開跨度_後綴形式不能被預先消耗",
        "相同_utf8_節點跨度與游標只有成功後提交",
        "真正私有_ast_保存十六進位原文及_bool_語意",
    ),
    "F02-02": (
        "雙正式_parser_皆以_binaryint_與同一原始跨度記錄左結合加減",
        "三重加減巢狀括號直接比對真正_ast_運算子及_hir",
        "bool_及字串運算不得因共用型別檢查器偷渡合法化",
    ),
    "F02-03": (
        "真正括號來源保留相同半開位置及不同語法診斷",
        "三重加減巢狀括號直接比對真正_ast_運算子及_hir",
        "算術括號保留且不變更結合方向",
        "未核准私有資源類型不能成為公開純值解析授權",
    ),
    "F02-04": (
        "共同_ParsedNamedCall_由唯一_enum_宣告生成_保留不同模式引數權限",
        "私有函式呼叫直接保存共同_ast_名稱與引數_公開仍不能接受相同參數",
        "共同呼叫核心拒絕未核准參數且不吞掉原始診斷位置",
        "私有方法呼叫零個及多個位置引數保留順序與完整來源跨度",
    ),
    "F02-05": (
        "私有_name_空括號保留待解析且完整符號表裁決已宣告函式",
        "私有_name_空括號同名結構優先且具名結構仍使用_construct",
        "私有_name_空括號未知與錯誤引數數量維持既有精確診斷",
    ),
    "F02-06": (
        "公私唯一後綴詞彙分類保持原始_utf8_跨度與游標不變",
        "真正公私解析器沿用後綴分類但保留不相等的核准語法集合",
        "後綴符號錯誤不被分類器吃掉且維持原有私有安全診斷",
        "缺逗號或未封閉括號仍使用原來私有方法診斷",
    ),
}

# 文字稽核是第二獨立證據：僅證明已核准共同結點沒有重新
# 定義的第二份「可寫來源」，不取代語意測試／人工總驗收。
WRITERS: dict[str, tuple[str, ...]] = {
    "src/common_ir.rs": ("macro_rules! define_shared_expression_kind",),
    "src/syntax_shared.rs": (
        "pub(crate) fn parse_scalar_atom",
        "pub(crate) fn parse_bare_name",
        "pub(crate) fn fold_additive_spanned",
        "pub(crate) fn parse_named_call",
        "pub(crate) struct ParsedNamedCall",
        "pub(crate) enum NameSuffixHead",
        "pub(crate) fn name_suffix_head",
    ),
    "src/expr.rs": (
        "define_shared_expression_kind!",
        "crate::syntax_shared::parse_scalar_atom(",
        "crate::syntax_shared::parse_bare_name(",
        "crate::syntax_shared::fold_additive_spanned(",
        "crate::syntax_shared::parse_named_call(",
        "crate::syntax_shared::name_suffix_head(",
    ),
    "src/struct_stage.rs": ("define_shared_expression_kind!", "UnresolvedZeroArgs(String)"),
    "src/struct_stage/syntax.rs": (
        "crate::syntax_shared::parse_scalar_atom(",
        "crate::syntax_shared::parse_bare_name(",
        "crate::syntax_shared::fold_additive_spanned(",
        "crate::syntax_shared::parse_named_call(",
        "crate::syntax_shared::name_suffix_head(",
        "TermKind::UnresolvedZeroArgs(s)",
    ),
    "src/struct_stage/safety_checker.rs": ("fn resolve_empty_call(",),
}


def authoritative_source_check(root: Path) -> dict[str, int]:
    counts = {}
    for relative_path, markers in WRITERS.items():
        path = root / relative_path
        code = path.read_text(encoding="utf-8")
        for marker in markers:
            count = code.count(marker)
            if count < 1:
                raise RuntimeError(f"{relative_path}: 缺少真實共同權威 {marker!r}")
            # 源碼定義與跨模式 enum 展開必須唯一；呼叫者可以多次引用。
            if (
                marker.startswith(("pub(crate) fn ", "pub(crate) enum ", "pub(crate) struct ",
                                   "macro_rules! ", "fn resolve_empty_call("))
                or marker in ("define_shared_expression_kind!", "UnresolvedZeroArgs(String)",
                              "TermKind::UnresolvedZeroArgs(s)")
            ) and count != 1:
                raise RuntimeError(
                    f"{relative_path}: {marker!r} 應只有一份可寫來源，實際 {count}")
            counts[f"{relative_path}::{marker}"] = count
    checker = (root / "src/struct_stage/safety_checker.rs").read_text(encoding="utf-8")
    syntax = (root / "src/struct_stage/syntax.rs").read_text(encoding="utf-8")
    if "TermKind::Construct(name, args) if args.is_empty()" in checker:
        raise RuntimeError("Checker 不得保留將 Construct 空引數重新分類為 fn 的舊分支")
    if "TermKind::Construct(s, args), end" in syntax:
        raise RuntimeError("真 Parser 不得將所有 name() 直接預設成 Construct")
    return counts


def rust_run(binary: Path, args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(binary.resolve()), *args],
        text=True, encoding="utf-8", errors="strict",
        check=False, capture_output=True,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", required=True, type=Path)
    parser.add_argument("--repo-root", type=Path, default=Path("."))
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.check:
        parser.error("只允許 --check：禁止編輯既有 G0 快照或測試結果")
    if not args.test_bin.is_file():
        parser.error(f"沒有真正 Rust 測試執行檔：{args.test_bin}")
    writers = authoritative_source_check(args.repo_root)
    listing = rust_run(args.test_bin, ["--list"])
    if listing.returncode != 0:
        raise RuntimeError(f"Rust --list 失敗：{listing.stderr}")
    names = [line.removesuffix(": test")
             for line in listing.stdout.splitlines() if line.endswith(": test")]
    if not names:
        raise RuntimeError("Rust 真測試清單為空，拒絕假成功")
    unique = set()
    for item, suffixes in GATES.items():
        if not suffixes:
            raise RuntimeError(f"{item}: 測試集合為空")
        for suffix in suffixes:
            matches = [name for name in names if name.endswith("::" + suffix)]
            if len(matches) != 1:
                raise RuntimeError(
                    f"{item} 唯一性失敗：{suffix!r} 實際 {len(matches)} 項")
            unique.add(matches[0])
    for test in sorted(unique):
        result = rust_run(args.test_bin, ["--exact", test, "--test-threads=1"])
        if result.returncode != 0 or "1 passed; 0 failed" not in result.stdout:
            print(result.stdout, end="", file=sys.stderr)
            print(result.stderr, end="", file=sys.stderr)
            raise RuntimeError(f"實際 Rust 測試失敗：{test}")
    print(json.dumps({
        "gate": "P16.6-R02-A2-F02",
        "result": "Passed",
        "github_sha": os.environ.get("GITHUB_SHA", "local-unbound"),
        "subitems": {key: len(tests) for key, tests in GATES.items()},
        "unique_rust_tests_executed": len(unique),
        "authority_markers_checked": len(writers),
        "note": "此閘門不替代 G0 全四套或 A6 最新 main SHA 正式總驗收",
    }, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
