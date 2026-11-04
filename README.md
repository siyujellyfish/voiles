# voiles

**voiles** 是開發中的自製程式語言，目標為 Python 式易讀簡潔的編寫風格、Rust 式記憶體安全、直譯優先，後續再提供原生可執行產物及單檔封裝，長期以自舉為目標。

> **目前能力（2026-11-04）**：公開 MVP-A P00～P12 單檔靜態檢查器已驗收；P13～P16 原始私有受限安全 24/24、P16.6-R **6/6**、F00～F14 **15/15** 全部在最新 main 正式通過。**公開 CLI 仍不執行程式、不產生可執行檔；P17～P30 未驗收。** 見 [全專案統整](AI-build/專案全域統整-20261104.md)。

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

2026-11-04 真正 main `16f9fa9a2c76cd3a8acccda77d908d88c3b04ab4`：Rust 核心 **1,070 通過、0 失敗、0 忽略**，三項必要 CI 成功；F00～F14 **15/15**、R02/R03 **6/6**，G6 `Passed/main_verified=true/outstanding=[]`。

G0 四組凍結：P01 49 來源／30 診斷、原 P13～P16 24 項／157 選取、私有 113 來源／63 預期拒絕、受限 P16 八橋接 15 路徑／57 真 ResourceEvent。G3 50、G4 55、G5 50 真 Rust 正反測試均保留。整理 CI 僅刪除 **32 個重複執行步驟**，不刪 Rust 測試或保守安全負例；詳見 [全專案統整](AI-build/專案全域統整-20261104.md)、[測試清理](AI-build/P16.6-R-結案後測試清理-20261104.md)及 [GitHub Actions](https://github.com/siyujellyfish/voiles/actions)。上述測試只證明核准範圍，不代表 P17 直譯器已完成。

## 語言設計與後續階段

完整 voiles 語言仍規劃 `float`（decimal64）、`str`、容器、受限 Optional、`struct` 方法、資源複製／明確移動／借用、例外、非同步等。這些已核准的設計方向與未決細節**並未因 MVP-A 支援集合較小而刪除**。

| Phase | 內容 | 狀態 |
|---|---|---|
| P00～P12 | 儲存庫、語言契約、Rust 前端工程、來源／語法／名稱／型別／資料流及首版靜態檢查 | **MVP-A 已驗收** |
| P13～P16 | 資源複製、明確移動、借用、有效期與 UTF-8 資源責任 | **內部受限子集 24／24 正式工程驗收完成；尚未公開** |
| P17～P22 | 基於已選安全模型的 voiles 直譯執行 | 尚未驗收 |
| P23～P26 | 按 AR04 相依形成可執行編譯產物 | 尚未驗收 |
| P27～P30 | 發行、自舉及後續完整工作 | 尚未驗收 |

完整語言延伸驗收包括 P09～P12 各 F 系列的 Optional、struct／模組、字素、方法與安全義務，目前仍標為未實作；21 個 `struct` S 規格項也沒有被虛報為編譯器驗收通過。不能從首版靜態檢查的 `Passed` 推論 P13～P16 私有安全分析已套用於該來源；P17 之前，公開命令與私有驗收仍是不同入口。

## 規格與工作依據

- [AI-build 權威索引](AI-build/README.md)：既有決策、適用範圍及文件閱讀順序。
- [新版 voiles 語言規格](AI-build/語言規格-v0.2.md)：歷史檔名保留，內容已依 2026-10-23 起的新版設計更新。
- [P01 首版驗收契約](AI-build/P01-驗收契約.md)：已驗收的 MVP-A 支援與拒絕矩陣。
- [建構階段](AI-build/建構階段.md)：P00～P30 相依、P10～P12 真實驗收證據（第五十一節）。
- [待確認事項](AI-build/待確認事項.md)：仍未核准與未實作的完整語言語意。
- [P16 工程總整理與效能稽核](AI-build/P16-總整理與程式碼稽核-20261030.md)：模組審查、性能測量方式、風險與後續重構優先序。
- [全專案統整](AI-build/專案全域統整-20261104.md)：Phase 狀態、模組責任、文檔與測試治理。
- [P16.6-R G6/F14 最終結案](AI-build/P16.6-R03-G6-F14-正式最終結案-20261104.md)：R02/R03 6/6、F00～F14 15/15。
- [歷史追加工程稽核](AI-build/P16.6-R-追加工程稽核結論-20261030.md)：10 月時點差距，非當前缺口。

儲存庫：[siyujellyfish/voiles](https://github.com/siyujellyfish/voiles)。
