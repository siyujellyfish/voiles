# G18／P18.1、P18.3、P18.4｜已驗證 Copyable int 呼叫圖顯式執行堆疊與遞迴耗盡（2026-11-06）

> 基線 main f9eb22f298de92e7ea3c0dba6710c9833b786efe 三項必要 CI completed/success。沿用原私有 Lexer／Parser／Checker／TypedHIR、AW06 與使用者已核准 D18-01-A～D。**本批實作不改語言語法，不新增公開 CLI、固定語言遞迴深度或 OS exit code；原 G18 六項仍非全 Passed。**

## 對原驗收的直接缺口

現有原私有 Copyable int 函式執行器已處理一至三個參數、三層巢狀函式圖，但仍在 Prepare 階段拒絕四個以上函式及完整呼叫圖中任何環。這直接違反已核准 AW06/D18-01 允許合法直接／互相遞迴、不強制靜態證明終止的要求。其函式調用採 Rust 宿主遞迴，無法在大量動態真框架下證明受控錯誤與完整逆序退出。已有 bool／UTF-8 的另一個有限遞迴執行器，但不能替代已核准 Copyable int 一般普通函式的同路徑證據。

## 實作範圍

- 將 src/struct_stage/execute_call_scalar.rs 原 evaluator 的巢狀求值替換成明確的 expression 工作堆疊與獨立函式參數框架 Vec。依原 Checker 已驗證的 Call、Parameter、BinaryInt AST／TypedHIR 來源先左後右求值，真正進入被呼叫者才綁定當次 i64 參數槽；正常返回與整數溢位、預算耗盡均維持精確 FunctionEntered/Returned/FunctionExited 事件，子框架先逆序退出。不是模擬執行也不倚賴宿主 Rust 堆疊深度。
- 去除僅屬舊施工切片的「最多四函式、三參數、三層且禁止圖成環」限制。所有原 Checker 成功的非零數量 int 參數與單一純 int 返回／巢狀算式，依原 Parser 函式簽章、DeclarationId、P16 TypedMember、ScalarHIR 核對；原 Checker 拒絕的型別與借用仍絕不執行。
- 使用 PrivateScalarExecutionBudget {max_call_frames, max_steps}；數值只屬**本次 Rust 內部執行環境設定**，可由 execute_with_budget 覆蓋，與真正函式數量和步數綁定而非虛構語言上限。正常合法直接／互相遞迴可執行至環境預算耗盡，回報 CallBudget／StepBudget，禁止假成功 return，清除已進入全部動態參數槽。
- 將舊「遇循環圖必拒絕」過時負測改為真正可執行且受控耗盡的互相遞迴；額外七項真正反涵蓋合法直接遞迴、九層前向返回、五個 int 參數、步數耗盡、零預算零事件、未被選中遞迴函式不妨礙正向執行、兄弟兩呼叫各自框架。原 Test Gate 32→39，G18 原始六項中的 P18.1／P18.3 增列真來源追溯。

## 仍待正式驗收

此切片僅處理完整原 Checker 允許的純 Copyable int 單返回式呼叫圖，並不能替代一般條件／迴圈與混合 UTF-8/borrow 的所有運算，也沒有非同步方法調度取消語意。P18.1／P18.4 仍 SubsetVerified、P18.3 仍 Incomplete、P18.6 仍 BlockedByDecision，**G18 六原項不提前勾選**。要求 PR 最新 HEAD 完整 Rust／必要 CI completed/success、作者／base／mergeable 與 Squash merge 後新 main 同 SHA 再驗。


---

## V1-A：合法零參數巢狀函式圖與遞迴驗收子批（2026-11-06）

