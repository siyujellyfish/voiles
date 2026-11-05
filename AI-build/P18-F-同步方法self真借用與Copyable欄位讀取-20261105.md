# G18／P18.6｜原私有 Checker 同步方法 `self` 真 BorrowTicket 與 Copyable int 欄位讀取（2026-11-05）

> **僅為已核准同步方法真執行子集，絕非 P18.6 多載／async 正式驗收。** 起點 `main eb072ad67f70792b8bd2f517c25486ae92ef671c` 的三必要 CI 成功；D18-02-A～D 原則已由使用者採納，非同步調度／取消點尚不自行補定。

## 原有核准權威與真執行缺口

現有私有 P13～P16 Parser／Checker 支援 S16 `instance.method()`，`Checker::check_method` 簽發 `VerifiedMethodSafety`（方法唯一來源跨度、共享／獨占 `self_borrow`、每個合法 `return` 來源跨度），`VerifiedDeclarationFacts::call_borrows_at(NodeSpan)` 保存真接收者 DeclarationId 與 BorrowKind。然而既有 G18 執行器尚未將**真正方法呼叫**接入動態 `BorrowSafety::acquire`／`check_ticket_access`／`finish_scope_verified`，故單純原 Checker 成功仍不能視為方法已執行。

## 本批已接通安全子集

1. 新增 `src/struct_stage/execute_method_self.rs`，僅透過原 `verify_source_with_ability` 取得真實已核准 AST／P16 安全計畫。僅處理單一結構、單一 `int` 欄位有合法整數預設值、單一同步零參數且唯讀 `self` 的方法，並由 main 建立唯一結構持有者後直接 `return owner.method()`。
2. 先核對**原** `VerifiedTypedMember` 欄位名稱、型別與源跨度、`declarations.matches(owner_id, struct_name, mutable)`、方法 `VerifiedMethodSafety` 的來源 `span`／`self_borrow=SharedRead`／合法 return 集合、呼叫位置 `scalar_hir.expression(ty=int)`、自動接收者真 `call_borrows_at(call_span)`。絕不由 AST 外觀自行猜 BorrowTicket。
3. 真執行以既有 `MixedStructRuntime` 建立實際 Copyable `int` 欄位與唯一持有者，方法進入時由 `BorrowSafety` 真簽票據、檢查原持有者直接存取被封鎖，**每次真讀 self 欄位前核對 active tracker／serial／BorrowKind／原 DeclarationId**。共享借用不提升可寫權限。
4. 真方法退出、Rust 測試專用故障都先按原 Checker 的 BorrowFact 快照原子歸還票據，再呼叫 `MixedStructRuntime::finish()`；無 `str` 的純值結構不能製造假的 ResourceEvent。錯誤不能假裝正常 return。
5. `execute` 在動態配置前先由**同一原 UTF-8 真來源**重新完成 Checker 與 P16 計畫的一致性比對，偽造接收者 ID、Kind、刪除 method_safety 或 call_borrows 等在第一筆 Runtime 資源前拒絕。

## 六項 Rust 正反及 CI

- var 結構真共享 self 票據、實際 Copyable int 欄位值 7 返回；無假 ResourceEvent。
- const 結構可合法唯讀方法，原先「只有 var 才能呼叫方法」的錯誤假定不成立。
- 方法借用期間 Rust 專用故障仍先歸還票據；無假成功返回。
- 偽造自動 self DeclarationId／Kind 在配置前拒絕。
- 刪除原 Checker 方法安全或自動借用事實不可沿用過時執行許可。
- 方法返回型別錯誤、明確 `self.move()` 由原 Checker 靜態拒絕。

`tools/核對G18同步方法self真借用.py` 對六項 Rust 正反做真測試清單 Gate；`tools/核對G18原六項驗收差距.py --audit` 擴展為**六組**獨立真 Rust 執行，P18.6 有實際方法正反但仍 `BlockedByDecision`，不得在 G18 `--check` 上假冒完成。

## 保留未完成的原工作

本批**尚未**執行有寫入的獨占 `self`、多載唯一靜態解析、帶參數／多欄位方法、非 Copyable 欄位、async 方法、await／取消／異常退出等全項。因此 P18.6 仍非 `Passed`，跨 await 仍依 VB15-01-A 既有核准規則守衛。與 G18 其他原項和 P19～P22 相依一樣，不增設公開 CLI。
