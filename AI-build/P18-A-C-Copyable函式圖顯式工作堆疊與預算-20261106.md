# G18／P18.1、P18.3、P18.4｜已驗證 Copyable int 呼叫圖顯式執行堆疊與遞迴耗盡（2026-11-06）

> 基線 main f9eb22f298de92e7ea3c0dba6710c9833b786efe 三項必要 CI completed/success。沿用原私有 Lexer／Parser／Checker／TypedHIR、AW06 與使用者已核准 D18-01-A～D。**本批實作不改語言語法，不新增公開 CLI、固定語言遞迴深度或 OS exit code；原 G18 六項仍非全 Passed。**

## 對原驗收的直接缺口

現有原私有 Copyable int 函式執行器已處理一至三個參數、三層巢狀函式圖，但仍在 Prepare 階段拒絕四個以上函式及完整呼叫圖中任何環。這直接違反已核准 AW06/D18-01 允許合法直接／互相遞迴、不強制靜態證明終止的要求。其函式調用採 Rust 宿主遞迴，無法在大量動態真框架下證明受控錯誤與完整逆序退出。已有 bool／UTF-8 的另一個有限遞迴執行器，但不能替代已核准 Copyable int 一般普通函式的同路徑證據。

## 實作範圍

- 將 src/struct_stage/execute_call_scalar.rs 原 evaluator 的巢狀求值替換成明確的 expression 工作堆疊與獨立函式參數框架 Vec。依原 Checker 已驗證的 Call、Parameter、BinaryInt AST／TypedHIR 來源先左後右求值，真正進入被呼叫者才綁定當次 i64 參數槽；正常返回與整數溢位、預算耗盡均維持精確 FunctionEntered/Returned/FunctionExited 事件，子框架先逆序退出。不是模擬執行也不倚賴宿主 Rust 堆疊深度。
- 去除僅屬舊施工切片的「最多四函式、三參數、三層且禁止圖成環」限制。所有原 Checker 成功的非零數量 int 參數與單一純 int 返回／巢狀算式，依原 Parser 函式簽章、DeclarationId、P16 TypedMember、ScalarHIR 核對；原 Checker 拒絕的型別與借用仍絕不執行。
- 使用 PrivateScalarExecutionBudget {max_call_frames, max_steps}；數值只屬**本次 Rust 內部執行環境設定**，可由 execute_with_budget 覆蓋，與真正函式數量和步數綁定而非虛構語言上限。正常合法直接／互相遞迴可執行至環境預算耗盡，回報 CallBudget／StepBudget，禁止假成功 return，清除已進入全部動態參數槽。
- 將舊「遇循環圖必拒絕」過時負測改為真正可執行且受控耗盡的互相遞迴；額外七項真正反涵蓋合法直接遞迴、九層前向返回、五個 int 參數、步數耗盡、零預算零事件、未被選中遞迴函式不妨礙正向執行、兄弟兩呼叫各自框架。原 Test Gate 32→39，G18 原始六項中的 P18.1／P18.3 增列真來源追溯。

## 仍待正式驗收

此切片僅處理完整原 Checker 允許的純 Copyable int 單返回式呼叫圖，並不能替代一般條件／迴圈與混合 UTF-8/borrow 的所有運算，也沒有非同步方法調度取消語意。P18.1／P18.4 仍 SubsetVerified、P18.3 仍 Incomplete、P18.6 仍 BlockedByDecision，**G18 六原項不提前勾選**。要求 PR 最新 HEAD 完整 Rust／必要 CI completed/success、作者／base／mergeable 與 Squash merge 後新 main 同 SHA 再驗。


---

## V1-A：合法零參數巢狀函式圖與遞迴驗收子批（2026-11-06）

- 基線：V0 文件 PR #564 已 Squash merge 的 main ae6faee485a643a65aef7d8a75376bb1f1070d3c；原本純值呼叫圖仍在 Prepare 階段把所有**巢狀零參數函式**一律 Unsupported，雖然 AW06／P01 原 Checker 已允許，與 P18.1 的已核准合法函式呼叫不一致。
- 工程：移除對巢狀函式的舊 params.is_empty 人為限制，並依原 A2/F02 Parser 的 TermKind::UnresolvedZeroArgs 分類與正式 Checker 函式宣告，核對完整 int TypedHIR、零參數簽章及精確 Lexer 名稱／右括號詞彙來源後，才轉為原顯式工作堆疊 Call；遇 struct、借用或非 int 仍不准偷渡。保留 main 已核准參數函式、原 Checker 簽章、各函式唯一 DeclarationId 與可配置預算。
- 真實正反新增四項：零參數前向葉函式與帶參數父框架混合真返回、零參數葉函式兩次獨立動態呼叫與事件順序、零參數自遞迴 CallBudget 且每次 Entered 都有非成功 Exited、子函式參數宣告身份篡改必在配置動態框架前 SourceChanged。
- 專用真 Gate 由 39 項增至 43 項；G18 原六項的 P18.1、P18.3、P18.5 真來源追溯同步納入，不以子批綠燈宣告任何原項 Passed。
- 本批不是整個 V1 完成：仍待跨一般條件／迴圈／資源、多種合法根呼叫型態、複雜返回與 mixed parameters 的原始完整正反矩陣。未核准語意及 async 不變，公開 MVP-A CLI 亦不變。
- 驗收條件：PR 最新 HEAD 三項必要 CI completed/success，確認作者、main base、mergeable 後 Squash merge；在新 main 同 SHA 真重跑 G18 --audit、G17/G6/F00 回歸。後續 V1 批次應繼續拓展已核准合法來源，最後才可標 P18.1 Passed。