- 基線：V0 文件 PR #564 已 Squash merge 的 main ae6faee485a643a65aef7d8a75376bb1f1070d3c；原本純值呼叫圖仍在 Prepare 階段把所有**巢狀零參數函式**一律 Unsupported，雖然 AW06／P01 原 Checker 已允許，與 P18.1 的已核准合法函式呼叫不一致。
- 工程：移除對巢狀函式的舊 params.is_empty 人為限制，並依原 A2/F02 Parser 的 TermKind::UnresolvedZeroArgs 分類與正式 Checker 函式宣告，核對完整 int TypedHIR、零參數簽章及精確 Lexer 名稱／右括號詞彙來源後，才轉為原顯式工作堆疊 Call；遇 struct、借用或非 int 仍不准偷渡。保留 main 已核准參數函式、原 Checker 簽章、各函式唯一 DeclarationId 與可配置預算。
- 真實正反新增四項：零參數前向葉函式與帶參數父框架混合真返回、零參數葉函式兩次獨立動態呼叫與事件順序、零參數自遞迴 CallBudget 且每次 Entered 都有非成功 Exited、子函式參數宣告身份篡改必在配置動態框架前 SourceChanged。
- 專用真 Gate 由 39 項增至 43 項；G18 原六項的 P18.1、P18.3、P18.5 真來源追溯同步納入，不以子批綠燈宣告任何原項 Passed。
- 本批不是整個 V1 完成：仍待跨一般條件／迴圈／資源、多種合法根呼叫型態、複雜返回與 mixed parameters 的原始完整正反矩陣。未核准語意及 async 不變，公開 MVP-A CLI 亦不變。
- 驗收條件：PR 最新 HEAD 三項必要 CI completed/success，確認作者、main base、mergeable 後 Squash merge；在新 main 同 SHA 真重跑 G18 --audit、G17/G6/F00 回歸。後續 V1 批次應繼續拓展已核准合法來源，最後才可標 P18.1 Passed。


---

## V1-B：跨函式巢狀實參真正求值即時事件（2026-11-06）

- 基線 main 18c24260efcada67084f5ad92cfadf96c2731cc3（#565 已通過三必要 CI 並 Squash merge）。原 Copyable int 真顯式工作堆疊會在所有子實參求值完畢、準備進入目標函式 EnterCall 時才批次產生 ArgumentEvaluated 事件：若第一實參已求值、第二實參溢位，第一實參的真事件會消失，與 P18.1／P18.5「只記錄真正已走到的控制及求值」不一致。
- 新增非語言層的 RecordArgument 動態工作堆疊任務：每個原 Parser 實參由左至右完成 Evaluate 後立即記錄真值，不等待 EnterCall；若下一實參失敗，保留已完成的事件，但目標函式不產生假的 FunctionEntered／Returned。記錄事件本身不消耗另一步 voiles 語意預算，因此 D18-01 原可配置耗盡政策不因證據記錄改變。
- 新正反兩項：兩個兄弟葉函式的多層引數 2／2／3／4／5 必依真求值序列發出且不重複；第二個巢狀實參溢位時第一已完成結果 3 必保留、尚未進入的 pair 零進入事件，全部已進入框架均退出、不殘留參數。
- 原專項真 Gate 43→45 項，G18 原 P18.1／18.5 真來源追溯同步增加。維持原 Lexer／Parser／Checker／TypedHIR、防偽和預算，無新公開語法或例外語意；本子批**不是 V1／G18 正式結案**。
- 先在最新 PR HEAD 核對三項 CI completed/success 與 PR 身分／base／mergeable，再 Squash merge；新 main 同 SHA 重新驗證 G17／G6／F00、G18 --audit，維持原六項 0/6 Passed 直至完整 Gate。


---

## V1-C：main 真零參數普通函式根呼叫與遞迴（2026-11-06）

