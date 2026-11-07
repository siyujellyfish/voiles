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


### V1-O 首輪 CI 數值預期修正

- P04 Rust 測試首次執行的 1548 項已通過，惟新測試 `inc(4)` 返回 5 後 `main` 的 `return result+1`，原斷言誤寫為 7。實際正確結果 **6**；執行器與原 Checker 的已核准整數加法一致。
- 僅將此項測試預期修正為 6，保留對 `Returned(5)`、唯一真函式執行與局部初始化的檢查。**不能為符合錯誤測試而更動計算語意**；以修訂後最新 HEAD 三必要 CI 重新驗收，未成功前不得合併。


---

## V1-R01／R02-01｜普通函式原語句次序與已核准 var int 更新（2026-11-07）

- 基線 `main=6cfb79aca1edf4f785b1626780aa99886c2b9916`，102 項函式真來源 Gate 和新 main 的三必要 CI 全 success。依前述 V1 R01～R07 完整施工規劃，首批將原有僅 `Declare…Return` 的執行形狀向**有序語句計畫**延伸，不藉此宣稱已接通所有 if／迴圈／資源。
- 新增 `PrivateScalarLocalStep` 的真 `Declare`／`Assign` 兩種語句；由原 Parser 的 `StmtKind` 順序準備，由原 Checker `DeclarationId`、`typed_declaration(mutable)`、`VerifiedScalarHir.assignment`、原 Lexer Name Token 與每個 RHS 的 `TypedHIR` 編譯，不自行解讀語法、不允許 const 重賦值。每個函式的 `root_local_steps`、`nested_callees.local_steps`、`main_dynamic_steps` 都納入來源重驗，原有 `root_local_inits` 繼續作為只讀動態宣告追溯。
- 同一顯式工作堆疊 `AssignLocal(DeclarationId)` **只在 RHS 真求值完成後更新原框架已存在 slot**，寫入時產生 `LocalAssigned`；失敗不更新，也不假造新 `LocalInitialized`／新資源。所有呼叫的區域宣告／寫入保持原來源順序，各真呼叫框架具獨立 slot。原已核准的 `LocalScopeEnded` 仍僅對真正初始化的局部宣告記錄離域。
- 新增七項原 Checker 真 Rust 正反：多筆更新順序、RHS 子函式先返回再更新、main 多次更新後傳參、兩函式各自更新、第二 RHS 溢位不寫入且真離域、const／錯型別／未知目標 Checker 拒絕、竄改已編譯賦值在執行前 SourceChanged。
- 函式專項 Gate **102→109**；原 G18 P18.1／P18.3／P18.5 追溯、CI 與 AI-build 同步。**R01/R02 只是首批子集**：仍待控制流程、條件提前返回、所有權混合函式及完整總驗收。原 P18 六項仍 0/6 正式 Passed；最新 PR HEAD 三必要 CI completed/success、作者／base／mergeable 檢查後 Squash merge，新 main 同 SHA 重驗。


---

## V1-R03-01｜真 bool 條件、提前 Return／ElseIf 與動態退出（2026-11-07）

- 基線 `main=26c85ff52b03497da181ab59b1d5ca628c4b8688`，R01／R02 首批 #580 已最新 HEAD 三必要 CI success、Squash merge。尚缺函式本體的原 Checker if 返回控制轉移：舊函式只有最終 Return，即使前置已核准 if 分支有 return 亦 Unsupported。
- 引入 `PrivateScalarBoolExpr` 的真 bool 常值、具名與 Group 來源：原 Parser AST、原 Checker ScalarHIR bool Expression、Reference HIR 的唯一 DeclarationId、Lexical Name／Keyword／括號都必須同源。具名 bool 宣告與原 Checker 可寫 var bool 更新逐筆記入 `PrivateScalarLocalStep`。布林純值在每一個真函式框架有獨立 slot，不產生資源擁有權／BorrowTicket。
- 原 Parser `StmtKind::If` → 已核准單筆 return 葉分支 `PrivateScalarEarlyIf`，包含 `if／else if／else` 原宣告順序；僅在執行到該條件時產生 `ConditionalChecked`，只選擇一個真分支，所選 return 以同一顯式工作堆疊求值，跳過**該函式**後續不可達工作項但保留父函式的 `ExitCall`。不接受 if 分支內其他未知語句或新增 bool 語意；不藉常值預判跳過真 Checker。
- 動態 bool+int 局部宣告統一 `PrivateScalarLocalBinding` 的實際建立順序，函式成功、溢位、故障或受控預算退出時反向記錄 BoolScopeEnded／LocalScopeEnded；這兩者都是 Copyable 純值離域，**不是 ResourceId 或 ReleaseEvent**。
- 八項真 Rust 正反：true 早返不可達路徑、false 落至尾端 return、var bool 更新後條件真重讀、else-if 依序選擇、巢狀提前 return 僅退出該框架、所選提前 return 溢位只回收真初始化、const bool／未知／錯型別 Checker 拒絕及來源條件計畫篡改前 SourceChanged。
- 函式專項 Gate **109→117**，G18 原 P18.1／P18.3／P18.5 追溯與 CI 同步。這只完成 R03 的真分支／提前返回子集；for／while、任意區塊作用域與 str 資源仍在 R03／R04～R07 待整合，P18 原六項仍 0/6 Passed。須最新 PR HEAD 三必要 CI completed/success、作者／base／mergeable 及新 main 同 SHA 驗證。


