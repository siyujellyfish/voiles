# P16 真實 voil 結構整筆移動至持續 UTF-8 資源模型（2026-10-30）

> 基於 #257 已在最新主線完成的持續結構持有者模型，本批僅橋接可完整靜態證明的一條真正 .voil 直線路徑；不代表公開編譯／執行器、P16.1～P16.6 全量驗收或 P17 開始。

## 核准相依

- P13.3：明確整筆 .move() 後原來源永久失效；含 str 的結構受 S19 全欄位 Copyable 合取、S21 禁單一欄位移動約束。
- VB16-01-A：同一建構式先按具名引數來源順序求值，再按欄位宣告順序求值未被覆蓋預設；不計入已覆蓋預設值。
- VA16-01/02-A：真正 UTF-8 緩衝區唯一擁有者，成功建構後直到責任持有者退出作用域才清理；失敗按成功取得次序的相反順序清理。
- P14／P15：先完成既有私有來源 Lexer／Parser／Checker 安全檢查，不得由資源模擬掩蓋非法別名／逃逸。

## 實作與範圍

- src/struct_stage.rs 新增測試限定 trace_persistent_struct_move_source。真正 .voil 來源先經 UTF-8、Lexer、Parser 與現行 Checker；只接受主函式中「建立具名字串結構 → 將完整實例 .move() 至新宣告 → return 0」三語句，並要求所有欄位均為 str、預設及具名輸入均是未經轉義字串常值。
- 在全量來源靜態安全成功後，依同一 AST 定義建立 StringField 與 NamedString，委派 #257 的 OwnedStructRuntime.construct／move_whole／finish，逐資源比對 Created、Moved、Released 與實際持續存活值。
- 注入失敗只容許真正執行的具名／未覆蓋預設欄位，必須由 OwnedStructRuntime 回滾已配置資源；不得因不存在的切點或未求值 var 而虛報測試通過。
- 九項真實 .voil 正反：合法 UTF-8 中文與表情、具名覆蓋預設、不同行階段失敗回滾、無效注入點、禁止 str 結構隱含複製、重複移動、S21 單欄位移動、額外語句缺少執行器、未接通字串轉義。

## 尚未實作與正式驗收

- 此橋接只覆蓋真正 .voil 受限來源的單次結構建構與整體移動；跨函式整體結構返回、複數結構及區塊中的一般控制流程，仍需完整真實來源映射。
- P16.6 真正的多欄位混合 Copyable／非 Copyable 結構、原子覆寫與失敗回滾全量語意，尚需原始矩陣對照；不得僅靠九項來源即簽核任何 P16 原始項目。
- 最新 PR HEAD 三項必要 CI completed/success，並核對 GitHub App 身分、mergeable 與最新主線後才 Squash merge。
