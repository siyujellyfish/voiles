# G18／P18.2｜已核准具名 bool 條件的 while 真重讀、退出與資源清理（2026-11-05）

> **非公開已核准動態執行子批，並非 G18 正式 Passed。** 工程基準為 `main fea92fca24f0a641178bc6ca8dbca59c5b1807ec`：G17 已 6／6 Passed、G18 六項 `BlockedByDecision`，三必要 CI success。

## 專項安全問題與方案

原 G18 有限迴圈切片僅接受 `while(true/false)`，未接通原 P15 已核准的「每輪重新檢查已初始化 bool 具名變數」與 P13 Copyable bool 重新賦值。此子批不創設新迴圈語意；由唯一真私有 Checker 的 `scalar_hir.reference(NodeSpan)`／`scalar_hir.assignment(NodeSpan)`、Lexer token、`DeclarationId`、var／const 權限檢查構建真正執行計畫：

- 僅根作用域的已驗證 `bool` 宣告可建立純值 slot，來源限 bool 常值或合法具名 Copyable bool 複製；同名／不合法遮蔽不自行放行。重新賦值限原 Checker 已核准的 **var bool**、已繫結目標身份和真 `bool` RHS；非 bool、const 或未初始化仍由原 Checker 事前拒絕。
- `while(flag)` 每次進入／重新測試從真 slot 讀取當下 bool 值，而非編譯期常數、AST 字面文字或前一次快照；`condition_checks` 只記錄真正已求值的每個 bool，不把靜態 CFG 冒充已執行分支。
- 在 D18-01 一般耗盡政策未核准前，只接通**能證明最多一次本體執行**的有限形狀：本體末尾直接 `break`／`return int 常值`，或 `while(flag)` 末尾已驗證 `flag = false`，並明確拒絕能先行跳過末尾終止義務的 `continue`。這只是內部私有 evaluator 支援的安全子集，**不是語言對 while 所新增的普遍限制**。
- 真走到的 `str` 才建立 UTF-8 `ResourceId`，false 分支零配置。break／return／測試期故障先 `leave_scope`，再清理根所有者。若執行期未如原 Checker 計畫終止，安全拒絕而非使用 Rust 無界迴圈。既有 `for`、巢狀、失敗回滾真資源和 G17 Gate 不變。
- 執行前重跑原來源 Checker，確認原始 UTF-8 bytes、全體已準備動作和 Checker 資源／控制／宣告一致；篡改條件 `DeclarationId` 必須零資源拒絕。任何可能需要複雜 bool 表達式、條件副作用、方法或 async 的情境仍 `Unsupported`。

## 十項新增 Rust 真正正反

| 情境 | 證據 |
|---|---|
| `while(flag)` 初始真、內部 `flag=false` | 動態條件序列 `[true, false]`，一輪 str 建立與釋放 |
| 初始假 | 真條件 `[false]`，零迭代 str |
| `flag=false` → `flag=true`、末尾 break | 只一輪資源，已重新賦值決定選路 |
| `flag=source` 後修改 source | bool 值複製非別名，loop 仍執行真路徑 |
| 具名 `const flag=true` 直接提前 return | 內層再外層逆序 Released |
| 內部第 N 個資源後注入故障 | 實際條件與已完成事件保留，真清理不觸發後續賦值 |
| 準備後偷換條件 DeclarationId | SourceChanged，資源開始前拒絕 |
| 沒有直接退出／使 flag=false 的 while | 只回內部 Unsupported，不自訂語言耗盡政策 |
| 未完成退出前有 continue | 保守拒絕，不能以末尾未達 flag=false 證明有限 |
| const bool 在 body 中重新賦值 | 原正式 Checker 靜態拒絕 |

新增十項後，`tools/核對G18有限迴圈真資源.py` 的原 13 項增至 **23 項**，仍全部由原 Rust 全量真正執行、專項只核對測試名稱存在；`tools/核對G18原六項驗收差距.py --audit` 仍須真重跑所有 P18 群組。**G18 原六項仍 `SubsetVerified/BlockedByDecision`，不得把新 10 項、三 CI 綠燈或非公開子集當成 P18.2 整項正式 Passed**。

## 下一批及必要阻塞

可繼續改善原 P18.1 無參數函式前向／多層返回、已核准 Copyable 參數與錯誤路徑等；一般化動態 while、迴圈裡多種借用／資源操作及共同控制邊仍需逐個真來源證明。D18-01 遞迴／中止耗盡與 D18-02 方法多載／async 生命週期仍保留原必要決策，不從本批 `bool` 真求值推定已核准。
