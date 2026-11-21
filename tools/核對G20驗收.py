"""在當次 rustc 測試執行檔逐名核對 P20 的五項義務。"""
import argparse
from pathlib import Path
import subprocess

CASES = {
    "P20-A": ["declarations_enforce_scope_type_and_initialization",
              "same_module_calls_observe_the_latest_value"],
    "P20-B": ["external_read_requires_visibility_and_external_write_is_rejected"],
    "P20-C": ["last_use_and_transitive_callee_effects", "alias_permissions",
              "await_respects_borrows_in_ancestors_and_last_use"],
    "P20-D": ["failed_rhs_preserves_storage_and_cleans_up",
              "exclusive_alias_and_unicode_replacement_use_the_original_owner"],
    "P20-E": ["same_module_calls_observe_the_latest_value",
              "external_read_requires_visibility_and_external_write_is_rejected",
              "failed_rhs_preserves_storage_and_cleans_up"],
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-bin", type=Path, required=True)
    binary = parser.parse_args().test_bin.resolve()
    for phase, cases in CASES.items():
        for case in cases:
            name = f"struct_stage::module_share::tests::{case}"
            result = subprocess.run([binary, "--exact", name], capture_output=True,
                                    encoding="utf-8", check=False)
            if result.returncode or f"test {name} ... ok" not in result.stdout.splitlines():
                raise SystemExit(f"{phase} 失敗 {name}\n{result.stdout}{result.stderr}")
        print(f"{phase} Passed ({len(cases)} 項)")
    print("G20 本機驗收 5/5")


if __name__ == "__main__":
    main()
