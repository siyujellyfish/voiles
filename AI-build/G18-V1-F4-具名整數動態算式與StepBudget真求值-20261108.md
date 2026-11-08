# G18／P18 V1-F4：結構具名 int 欄位真動態加減與安全預算（2026-11-08）

> 本批以 `main acb5f7945097b4e0a08cce9c91ddb4031c3cb752` 的三項必要 CI completed/success 作基線；接續既有具名 `int/bool` 局部及跨結構欄位真值來源、以及 Copyable 結構 `.move()` 子集。這是 **V1-F4 的一個工程子批**，非 F1～F4 完整收口，也非 P18 六項 Passed。

## 原 Checker 合法但執行器錯拒

原 Lexer／Parser／Checker 的 `int` TypedHIR 已可證明 `var source:int=4; source=source+3; var item:Meter=Meter(count=(source+2))` 及以真 `left.count` 參與加減的具名欄位值，但舊的 `checked_copy_struct` 對具名 `BinaryInt`／`Group` 一律回退到 P16 靜態 `approved_seed`，未接通從本幀最新 `DeclarationId` 與真 P16 欄位求值的運算，因而對合法來源 `Unsupported`。

本批只接通已由 **原 Checker 每個算式節點 TypedHIR、算術運算子／端點、Lexer** 編譯成功的無副作用 `int` 加減及括號：
1. `PrivateScalarCopyStructSeed::IntArithmetic` 保存與普通函式返回相同的 `PrivateScalarReturnExpr`，不重建第二套解析器；若已編譯樹有 `Call`，仍拒絕，因呼叫參數與結構具名來源尚未整合為同一工作圖。
2. 動態 `IntArithmetic` 用顯式後序堆疊 `Work::Read/Apply` 求值，每個葉與操作對 D18 `StepBudget` 逐步計費；不用 Rust 原生遞迴或事前取局部初值。
3. 葉節點讀真正的 `frame.values[DeclarationId]` 或 `MixedStructRuntime::field`；要求持有者詞彙身分、型別及原 Checker 證據完整。加減以 `checked_add/checked_sub` 保守回報 `IntegerOverflow`。
4. 全部具名 Copyable RHS 真求值成功之後才進原 P16 `construct`；錯誤時不得留下半個結構、假 `CopyStructConstructed` 或 UTF-8 `ResourceId`。其餘 `bool` 與原靜態預設沿用既有執行語意；`str` 欄位、借用、含函式呼叫的建構表達式仍不放行。

## 本批五項真正反案例

- 同一 int 局部被真正更新後，以 `(source+2)` 初始化結構，返回最新值 9。
- 真結構來源欄位 `left.count` 更新後，`(left.count-2)+5` 初始化另一結構，返回 12，兩個真建構與兩次逆序離域。
- 具名 `count=source+4,enabled=flag` 與 bool 實參的同一 P16 求值計畫，後續更新來源不影響已建構的快照，返回 7。
- `i64::MAX+1` 由真局部來源觸發動態溢位，回滾框架平衡，**零結構建構、零 UTF-8 資源**。
- 錯型別算式由原 Checker 靜態拒絕；已執行成功計畫被故意篡改具名 Seed，執行前必須 `SourceChanged`。

普通函式專項 Gate 從 **382 增至 387**，P18.1／P18.4／P18.5 逐案追溯；不得修改 V0 原 26 義務清冊、G17、F00、G6 凍結、公開 CLI 或將 R07 `--audit` 當正式 `--check`。

## 尚未結案的 V1 義務

F1：完整已核准前向／互相及多層普通函式呼叫與返回；F2：所有合法 int/bool/str 混合參數求值次序、跨幀借用與資源轉移；F3：條件／迴圈／宣告／賦值中各種合法呼叫點與零可達影響；F4：含 `str` 欄位的全部已核准結構來源、跨幀唯一 ResourceId 與借用安全，以及具名欄位裡可達函式 Call 的同一工作圖。這些仍需各自閉合真 Checker→Runtime→Resource/Loan/Exit 義務，六個原項正式狀態維持 **0/6 Passed**。

本 PR 子批必須最新 HEAD 三必要 CI completed/success、來源身分／mergeable／base 核對後 Squash merge，再以新 main 同 SHA 三必要 CI 完整驗證才能標記子批完成。
