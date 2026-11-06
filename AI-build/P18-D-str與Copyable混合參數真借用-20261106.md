# G18／P18.4｜普通函式非 Copyable str 與 Copyable int 真混合傳參／借用（2026-11-06）

> 基線 main 179ffa2fabc05b36ff39e6a1f227244e5d6accfa，最新主線三必要 CI 已成功。直接擴充已存在的 src/struct_stage/execute_call_borrow.rs 與同來源已核准的 PrivateScalarReturnExpr；原 Parser／Checker／BorrowFact／TypedHIR 為唯一靜態授權，不建立第二套解析器或公開入口。

## 本次解決的原始驗收缺口

既有 P18.4 已有單獨 str BorrowTicket 或一至三個 Copyable int 的真函式動態執行，但**同一次呼叫需要同時有非 Copyable 預設借用及 Copyable 傳參**的合法形狀仍無真正資源／參數框架實測。原私有執行器只能一 str 參數並返回 int 常值，不能核實混合實參順序、借用生命期、計算溢位與錯誤清理的一致性。

## 工程內容

- 同一原 str 借用 executor 接通單一具名 str 持有者與可選一個 int 常值參數，原 Parser 正式簽章順序允許 str,int 或 int,str。每筆參數的原 DeclarationId、原 TypedMember 與 Lexer 名稱／型別、call_borrows_at 真 BorrowFact 一一核對；不根據 runtime 資料型別選擇借用種類，var/const 持有者分別沿用既有 ExclusiveWrite／SharedRead 契約。
- 真被呼叫者擁有本次 Copyable int DeclarationId→i64 動態槽，在仍持有同張真正 BorrowTicket 時，透過同一來源原 Checker ScalarHIR／Arithmetic HIR 執行 return n、return n±int 及群組加減。不能隱含複製 str、轉移所有權或偽造新 ResourceId。
- 正常、受控 int 算術溢位及測試故障均清空全部參數槽、以原 call_borrows 快照歸還真票據，最後由原 P16 OwnedStrRuntime 唯一釋放 UTF-8 持有者。故障不得標記成功呼叫退出；重新 execute 不得殘留上一輪參數槽或票據。
- 原預備計畫在實際配置任何資源前重新執行原 Lexer／Parser／Checker 並比對整份來源及真借用表，篡改 int 值、DeclarationId、返回算式或刪除 BorrowFact 均保守拒絕。
- 新增八項真正反：const str + int 真加法、反向參數順序與 var str 獨占借用、同來源重複執行、溢位、故障、參數與 HIR 篡改、刪除真借用授權、錯誤型別／個數與尚未接通的具名 int 子形狀拒絕。函式真借用 Gate 12→20，且由僅檢查名稱升級為全部真正重跑；G18 原六項 P18.4 追加八項真來源追溯。

## 必要驗收與未完成

本批不擴張混合參數的一般整筆 Move、借用逃逸、跨巢狀呼叫複合資源圖、async 取消或例外。P18.4 仍 SubsetVerified，P18.3/P18.5/P18.6 等仍依原狀，不把受限子集當正式 G18 Passed。需最新 PR HEAD Rust 全量／全部三必要 CI completed/success、GitHub App 與作者/base/mergeable 核對後 Squash merge，再以最新 main 同 SHA 重新實測。