- 基線 main 4cf068a413adc47ef39ebb3e90436503f7a94d78（#566 成功合併）。原程式的 fn main()->int 可合法 return value()，但私有 Copyable int 呼叫執行器只接受 main 的 TermKind::Call（有實參），且使用必有第一參數的欄位與 all_arguments[0]；原 Parser 對 name() 使用 A2/F02 的 TermKind::UnresolvedZeroArgs。這是已核准零參數正例被人為拒絕的 V1 遺漏。
- 施工：在原 prepare 主函式實際 return 以 TermKind::Call／UnresolvedZeroArgs 兩種真 AST 形態抽取相同完整函式名稱與實參列，仍須原 Checker 確認唯一函式、int TypedHIR、無借用、非 struct。原「第一參數」資料改為明確 Option，不使用虛構 DeclarationId 或 i64 佔位；原有多參數路徑透過 Some 保持相同事件、來源防偽及真執行。零參數函式動態進入框架但不產生虛構 ParameterBound／ArgumentEvaluated。
- 新四項真 Rust 正反：main 呼叫零參數函式正常返回、main 前向零參數串接兩層真正 Return、直接零參數遞迴耗盡 CallBudget 時進出框架逐一成對無假 Return、竄改根函式不存在的參數與實參在執行前 SourceChanged。
- 專項真 Gate 45→49 項，原 G18 P18.1／18.3／18.5 來源追溯同步；沒有公開 CLI、語法或未核准異常語意。**本批仍不是整個 V1 或 G18 正式 Passed**，一般控制流與跨所有權函式仍應推進 V1 後續矩陣。
- 本 PR 須最新 HEAD 三項 CI completed/success、作者與主線 base／mergeable 確認後 Squash merge，合併後主線同 SHA 再驗 G17／G6／F00、G18 --audit。


---

## V1-D：原 Checker Group 括號 int 實參同源交接（2026-11-06）

- 基線 main 35aed82bbb5926edcc330b93045c05a0c0aa8f75，前批 #567 的零參數根函式成功合併。本批針對已核准的普通函式 int 實參包含括號 Group 時，私有 Prepare 僅接受裸 Integer／Name 而 Unsupported 的真執行缺口。
- 實際執行仍由原 Lexer／Parser／Checker／TypedHIR 核准；每層 Group 精確核對原 AST 節點 int 型別、開閉括號 token 來源，然後使用真正內層常值或 DeclarationId，不引入第二套語法分析或隱含轉型。
- 新增三項真正反：兩個括號常值 int 參數的實際綁定與返回、多層括號包住具名 const int 仍保持唯一原宣告身分、括號 bool／不存在名稱必由原 Checker 拒絕。原 Copyable int 函式 Gate 49→52，G18 原 P18.1、18.5 來源追溯更新。
- 本批不支援任意尚未實作的引數算術或具有其他動態效果的實參；V1 與原 P18 六項不提前 Passed。PR 需最新 HEAD 三項 CI completed/success、作者／main base／mergeable 核對後 Squash merge，再於新 main SHA 重驗。


---

## V1-F：未使用純值宣告正反，接續 Group 實參（2026-11-06）

- 原 V1-E／PR #568 已在主線 `aee0a1b451db22123ae24a221583ff5ab3802d24` 合併括號 int 真求值，故以**最新主線重新施作**原 #569 的獨立缺口，避免同時修改 G18 測試清單時互相覆蓋；保留 V1-E 三項 Group 正反。
- 原純值 main 處理在 `verified.plan`、Lexer、TypedHIR 和 DeclarationId 已逐筆核准前置 `const／var int = 整數常值` 後，仍要求其每筆必在函式實參中使用；這違反 Copyable 無資源且合法未使用的宣告語意。此次只移除「未使用即拒絕」人為限制；不放行非 int／非字面、有資源或任意有副作用的語句。
- 修正既有錯誤反例為合法結果 3，保留錯型別與參數個數的原 Checker 拒絕；新增兩項真 Rust 正反：多個未使用 int 混合具名實參應真返回 4、竄改未使用 `main_owners` 原值必在分配動態框架前 `SourceChanged`。
- 原正反 Gate 在 V1-E 52 項基礎上增至 **54 項**，G18 原 P18.1 追溯、CI 名稱與此 AI-build 同步。保持原六項 0／6 正式 Passed；最新 PR HEAD 三必要 CI 成功後 Squash merge，新 main 同 SHA 複驗。


---

## V1-G：main 已核准括號包裹的函式 return 真執行（2026-11-06）

