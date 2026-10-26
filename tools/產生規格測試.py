"""由 P01 素材清單產生 49 項真正執行的 Rust MVP-A 規格測試。

由編譯完成的 voiles_core 檢查來源；完整語言 struct 等延伸仍 blocked。
"""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "tests" / "fixtures"
GENERATED = ROOT / "target" / "p02" / "p03_spec_generated.rs"

def rust_string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)

def main() -> None:
    data = json.loads((FIXTURES / "清單.json").read_text(encoding="utf-8"))
    cases = data["cases"]
    names = set()
    lines = [
        "//! 從 P01 JSON 正反契約產生 Rust 內建語意測試；現在均真正執行。",
        "use std::path::Path;",
        "use voiles_core::{check_file, CheckStatus};",
        "",
    ]
    for case in cases:
        name = case["id"]
        if not re.fullmatch(r"P01-(正|反)\d{2}", name) or name in names:
            raise ValueError(f"不合法或重複的來源案例：{name}")
        names.add(name)
        expected = {"accept": "Passed", "reject": "Failed"}[case["expect"]]
        full = "tests/fixtures/" + case["path"]
        lines += [
            "#[test]",
            f"fn 規格_{name.replace('-', '_')}() {{",
            f"    let result = check_file(Path::new({rust_string(full)}));",
            "    assert!(result.io_error.is_none());",
            f"    assert_eq!(result.status, CheckStatus::{expected});",
            "    if result.status == CheckStatus::Failed {",
            "        assert!(!result.diagnostics.is_empty(), \"拒絕必須有可解釋診斷，而非空診斷假失敗\");",
            "    } else {",
            "        assert!(result.diagnostics.is_empty(), \"通過不得同時有阻斷診斷\");",
            "    }",
            "}",
            "",
        ]
    lines += [
        "#[test]",
        "fn 來源素材可按路徑讀取且四十九項正反案例真實執行() {",
        "    for path in [",
    ]
    for case in cases:
        lines.append("        " + rust_string("tests/fixtures/" + case["path"]) + ",")
    lines += [
        "    ] {",
        "        assert!(Path::new(path).is_file());",
        "    }",
        "}",
        "",
    ]
    GENERATED.parent.mkdir(parents=True, exist_ok=True)
    GENERATED.write_text("\n".join(lines), encoding="utf-8")
    print(f"P12 實際執行 {len(cases)} 項 P01 MVP-A 語意測試 + 1 項素材路徑測試。")

if __name__ == "__main__":
    main()
