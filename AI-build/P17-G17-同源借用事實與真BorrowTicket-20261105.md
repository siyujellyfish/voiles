# G17／17.3｜同源借用宣告、真 BorrowTicket 與 UTF-8 實際持有者（2026-11-05）

> **G17 工程候選，不表示 G17 原始 17.1～17.6 已全部正式驗收。** 基準：`main f0d187077c7534991c0950c93d372c0404035ca5`，此 SHA 三項必要 CI `completed/success`。

## 正式靜態來源權威

- 從原 `verify_source_with_ability` 的 Lexer／Parser／Checker 成功核准的直接 `const view:str = text.borrow()` 或同型別 var/var 借用取得原始 `DeclarationId`、類型、var／const 與 `BorrowFact`。新增 `recorded_direct_borrows` 在 Checker 已檢驗來源有效、權限吻合、型別相符、衝突安全且提交成功後才記錄直線借用；巢狀不確定分支不偽裝成必然執行。
- `VerifiedDeclarationFacts.direct_borrows` 唯讀保存「別名宣告身分 → 原來源 BorrowFact」，`evidence_is_consistent` 核對同一宣告表內型別、來源先於別名、權限（const/SharedRead；var/ExclusiveWrite）；`operations_match_verified_ast` 僅用真 AST 核對操作位置，不從文字推導權限，亦不把任何 BorrowFact 當作實際票據。
- `PreparedPrivateBorrow` 僅接受原 Checker 允許的單一 main、先建立獨立 UTF-8 `str` 的原所有者、接續一或多筆直接 `.borrow()` 宣告及 int 常值返回；複雜別名逃逸／任意函式、方法或結構不以推測授權。執行前重新對相同來源通過原正式 Checker，核對 `VerifiedSafetyPlan` 與原始全量投影和 bytes，任何更改均在配置任何資源之前拒絕。

## 真執行期事件及票據

1. 原 `OwnedStrRuntime::declare` 真配置唯一 UTF-8 String，真正產生 `Created(ResourceId)`。
2. 執行到每個已驗證別名語句才由 `BorrowSafety::acquire` 真簽發、追蹤 `BorrowTicket`；逐張以原 Checker `BorrowFact` 快照進行 `verify_static_snapshot`，不能從票據倒推靜態授權。
3. 透過真正仍存活的 UTF-8 `OwnedStrRuntime::read` 取得唯讀位元組長度，驗證票據存在期間直接所有者被禁止存取；別名不擁有、複製或移動 String，沒有假造 `ResourceEvent`。
4. 在函式退出前 `finish_scope_verified` 原子驗證並歸還所有動態票據，確認恢復原持有者直接存取；`OwnedStrRuntime::finish` 釋放唯一 String，真 `Released(ResourceId)`，`event_trace_balanced` 確保無漏釋或重釋。
5. 原 P16 靜態 `SafetyState`、其他一般 `resource_borrow_pipeline`、合法的 str/struct move、F00 四凍結與 G6 行為不更動；CLI 仍是 P01 靜態檢查。

## 新八項 Rust 真正正反

| 案例 | 必需觀察 |
|---|---|
| const UTF-8 來源 const 借用 | 真 SharedRead ticket、真正 UTF-8 bytes、恰好 Created→Released、重複執行不沿用票據 |
| var UTF-8 來源 var 借用 | 真 ExclusiveWrite ticket、持有者直接存取封鎖直到歸還、單次清理 |
| 同來源雙 const 借用 | 多張真正獨立共享票據，仍只建立一份 String |
| 錯誤型別與 var／const | 原 Checker 事前拒絕，不開始動態資源 |
| 偽造別名 DeclarationId | 同名不能代替確切原宣告身分，配置前拒絕 |
| 偽造 BorrowKind／來源 id | 任何與 Checker 不同的權限或來源不得取得真票據 |
| 清空原 Checker 借用事實 | 計畫漂移無法沿用先前執行授權 |
| 改寫來源位元組 | 同原來源審計失敗，不開始任何資源效果 |

所有八項由 Rust 核心完整測試真正執行，`tools/核對G17真借用票據.py` 以精確測試名稱檢查存在性、不重複執行，避免檢查空結果冒充通過。

## 尚待 G17 正式總結案

- 17.3 已有本批真借用票據正反，但仍須逐項核對原 G17 17.1～17.6 對內部執行的完整最小已核准範圍；不能僅靠本批直接簽核。
- 17.4 公開 OS 退出碼仍屬 P22 未決，G17 應在不改公開命令的前提正式界定內部失敗／退出契約。
- 17.6 真結構的具名順序、欄位預設與失敗回滾已有 Rust 正反；仍須完整核對是否存在由原 Checker 核准但本批求值器保守拒絕的建構形式及相依決策。
- 所有合併需最新 PR HEAD 三項必要 CI success、作者/base/mergeable 後 Squash merge，合併後 main 同 SHA CI 再驗，才更新原始項目勾選狀態。