### V1-R03 首輪 CI 修正：全 if 分支返回後的 Checker 不可達 HIR

- 第一輪 CI Rust P04 共 1563 項成功，僅新增 else-if 回歸因尾端 `return n+4` 無 HIR 而被 `Unsupported`。原 Checker 能證明本來源 `if(first) return／else if(second) return／else return` 全部返回，所以尾端在原靜態控制流上完全不可達，不應造假必需的 TypedHIR。
- 新增 `PrivateScalarEarlyIf::always_returns()` 只對完整已編譯真葉返回的 else／else-if 鏈證明全路徑退出；**僅當原 Checker 確實沒有該尾端 Expression HIR** 才使用不可達 Sentinel。其執行期若被意外執行，回報 `StateMismatch`，絕不假裝正常 int return；不省略任何仍可達且缺 HIR 的合法執行。
- 已更新 Rust 原來源邊界與文件。需要在修訂後最新 PR HEAD 三項必要 CI 全 completed/success，再合併。


---

## V1-R04-01：原 Checker int/bool 混合正式函式參數（2026-11-07）

- 基線 main `67863e834ba58b78ee7dabe84ed7359f40df8d70`（R03 #581 以最新 HEAD 三必要 CI success 合併）；原 evaluator 已接通 bool 局部宣告與提前 Return，但所有普通函式簽章和引數仍強制 `int`，原 Checker 核准的 `fn choose(flag:bool,n:int)->int` 遭人為 Unsupported。
- **保持真 Checker 完整型別決議**，在 `PrivateScalarCallArgument` 明確分流 `Int／Bool`。參數實參解析需原 Lexer、TypedHIR bool／int、Reference HIR、DeclarationId 及原 Checker 簽章個數／順序完全相符；不得用 runtime 值推斷類型。原動態顯式工作堆疊可以於 int 暫存槽中短暫編碼 bool 0／1 供已核准參數傳遞，進入正式函式 frame 時**必依簽章重新恢復 bool slot**；不得讓 bool 參與 int 算術，也不產生借用或 Move。
- 新增 `BoolArgumentEvaluated`／`BoolParameterBound` 真事件，並且為每個呼叫函式保存 `parameter_is_bool` 真 Checker 完整簽章；`main` 即使傳字面 bool 也改走原動態實參工作堆疊，巢狀函式 int/bool 混用不共用參數 slot，函式內 bool Name 讀取同一框架。
- 新增七項 Rust 原來源正反：bool true 單參數提前 return、bool false+int 混合宣告順序、巢狀 bool+int 跨兩框架、main bool var 更新後傳參、綁定後故障真退出、錯型別／漏參數原 Checker 拒絕、竄改簽章布林標記執行前 SourceChanged。函式專項 Gate **117→124**，G18 原 P18.1／P18.3／P18.4／P18.5 與 CI 追溯同步。
- **R04 仍未全部完成**：str OwnedStr／BorrowTicket／明確 move 的多參數混合來源已有獨立 G18 執行器，但尚未與此 Copyable 函式圖共享同一統一框架／退出協定；此批不將 bool 純值事件冒充真資源釋放或 P18.4 全部驗收。P18 六原項維持 0/6 正式 Passed。僅依 PR 最新 HEAD 三必要 CI completed/success 並 Squash merge、新 main 同 SHA 再驗。


### V1-R04 CI 修正：具名 bool 區域來源須交付同一原 Checker 綁定表

