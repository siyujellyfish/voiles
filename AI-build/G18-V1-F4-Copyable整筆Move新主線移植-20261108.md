# G18／P18 V1-F4：純值結構整筆 Move 真執行及退出（2026-11-08）

> 本批從舊 #664 的未合併實作重新移植至最新已合併主線 `f57cee775880700f6d5d0e0b544e5d07395cce40`，該 SHA 必要三項 CI completed/success。已保留 #668/#670 新增的具名 int/bool 名稱與跨結構欄位動態來源，沒有用舊 Checker 路徑覆寫新工作。**只接通原 Checker 已允許的純 int／bool 欄位結構整筆 `.move()`，不是 F4 完整關閉，也不是 P18 正式 Passed。**

## 原始工程缺口

原 Checker 已對 `TermKind::Move` 及結構宣告簽發 `DeclarationId` 與 `direct_moves` 同型別轉移配對；P16 `MixedStructRuntime::move_whole` 已能真正消耗舊實例並交給新持有者，但 G18 普通函式 `prepare_function_locals` 只認結構新建式，對合法 `var second:Marker = first.move()` 錯回 `Unsupported`。

## 本批安全施工

1. 用原 Parser/Lexer 的真具名 `.move()`，由 Checker 唯一 `direct_moves(source,target)`、宣告型別、實例身份驗證移動配對。先限定結構所有欄位為 int/bool，不把原安全規格的 `str` 當 Copyable。
2. 從真 `PrivateScalarLocalStep` 及 `PrivateScalarBranchAction` 編出 `MoveCopyStruct`，只有動態工作真正抵達時，先核對目前 Frame 的活持有者 `DeclarationId` 與 `BorrowSafety`，再呼叫 P16 真 `move_whole`；來源在 P16 原位失效，目標接管同一份值。來源只能在合法新建後 Move，不提前求值。
3. 唯一同源稽核事件 `CopyStructMovedBound(source,target)` 不代表 `ResourceEvent`：更新 frame 的真局部建立／退出順序、對來源再次寫入保守拒絕及退出後無殘留移動責任。既有 P16 無字串欄位時 OwnedUtf8 事件必仍為零。
4. 不放寬 Lexer／Parser／Checker、沒有額外隱含複製、未授權一般化 struct 傳參、未改動公開 CLI、F00/G6/G17 與 V0 歷史清冊。

## 六項真正反

- 單次 whole Move，從目標讀真欄位，結果 7，僅一次建構、兩次局部離域、無 UTF-8。
- 兩次連鎖 whole Move，最後持有者讀 6，只有 2 次轉移及 3 次離域。
- 選中分支內 whole Move 提前 return，未選分支不執行，清理正確。
- 同來源第二次 whole Move 由原 Checker 靜態拒絕。
- 真已編譯 Move 的 `DeclarationId` 遭竄改，執行前 `SourceChanged` 拒絕。
- 含 str 欄位的合法整筆 Move，因尚無統一結構與函式的 `ResourceId` 帳而受限執行器 `Unsupported`，不假放行。

普通函式專項 Gate **376→382**，P18.1／P18.2／P18.4／P18.5 真來源追溯同批。最新 PR HEAD 及合併 main 同 SHA 三必要 CI 成功才認定此**子批**驗證完成。F1～F4 等價類、非 Copyable 結構真資源交接與原 P18 六項正式 0/6 Passed 均不因此改判。

## 最新主線整合說明

本分支不是將 PR #664 直接合併：舊 PR 以更早來源圖為基準，與已合併 #668／#670 的 `checked_copy_struct` 入口與 `PrivateScalarCopyStructSeed` 擴張不同。重新移植時 22 個原 Rust patch hunk 中 20 個可由實際上下文一一核對保留，另兩處合法結構宣告改採**先判原 Parser 真 `.move()`**，其餘仍交給新主線 `checked_copy_struct(...,bindings,verified)` 處理，避免退回無真局部/欄位來源能力。六項 Rust 真來源測試、P18.1/2/4/5 追溯及 Gate 重新對齊新主線 376→382。

真正可達但含 str 欄位的結構整筆 Move、跨函式結構持有者參數與資源票據共同帳仍未接通；舊主線沒有成功的新語意不得因移植而擅自放行。原 P18 正式 0/6 Passed、嚴格 `--check` 與 G17/F00/G6 凍結都維持。
