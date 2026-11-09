# G18／P18 V7 E2／E3／E5：真私有 Checker 動態工作圖單執行緒 FIFO 排程（2026-11-09）

> 工程起點：已合併的 `main 9c8e63cdfe687cfa0339548046cdf8be980cce1e`，同一 SHA「Rust 前端建置」「Rust 獨立架構測試」「專案檢查」必要 CI 均完成成功。這批是已核准 E2-A（單執行緒協作排程）在**真私有已執行工作圖**上的內部多工作排程子集。正式 P18.1～P18.6 維持 **0/6 Passed**，V0 原 26 條工程義務仍依 R07 保留 OpenEngineering，不提前結案。

## 工程缺口與設計範圍

PR #710～#712 已讓原 Checker 的一般 main 函式圖、單純直接 main→wrapper 快路，真正執行到 await 後將 PrivateScalarTask、PrivateScalarActiveFrame、P16 OwnedStrRuntime、HC14 BorrowSafety、CopyStruct、資源事件與步數存進 PrivateScalarGraphCheckpoint；其他 Rust 呼叫可恢復同一原始工作位置，合法安全點可真取消及逐框架退出。但原本只能由測試或呼叫者手動持有一個 PrivateScalarGraphContinuation，欠缺**多工作身分、可執行 FIFO、醒來事件對帳、取消正在待執行佇列中的真框架**。

在同一 src/struct_stage/execute_call_scalar.rs 新增僅供內部使用的 PrivateScalarCooperativeQueue<'a>，以 BTreeMap<工作ID, 真續行物件> 與 VecDeque<工作ID> 表示待執行順序；每一個工作擁有獨立 PrivateScalarGraphContinuation<'a>，其仍引用**呼叫者長期持有的原 PreparedPrivateScalarCall**，避免 unsafe 自我參照和無效計畫身分。本批沒有聲稱是完全持有原 Checker 來源的單一對外工作 session，也沒有開放 spawn、背景工作、跨執行緒、互斥或非核准非同步 API。

## 真執行與安全不變式

1. submit(&PreparedPrivateScalarCall)：先使用原 PrivateScalarGraphContinuation::new 對原 Checker 快路／動態圖核對；成功後才配發單調工作 ID、保存唯一真正原工作計畫及加入 FIFO。ID 耗盡時拒絕，不能重複或回繞，也不消耗既有工作。
2. drive_one()：**由呼叫端顯式驅動**，每次只執行 FIFO 頭的一份已核准工作，至下一個真正 await 安全點或原 Runtime 正式完成。尚未成功 poll 前不彈出 FIFO；每次真成功交付才記錄 (工作 ID, Suspended/Completed)。原前置資源、函式事件與運算值均由唯一原 evaluate_graph_driver 執行，不造第二份 AST／ResourceId／借用權威。到達 await 的工作**不自動**重新排入 FIFO，下一個工作可先執行。
3. wake(id)：僅容許原 continuation 真正 Suspended、具有唯一待處理 async 呼叫，且尚未在 FIFO 的工作。醒來仍只有「排入 FIFO」，不會在呼叫背景執行，雙重 wake 不可產生第二份待消耗 token。
4. E5 重新守門：wake(id) 及後續 drive_one() **分別**使用已保存真 checkpoint 的所有祖先框架 BorrowSafety／incoming BorrowFact 進行完整安全核對；即使在醒來與正式排程之間又取得票據，未歸還時也必須原子拒絕，不得 pop_front、改變工作身分、消耗續行或提前釋放原 ResourceId。
5. cancel(id)：尚未開始的 Prepared 直接取消，零真正 Runtime event。已 Suspended 的真工作一律委派現有 continuation.cancel_suspended，使用原 R05/HC14/P16 動態退出核心及雙帳檢查；若 wake 已排隊，**只有真取消成功後**才從 FIFO 移除並記錄 cancellation_order。有效借用拒絕時保留原 FIFO 位置、原 checkpoint 和 ResourceId，歸還票據後才准取消。
6. 原 Checker 驗證拒絕／來源一致性異常不可以假裝真 poll 成功；能安全重試或取消的 Prepared／Suspended 工作保留 FIFO。若底層已進入但稽核失敗則不重排該工作，保守 Failed。
7. 工作結果與取消證據只能讀取原 PrivateScalarCallOutcome、PrivateScalarGraphCancellation 的真正 Trace 與 P16 ResourceEvent；不能從排程器自行產生成功返回或 ResourceReleased。沒有任何自動計時器、緒程、作業系統背景派發。

## 八項真 Parser／Checker→Runtime 正反

獨立 `tools/核對G18-V7真工作圖協作排程.py` 在現有 Rust 內部測試執行檔以 `--exact` 逐筆執行：

- 三份真工作按 FIFO 前兩份抵達合法 await、第三份同步完成；隨後由呼叫端先醒第二份再醒第一份，按新 FIFO 真正完成；第一份已建立的 UTF-8 原 ResourceId 暫停期間維持存活，最後唯一釋放。
- 同一工作可在 first→second 兩個不同真 await 指令位置反覆 yield；另一份核准工作可插入並先完成，前一 await 不重執行。
- 真父框架已建 UTF-8、醒來但尚在 FIFO 的工作可安全取消；當下按原 P16 清理原 ID、刪除那一份待執行 token，不會 Enter 未啟動的 async 子函式或影響其它佇列工作。
- 暫停後新增真祖先 BorrowTicket 時 wake 拒絕；歸還後 wake 成功，再於真正 drive 之前新增有效票據時仍須拒絕且 FIFO、資源與 checkpoint 不動；再歸票才能恢復。
- 已在 FIFO 的真父資源工作若有活 BorrowTicket，取消拒絕而保持位置／資源，歸還後才可真取消釋放。
- 普通同步工作不具有待醒 await，不能 wake；重複 wake 禁止、不能重複入 FIFO。
- Prepared 尚未開始的工作取消不產生假資源／函式事件，未知工作 ID 不改動排程。
- 工作 ID 用盡時不污染現有 FIFO 與真工作。

上述在原 V7 直接快路四項、動態續行六項、已執行幀取消五項、外層借用五項、P01 分段等待九項、私有延遲十項和既有 F00／G6／G17／R07 Gate 之上**增量添加**，絕不刪除或弱化原有測試。

## 剩餘必要工程與嚴格狀態

- 本排程器內工作引用呼叫者持有的 PreparedPrivateScalarCall；**尚未完成原 Parser／Checker 計畫與真續行物件的「完全持有式」安全封裝**，也尚未正式消除 src/execution/async_task.rs 的 P01 DirectAwaitSession 與本排程器之間的內部分離。不能稱為完整 V7 A1/A2 結案。
- 尚須補齊已核准 async self／方法身分與返回、遞迴／迴圈反向邊、資源欄位與跨幀真票據交錯，以及與既有 P18.1～18.6 原六項義務逐項對應的必要測試。原 26 條 OpenEngineering 與正式 **0/6 Passed** 不提前調整。
- 仍保持 D18-02 E1=A 延遲啟動直接 await、不開放背景工作；E2=A 單執行緒協作；E3=A 只安全點取消並逐框架清理；E4=A 保留既有執行期錯誤；E5=VB15-01-A 不允許有效借用橫越 await。公開 MVP-A CLI／F00／G6／G17 安全界線不變。

僅最新 PR HEAD 必要 CI「Rust 前端建置」「Rust 獨立架構測試」「專案檢查」全 completed/success、核對作者/base/mergeable 後 Squash merge；新 main **同 SHA** 三項再驗才能記錄此子批已合入。
