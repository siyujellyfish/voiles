# G18／P18.6-A1／M2：已核准 V7 async self 直接 await 語法與第一批真借票實作（2026-11-15）

## 已正式核准的最小語意

使用者於 2026-11-15 明確核准前一輪待決方案 A：V7 **僅**新增 `await owner.method(args)` 直接等待已由 S20 唯一靜態簽章決議的 `async fn` 方法；方法**真正開始執行時**才簽發其原 Checker 核准的 `self` BorrowTicket；若方法內再遇到 `await` 且借用仍有效，一律遵循既有 VB15-01-A 靜態拒絕。不得由此批准背景工作、任意取消語法、跨等待點重新借用、跨執行緒排程、其他非同步語法、公開 CLI 或新增異常回傳種類。

施工主線起點 `762021664222fc35ad41400171041a88cc27144d`，同 SHA「Rust 前端建置」「Rust 獨立架構測試」「文件與工作紀錄」三項均 `completed/success`。原 P18／V7 已核准 E1 延遲、直接 await，E2 單執行緒協作，E3 安全點逐框架取消，E4 現有受控失敗，E5 保留 VB15-01-A。這次選擇是**在既有 E1～E5 規則內的 async 方法入口補裁決**，沒有改動任何已核准借用安全界線。

## 工程改動與證據來源

1. 私有共用 expression enum 新增 `TermKind::AwaitMethodCall(receiver, method, arguments)`，由**唯一私有 Parser** 在原 `await` Name token、receiver、單一 `.`、方法名及共用 `parse_call/MethodCallSyntax` 的真 Lexer 跨度中生成；一般 `await fn(args)` 保留原 `TermKind::Call {awaited:true}`，同步 `owner.method(args)` 保留原 `MethodCall`。公開 MVP-A 不新增任何語法，也不將來源字串改寫成同步方法。
2. 原 Checker `evaluate` 中同步與直接等待兩種 AST 共用 S20 方法唯一解析、型別檢查、真 DeclarationId、`call_borrows`、`MethodSafety` 與保守的借用衝突判定。只有 `method.is_async == direct_await` 合法；在**求值及借票之前**驗證原 `active_loan_snapshot(vars)` 中無任何活躍 BorrowFact、參數均已核准 Copyable 且不含巢狀 await；禁止 `self` 重入及未核准同源額外後綴。新 AST 加入詞彙活躍讀取與含 await 引數掃描，避免逃過原 P15 last-use 與跨等待安全點的決議。
3. 已驗收的 `PreparedPrivateSelfMethod` 第一批僅接**一個已核准單欄位 int、方法控制圖無內部 await** 的 async self 直接 await；原 `verify_source_with_ability`／Checker `ScalarHIR`／MethodSafety、精確原 `await` Token 與真 Caller `call_borrows` 必須全部匹配。新 `awaited_method` 被封存於計畫及來源重驗 `matches_source`，不能把原 async 方法變為同步再冒充執行。
4. `execute` 按原 main Copyable 局部、原方法所有實參從左到右**真執行完畢後**，僅在抵達直接 await 啟動方法的真 Runtime 位置，才用原 HC14 `BorrowSafety::acquire` 取得唯一 self 票並 `FunctionEntered`，執行原已驗收 if/else／直接寫欄位／純 int 返回與真共享退出；任何函式實參溢位都不簽發 self 票、不虛造 async 方法入場。真 `await_start_loan_index` 是動態記錄在原 R05 `exit_events` 的票據位置，`has_balanced_self_borrow_trace` 須同時核對其後緊接的真方法入框架與唯一 LoanReturned。純 int 沒有 P16 UTF-8 ResourceId，嚴禁創造假資源。
5. 這是「在直接 await 當下執行至返回」的**無方法內等待點子集**；既沒有冒充 `async fn` 方法內 `await` 可被安全暫停與恢復，也沒有將同步 self 方法包裝成背景任務。既有 V7 直接等待普通函式的真正可恢復 `PrivateScalarGraphContinuation` 與 FIFO 繼續獨立；後續 A1/A2 需要它與 self 票據／資源的共通控制工作圖完整合流，方能正式 Closed。

