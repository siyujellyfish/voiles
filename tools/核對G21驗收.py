"""在當次 rustc 測試執行檔逐名核對 P21 的六項義務。"""
import argparse
from pathlib import Path
import subprocess

CASES = {
    "P21-A": ["diamond_calls_initialize_one_shared_owner", "cycle_missing_file_and_private_import_preserve_paths"],
    "P21-B": ["missing_duplicate_and_lexical_diagnostics_preserve_importing_file", "checked_calls_reject_private_members_wrong_nominal_type_and_use_after_move"],
    "P21-C": ["imported_alias_cannot_write_or_hold_loan_over_remote_write", "moved_struct_reaches_imported_method_and_cleans_original_resource"],
    "P21-D": ["diamond_calls_initialize_one_shared_owner", "cycle_missing_file_and_private_import_preserve_paths"],
    "P21-E": ["changed_dependency_rejected_before_any_initialization", "missing_duplicate_and_lexical_diagnostics_preserve_importing_file"],
    "P21-F": ["qualified_constructor_overrides_public_fields_without_exposing_private_fields", "public_type_keeps_member_visibility_in_its_defining_module", "moved_struct_reaches_imported_method_and_cleans_original_resource", "checked_calls_reject_private_members_wrong_nominal_type_and_use_after_move"],
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    binary = parser.parse_args().test_bin.resolve()
    for phase, cases in CASES.items():
        for case in cases:
            name = f"struct_stage::module_share::modules::tests::{case}"
            result = subprocess.run([binary, "--exact", name], capture_output=True,
                                    encoding="utf-8", check=False)
            if result.returncode or f"test {name} ... ok" not in result.stdout.splitlines():
                raise SystemExit(f"{phase} 失敗 {name}\n{result.stdout}{result.stderr}")
        print(f"{phase} Passed ({len(cases)} 項)")
    print("G21 六項多檔驗收通過")


if __name__ == "__main__":
    main()
