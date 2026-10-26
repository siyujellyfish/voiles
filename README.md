# voiles

**voiles** 是規劃中的自製程式語言，目標為**Python 式易讀簡潔的編寫風格、Rust 式記憶體安全、直譯優先**，後續提供編譯與單檔封裝能力，長期以自舉為目標。

> **現況（2026-10-26，台灣時間）**：P00～P08 的首版範圍已驗收；P09 的 MVP-A 名稱解析已完成正式驗收（PR #84）；完整語言 Optional／struct 與模組仍列為獨立未完成延伸項。現有 Rust 命令與核心**可實際讀取來源、詞彙／語法解析、建立內部 AST、檢查部分名稱錯誤**；但尚未完成 P10～P12 型別、初始化與函式靜態檢查，因此**仍不能將合法 voiles 程式判為檢查通過，也沒有直譯器或可執行編譯產物**。

## 語言的編寫風格

voiles 取法 Python 的可讀性，但**不複製 Python 的全部語法或隱含型別規則**。已核准的語言外觀使用 `fn`、`struct`、`{}` 語句區塊、`var`／`const`／`share` 和明確型別標註；其安全模型以明確複製、移動、借用與靜態檢查為方向。

以下是**第一個單檔靜態前端的預期支援子集**之語法示意，**不是現在可執行的程式**：

```voiles
fn main()->int{
    const base:int = 2
    var total:int = 3
    return base + total
}
```

上述形式承接已核准的首版無參數 `fn`、`main()->int`、明確型別宣告、合法二元加法及尾端 `return`。本專案**不宣稱**已實作求值、編譯或程式退出碼映射。

## 已核准的完整語言設計方向

- **基本型別**：`int` 為有號 64 位元且不允許溢位靜默回繞；`float` 採 IEEE 754 `decimal64`、十進位有效數字與已核准捨入模式；另有 `bool`、`str`、`none`。
- **宣告、結構與函式**：明確的 `var`、`const`、`share` 型別標註；`struct` 在同一區塊宣告欄位與方法；方法接收者 `self` 自動提供且僅能借用；函式、方法及結構的既定 `_` 前綴模組可見性。
- **字串與 Unicode**：`str` 第一層索引使用 Unicode 字素叢集，第二層存取相應 UTF-8 位元組；第一層索引讀取／已核准的寫入運算式回傳採獨立複製 `str`。完整語言已選的字串運算限制不得按 Python 慣例自行補成合法操作。
- **容器與 Optional**：已規劃 `list[T]`、`dict{K,V}`；`Optional` 限用於已核准的容器元素／字典值位置，具有缺失比較及專用候選型別條件判斷規則。這些**不是**首個 Rust 前端的全部支援項目。
- **控制流程與安全方向**：一般 `if`／`else if`／`else`；完整語言亦規劃迴圈、例外與非同步。`copy`、`.move()`、`.borrow()` 的靜態安全及資源生命週期仍需依已有決策繼續收斂，**尚不能宣稱完整實作或驗證**。

所有尚未確定的語法、容器 API、所有權／生命週期與執行期行為都以內部權威規格中的「待確認」為準，**不因上面列出設計方向就視為首版已支援**。

## 編譯器里程碑

| 里程碑 | 已核准目標 | 狀態 |
|---|---|---|
| 首個單檔 Rust 前端 | 解析 `.voil`、建立內部 AST、來源診斷與指定子集靜態檢查；不執行、不輸出可執行檔 | P03～P08 首版詞彙／語法／AST 已驗收；P09 MVP-A 名稱解析與來源身分驗收已成功；P10～P12 靜態檢查未完成 |
| 記憶體安全與直譯器 | 完成既有所有權／借用／生命週期安全依賴後提供直譯執行 | 尚未實作 |
| 日後可執行編譯產物 | 先滿足前述安全與直譯器依賴，再轉譯至可編譯表示、交既有編譯器產生產物 | 尚未實作 |

依正式 Phase 表，**P00～P09 首版範圍已驗收；完整語言 P09-F01／F02 及 P10～P30 尚未實作／驗收**。新增 Rust CI 測試工程可建置與 IO／CLI 骨架，**不等於** voiles 語法或語意已被驗證。

## Rust 前端工程骨架（P02）

使用 Rust stable、直接以 `rustc` 編譯，不以 Cargo 為首版必要工具：

```sh
rustup toolchain install stable --profile minimal
mkdir -p target/p02
rustup run stable rustc --edition=2021 --crate-name voiles_core --crate-type rlib src/lib.rs -o target/p02/libvoiles_core.rlib
rustup run stable rustc --edition=2021 --crate-name voiles --extern voiles_core=target/p02/libvoiles_core.rlib -L dependency=target/p02 src/main.rs -o target/p02/voiles
./target/p02/voiles tests/fixtures/voiles/P01-正01.voil
```

最後一個命令**目前預期退出 1 並向 stderr 說明前端尚未實作**；這不代表該正向素材的語法非法。Rust 架構測試及相依 CI 的完整指令見 `AI-build/建構階段.md` 第三十六節。

最新 Rust CI 驗證（PR #84）：**43 項核心測試、5 項架構測試與 1 項測試素材框架測試通過；49 個完整語意案例仍暫緩**。詳細進度見 `AI-build/建構階段.md` 第四十五節。

## 規格與開發文件

- [AI-build 專案索引](AI-build/README.md)：文件閱讀順序、已核准批次、版本效力及目前工程狀態。
- [新版 voiles 語言規格](AI-build/語言規格-v0.2.md)：完整語言已核准的正式草案及前端決策。檔名保留歷史，但內容自 **2026-10-23 起取代舊版 v0.2**。
- [待確認事項](AI-build/待確認事項.md)：現行仍未封閉的語意、工具鏈與驗收關卡。
- [建構階段](AI-build/建構階段.md)：P00～P30 相依與 MVP 進度重估。

## 設計變更的界線

**相較 2026-10-23 新版草案，至本次文件稽核未發現未經核准而改變 voiles 的高階設計風格。** 但 2026-10-23 **確實曾明確以新版草案取代舊語法設計**，所以不能說從專案創立至今從未改動。此處核對的是文件決策與設計方向，不是尚未存在的編譯器實際行為。

儲存庫：[siyujellyfish/voiles](https://github.com/siyujellyfish/voiles)。
