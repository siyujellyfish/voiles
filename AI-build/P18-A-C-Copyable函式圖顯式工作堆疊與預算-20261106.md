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


---

## V1-J：main 真動態整數算式／巢狀呼叫實參交接（2026-11-06）

- 基線 main `33625380aebcdc9d7748611e0babc4e12e63fb5a`，#573 已在原三必要 CI completed/success 後 Squash merge。先前 `main` 呼叫實參僅支援 int 字面、具名 int 及外層 Group，**合法原 Checker int 加減與子函式實參**被私有執行器誤拒；不能在 Prepare 階段猜測或預算動態值。
- 保留原 Lexer／Parser／Checker／TypedHIR 作唯一權威。將既有 `PrivateScalarReturnExpr` 的具名來源解析抽成 `from_checked_bindings`：普通函式參數與 main 已初始化 int 持有者都使用原 Checker Reference HIR 的唯一 `DeclarationId`，算術只接受 HIR 核准的整數加減、原左右端點／運算子；Group／Literal 逐層核對真 Lexer Token；呼叫使用原 Parser／Checker 全簽章，不自行放寬借用／型別。
- 當 main 實參含原 Checker 算式或巢狀函式，將真 `Call` 與子實參編成唯一動態工作計畫 `main_dynamic_call`，以原 `main_owners` 建立執行期來源框架，沿用既有**顯式 expression 工作堆疊與真函式框架**逐個求值；每個實參完成即記錄事件，溢位、步數／框架預算及注入故障均逐框架退出，不補寫未曾真正產生的參數／返回。額外的 main 來源框架不是函式，不計入使用者核准的 D18-01 可配置函式額度。
- 原先簡單字面／具名實參的 66 項真測試、身分對照與事件保持原樣；新路徑只處理尚未接通的真動態實參。新增 `callee_parameter_ids` 源自原被呼叫函式的所有正式 Parameter DeclarationId，避免為動態實參虛構靜態值；`matches_source` 在任何執行前比對完整動態計畫、參數簽章和原全量安全計畫。
- 新增八項 Rust 正反：`sum(1+2,7-4)`、具名 int 混合巢狀 `inc`、零參數葉函式作為引數、第二實參溢位保留第一已完成事件、步數耗盡零假進入、真參數綁定後故障逆序退出、動態 HIR 計畫遭改先拒、錯型別與未知名稱原 Checker 拒絕。專用 Gate **66→74**，G18 原 P18.1／P18.3／P18.5 動態來源追溯同步納入。
- **此批不代表全部一般合法函式／控制流程已覆蓋**；V1/P18 原六項維持 SubsetVerified／Incomplete／BlockedByDecision、0/6 正式 Passed。只在最新 PR HEAD 文件、Rust 建置與完整架構測試全 completed/success、作者／base／mergeable 核對後 Squash merge，新 main 同 SHA 重跑 G17／G6／F00 與 G18 `--audit`。


---

## V1-K：main 動態實參中真正根函式的故障邊界（2026-11-06）

- 基線 main `99bba7608abc69ba2c8f2a545c814f932b81d7f1`（V1-J／#574 的 74 項已通過三必要 CI 並 Squash merge）。在 main 的真動態實參編譯後，既有測試故障注入透過 `frames.len() == 1` 決定「根呼叫」，但 `sum(inc(4),2)` 或 `sum(sum(1,2),3)` 必須先在 main 來源作用域執行子函式；因此它會錯誤中斷第一個實參子函式，而非預期的最外層 `sum` 綁定。
- 將既有 `PrivateScalarTask::Evaluate`／`EnterCall` 工作項保留明確 **is_root_expression** 旗標：只有整個 main Return 根節點的 `Call` 才被判定為此批原已核准的故障注入目標；其所有算術子式、巢狀引數及函式本體呼叫一律不繼承根身份。正式 Lexer／Parser／Checker、動態真執行堆疊、事件、清理與 D18-01 預算不變。
- 新增四項真正 Rust 正反：同名 `sum(sum(1,2),3)` 在故障注入時必先完成內層，再在外層注入且逐層退出；不同名 `sum(inc(4),2)` 子函式真返回與根故障分界；僅一可用函式框架預算時合法 `sum(1+2,3+4)` 必成功（main 的來源框架不占額）；巢狀 `sum(inc(1),3)` 在同樣框架預算下只能先進入 inc，真正尚未進入的 sum 不可虛構 Entered。
- 真 Gate 74→78 項，G18 原 P18.1／P18.3 追溯及 CI／AI-build 同步，僅為內部測試故障政策，不新增 voiles 異常／例外語意；仍不代表 V1 完全 Passed 或 G18 原六項結案。
- 驗收：PR 最新 HEAD 文件、Rust 建置及完整架構測試 completed/success，PR 作者／base／mergeable 確認後 Squash merge，再於新 main 同 SHA 驗證原 G17／G6／F00 與 G18 `--audit`。


### V1-K CI 修正：真正的框架額度是同時存活數，而非累計呼叫數

