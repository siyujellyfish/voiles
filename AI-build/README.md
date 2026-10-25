# AI-build｜voiles 專案工作索引

本資料夾是專案內部規劃與開發依據，對外介紹仍由根目錄 `README.md` 負責。所有文件使用繁體中文與 UTF-8 編碼。

## 閱讀順序

1. [專案規則.md](專案規則.md)：分支、合併、持續整合與文件更新規範。
2. [語言規格-v0.2.md](語言規格-v0.2.md)：**新版 voiles 語言的權威草案**，含 21 項 `struct` 決議、D01–D13（D11 經 Y/Z/AA/AB/AC/AD/AE/AF/AG/AH/AI/AJ/AK/AL/AM/AN/AO/AP/AQ 逐步細化 Optional；AQ01～AQ06 已定候選檢查順序自由、一般條件分支可在 `is` 前中後、非具名合法 Optional 表達式左值為靜態型別錯誤、唯一候選重檢產生非阻斷警告、同鏈無固定次數上限，以及左未宣告且右缺名稱時優先名稱解析錯誤；既有 AO05／AL02 及其他未選語意不變）、E01–E07、F01–F06（F01 修訂 D09、F04 已由 H01 決定）、H01–H06 六項決策、I01–I06（I04 已由 T06 裁決）、J01–J06、K01–K06、L01–L06、M01–M06、N01–N06、O01–O06、Q01–Q06、T01–T06、U01–U06、V01–V05、W01–W06、X01–X06、Y01–Y06、Z01–Z06、AA01–AA06、AB01–AB06、AC01–AC06、AD01–AD06、AE01–AE06、AF01–AF06、AG01–AG06、AH01–AH06、AI01–AI06、AJ01–AJ06、AK01–AK06、AL01–AL06、AM01–AM06、AN01–AN06、AO01–AO06、AP01–AP06、AQ01–AQ06 Optional 容器位置／索引型別／Unicode 版本及詞彙／Unicode／字串、多行註解與運算限制決策（Q01/Q02 已補定 Unicode 前導零及無效碼位替換；Q05/Q06 限制 str 比較／乘法）及 V06 已由 W/X 補定一般變數寫入權限、第二層不可賦值與模組級 share str 同模組修改（第一層 UTF-8 安全仍待決）、完整性盤點。檔名為歷史沿用，內容不再是舊 v0.2，也尚非封閉的實作規格；2026-10-23 新決策優先於舊 PR #4 與舊範例。
3. [待確認事項.md](待確認事項.md)：新版未決細節與 **R01–R07 語言規格缺口**；AR 訂定前端先行與原 Phase 順序，AS01～AS06 補定 int／bool、const／var、一般 fn、if／else 及 Rust；AT01～AT06 已限定首版頂層／型別／條件，AU01～AU06 已細化 fn／return／條件／初始化；AV01～AV06 已補首版算式／函式呼叫與 Linux CI，AW01～AW06 已補算術分組／頂層變數與 AST／遞迴；AX01～AX06 已補首版函式值加減、初始化、入口與核心依賴；AY01～AY06 已補分支 return、var 再賦值、main 與來源解碼；AZ01～AZ06 已補來源／CLI／rustc 方向；BA01～BA06 已補 CR 換行／逐非法位元組詞彙錯誤及 CLI／測試方向；BB01～BB06 已補來源跨度、非法來源標記及 CLI／Rust 程式庫測試；BC01～BC06 已補來源偏移與 CLI 錯誤／單一測試介面；BD01～BD06 已補來源端點／語法復原與 CI 環境；BE01～BE06 已補非空 EOF、獨立診斷、原始合法 U+FFFD 與核心檢查；BF01～BF06 已補特殊 EOF／詞彙診斷與 CI；BG01～BG06 已補原始來源字元復原、CRLF 終點與 Rust 核心形式；BH01～BH06 已補 CRLF 中間位置／AST 錯誤處置及 Rust 介面方向；BI01～BI06 已補 Rust 結果名稱／診斷文字與未來前端 PR CI；BJ01～BJ06 再核准 `CheckStatus` 變體 `Passed`／`Failed`、`CheckResult.status`、診斷項目具名 `Diagnostic` 結構體、其來源範圍採已換算的一基行／原始 UTF-8 位元組欄半開座標、`CheckResult.io_error` 仍為 `Option<std::io::ErrorKind>`，及未來 PR CI 分成相依的建置與測試工作（產物跨工作傳遞未決）。**建構階段的 G01–G06 則是不同的實作決策關卡。**
4. [建構階段.md](建構階段.md)：工作階段、依賴、`struct` S01–S21 的 Phase／驗收對照，以及依 AR01～AR06、AS01～AS06、AT01～AT06、AU01～AU06、AV01～AV06、AW01～AW06、AX01～AX06、AY01～AY06、AZ01～AZ06、BA01～BA06、BB01～BB06、BC01～BC06、BD01～BD06、BE01～BE06、BF01～BF06、BG01～BG06、BH01～BH06、BI01～BI06、BJ01～BJ06、BK02／03／04／06 更新估算的最小前端與後續產物所需決策；語言語意一律以新版規格為優先。
5. [工作紀錄.md](工作紀錄.md)：最近的確定決策、修改與驗證狀態。