## 十二項真 Lexer／Parser／Checker→Runtime 正反

`src/struct_stage/g18_v7_async_self_direct_await_tests.rs` 及 `tools/核對G18-V7-A1原Checker直接await非同步self方法.py`，要求 Rust 真 `--list` 對齊所有 12 個唯一完整名稱，逐條 `--exact --test-threads=1` 1 passed、0 failed、0 ignored：

- 真 async 方法 AST／await Token 跨度／Checker BorrowFact、SharedRead 原接收者身份。
- main 正式直接 await 才開始唯讀 async 方法，真的進入、正常 Return、同一唯一 BorrowTicket 正式歸還。
- 已核准 ExclusiveWrite async 方法動態寫入欄位、真結果與原票據。
- 方法 Copyable 實參中同步普通 helper 必真先退出，後續才進入 await／取得 self 借票。
- 真 if 選中寫欄位、未選中葉不寫；每個分支沿用原已驗收控制圖。
- 方法本體受控 `IntegerOverflow` 只報 Unwind，先錯誤清理再歸 self 真票，不能假成功。
- await 以前實參 helper 溢位不建立方法框架或票據，動態索引必為 None。
- async 方法漏 await、同步方法被偽 await、方法內再次 await、錯型別、未知方法、非 Copyable 實參及 const owner 嘗試獨占寫入均拒絕。
- 竄改封存 `awaited_method` 或其動態入場索引，原 `SourceChanged`／R05 trace 稽核拒絕。

## 實作邊界、下一步及正式狀態

本批必須最新 PR HEAD 的「Rust 前端建置」「Rust 獨立架構測試」「文件與工作紀錄」三項均 `completed/success`，作者/base/mergeable=clean 查實才 Squash merge；合併後新 main **同 SHA** 三項成功才採計。若原 Parser／Checker 靜態拒絕或 Rust 回歸揭露範圍不符，須先依 CI 原因保守修正，不得為追求驗收擅自放寬借用／取消。

原 P18 六項正式 **3/6 Phase Passed，17/26 原義務 Closed**，B4、T1～T4、M2/M3、A1/A2 **仍為 OpenEngineering**。本批僅解除 A1 async self 真來源的第一個子缺口，不代表 A1/A2、P18.6 Passed；原 R07 `--check` 應維持嚴格未完成判定。後續仍須完成方法內的真安全點、同一原工作圖跨呼叫 suspend/resume、取消、失敗傳遞、逐幀 P16／HC14 清理與高風險交叉驗收，不得以本批十二項代替。

## 首輪必要 CI 編譯失敗與來源稽核閉合

PR #810 首輪 HEAD `34a594078b9e463ec116275b68c5c41f9ff318c1` 的「Rust 前端建置」失敗（Rust E0004，兩筆），出現在原私有 AST 來源核對 `src/struct_stage/source_identity.rs` 與 `src/struct_stage/token_provenance.rs`，因新增 `AwaitMethodCall` 尚未被完全覆蓋。這兩處是原安全權威，**不能用 `_ => true` 或省略檢查**修復。

修正後：`verified_private_syntax_roles` 必驗完整原 UTF-8 `await` 前綴及子引數角色包含；`verified_private_token_roles` 在原相鄰 Lexer Tokens 嚴格檢查 `await`、receiver、`.`、method、`(`、最後 `)` 與原 `NodeSpan`，並拒絕把 `await` AST 身分改為同步 `MethodCall`。此補齊正是獨立真 Checker 驗收的一部分。原 PR HEAD 編譯紅燈不可採計；必在修復後的最新 HEAD 重跑全部必要 CI。
