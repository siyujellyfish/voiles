"""驗證 P03 結構規格阻塞清單，及產生的 Rust 測試與案例編號一致。"""

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIX = ROOT / "tests" / "fixtures"

def main() -> None:
    data = json.loads((FIX / "結構驗收追溯.json").read_text(encoding="utf-8"))
    tests = data["tests"]
    rules = [f"S{i:02d}" for i in range(1, 22)]
    assert len(tests) == 21
    assert [x["rule"] for x in tests] == rules
    assert all(x["expect"] == "blocked" for x in tests)
    assert len({x["id"] for x in tests}) == 21

    cases = json.loads((FIX / "清單.json").read_text(encoding="utf-8"))["cases"]
    generated = (ROOT / "target" / "p02" / "p03_spec_generated.rs").read_text(encoding="utf-8")
    for entry in cases:
        name = "規格_" + entry["id"].replace("-", "_")
        assert f"fn {name}()" in generated, name
        assert entry["expect"] in ("accept", "reject")
    assert generated.count('#[ignore = "P04～P12') == len(cases), "語意測試不得偷偷當作已實測"
    print(f"P03 追溯一致：{len(cases)} 個暫緩語意測試、21 個 struct S 規格阻塞項。")

if __name__ == "__main__":
    main()
