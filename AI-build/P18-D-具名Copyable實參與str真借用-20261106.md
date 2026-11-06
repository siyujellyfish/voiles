# G18／P18.4｜真 str BorrowTicket 混合已初始化具名 Copyable int（2026-11-06）

> 基線 main b75c3ad5a7387c4fb4bd637b14b22dd16ba8b914 三必要 CI success。承接既有 P18.4 混合 str/int 函式真執行，直接擴充同一 src/struct_stage/execute_call_borrow.rs；不改公開 CLI 或原已核准語言文法。

## 原驗收差距

已驗證混合參數形狀只接受 int 常值實參，即使原 Checker 已接受 main 中完成初始化的 var／const int 具名來源。這使 P18.4 真正 Copyable 參數的宣告身分、複製時效與同次非 Copyable 借用責任欠缺正向動態證據；原負測試更把合法具名呼叫當成必拒絕。

## 本批工程

- 真 main 路徑可依序建立已核准唯一 str 所有者和可選已初始化 int 局部 var／const；後續 return 呼叫的 int 實參既可為原合法 i64 Lexer 常值，也可為原 Parser 具名 int。其原 Checker ResolvedUse／DeclarationId、Lexer Name token、初始化 int Expression TypedHIR／Lexer Integer token 與宣告可變性，均在執行前逐項核對。語言不設固定先後順序；此私有切片只接通 str 所有者先建立、具名 int 在其後建立的可證明真路徑。
- 由 main 的獨立 Copyable int 值讀取後，與真正非 Copyable str 持有者同時傳入既有兩種合法參數簽章順序 str,int 或 int,str。int 從本次真正參數槽取得並參與原 Checker HIR 計算，str 不複製、不轉移，依原 call_borrows 發放並歸還 BorrowTicket。
- 成功、算術溢位、測試故障都清空參數槽並歸還動態票據，真 UTF-8 ResourceId 仍僅釋放一次；重複呼叫不殘留狀態。偽造具名 int 來源 DeclarationId、初始化值或框架值都必須在資源建立前拒絕。
- 新增五項真正反：const int + const str、var int + var str 反向參數順序、真借用期間注入錯誤、竄改具名整數來源身份／值、未初始化／型別不符的 Checker 拒絕。先前「具名 int 必拒絕」反例改為真正未執行的多餘宣告必拒絕。函式借用動態 Gate 20→25，G18 原六項 P18.4 真來源追加五項。

## 未完成與驗收規則

本批依然不是一般跨函式整筆 Move、混合參數任意初始化表達式、複雜所有權控制圖或 async 例外／取消；P18.4／P18.5 仍不能 Passed。須 PR 最新 HEAD 全 Rust 與三必要 CI completed/success、GitHub 作者／base／mergeable 核查、Squash merge 和最新 main 同 SHA 重新執行。
