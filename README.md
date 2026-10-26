# voiles

**voiles** 是開發中的自製程式語言，目標為 Python 式易讀簡潔的編寫風格、Rust 式記憶體安全、直譯優先，後續再提供原生可執行產物及單檔封裝，長期以自舉為目標。

> **目前能力（2026-10-26）**：**P00～P12 的 MVP-A 單檔 Rust 靜態檢查前端已正式驗收**。可以實際檢查已支援的 `.voil` 程式是否符合首版語法與靜態規則，並回報中文診斷。**尚不能執行 voiles 程式，也不能產生可執行檔**；完整語言記憶體／所有權／借用安全、Optional、struct 等仍未完成。

## 編寫風格與已支援的首版語法

voiles 借鑑 Python 的可讀性，但不複製 Python 全部語法或隱含型別規則。首版 MVP-A 使用 `fn`、`{}`、明確型別的 `var`／`const`、`int`／`bool`、一般 `if`／`else if`／`else`、無參數函式、明確 `return`、`int` 二元 `+`／`-` 與括號分組。

以下程式**現在可以通過首版靜態檢查**，但不能執行：

```voiles
fn main()->int{
    const base:int = 2
    var total:int = 3
    return base + total
}
```

支援的宣告、初始化、名稱作用域、分支可達路徑、回傳型別及 `int64` 確定溢位均已具備真實 Rust 回歸測試；詳細支援／拒絕矩陣見 [P01 首版驗收契約](AI-build/P01-驗收契約.md)。首版不接受完整語言的 Optional／`struct`／容器等，不代表完整語言將永久禁止。

## 建置與使用靜態檢查命令

使用 Rust stable、直接以 `rustc` 編譯標準函式庫核心，不要求 Cargo：

```sh
rustup toolchain install stable --profile minimal
mkdir -p target/p02
rustup run stable rustc --edition=2021 --crate-name voiles_core --crate-type rlib src/lib.rs -o target/p02/libvoiles_core.rlib
rustup run stable rustc --edition=2021 --crate-name voiles --extern voiles_core=target/p02/libvoiles_core.rlib -L dependency=target/p02 src/main.rs -o target/p02/voiles
./target/p02/voiles tests/fixtures/voiles/P01-正01.voil
```

最後一步應**退出 0**且沒有阻斷診斷；換成 `tests/fixtures/voiles/P01-反08.voil` 時，應**退出 1**並於 stderr 輸出有來源位置的繁體中文靜態錯誤。命令只檢查一個 `.voil` 檔案，不執行程式、不生成產物，也不把 `main()` 回傳的 `int` 當作命令退出碼。

## 真正執行的自動測試

PR [#88](https://github.com/siyujellyfish/voiles/pull/88) 的 [GitHub Actions Rust CI](https://github.com/siyujellyfish/voiles/actions/runs/40258729876) 已驗證：**62 項 Rust 核心測試、5 項工程測試、49 項 P01 正反語意測試與 1 項素材測試全部通過，0 失敗／0 忽略**。必要文件 CI 亦已成功。這些數字代表真實執行結果，不只是文件與生成測試數量。

## 語言設計與後續階段

完整 voiles 語言仍規劃 `float`（decimal64）、`str`、容器、受限 Optional、`struct` 方法、資源複製／明確移動／借用、例外、非同步等。這些已核准的設計方向與未決細節**並未因 MVP-A 支援集合較小而刪除**。

| Phase | 內容 | 狀態 |
|---|---|---|
| P00～P12 | 儲存庫、語言契約、Rust 前端工程、來源／語法／名稱／型別／資料流及首版靜態檢查 | **MVP-A 已驗收** |
| P13～P16 | 資源複製、明確移動、借用及生命週期安全 | P13 已有真正 `.voil` 的內部 move 試驗，完整 Phase 均未驗收 |
| P17～P22 | 基於已選安全模型的 voiles 直譯執行 | 尚未驗收 |
| P23～P26 | 按 AR04 相依形成可執行編譯產物 | 尚未驗收 |
| P27～P30 | 發行、自舉及後續完整工作 | 尚未驗收 |

完整語言延伸驗收包括 P09～P12 各 F 系列的 Optional、struct／模組、字素、方法與安全義務，目前仍標為未實作；21 個 `struct` S 規格項也沒有被虛報為編譯器驗收通過。完整語言所有權安全尚待後續 Phase，不能從首版靜態檢查的 `Passed` 推論。

## 規格與工作依據

- [AI-build 權威索引](AI-build/README.md)：既有決策、適用範圍及文件閱讀順序。
- [新版 voiles 語言規格](AI-build/語言規格-v0.2.md)：歷史檔名保留，內容已依 2026-10-23 起的新版設計更新。
- [P01 首版驗收契約](AI-build/P01-驗收契約.md)：已驗收的 MVP-A 支援與拒絕矩陣。
- [建構階段](AI-build/建構階段.md)：P00～P30 相依、P10～P12 真實驗收證據（第五十一節）。
- [待確認事項](AI-build/待確認事項.md)：仍未核准與未實作的完整語言語意。

儲存庫：[siyujellyfish/voiles](https://github.com/siyujellyfish/voiles)。
