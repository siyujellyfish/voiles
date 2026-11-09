# G18／P18 V7 E2：私有 Checker 真 async await 的同執行緒協作讓出（2026-11-09）

> 基準：PR #706 已 Squash merge 的 `main 777d1ac937c07eef38ca0c0f9cf02cca8cb2a40e`；同 SHA 三必要 CI completed/success。工程範圍為 V7 E2-A 的**真私有來源動態安全點協作切換子集**。不等於能跨 Rust API 呼叫持久保存協程／多次 suspend-resume，不提前宣告 A1/A2 Closed 或 P18.6 Passed。G18 原六項正式 0／6 Passed。

## 已證工程缺口與這批目的

原 `src/struct_stage/execute_call_scalar.rs` 已能由同一私有 Lexer／Parser／Checker／TypedHIR 執行 `await async_fn(...)`，並由真正 `EnterCall` 發出 `AwaitSafePoint` 事件。然而程式在同一 Rust 呼叫中立刻建立 async 子框架，**沒有真正的協作執行權交付**。另一邊 `src/execution/async_task.rs` 的已合併分段等待／取消安全點仍只封存原 P01 純值 PreparedProgram，未能證明原私有來源的父函式帶 P16 真資源時可在 await 點暫時讓出，讓另一個獨立已核准工作先執行後恢復原圖。

本批不透過改寫來源字串、模擬假 Enter/Exit 或發第二份 ResourceId 事件過關；在原真函式圖工作堆疊中加入內部可選協作排程回呼，僅供已經完成 Checker 及動態安全核對的 await 邊使用。

## 具體程式改動

1. 對原 `PreparedPrivateScalarCall::evaluate_graph` 增加可選的 Rust 內部單執行緒回呼，並由 `execute_with_budget_and_fault_scheduler` 及 `execute_with_await_scheduler` 提供內部呼叫入口。既有 `execute`／`execute_with_budget`／故障注入路徑保留原行為，排程者不存在時無任何新的公開語意。
2. 在原 `PrivateScalarTask::EnterCall` 中，先比對唯一原 Checker async 函式簽章、參數型別與求值結果、資源移動有效性、呼叫預算及 E5 的 BorrowSafety／incoming_borrows；**只有真正 await** 在完成全部檢查之後、async 子函式尚未 `FunctionEntered` 之前，才可以呼叫內部排程者。
3. 排程者收到原 async 函式名稱及**當下唯一 OwnedStrRuntime 已真正產生的 ResourceEvent 快照**。它可在此時於同一 Rust 執行緒執行另一份原 Checker 核准工作；回傳後由原 Rust 顯式工作堆疊接續原 await 子函式與 P16 資源／票據退出。沒有新借用權威、第二份 CFG、背景派發、跨執行緒與函式內部的新返回語意。
4. `AwaitSafePoint` 仍由真正到達的 Call 發出，原 `has_balanced_frame_trace` 和 `audit_graph_outcome` 必須保持通過；同步 Call、沒有足夠框架預算及未觸及的來源分支均不得呼叫排程者。

## 真 Checker → Runtime 五項獨立正反

- **原 wrapper 真動態幀**：原私有 `wrapper()` 先建立 P16 UTF-8 str 資源，再在其 `await target(6)` 處將執行權交給另一份已核准 async 來源 `await side()` 真執行；回到原 target 得 7，原 wrapper 資源在讓出時仍 `Created`、最後唯一 `Released`，Frame／ControlEvent 雙帳一致。
- **兩個真 await 順序**：原 main 的左右兩個 async 算術呼叫，排程回呼只在各自實際到達時發生，順序 first→second、真結果 8。
- **動態資源預算負例**：子函式框架預算不足，尚未合法進 await，排程者零呼叫、零假安全點，沿用既有 CallBudget 失敗。
- **同步函式負例**：普通同步 Call 仍真執行與返回，但排程者零呼叫，不能冒稱 async。
- **已讓出後溢位**：真正 await 允許協作回呼，原 async 整數溢位後保留 IntegerOverflow、UnwindStarted、零成功返回、P16 原 ResourceId 唯一清理。

五項加入原普通函式清單 Gate **483→488**，且另以 `tools/核對G18-V7真Checker協作讓出.py` 逐項 `--exact` 真執行；CI 在原 P17/P18 全量、F00、G6、G17 和 G18 R07 均保留之下增加此一關卡。

## 未完成與嚴格界線

- **這批是一個 Rust 呼叫內的真單執行緒協作讓出／返回**：原父函式資源持續存活，回呼期間可完成獨立核准工作。尚未把 `PrivateScalarTask`、`PrivateScalarActiveFrame`、算術值棧、Checked CFG 轉為可持久封存的跨呼叫暫停工作物件；仍需要真正 `suspend` 返回調度器、未來 `wakeup` 再恢復的分段執行器。
- 沒有在此批實作已進入 async 函式本體後的第二個 await、多步數交錯、取消執行中的真私有幀、async self、跨函式 BorrowFact／ResourceId 完整調度對帳。已有 `DirectAwaitSession` P01 工作狀態與本批私有工作尚未共用單一可持久化排程佇列，不得虛稱全 V7 已完成。
- 既有 D18-02-E1=A 延遲開始直接 await、E2=A 單執行緒協作、E3=A 僅安全點取消、E4=A 原錯誤傳遞、E5=VB15-01-A 均不改。公開 MVP-A Lexer、CLI、F00/G6/G17 凍結與原六項狀態不變。
- V0 原 P18 26 條義務仍 `OpenEngineering`，P18.1～P18.6 正式 0／6 Passed，原 `tools/核對G18原六項驗收差距.py --check` 仍須保持非零。

## 合併與驗收程序

最新 PR HEAD「Rust 前端建置」「Rust 獨立架構測試」「專案檢查」均 completed/success，核對作者、base=main、mergeable 後 Squash merge；合併後新 main **同 SHA** 三必要成功才標本次子批已合併。下一批優先將原私有真 await 的動態工作狀態搬入可持久跨呼叫保存的單一排程器，明確保留父子框架與 P16／HC14 安全責任。