- 基線 main `514b8a72fa5de60b61ec8e6f0b24f54412877908`（#570 的 Group 實參＋未使用純整數合流），原合法 `return (((id(7))))`／`return ((value()))` 在 main 根返回只依 `TermKind::Call／UnresolvedZeroArgs` 辨識而 `Unsupported`。此為 V1 合法函式返回的舊切片限制，並非語言規格禁止括號。
- 工程只於原 main Return AST 逐層剝除 `Group`，每一層先核對原 Checker int TypedHIR、Lexer 真左／右括號 Token 與 UTF-8 半開跨度；最內層仍必須由原 Checker 唯一函式名稱、完整 Copyable int 參數簽章、零借用及真 HIR 授權。真正呼叫跨度採最內層原 Parser 節點，不以來源字串推測新語法。
- 新四項 Rust 正反：三層括號包裹帶 int 參數呼叫返回 8；三層括號包裹零參數前向呼叫返回 23 且零假 ArgumentEvaluated／ParameterBound；篡改原內層 Call source span 必在任何動態框架前拒絕；錯參數型別與未知函式仍由原 Checker 拒絕。
- 專項 Gate 54→58，G18 原 P18.1／18.5 追溯、CI 與 AI-build 同步；僅使用已核准來源，不新增公開 CLI、異常或語法。**V1／P18 六原項仍非正式 Passed。**
- 驗收需 PR 最新 HEAD 三必要 CI completed/success，作者／base／mergeable 與 Squash merge 核對；合併後新 main 同 SHA 再驗 G17／G6／F00、G18 --audit。


---

## V1-H：main 合法括號 int 純值初始化（2026-11-06）

- 基線 main `47f27814910e45ffba6155ee10d164b95a0f6731`（#571 正式三必要 CI completed/success 後已 Squash merge）。原 main Copyable int 宣告僅接受 `TermKind::Scalar(Integer)`，合法 `const base:int = ((((6))))` 被私有執行器誤判 Unsupported；而原 Checker 已可證其為 int 的純常值來源。
- 僅對原 `TermKind::Group` 逐層驗證 Lexer 左右括號 UTF-8 精確跨度、已核准 Checker int TypedHIR，再取最內層原 Integer Literal 與同源 Lexer Token；沒有進行任意算術的 prepare-time 預先求值，沒有新的語法或放寬資源安全。
- 四項真來源 Rust 正反：具名 Copyable 多層 Group 初始化後真函式返回 7、未使用 Group int 宣告不阻擋零參數函式返回 17、Group bool／未宣告名稱仍由 Checker 拒絕、Group 初始化持有者值竄改在動態框架前 SourceChanged。
- 專項 Gate 58→62，G18 原 P18.1／P18.5 真來源、CI 與 AI-build 同步。尚非 V1/P18 完整驗收；主線最新 PR HEAD 三必要 CI success／Squash 後同 SHA 需再次驗證。


---

## V1-I：已核准 Copyable int 具名初始化串接與身分防偽（2026-11-06）

- 基線 main `be979cf1d7106533d6a87061bd4aa9bd934b0f1f`，其上 V1-H／PR #572 的 Group int 初始化已於最新 HEAD 三必要 CI 成功後 Squash merge、合併後 main 三必要 CI completed/success。既有 main 前置宣告切片仍只接受原 int 字面或 Group int 字面；合法 `const origin:int = 6; var copied:int = origin` 會遭私有 Prepare 人為拒絕。
- 本批僅允許使用**相同 main 中已先驗證且完成初始化的 Copyable int** 作為後續 `var／const int` 初始化來源；每次仍須原 Lexer 名稱 Token、Checker ScalarHIR int 表達式、Reference HIR 的唯一 DeclarationId、宣告型別與可用狀態一致。原擁有者不被 Move，也不核發 BorrowTicket；來源不得是未宣告、尚未初始化或不同型別。已驗證 Group 逐層身分核對不變。
- 新增四項真來源 Rust 正反：三段 int 名稱拷貝再真傳參返回 7、Group name 與未使用副本不消耗來源且零虛構引數、bool／未知／未初始化靜態負例，以及竄改複製後持有者 DeclarationId／值皆 SourceChanged 零動態執行。
- 函式真 Gate 62→66 項；G18 原 P18.1／P18.5 稽核清單、必要 CI 名稱與本 AI-build 同步，其他語言或公開執行入口未調整。**尚非 V1 完整驗收，P18 六原項仍 0/6 Passed。**
- 只在最新 PR HEAD 三必要 CI completed/success、作者／base／mergeable 核實後 Squash merge；合併後新 main 同 SHA 再核實 G17／G6／F00 與 G18 `--audit`，不得將 `--audit` 當 `--check`。
