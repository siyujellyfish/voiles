# G18／P18.4｜同一函式 Call 中明確 str.move 與預設 str 借用（2026-11-06）

> 基線 main c98a24c7bc5e79f2643bd4f7133c5945f57fa489 三必要 CI success。延續 HB13-A、P15.6、HC14、VA16／P16、D18-01，禁止以 BorrowTicket 假冒所有者或藉 move 轉換非擁有別名。不擴張公開 MVP-A 或未核准語法，原 G18 六項不提前 Passed。

## 實際原驗收阻礙

前批 #562 證明單一真 str 可跨普通函式／遞迴以 .move() 轉交；#561 的同 Call 多張 BorrowTicket 只涵蓋非擁有預設借用。但原 Checker 的 TermKind::Call 明確支援不同來源同時混合整筆 Move 與 BorrowFact：其中一個實參被消耗，另一個獨立來源只借用。尚無真正同次執行與故障交叉資源清理正反，P18.4 無法完整驗收。

## 真執行工程

- 直接在既有 src/struct_stage/execute_call_borrow.rs 增加原 Checker 已核准的同次混合呼叫分支。main 建立兩個原 Lexer 真 UTF-8 str 所有者，callee 具有兩個原 Parameter TypedMember/DeclarationId：一個必須由明確 TermKind::Move(原具名所有者) 實參提供；另一個由 TermKind::Name(另一獨立所有者) 提供預設借用。整份原 Checker call_borrows_at 必須**恰好等於**借用來源與 SharedRead／ExclusiveWrite Kind，絕不自訂借用合法性。
- **依原 Parser 實參左右順序**實際呼叫 OwnedStrRuntime::move_named 或 BorrowSafety::acquire；從原父 tracker 的活躍 BorrowTicket 驗證來源／序號／讀取 Borrowed 原 UTF-8 真位元組；被移動值由新的 callee 局部持有或再次整筆轉交該函式內局部。不同 ResourceId 不複製、借用來源不轉交所有權。
- 正常 return、實參一／二求值故障及函式內二次轉交故障時，先在原父 tracker 完整歸還所有有效票據，再離開 callee 真資源作用域，最後由 root 釋放仍有效的借用所有者；move 後失效的主來源不可重複釋放，每個 Created 恰好一次 Released，Moved 僅指向原唯一 ResourceId。
- 八項原 Checker 真來源正反：move→borrow 與 borrow→move 雙求值順序、const 共享和 var 獨占借用、兩種順序的四個故障注入階段、callee 未再次移動的持有者清理、同源借用與移動衝突、借用別名非法轉 owner、錯型別／漏參數及 BorrowFact／第二參數／UTF-8 篡改零執行拒絕。獨立真 Rust Gate 8／8，G18 原 P18.3、18.4、18.5 來源追溯增列。

## 尚未正式通過

本批只涵蓋兩個不同 str 所有者、同次一筆 move 加一張合法借用票據的真正直線函式呼叫；一般多個 move 與多張借用交錯、跨嵌套和條件／迴圈控制、借用逃逸及 async 取消仍待工程或語意裁決。P18.3 Incomplete、P18.4/P18.5 SubsetVerified、P18.6 BlockedByDecision，不可把單批 Gate 當原六項 Passed。需 PR 最新 HEAD 三必要 CI completed/success、作者／base／mergeable 核對、Squash merge 後新 main 同 SHA 再驗。