- 第一輪 Rust P04 有 1570 項成功、唯一新正例 `main` 在 `switched = true` 後傳 `choose(switched,5)` 被私有 Prepare 錯誤拒絕。原 Checker 已確認合法，錯誤原因是 bool 宣告只加入條件專用 `bool_bindings`，尚未加入普通函式實參共用 `bindings`。
- 修正真 bool 區域宣告成功後同時交付同一 `DeclarationId` 至共用來源名稱表；bool 實參仍須原 HIR bool 類型、真 Lexer Name 與宣告身份一致，不進行隱含 int 轉型。不是放寬錯誤程式或測試預期。
- 以最新 HEAD 重跑三項必要 CI，全 success 才能合併本批。


---

## V1-R05／R06-01：真 int／bool 動態框架與離域事件強制一致性（2026-11-07）

- 基線 main `ec5d16bb822e4035ae5d3974d83a7b7ae1d9a8b0`（R04 #582 以最新 HEAD 三必要 CI success 後 Squash merge）。原 Copyable 執行器雖真正生成 Entered、ParameterBound、LocalInitialized／LocalScopeEnded、Returned 與 Exited，卻尚無**同一動態事件帳簿交付前的一致性守門**。單靠 individual Rust 範例無法保證未來修改不會誤發重複 Returned、失序離域或退出錯框架。
- 新增 `PrivateScalarCallOutcome::has_balanced_frame_trace`：動態逐事件推進框架堆疊。每個 `FunctionEntered(name)` 建立獨立生命週期，ParameterBound／BoolParameterBound 只能在活躍函式中，LocalInitialized／BoolInitialized 只能各框架真正建立一次，LocalAssigned／BoolAssigned 僅能修改該框架已建立的局部宣告；LocalScopeEnded／BoolScopeEnded 嚴格依原始實際初始化的**反向型別與 DeclarationId** 配對；Returned 恰一次且只在合法真函式，Exited 的名稱／returned flag 與本框架事實必完全一致。遞迴重入即使同一靜態 DeclarationId，也以獨立動態框架核對，禁止跨框架錯認清理。
- `PreparedPrivateScalarCall::audit_outcome` 現在在零預算、一般返回與 main 動態分支**對外交付前**全部強制通過同一 trace guard，若不一致則 `RuntimeTraceMismatch` 而非冒充成功或忽略；不更動已核准 D18-01 的語言錯誤／預算規則。
- 新增七項真 Rust 正反：混合 bool/int 巢狀提前返回、算術溢位失敗退出、重入同宣告身份的受控遞迴、假初始化／錯序／重複離域、偽造 Return 與 Exited 名稱或結果、跨型別離域錯序，以及沒有有效框架的假參數或 bool 更新。
- 專項 Gate **124→131**，G18 P18.1／P18.3／P18.5 和必要 CI、AI-build 同步。**本守門只驗證 Copyable int／bool 的 Scope Event，不是 ResourceId 或真 BorrowTicket／ReleaseEvent**。R05 還須跨 `execute_recursive_str`／`execute_call_borrow`／Move／self 等執行器整合原 ResourceId 與 BorrowFact 退出責任，R06 全矩陣、R07 正式驗收仍未達標；P18 六項仍 0/6 Passed。
- 僅最新 PR HEAD 三必要 CI completed/success、作者/base/mergeable 確認後 Squash merge，再於新 main 同 SHA 重驗。


---

## V1-R05-02：已核准真遞迴 str、BorrowTicket 與資源離域聯合守門（2026-11-07）

