# voiles

**voiles** 是開發中的自製程式語言，目標為 Python 式易讀簡潔的編寫風格、Rust 式記憶體安全、直譯優先，為已支援的子集提供原生可執行產物及單檔封裝，以 voiles 自舉編譯器及 Rust 宿主建置工具鏈。

> **目前能力（2026-11-21）**：公開 CLI 保留 MVP-A 單檔靜態檢查，並提供模組子集的檢查、直譯、原生建置與單檔封裝命令。見 [建構階段](AI-build/建構階段.md)。

## 下載 0.1.0-preview.1

Linux x86-64 預覽發行包附有靜態 CLI、校驗碼、範例與繁體中文說明。[版本檔案與 SHA256SUMS](https://github.com/siyujellyfish/voiles/tree/releases/0.1.0-preview.1/dist) · [安裝與相容範圍](release/使用說明.md) · [變更紀錄](release/變更紀錄.md)

## 編寫風格與已支援的首版語法

voiles 借鑑 Python 的可讀性，但不複製 Python 全部語法或隱含型別規則。首版 MVP-A 使用 `fn`、`{}`、明確型別的 `var`／`const`、`int`／`bool`、一般 `if`／`else if`／`else`、無參數函式、明確 `return`、`int` 二元 `+`／`-` 與括號分組。

以下程式可用舊命令靜態檢查，也可用 `run` 執行，結果為 5：

```voiles
fn main()->int{
    const base:int = 2
    var total:int = 3
    return base + total
}
```

支援的宣告、初始化、名稱作用域、分支可達路徑、回傳型別及 `int64` 確定溢位均已具備真實 Rust 回歸測試；詳細支援／拒絕矩陣見 [P01 首版驗收契約](AI-build/P01-驗收契約.md)。首版不接受完整語言的 Optional／`struct`／容器等，不代表完整語言將永久禁止。

## 快速開始

使用 Rust stable、直接以 `rustc` 編譯標準函式庫核心，不要求 Cargo：

```sh
rustup toolchain install stable --profile minimal
mkdir -p target/p02
rustup run stable rustc --edition=2021 --crate-name voiles_core --crate-type rlib src/lib.rs -o target/p02/libvoiles_core.rlib
rustup run stable rustc --edition=2021 --crate-name voiles --extern voiles_core=target/p02/libvoiles_core.rlib -L dependency=target/p02 src/main.rs -o target/p02/voiles
./target/p02/voiles examples/入門/基本加法.voil
```

直接提供路徑會執行舊版單檔檢查，成功時安靜退出 0。以下命令使用同一個剛建置的 CLI：

```sh
./target/p02/voiles --help
./target/p02/voiles --version
./target/p02/voiles check examples/入門/基本加法.voil
./target/p02/voiles run examples/入門/基本加法.voil
./target/p02/voiles check examples/入門/多檔/main.voil
./target/p02/voiles run examples/入門/多檔/main.voil
```

兩次 `run` 分別輸出 `5` 與 `42`。多檔範例從入口同目錄的 `numbers.voil` 匯入 `value` 並命名為 `start`。搜尋根目錄取自入口路徑，依賴優先，循環匯入會產生診斷。

`check` 與 `run` 都先檢查模組，僅 `run` 會執行。成功時退出 0，`run` 將完整有號 64 位元回傳值寫至 stdout。讀檔、來源檢查與執行失敗退出 1，診斷寫至 stderr。新命令引數錯誤退出 2，舊式檢查的用法錯誤沿用 1。

`check` 與 `run` 目前支援 P21 已驗證的直線函式、整數運算、模組與受限資源操作，也支援兩側直接返回純 int 運算式的 bool 常值分支。舊版單檔檢查另支援 MVP-A 分支等文法。兩個入口的支援範圍不同。工具優先順序為命令說明與可執行範例，其後依序為格式化、編輯器支援與互動除錯。

## 建置與攜帶原生程式

在 Linux x86-64 GNU 開發機安裝 stable rustc、系統連結器及 glibc 靜態函式庫。依上方步驟建置 voiles，使用新檔名產生執行檔：

```sh
./target/p02/voiles build examples/入門/多檔/main.voil -o ./numbers
./target/p02/voiles pack examples/入門/多檔/main.voil -o ./numbers-portable
./numbers-portable
echo "$?"
```

範例的程序結束狀態為 42。僅需將 pack 產生的單一 ELF 複製到相容的 Linux x86-64 機器；來源、voiles、rustc、動態載入器與動態函式庫均無須隨附。原生程式以結果的低八位作退出碼，執行錯誤退出 1 並寫入 stderr。目的檔已存在時建置失敗並保留舊檔。

目前 main 的 check、run、build、pack 支援具名純量呼叫、控制流程、結構與資源移交、容器、Optional、字素索引與替換，以及有限 decimal64、字串格式化、具型別例外與直接 await。語法、相容決策及各項正反例見 [M7 公開語言施工](AI-build/M7公開語言施工.md)。

## 已驗收範圍

P00～P12 的單檔前端、P13～P16 的受限所有權與資源檢查、P17～P22 的直譯器及公開命令均已完成各階段驗收。P22 主線提交 `598a1e0c` 的核心測試為 3,327 項通過、零失敗、零忽略。P23～P27 的原生子集、封裝、限制及雙模式驗收見[原生工具鏈紀錄](AI-build/P23-P27原生工具鏈.md)。P28～P30 的自舉範圍、兩代重建與發行驗證記錄於 [M6 驗收](AI-build/P28-P30自舉與發行.md)。[主線 CI](https://github.com/siyujellyfish/voiles/actions/runs/44897419801)

可用目前 main 建置的 CLI 執行 UTF-8 統計範例。程式透過命令列取得檔案路徑，讀取 UTF-8，按字素走訪並計算非空格數量。執行方式為 `voiles run tests/m7/字素統計.voil -- 輸入.txt`。獨立產物直接接收相同的檔案引數。逐階段能力、專案結構與已知缺口見 [M4 實作範圍與程式碼審查](AI-build/M4實作範圍與程式碼審查.md)。

## 規格與工作依據

- [AI-build 權威索引](AI-build/README.md)：既有決策、適用範圍及文件閱讀順序。
- [新版 voiles 語言規格](AI-build/語言規格-v0.2.md)：歷史檔名保留，內容已依 2026-10-23 起的新版設計更新。
- [P01 首版驗收契約](AI-build/P01-驗收契約.md)：已驗收的 MVP-A 支援與拒絕矩陣。
- [建構階段](AI-build/建構階段.md)：P00～P30 相依、P10～P12 真實驗收證據（第五十一節）。
- [待確認事項](AI-build/待確認事項.md)：仍未核准與未實作的完整語言語意。
- [P16 工程總整理與效能稽核](AI-build/P16-總整理與程式碼稽核-20261030.md)：模組審查、性能測量方式、風險與後續重構優先序。
- [P16.6-R G6/F14 最終結案](AI-build/P16.6-R03-G6-F14-正式最終結案-20261104.md)：R02/R03 6/6、F00～F14 15/15。
- [歷史追加工程稽核](AI-build/P16.6-R-追加工程稽核結論-20261030.md)：10 月時點差距，非當前缺口。

儲存庫：[siyujellyfish/voiles](https://github.com/siyujellyfish/voiles)。
