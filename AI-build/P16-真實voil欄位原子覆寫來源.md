# P16 真正 voil 結構 var str 欄位安全覆寫來源（2026-10-30）

> 以 VA16-03-A／S18／VA15 及最新 main 474a2263cd6c35e136ea6e3604beec9b8e7d21a7 為準，本批把真正 .voil 欄位寫入接上已驗證的持續 UTF-8 唯一資源模型；不推定未知 RHS 及任意方法賦值可安全執行。

## 來源與權限

- 之前 struct_stage.rs 的 FieldAssign 靜態入口一律禁止已存在非 Copyable str 欄位覆寫，與現有已核准 VA16-03-A 的「新右值先完成，成功後原子提交，失敗只清理新資源」不符。
- 現只放行型別為 str 且 RHS 確實為獨立字串常值的 var 結構 var 欄位安全覆寫；既有 HC14-A／VA15 借用衝突、S18 const 實例／欄位、已移動所有者及非 Copyable 具名隱含複製一律保持拒絕。
- 新增 test-only trace_persistent_struct_field_write_source：真正 Lexer／Parser／Checker 成功後，只接受 main 內單一結構建立、同來源 var str 欄位賦值、直接 return 0；實際由 OwnedStructRuntime.construct_with_access 與 assign_field_checked 產生真實 UTF-8 Created／Released 身分事件。
- RHS 故障注入表示在新 String 建立後、提交前；清理新暫存後舊值仍由原 owner 持有，所有事件最後必須逐 ID 平衡；已存在及首次初始化 var 皆證明清理責任。

## 九項真實來源

1. 完整正例：新 RHS 建立成功、舊值只在提交後釋放，最終逆序清理。
2. RHS 故障：僅釋放暫存，舊值仍可讀。
3. 無預設 var str 欄位首次初始化，不釋放不存在舊值。
4. const 結構實例拒絕覆寫。
5. const 欄位拒絕覆寫。
6. 非 Copyable 具名 RHS 隱含複製拒絕。
7. 借用來源與尚未驗證的額外控制路徑拒絕。
8. 不存在欄位及額外語句不能誤當已執行。
9. 尚無完整字串轉義執行器時拒絕直接拿原始詞彙充當 UTF-8 新資源。

## 驗收界線

- 此批增補 P16.1／.2／.3／.4／.5／.6 的真實來源證據，但尚未封閉混合 Copyable 與資源欄位、整體可複製結構、欄位間別名、任意控制流程與 P17；原始 P16 六項正式驗收矩陣保持不變。
- 最新 HEAD 三項必要 CI completed/success 後再核對 GitHub 身分、mergeable、最新 main 並 Squash merge。