- 基線 main `5576c3547ef0f720d564eeb17e39f4d4b557824a`（#583 Copyable 真框架一致性 Gate 131 項已以 PR 最新 HEAD 及 main 同 SHA 三必要 CI success Squash merge）。既有 `execute_recursive_str.rs` 已於真 Rust 中建立原 Lexer／Checker str 局部和原 BorrowFact 已核准的動態 BorrowTicket，正常或故障均會以 OwnedStrRuntime 產生真 `Created／Released` 與獨立 tracker 空快照。尚缺與 R05 純值框架同樣的**所有返回結果交付前**完整動態框架棧稽核，單靠 ResourceId 正確並不能證明未假造 main/子框架退出順序。
- 本批新增 `PrivateRecursiveStrOutcome::has_balanced_resource_frame_trace`，同時要求：原真 `OwnedStrRuntime::event_trace_balanced`（ResourceId 每份唯一 Created 與恰一次 Released）、實際 tracker 的 `loans_returned` 空集合及數量核對已通過、原動態 `PrivateRecursiveFrameEvent` 的 main 首次進入、各框架 LIFO 名稱配對與 `returned == result.is_ok()` 一致；零預算真未進入狀況必零資源／零借用／零虛構框架，只有受控錯誤。任何錯誤 `audit_before_return` 保守回報真 `CleanupTicket(SnapshotMismatch)`，不將不可信 Outcome 交給使用者。
- 新增七項真 Rust 正反：直接與互相遞迴正常、框架預算耗盡及第 N 次 OwnedUtf8 故障、零預算未建立任何實例、竄改退出函式名稱／returned flag、造假未歸還票據或刪除真 ReleaseEvent、同真 ResourceId 表正確但偽造多重 main／漏退出。由原 `execute_recursive_str` 的原 Checker 真來源執行，不採用模擬資源當 Passed 證據。
- P18.3／P18.5 原六項追溯、原遞迴資源專用 Gate **43→50** 及 CI/AI-build 同步。此批完成 R05 的**遞迴資源子集聯合退出守門**，仍非把 Copyable 與 OwnedStr/Move/self 方法完全收束為一個通用 runtime；不自行擴張語言資源規則、異常或 async。R05/R06/R07 仍有交叉矩陣及共用動態執行模型缺口，G18 原六項維持 0/6 Passed；最新 PR HEAD 三項必要 CI success 才 Squash merge，合併 main 同 SHA 重驗。


---

## V1-R03-02：已核准 bool 條件中的可寫 int/bool 分支後續執行（2026-11-07）

- 基線 main `272e7635fcfdac59312d62e9cf1eaf9e1a7ea51b`，#584 已在 PR 最新 HEAD 三必要 CI success 後 Squash merge。原 R03 `PrivateScalarEarlyIf` 僅支援葉分支一筆 `return`；但合法原 Checker `if(flag){value=value+1}; return value` 必須讓 if 分支**更新後繼續執行**後續語句，不能偷當提前返回或在 Prepare 人為拒絕。
- 不改原 Lexer／Parser／Checker：為既有 `if/else-if/else` 編譯葉分支新增 `PrivateScalarBranchAction::Return／AssignInt／AssignBool`，每個葉仍必是真 Parser 的單筆 `Return` 或對**原有已初始化可寫** int/bool 宣告的單筆 Assign；完整核對 `typed_declaration(mutable)`、`ScalarHIR.assignment` 的原 DeclarationId／型別、Lexer Name Token 以及 RHS Checker TypedHIR。分支新區域宣告、任意深度區塊與資源此批仍未核准此子集。
- 執行真選中 `Return` 才刪除本函式不可達後續工作項；選中 `AssignInt／AssignBool` 則依現有顯式工作堆疊先真求值 RHS、更新當前 frame slot、產生 LocalAssigned／BoolAssigned，**原程式後續語句必繼續執行**；未選中的分支沒有任何變更、返回或虛構事件。所有必需真位元資料仍由原 Checker 裁決。
- 補七項原來源 Rust 正反：true 分支更新後返回、false 分支完全不執行 RHS、else-if 原順序選擇唯一更新再呼叫、bool 更新後下一個 if 真重讀、分支內巢狀函式溢位不補寫或假返回、const／錯型別由原 Checker 拒絕、已編譯分支 RHS 被竄改先 SourceChanged。
- 專項 Gate **131→138**，G18 原 P18.1／P18.3／P18.5 追溯、CI／AI-build 同步。此批降低 V1 R01/R03 真控制流缺口，但尚不表示完整 for／while、局部區塊資源與 R01～R07 已結案。G18 六原項維持 0/6 Passed，必須最新 PR HEAD 三必要 CI completed/success、作者/base/mergeable 後 Squash merge，新 main 同 SHA 再驗。


---

## V1-R05-03：跨普通 str 借用／Move＋Borrow／同步 self 動態結果聯合守門（2026-11-07）

