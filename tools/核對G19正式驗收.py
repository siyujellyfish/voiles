"""以當次 rustc 測試執行檔逐項核對 P19。"""
import argparse
from pathlib import Path
import subprocess


CANDIDATES = "struct_stage::p19_candidate_execution::"
CONTAINERS = "struct_stage::p19_scoped_execution::"
LITERALS = "struct_stage::p19_literal_tests::"
STRINGS = "struct_stage::p17_private_str_execution_tests::"
UNICODE = "owned_str_runtime::graphemes::tests::"

CASES = {
    "P19.1": [
        CANDIDATES + name for name in (
            "arbitrary_candidate_order_and_interleaved_ordinary_branches",
            "named_alias_is_checked_after_direct_nonmissing_guard",
            "missing_candidate_and_missing_name_prioritize_name_resolution",
            "optional_element_expression_is_a_static_type_error",
            "nested_repeat_reports_only_the_redundant_check_and_still_executes",
            "repeat_after_local_statement_is_scoped_and_calls_invalidate_it",
            "candidate_chain_has_no_small_fixed_count_and_does_not_escape",
            "candidate_needs_same_name_immediately_after_proof",
        )
    ],
    "P19.2": [
        LITERALS + "full_checker_accepts_typed_literal_and_rejects_implicit_copy",
        CONTAINERS + "cross_container_copy_uses_the_whole_declared_set",
        CONTAINERS + "nested_container_move_and_missing_lookup_clean_every_owned_part",
        CONTAINERS + "moved_dictionary_owns_original_values",
        CONTAINERS + "readonly_owner_cannot_move_even_after_last_alias_use",
    ],
    "P19.3": [
        LITERALS + "declaration_drives_value_and_key_checks",
        CONTAINERS + "present_none_enters_body_and_absent_key_fails",
        CONTAINERS + "builtin_length_reads_list_and_dictionary_without_consuming",
        CONTAINERS + "composed_length_and_out_of_bounds_execute_from_source",
        CONTAINERS + "length_inside_readonly_scope_returns_only_its_own_ticket",
        CONTAINERS + "repeated_key_points_to_the_second_literal",
    ],
    "P19.4": [
        UNICODE + "unicode_15_1_grapheme_conformance",
        UNICODE + "changed_boundaries_and_independent_results",
        UNICODE + "invalid_indexes_and_replacements_are_atomic",
        STRINGS + "p19_dynamic_replacement_checks_before_commit",
        STRINGS + "p19_const_literal_bounds_use_the_same_unicode_rules_as_runtime",
        STRINGS + "standalone_write_releases_the_independent_result_and_resegments",
        STRINGS + "p19_unicode_literal_indexes_select_a_grapheme_or_byte",
        STRINGS + "p19_named_indices_follow_latest_value_and_bounds",
    ],
    "P19.5": [
        CONTAINERS + "繁中來源從解析到查找與區塊返回",
        CONTAINERS + "length_inside_readonly_scope_returns_only_its_own_ticket",
        CONTAINERS + "moved_dictionary_owns_original_values",
        CONTAINERS + "cross_container_copy_uses_the_whole_declared_set",
        STRINGS + "p19_checked_replacement_resegments_and_returns_independent_copy",
    ],
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    binary = parser.parse_args().test_bin.resolve()
    for phase, cases in CASES.items():
        for case in cases:
            run = subprocess.run([binary, "--exact", case], capture_output=True,
                                 encoding="utf-8", check=False)
            if run.returncode or f"test {case} ... ok" not in run.stdout.splitlines():
                raise SystemExit(f"{phase} 失敗 {case}\n{run.stdout}{run.stderr}")
        print(f"{phase} Passed ({len(cases)} 項)")
    print("G19 Passed (5/5)")


if __name__ == "__main__":
    main()
