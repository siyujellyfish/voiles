"""由 P01 素材清單產生 Rust 規格測試；尚未支援的語意明確標示忽略。

此程式不依賴第三方套件，也不檢查任何 voiles 語法。
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
        "//! 由素材產生的 Rust 內建測試。被忽略的語意案例不能計作成功。",
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
            '#[ignore = "P04～P12 解析與靜態檢查尚未具備，不得虛報語意通過"]',
            f"fn 規格_{name.replace('-', '_')}() {{",
            f"    let result = check_file(Path::new({rust_string(full)}));",
            "    assert!(result.io_error.is_none());",
            f"    assert_eq!(result.status, CheckStatus::{expected});",
            "}",
            "",
        ]
    lines += [
        "#[test]",
        "fn 來源素材框架可按路徑讀取且不把結果誤作語法驗收() {",
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
    print(f"P03 產生 {len(cases)} 項暫時忽略的語意測試 + 1 項素材路徑測試。")

if __name__ == "__main__":
    main()