- 基線 `096de88f07b7d9fedd63010587cf2f7384c60efc`，#585 已以最新 PR HEAD 三必要 CI success Squash merge。繼 R05-01 Copyable 框架和 R05-02 真遞迴 str 來源／資源聯合守門，其他獨立執行器僅核對 OwnedStr `event_trace_balanced`，缺少「票據已歸還、合法進入／退出、真所有者資源數量」交叉驗證。
- 在既有 `execute_call_borrow.rs` 對原 Checker 已核准的共享／獨占普通函式 str 借用，新增 `PrivateCallBorrowOutcome::has_balanced_borrow_resource_trace`：完整 ResourceId 對稱，確實一份 Created/Released，BorrowSafety 原票據結束後可直接存取來源，真參數框架 0，函式 entered/exited 與真結果一致。不新增動態借用權限或改動錯誤語意。
- 同一原 Checker Call 的 `str.move`＋另一獨立來源 str BorrowTicket，新增 `PrivateMixedOwnershipOutcome::has_balanced_mixed_resource_trace`：真兩個獨立 OwnedStr 身分各一次 Created/Released、BorrowTicket 必先歸還、正常成功有真讀取觀測，避免「空事件表也平衡」冒充合法來源。
- 同步方法 `execute_method_self.rs` 原 Checker 已核准的 int 欄位共享／獨占 self 借用，新增 `PrivateSelfMethodOutcome::has_balanced_self_borrow_trace`：真票據必歸還，無 OwnedStr 時任何假 ResourceEvent 均拒絕，方法 entered/exited 必依實際結果；上述三守門全部在真正 Outcome 對外交付前執行，不因測試後才核對而放行無效事件。
- 新增六項原執行器真來源 Rust 正反：普通共享／獨占 str 成功與注入故障、假歸還／假進入／丟 ReleaseEvent、Move＋Borrow 真兩資源、偽造空表／未歸票據／未實讀、同步共享／獨占 self 成功與失敗、假借用歸還或憑空添加 Created+Released。G18 原 P18.3／18.4／18.5／18.6 稽核與三組專用 Gate 32→34、8→10、38→40，同步更新 CI。
- 這是各自既有原 Checker／BorrowFact 真執行器的退出結果合約收緊，**仍非單一通用 OwnedStr＋Copyable＋方法＋異步 interpreter**；多擁有者跨不同函式／迴圈與完整 V1 R06/R07 正反交叉尚未封閉，G18 原六項保留 0/6 Passed。只在 PR 最新 HEAD 三必要 CI completed/success、身分／base／mergeable 全核對後 Squash merge，新 main 同 SHA 再驗。


---

## V1-R06-01：跨 Copyable、真借用、Move、self、遞迴資源退出整合矩陣（2026-11-07）

- 基線 main `bffbab00602485c4146f12d2a39f3bf5d8ad4ffa`（#586 R05-03 真 BorrowTicket／Move／同步 self 結果聯合守門已最新 PR HEAD 三必要 CI success Squash merge）。
- 建立 **`struct_stage::g18_r06_cross_runtime_tests`** 五項**同一 Rust 測試內**跨既有原 Lexer／Parser／Checker 真來源執行器的動態結果驗證，而非僅用靜態語法測試替代：一、Copyable bool/int 分支賦值與真普通 str BorrowTicket 同時通過各自守門；二、真整筆 Move＋另一 str 借用兩個 ResourceId 與同步方法 self 無 OwnedStr 正確區分；三、遞迴 str 借用在 D18-01 CallBudget 耗盡下無假返回且釋放所有 Created，普通函式 BorrowTicket 注入錯誤亦歸還；四、int 溢位、同步 self 故障及混合 Move＋Borrow 中途故障都不假返回且守門有效；五、對純值離域、借用歸還、真資源事件與遞迴票據的偽造分別拒絕。
- 新增獨立工具 `tools/核對G18-R06跨執行器正反矩陣.py`（UTF-8），真 Rust `--list` 精確核對五項後完整執行該測試組，要求正好 5 passed、0 failed、0 ignored；同步新增必要 CI G18 R06 專項步驟和 G18 原 P18.1／18.3／18.4／18.5 追溯與真執行群組。
- **證據邊界**：這是多執行器跨模型回歸，不是宣稱所有已核准函式與任意 str／Move／Borrow 組合已共用一個通用 runtime。沒有新增語言規格、async、except 或 P22 公開執行入口。R06 全矩陣和 R07 正式 G18 `--check` 仍待封閉，原六項保持 0/6 Passed。
- 僅最新 PR HEAD 三必要 CI completed/success、作者／main base／mergeable 核對後 Squash merge，新 main 同 SHA 再驗全套 G17/G6/F00 與 G18 `--audit`；不能以 CI 綠燈冒稱正式驗收。


---

## V1-R03-04：已核准 if／else-if／else 同葉多筆 int/bool 更新真執行（2026-11-07）