- 首輪必要 CI 的 P04 真 Rust 測試指出原反例 `sum(inc(1),3)` 在「最多一個真函式框架」下可正常返回 5。這是正確的已核准語意：子函式 `inc` 已先退出，後續 `sum` 可重用同一執行環境額度。**不得為使錯誤測試通過而更動執行器拒絕合法來源**。
- 修正正例為 `g18_a_main_巢狀實參先返回後可合法復用唯一框架額度`，確認兩次真 Entered／Exited 與來源值順序；另新增真正同時巢狀的負例 `outer(1+2)` 內部尚需呼叫 `leaf(x)`，同時只有一框架額度時必 `CallBudget` 且 `leaf` 零進入、`outer` 失敗退出。
- 專用 Gate 由初版 78 **修正為 79** 項；G18 原 P18.1／P18.3 來源對照與必要 CI 名稱同步修改。此為測試契約修正，不改 D18-01 的配置預算語意、不冒充 V1／G18 正式 Passed。


---

## V1-L：main 直接複合函式返回的真動態交接（2026-11-06）

- 基線 main `0f9fe71c8b83994bd6ccd93a4d983248f9f48e2e`（#575 已修正 D18-01 同時框架預算並 Squash merge）；原 main 返回只能是最外層 `Call`／`UnresolvedZeroArgs`，儘管原 Checker 已核准 `return inc(5)+2`、`return inc(2)+inc(4)` 或 Group 包裹的 int 算式。此為私有 evaluator 的合法函式結果再運算覆蓋缺口。
- 只使用原 Parser int AST 搜尋具名 `Call`／`UnresolvedZeroArgs` 以取得原已核准函式圖，不將純算術冒充函式驗收。選中的函式仍需原 Checker 已核准的完整 int 參數簽章、TypedHIR／Parameter 身分；其餘同一來源所有合法函式由既有巢狀呼叫表管理。對 main `BinaryInt` 根返回使用 `from_checked_bindings`，逐層核對已核准 arithmetic HIR 端點／運算子、名稱 Reference HIR／DeclarationId、真 Lexer 字面與 Group Token，**所有運算在顯式工作堆疊執行時才求值**。
- 由於 `return inc(2)+inc(4)` 並無唯一「根函式」可供內部測試 `execute_with_fault(true)` 指定綁定後故障，該測試專用要求明確 Unsupported，禁止假造或默默忽略；正式正常執行、溢位和 D18-01 執行預算仍依真來源與真事件。
- 新增七項真 Rust 正反：函式結果與常值相加；兄弟函式左至右兩次真正返回；多層 Group 複合返回；子函式成功後外層算術溢位而不假造最後 Return；竄改編譯動態樹執行前拒絕；型別錯誤／未知名稱仍由 Checker 拒絕；多兄弟複合返回無單一根故障點時不得默默跳過故障。專項 Gate 79→86，G18 原 P18.1／P18.3／P18.5 與 CI／AI-build 同步。
- **未更動** P13～P16、Move／BorrowTicket／ResourceId、安全退出的核准語意，亦未發布 P22 CLI 或先決定 D18-02 async 細節；V1／G18 原六項仍不提前 Passed。PR 最新 HEAD 三必要 CI completed/success／作者、base、mergeable 核對後 Squash merge，合併 main 同 SHA 重跑 G17／G6／F00 與 G18 `--audit`。


---

## V1-M：普通函式已核准 Copyable int 區域初始化與逐框架清理（2026-11-06）

- 基線 main `40c7fc73c4e85a5af7441bad955f25dc73e49738`；G18 函式真 Gate 已 86 項且最新 main CI success。先前私有 evaluator 只允許普通函式本體 **一筆 return**，合法 `fn calc(n:int)->int { const one:int=1; var next:int=n+one; return next+2 }` 遭人為 Unsupported，不能將其當作已核准語意不合法。
- 新增 **PrivateScalarLocalInit**：每個普通函式前置 `var／const int` 必須是原 Parser 真 Declare，逐筆核對原 Checker `DeclarationId`、int 型別及合法初始化式。使用既有 `from_checked_bindings` 核對所有名稱 Reference HIR、BinaryInt Arithmetic HIR、Lexer 真字面／Group 來源及已核准普通函式完整簽章。每筆新的名稱只在其真正宣告之後加入該函式的編譯環境；未初始化、型別錯誤或其他語句不得跳過。
- 將完整 `root_local_inits`／各 `PrivateScalarNestedCallee.local_inits` 納入來源防偽、函式圖的全部真呼叫邊與真動態框架。在原顯式工作堆疊加入 `BindLocal(DeclarationId)`，每次進入函式先依原宣告順序執行各初始化式，再求值 return；每框架有獨立的 `BTreeMap<DeclarationId,i64>`，不得共享其他遞迴框架的持有者、不得發出假 ParameterBound／Returned；退出或故障時逐框架清理。
- 新增七項原 Lexer／Parser／Checker 真正 Rust 正反：多筆區域初始化與參數返回、巢狀父子函式各自局部持有者、main 動態實參呼叫有區域宣告的函式、區域算術溢位正常失敗退出、多層遞迴框架與 CallBudget、竄改已驗證局部初始化式必先 SourceChanged、非法型別或未知名稱靜態拒絕。
- 專用 Gate **86→93**，原 G18 P18.1／P18.3／P18.5 同 SHA 真來源追溯、CI 名稱及 AI-build 同步。此子集仍不允許任意賦值、控制流程或未核准的資源操作；不修改公開 CLI、P13～P16 的安全契約，P18 原六項不提前 Passed。正式合併只依最新 PR HEAD 三必要 CI success、作者／base／mergeable；Squash merge 後新 main 同 SHA 再驗。


