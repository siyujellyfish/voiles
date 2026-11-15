# G18／P18.6-A1／V7-E2：async self 未啟動真 Ready FIFO 與轉移歷史（2026-11-15）

## 已核准語意與主線前置證據

起點 `main fb491f3a6cddff52e62bb5b27192ae13c8d3b626`，GitHub 同 SHA「Rust 前端建置」「Rust 獨立架構測試」「文件與工作紀錄」三項均 `completed/success`。已核准 D18-02-E1=A 延遲啟動且限直接 await、E2=A 單執行緒協作式排程、E3=A 安全點取消及逐框架資源清理、E4=A 既有錯誤傳播、E5=VB15-01-A；另已正式核准 `await owner.method(args)`，方法真正開始才取得自動 `self` 借用票，方法內若再等待但仍有活躍票據則靜態拒絕。

前批 #810～#813 已有原私有 Parser／Checker 真 `AwaitMethodCall`／`TypedHIR`、真 `self` 借票、無內部第二個 await 的動態方法入口，以及跨 Rust 呼叫保留 `Prepared`／`Suspended` 工作、`wake(id)` 真執行／`cancel(id)` 未啟動零事件。但多份工作由外部明確選 ID 喚醒，缺少**可重播的單執行緒 FIFO 就緒憑據與整批安全關閉**；僅有 `execution_order` 不代表 E2 真協作排程。

## 本批工程：同一既有工作身分與單一執行器

只擴充 `src/struct_stage/execute_method_async_deferred.rs` 的 `PrivateCheckedDeferredSelfAwaitQueue`，**不**重編譯另一棵 AST、不新增公開語法，不自行派發背景工作、不讓 async self 方法內的待喚醒框架逃逸。

1. `prepare(source)` 繼續使用原完整 `PreparedPrivateSelfMethod::prepare`，唯一 ID 保留；另追加 `PrivateSelfReadyTransition::Prepared(id)` 來源歷史。被 Checker 拒絕或現存收據不一致時不得消耗 ID、不得建立新工作。
2. `suspend(id)` 只可從 Prepared，追加真 `Suspended(id)` 收據並發一張有順序的 `ready: VecDeque<id>` 憑據。重複 suspend／錯誤狀態零變更；原準備期間不簽 BorrowTicket。
3. `drive_next()` **僅在呼叫者明確驅動**時選 `ready.front()` 真 FIFO 工作；沿用已驗收 `wake(id)` 的原 `execute` 動態 BorrowTicket／SharedExit／RuntimeError 清理。當前工作完成正常 Return 或 Unwind＋LoanReturned，下一份才可開始，沒有兩份 async self 方法同時持票。就緒佇列空時只回 None，不虛造動態事件。
4. 原 `wake(id)` 保留已核准的直接指定工作喚醒；即使外部選非 FIFO ID，也只能**精確移除該張合法 Ready**，其他已發行的順序不動；成功時追加 `Woken(id, Succeeded/Failed)` 收據、永久記錄真 `execution_order`。原 `SourceChanged` 失敗保留仍 Suspended 的工作、就緒憑據及全歷史，不假完成。
5. `cancel(id)` 僅能取消 Prepared/Suspended；若已就緒只精確移除該 ID；追加 `Cancelled(id)` 和取消帳 `cancellation_order`，不建立 P16／HC14 假資源或錯報真正執行。已完成／取消不得再執行。
6. `verify_ready_history()` 在**所有狀態變更之前**完整重播每筆原序轉移。由 `Prepared` 唯一遞增 ID、Suspended 唯一就緒、Woken 真終態及 Cancelled 可達性還原 Ready FIFO、全工作 status、 `execution_order`、`cancellation_order`；比較全部現存 jobs，每份未啟動必持真原 Checker 計畫且無 Outcome，每份成功／失敗必已放棄 plan、有與 status 一致且 `has_balanced_self_borrow_trace` 的原執行結果，Cancelled 必零 plan／Outcome。這是同一來源工作歷史的結構稽核，**不是密碼學簽章**，不能宣稱防禦整個可信程式內對所有獨立帳冊同步偽造。
7. `shutdown()` 在所有工作歷史、現有終態都驗證後，依逆 ID 取消其餘未啟動工作；保留已完成的真 Outcome 與原事件責任。若任何工作狀態或佇列與收據不一致，預檢拒絕且其他工作零部分取消。原方法正在自身函式中執行時沒有外部可並行呼叫，不能把這個無內部暫停點的 shutdown 誤當任意搶占或跨 await 安全取消。

## 真來源十五項驗收及必要 CI

`src/struct_stage/g18_v7_async_self_fifo_receipts_tests.rs` 按真 `.voil` 原 Checker／S20／VB15－來源生成多份唯一真 `PreparedPrivateSelfMethod`，測：未就緒不執行、suspend 先後即 FIFO、三工作順序且每筆自借票全平衡、前份溢位 Unwind 再進後份、取消隊首不破壞後續、明確手動 wake 非 FIFO 只移除指定 ID、逆序整批取消、已完成與待命混合關閉、幽靈 Ready／交換就緒順序／重複憑據／偽造完成狀態或借票結果／抹除取消歷史／別份終態污染不得部分取消／原 Checker 封存來源變造保留可復原工作。十五項獨立完整名稱不可重複。

`tools/核對G18-V7-E2非同步self就緒FIFO及轉移收據.py` 先用正式 Rust 二進位 `--list` 嚴格確認 15 個唯一測試名稱集合，再對每筆單獨 `--exact --test-threads=1` 驗 `1 passed; 0 failed; 0 ignored`。加入 `.github/workflows/前端建置.yml` **必要 Gate**，仍完整重跑原 G17／G6／F00 等回歸。

## 正式狀態與未解工程

本批真 E2 子集是「**已暫停但尚未進 async self 方法的多份工作之可稽核 FIFO 排程**」。方法本體內再次等待時仍依 VB15-01-A 保守拒絕；同一協作圖內跨 async self 方法中途 `await`、合法恢復、P16 ResourceId／BorrowTicket 逐框架取消及普通／方法整合完整 A1/A2 尚未閉合。原義務仍 **B4、T1～T4、M2、M3、A1、A2 九個 OpenEngineering**；正式 P18 **3/6 Phase Passed、17/26 Closed** 不變，R07 `--check` 應維持未通過。每批需最新 PR HEAD 三必要 CI `completed/success`、作者/base/mergeable=clean 確認並 Squash merge，新 main **同 SHA** 三必要 CI 完成且成功才可採計；不提前宣稱 P18.6 Passed。