- 基線 main `48aac8e094c7f863b7758c2f82b98fad1b4b6a5c`；#587 已新增 R06 五項跨各真執行器聯合矩陣並合併。#588 的不同方案已在 CI 遭原 Checker 明確拒絕：**函式最外層最後必須是明確 return**，因此 #588 關閉未合併，禁止擅改 Parser／Checker 或用不合法程式虛增 V1 覆蓋。
- 改以真正已核准來源 `if(flag){ value=value+2; value=value+3 }; return value` 施工：原 `compile_early_branch` 只接受單筆 `Return／AssignInt／AssignBool`，將同一真 Parser 分支的多筆合法純值賦值依原序編譯為 `PrivateScalarBranchAction::Sequence`，每筆仍必有原 Checker Assignment HIR、DeclarationId／可寫權限、Lexer Token 與相符 int/bool RHS TypedHIR。序列中已 Return 的位置不得再接假可達語句。
- 原顯式工作堆疊新增 `ExecuteBranchAction` 任務，以 LIFO 反序排入、實際原序求值；選中 Return 才截斷本函式不可達工作，其餘 Assign 必依真值更新現有 frame，再執行後段；未選中任何分支不建立任務、沒有假事件。完整葉返回的 `always_returns` 也遵循 sequence 最後真 Return，但仍要求函式語法有已核准**明確最後 return**。
- 六項真 Rust 正反：兩次 int 更新與 false 零更新、else-if 只選一個真葉並按次序執行、同分支先 int 後 bool 更新而下一 if 重讀、先更新後提前 return 不執行不可達語句、第二更新溢位只保留首筆完成與真失敗退出、const 錯誤賦值及已編譯 action 序列遭竄改必先拒絕。函式專項 Gate **138→144**，G18 原 P18.1／18.3／18.5、必要 CI 與 AI-build 同步。
- 這批不包含分支內新區域變數、完整巢狀分支／一般 for/while，也不完成 OwnedStr/Move/Borrow 統一 runtime；保持 R01～R07 未封閉清單與 G18 0/6 Passed。只在最新 PR HEAD 三必要 CI completed/success、作者/main base/mergeable 確認後 Squash merge，再核對新 main 同 SHA。


---

## V1-R03-05：原 Checker 已核准巢狀 if／else 真分支與多筆更新（2026-11-07）

- 基線 main `546d8c468e44707a52e7461497a2acec038e66ae`（#589 R03-04 多筆 int/bool 分支更新已 PR 最新 HEAD 三必要 CI completed/success Squash merge）。原私有 `compile_early_branch` 只接通單筆 Return／Assign 或多筆純值 Sequence；但原 Parser/Checker 允許合法已核准的巢狀 `if`，目前仍在人為分支計畫中被 Unsupported。
- 將原 Parser `StmtKind::If` 巢狀節點編譯成 `PrivateScalarBranchAction::NestedIf(Box<PrivateScalarEarlyIf>)`，只依原 `PrivateScalarBoolExpr::from_checked` 和原 Checker／Lexer／Reference HIR 驗證，不自行解釋語法或放寬型別、所有權。每筆 LIFO 真工作任務在前一筆 int/bool 更新完成後，才以**最新同框架 bool 值**執行新的 `CheckEarlyReturn`，未走分支不產生事件。
- 內層提前 Return 仍只退出目前函式，外層父函式後續運算繼續；錯誤時先回收已真初始化的值並保留完成更新事件，不能虛構 Return 或執行被跳過兄弟分支。 `always_returns` 判定同時遞迴使用原已編譯巢狀真 If，惟函式最外層仍依原 Checker 必須有明確最後 return（不合契約 #588 已關閉、未合併）。
- 新增六項真 Rust 正反：巢狀 if 提前返回雙重條件順序、外層先變更 bool 再由內層真重讀並更新 int、外層 false→else 的內層 false→else 唯一執行、葉返回僅退出內函式而外函式加法續行、深層多次賦值在第二次溢位受控失敗、非法 Checker 名稱與已編譯巢狀計畫被篡改前置拒絕。
- 專項 Gate 144→150；G18 原 P18.1／18.3／18.5 真來源追溯、CI 與 AI-build 同步。此批仍未接通分支內真資源局部／一般 for/while 與全參數混合函式，R03/R06/R07 保持未正式結案，原六項 0/6 Passed。只在最新 PR HEAD 三必要 CI completed/success、作者/base/mergeable 確認後 Squash merge，新 main 同 SHA 再驗。