---

## V1-N：Copyable int 真初始化／離域事件帳簿（2026-11-06）

- 基線 main `2e024299a83f480d056ba1b7a094381daacd9877`（#577 最新 HEAD 三項必要 CI success、Squash merge，且合併後 main 同 SHA 三項 CI 全 success）。V1-M 已接通真函式區域初始化，但事件中尚缺少能獨立驗證「哪個已核准 DeclarationId 真的初始化、哪個未完成、離域順序為何」的動態事實。
- **新增純值專用事件**：`LocalInitialized(DeclarationId, i64)` 僅在原 `BindLocal` 真寫入成功後產生；`LocalScopeEnded(DeclarationId)` 在正常退出或錯誤／CallBudget 展開時，對該動態框架真正已完成初始化的局部宣告反序記錄。每個 `PrivateScalarActiveFrame` 擁有獨立 `initialized_locals`，即使相同函式被遞迴重入，也不得共享已初始化列表。
- 這些是 **Copyable int 作用域可觀察事件，不是 ResourceId／ReleaseEvent，不得拿來充當 P18.3 str 資源釋放證據**；原 Checker DeclarationId、BorrowTicket 安全及任何公開 CLI 均不變。一般呼叫的根框架、巢狀子框架與 main 動態實參框架在正常／異常退出時都使用同一真初始化／逆序離域演算法，未成功初始化者零事件。
- 新增三項真 Rust 正反：兩筆區域初始化 4、6 與反向離域順序；第二筆運算溢位時只回收已完成第一筆，絕不造假第二筆；直接遞迴跨三真框架，儘管靜態 DeclarationId 相同，各次的值仍為 2、3、4，預算耗盡後恰三次離域、零假返回。
- 專項 Gate 93→96 項；G18 原 P18.1／P18.3 追溯、CI 與 AI-build 同步，P18 正式原六項仍 **0/6 Passed**。僅最新 PR HEAD 三必要 CI 全 success，核對身分／base／mergeable 後 Squash merge，再以新 main 同 SHA 驗證，不以專用事件代替正式資源模型。


---

## V1-O：main 已核准動態 int 初始化與函式結果暫存（2026-11-06）

- 基線 main `5713e252a063ae34dbe062604fa8b41d6c53249f`（#578 新增 Copyable int 真初始化／離域事件並通過同 SHA 完整 CI）。先前只允許 main 的前置 `var／const int` 由整數字面、括號字面、已初始化 int 名稱直接產生，而 `const base:int=2+3; return inc(base)` 以及 `const result:int=inc(4); return result+1` 均為原 Checker 核准的合法純值流程，私有執行器卻 Unsupported。
- 本批不更動 Lexer／Parser／Checker／TypedHIR 或語言規格：如果 main 前置宣告含合法 int 算式或函式呼叫，**所有前置 int 宣告交由既有 `prepare_function_locals(main,...)` 逐筆編譯成 `main_dynamic_inits`**；其值只在真執行階段以原 `DeclarationId`、Reference HIR、Arithmetic HIR、Lexer Token 與顯式工作堆疊求得。原既有字面／名稱純值快速路徑保持不變。
- main 最終 `return` 即使只引用先前函式結果的具名 int，也能從前置宣告的原 Parser AST 找到已核准普通函式圖並依同一原 Checker 型別決議執行；不允許將**完全沒有函式呼叫**的純 main 算式冒稱為 P18.1 函式驗收。前置動態宣告發生溢位、步數耗盡時，不跳過已完成宣告的逆序離域，也不執行尚未可達的後續函式呼叫。
- `main_dynamic_call` 是原動態 main return 工作計畫，`main_dynamic_inits` 具有獨立來源一致性比對；使用同一 `evaluate_graph` 在最初 main int 來源框架先依序執行宣告、再返回；這個 main 來源框架不計入可配置 D18-01 使用者函式框架數。沒有新增 str 資源／借用語意，現有 P13～P16 安全界線與公開 CLI 不變。
- 六項真 Rust 正反：main `2+3` 初始化後傳參、函式結果存入 const 再純算術 return、前向零參數函式結果與 Copyable 副本、第二初始化運算溢位且只清理已完成宣告、竄改動態初始化計畫執行前 SourceChanged、錯型別與未知名稱維持原 Checker 拒絕。專項 Gate **96→102**，G18 原 P18.1／P18.3／P18.5、CI 與 AI-build 同步。
- 本批仍是 V1 子集，不以增加正反案例宣稱原 P18.1 正式 Passed；P18 原六項保持 0/6。依 PR 最新 HEAD 三項必要 CI completed/success、作者/base/mergeable 核對後 Squash merge，新 main 同 SHA 再驗。