## 維護原則

- 每次開始工作，先查閱儲存庫現行版本，勿以對話歷史猜測。
- 每次規劃或實作改動，同步修訂現有 `AI-build/` 文件；必要變更精簡記錄於工作紀錄。
- **舊版語言規格已被新版取代**；舊 PR #4 已標示過時並關閉，原始資料僅供歷史追溯。新版已確認事項與未確認事項必須嚴格區分。
- 未確認的技術選型、語意、模組邊界、資源安全條件不得自行確定。
- 工作先開新分支，以合併請求與必要檢查驗證後用 Squash merge 合入 `main`。
- 下一批**非純命名**的語言／前端規格決策選單須以**可直接點選的互動選項**呈現，分題獨立選 A／B、附簡短差異說明；允許部分選擇，未選保持未決。
- **2026-10-25 新增純命名委任**：使用者允許開發助手自行採用最適合 voiles 的純識別名稱，不再詢問名稱 A／B；本次已據此選定 `CheckResult.diagnostics` 診斷集合欄位（沿用已核准的 `Vec<Diagnostic>` 方向）。公開位置、資料型別、CI 行為或其他非命名選擇仍須使用者核准；詳見 [專案規則.md](專案規則.md) 第九節。 **BK 技術裁決**：已核准 BK02 診斷類別專用列舉（純命名 `DiagnosticKind`）、BK03 訊息 `Box<str>`、BK04 來源起終兩個位置結構體、BK06 相依 PR CI 測試工作自行重編 `rlib`；原 BK05 公開根層／模組層級問題已由 BL01-A 裁決為 `voiles_core` 根層。
- 目前仍是語言設計階段，沒有可用的直譯器或編譯器。

## BL01～BL06 已核准 Rust 前端最小契約（2026-10-25）

- **BL01-A**：`voiles_core::check_file` 直接位於核心程式庫根層；沿用 `&std::path::Path` 參數、核心自行讀檔、僅一項測試可呼叫檢查操作。不決定其他公開項目及穩定性。
- **BL02-A**：已換算診斷來源位置的起點、終點，一基行號／原始 UTF-8 位元組欄位使用 Rust `usize`；原始字元追蹤仍保留零基原始位元組半開跨度，CRLF／EOF 已選規則不變。
- **BL03-B**：`Diagnostic` 的已換算起終來源範圍採 `Option<來源範圍>`，容許沒有來源位置；但可定位語言診斷仍須提供合法位置，無合法行欄的用法／IO 錯誤不得捏造位置。不決定各類別的 `None` 適用條件。
- **BL04-A**：`DiagnosticKind` 採無附帶資料的 Rust 單元列舉變體；具體分類、變體名稱／數量、顯示文字仍未決。
- **BL05-A**：`check_file(&Path)` 直接回傳 `CheckResult`，不加外層 `Result`。保留 `status: CheckStatus`、`diagnostics: Vec<Diagnostic>`、`io_error: Option<std::io::ErrorKind>`；IO 失敗仍須為 `Failed`。其他完整 API 不推定。
- **BL06-A**：`CheckStatus::Passed` 與 `CheckStatus::Failed` 均為無附帶資料的 Rust 單元變體；不決定衍生 trait、成功判斷演算法或所有權安全。

**治理界線**：只同步使用者已選 BL01～BL06。診斷訊息依 BK03 為 `Box<str>`、已換算範圍依 BK04 由兩個位置結構體組成；Rust 核心、AST／詞彙語法復原、完整 rustc 指令、runner 實際標籤、CLI 模板、診斷變體清單／型別可見性及所有權安全仍未實作／未決。AR04 原 Phase 依賴不變；P01～P03 未驗收；最新 PR HEAD 必要文件 CI 成功後才允許 Squash merge。
