// G18／P18.4、P18.6：原私有 Checker 已核准的同步零參數唯讀
// struct 方法 self，真正 BorrowTicket 及 MixedStructRuntime Copyable int。
// 僅接通原 Checker 已核准的同步方法、多個 Copyable int 常值實參真值；不擴張 async 或公開 CLI。
#[derive(Debug)]
enum PrivateSelfMethodPrepareError {
    Check(crate::Diagnostic),
    Unsupported(&'static str),
    SourceChanged,
    Cleanup(crate::mixed_struct_runtime::MixedError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateSelfMethodRuntimeError {
    Resource(crate::mixed_struct_runtime::MixedError),
    Ticket(crate::borrow_safety::BorrowSafetyError),
    WrongReceiver,
    InjectedFailure,
    IntegerOverflow,
}

#[derive(Debug)]
struct PrivateSelfMethodOutcome {
    result: Result<i64, PrivateSelfMethodRuntimeError>,
    /// true MixedStructRuntime 的 ResourceEvent，沒有 str 就不得假造 Created。
    events: Vec<crate::owned_str_runtime::ResourceEvent>,
    method_entered: bool,
    method_exited: bool,
    ticket_returned: bool,
    /// B4：原 Checker 普通函式在真正 self 框架內執行的已完成次數。
    ordinary_calls_completed: usize,
    /// M2：真正進入同步方法後才觀測的原 Checker if 條件。
    /// 未進入的分支、借用前錯誤及方法入口注入失敗不得填寫。
    branch_checked: Option<bool>,
    /// M2：外層真選中才會進一步觀測原 Checker/Lexer 的巢狀 bool 值。
    nested_branch_checked: Option<bool>,
    nested_write_applied: bool,
    nested_else_write_applied: bool,
    expected_nested_write: bool,
    expected_nested_else_write: bool,
    /// 已核准巢狀條件靜態值，與真正選葉對照防偽。
    expected_nested_branch: Option<bool>,
    /// M2：只在已選中原 Checker if 葉且持有獨占票據時寫真 int 欄位。
    branch_write_applied: bool,
    /// 只有真落在 else 葉、具原 Checker ExclusiveWrite 證據時才可寫入。
    branch_else_write_applied: bool,
    /// 真原 Checker if 葉具有欄位寫入，不能因 false 葉擅自寫入。
    expected_branch_write: bool,
    /// 原 Checker 證明 else 有可達 self 欄位寫入；其他分支不得動用。
    expected_branch_else_write: bool,
    /// 原 Parser／Checker 唯一核准的分支常值，供真執行稽核。
    expected_branch: Option<bool>,
    /// R02／R05：main 已執行 int 局部不得逃逸到方法返回之後。
    main_local_slots_after_exit: usize,
    method_local_slots_after_exit: usize,
    method_local_events: Vec<PrivateSelfMethodLocalEvent>,
    /// 原 Checker/Parser 封存的方法真 int 局部 DeclarationId
    /// 按來源先後排列；故障執行只容許已走到的合法前綴。
    expected_method_local_ids: Vec<DeclarationId>,
    expected_method_assignment_ids: Vec<DeclarationId>,
    /// R05：真 BorrowSafety 簽發、方法 Return／Unwind 及歸還後
    /// 逐筆記錄；不再只靠方法布林旗標冒充退出已驗證。
    checked_loan: BorrowFact,
    exit_events: Vec<PrivateSharedCallExitEvent>,
}

impl PrivateSelfMethodOutcome {
    /// 只涵蓋目前 Checker 核准的 int 欄位同步 self 方法。
    /// 此子集沒有 OwnedStr，任何資源 Created／Released 皆屬偽造。
    fn has_balanced_self_borrow_trace(&self) -> bool {
        let entered = self.exit_events.iter().filter(|event|
            matches!(event, PrivateSharedCallExitEvent::FunctionEntered)
        ).count();
        let returned = self.exit_events.iter().filter(|event|
            matches!(event, PrivateSharedCallExitEvent::Returned)
        ).count();
        let acquired = self.exit_events.iter().filter(|event| match event {
            PrivateSharedCallExitEvent::LoanAcquired(ticket) =>
                ticket.fact() == self.checked_loan,
            _ => false,
        }).count();
        let all_issued = self.exit_events.iter().filter(|event|
            matches!(event, PrivateSharedCallExitEvent::LoanAcquired(_))
        ).count();
        let mut local_stack = Vec::<DeclarationId>::new();
        let mut initialized_local_ids = Vec::<DeclarationId>::new();
        let mut assigned_local_ids = Vec::<DeclarationId>::new();
        for event in &self.method_local_events {
            match event {
                PrivateSelfMethodLocalEvent::Initialized(id, _) => {
                    if !self.method_entered || local_stack.contains(id) {
                        return false;
                    }
                    local_stack.push(*id);
                    initialized_local_ids.push(*id);
                }
                PrivateSelfMethodLocalEvent::Assigned(id, _) => {
                    // 尚未完成所有原 Checker 局部宣告之前，不可能
                    // 合法進入後續賦值群；只能變更已真初始化的 var int。
                    if !local_stack.contains(id)
                        || initialized_local_ids.len()
                            != self.expected_method_local_ids.len() {
                        return false;
                    }
                    assigned_local_ids.push(*id);
                }
                PrivateSelfMethodLocalEvent::ScopeEnded(id) => {
                    if local_stack.pop() != Some(*id) { return false; }
                }
            }
        }
        // 兩份結果視圖必須投影到完全相同的實際局部事件，
        // 不能只靠各自末端都平衡而放行跨 Returned/Unwind 的錯序。
        // 原 Checker 所允許的宣告先後是唯一真來源；執行期
        // 出錯時只可能初始化其前綴，不能插入其他平衡的幽靈
        // 局部，成功時則應完整執行所有局部宣告。
        let approved_local_prefix = initialized_local_ids.len()
            <= self.expected_method_local_ids.len()
            && self.expected_method_local_ids.starts_with(&initialized_local_ids)
            && (!self.result.is_ok()
                || initialized_local_ids.len() == self.expected_method_local_ids.len());
        let approved_assignments = self.expected_method_assignment_ids
            .starts_with(&assigned_local_ids)
            && (!self.result.is_ok()
                || assigned_local_ids.len()
                    == self.expected_method_assignment_ids.len());
        // 同步方法只有一個已核准的 self 框架；局部不應被
        // 記錄在真正 FunctionEntered 之前，亦不應在其
        // FunctionExited 之後以 main 的匿名基底框架補造。
        let mut inside_method = false;
        let mut nested_depth = 0usize;
        let mut local_events_inside_real_method = true;
        for event in &self.exit_events {
            match event {
                PrivateSharedCallExitEvent::FunctionEntered => {
                    nested_depth += 1;
                    inside_method = true;
                }
                PrivateSharedCallExitEvent::FunctionExited { .. } => {
                    let Some(remaining) = nested_depth.checked_sub(1) else { return false; };
                    nested_depth = remaining;
                    inside_method = remaining > 0;
                }
                PrivateSharedCallExitEvent::LocalInitialized(_)
                    | PrivateSharedCallExitEvent::LocalAssigned(_)
                    | PrivateSharedCallExitEvent::LocalScopeEnded(_) if !inside_method => {
                    local_events_inside_real_method = false;
                }
                _ => {}
            }
        }
        let local_projection = self.method_local_events.iter().map(|event| match event {
            PrivateSelfMethodLocalEvent::Initialized(id, _) =>
                PrivateSharedCallExitEvent::LocalInitialized(*id),
            PrivateSelfMethodLocalEvent::Assigned(id, _) =>
                PrivateSharedCallExitEvent::LocalAssigned(*id),
            PrivateSelfMethodLocalEvent::ScopeEnded(id) =>
                PrivateSharedCallExitEvent::LocalScopeEnded(*id),
        }).collect::<Vec<_>>();
        let exit_projection = self.exit_events.iter().filter(|event| matches!(event,
            PrivateSharedCallExitEvent::LocalInitialized(_)
                | PrivateSharedCallExitEvent::LocalAssigned(_)
                | PrivateSharedCallExitEvent::LocalScopeEnded(_)
        )).cloned().collect::<Vec<_>>();
        let failure_unwind_precedes_cleanup = self.result.is_ok()
            || !self.exit_events.iter().any(|event| matches!(event,
                PrivateSharedCallExitEvent::LocalScopeEnded(_)
            ))
            || self.exit_events.iter().position(|event| matches!(event,
                PrivateSharedCallExitEvent::UnwindStarted
            )).zip(self.exit_events.iter().position(|event| matches!(event,
                PrivateSharedCallExitEvent::LocalScopeEnded(_)
            ))).is_some_and(|(unwind, cleanup)| unwind < cleanup);
        self.ticket_returned
            && approved_local_prefix
            && approved_assignments
            && local_events_inside_real_method
            && local_projection == exit_projection
            && failure_unwind_precedes_cleanup
            && local_stack.is_empty()
            && self.method_local_slots_after_exit == 0
            && self.main_local_slots_after_exit == 0
            && self.events.is_empty()
            && crate::owned_str_runtime::event_trace_balanced(&self.events)
            && private_shared_call_exit_balanced(
                &self.exit_events, self.result.is_ok(),
            )
            && nested_depth == 0
            && entered == usize::from(self.method_entered) + self.ordinary_calls_completed
            && returned == usize::from(self.method_exited) + self.ordinary_calls_completed
            && acquired == all_issued && all_issued <= 1
            && (!self.method_exited || self.method_entered)
            && self.method_exited == self.result.is_ok()
            && (self.branch_checked.is_none() || self.method_entered)
            && (!self.result.is_ok() || self.branch_checked == self.expected_branch)
            && (self.nested_branch_checked.is_none()
                || (self.method_entered && self.branch_checked == Some(true)))
            && (!self.result.is_ok() || self.nested_branch_checked
                == if self.branch_checked == Some(true) {
                    self.expected_nested_branch
                } else { None })
            && !(self.nested_write_applied && self.nested_else_write_applied)
            && (!self.nested_write_applied
                || (self.branch_checked == Some(true)
                    && self.nested_branch_checked == Some(true)
                    && self.expected_nested_write))
            && (!self.nested_else_write_applied
                || (self.branch_checked == Some(true)
                    && self.nested_branch_checked == Some(false)
                    && self.expected_nested_else_write))
            && (!self.result.is_ok() || self.nested_write_applied
                == (self.expected_nested_write
                    && self.nested_branch_checked == Some(true)))
            && (!self.result.is_ok() || self.nested_else_write_applied
                == (self.expected_nested_else_write
                    && self.nested_branch_checked == Some(false)))
            && (!self.branch_write_applied
                || (self.method_entered && self.branch_checked == Some(true)
                    && self.expected_branch_write))
            && (!self.result.is_ok() || self.branch_write_applied
                == (self.expected_branch_write && self.branch_checked == Some(true)))
            && (!self.branch_else_write_applied
                || (self.method_entered && self.branch_checked == Some(false)
                    && self.expected_branch_else_write))
            && !(self.branch_write_applied && self.branch_else_write_applied)
            && (!self.result.is_ok() || self.branch_else_write_applied
                == (self.expected_branch_else_write && self.branch_checked == Some(false)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateSelfIntReturn {
    Field,
    /// M2：原 Checker 驗證的 else 葉獨立 int 返回值，絕不提前執行未選葉。
    Constant(i64),
    Add(i64),
    Subtract(i64),
    /// 由已簽發的 Parameter TypedMember 和原來源名稱身分讀取真正框架。
    Parameter(DeclarationId),
    ParameterAdd(DeclarationId, i64),
    ParameterSubtract(DeclarationId, i64),
    /// 原 Checker 簽章的兩個不同參數，按真正動態框架值運算。
    ParameterPairAdd(DeclarationId, DeclarationId),
    ParameterPairSubtract(DeclarationId, DeclarationId),
    /// 同一真 self BorrowTicket 下讀取目前欄位及參數動態值。
    FieldAddParameter(DeclarationId),
    FieldSubtractParameter(DeclarationId),
    ParameterAddField(DeclarationId),
    ParameterSubtractField(DeclarationId),
    /// 與一般普通函式共用原 Checker ScalarHIR，直接從真方法
    /// 參數／局部框架取值，不另建一份方法內整數算式求值器。
    CheckedScalar(PrivateScalarReturnExpr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateSelfMethodLocalEvent {
    Initialized(DeclarationId, i64),
    Assigned(DeclarationId, i64),
    ScopeEnded(DeclarationId),
}

struct PreparedPrivateSelfMethod {
    verified: VerifiedPrivateSource,
    original: Box<[u8]>,
    struct_name: String,
    owner: String,
    owner_id: DeclarationId,
    owner_mutable: bool,
    field: String,
    field_value: i64,
    field_immutable: bool,
    /// 真方法首句可為原 Checker 已驗證的 self.int 欄位獨占寫入。
    write_value: Option<i64>,
    return_calculation: PrivateSelfIntReturn,
    /// 原方法本體中已核准的 int 局部，按 Lexer/Parser 先後執行，
    /// 不支援未經證明的控制流程、搬移或借用別名再借用。
    method_int_locals: Vec<(DeclarationId, PrivateScalarReturnExpr)>,
    /// B4：同一原 Checker 唯一零參數、純 int 返回的真普通函式計畫。
    method_helper: Option<(String, PrivateScalarReturnExpr)>,
    /// M2：真原 Parser 首句 if(bool 常值) 且葉內只提前 return self.int；
    /// 保留實際 Return 位置，未命中時落至最後一筆已核准 return。
    method_branch: Option<(bool, crate::common_ir::NodeSpan)>,
    /// M2：單一 else 葉必須是原 Checker 真 return int 字面值及其位置。
    method_branch_else: Option<(i64, crate::common_ir::NodeSpan)>,
    /// M2：else 分支獨立的真 FieldAssign 值／原 NodeSpan。
    method_branch_else_write: Option<(i64, crate::common_ir::NodeSpan)>,
    /// else 直接讀取或寫入後返回 self.int 的原 Checked Return NodeSpan。
    method_branch_else_field_return: Option<crate::common_ir::NodeSpan>,
    /// M2：原 Checker else 直接讀取或寫入後的 Copyable 欄位／加減；
    /// 來源位置由 method_branch_else_field_return 另外核實。
    method_branch_else_calculation: Option<PrivateSelfIntReturn>,
    /// M2：原 Checker if true 葉內唯一可寫 self.int 的真位置與值。
    method_branch_write: Option<(i64, crate::common_ir::NodeSpan)>,
    /// M2：if 真返回葉可讀取原 Checker self.int 或受控加減；只有
    /// 真的到達此葉才計算，避免未選分支提前溢位。
    method_branch_return: Option<PrivateSelfIntReturn>,
    /// M2：true 葉單一真巢狀 if 的 bool 條件及內層 else ReturnSpan。
    method_nested_branch: Option<(bool, crate::common_ir::NodeSpan)>,
    /// M2：內層 if(Name) 的原 Checker bool 形參宣告身分；
    /// 無此值時嚴格使用 Lexer 真 bool 字面，不能把 int 轉為 bool。
    method_nested_branch_parameter: Option<DeclarationId>,
    /// M2：原 Checker 巢狀 else 返回 int 常值、self.int 或其加減。
    method_nested_else_calculation: Option<PrivateSelfIntReturn>,
    /// 原 Checker 內層各葉可有一筆獨占 self.int 寫入，僅選中葉執行。
    method_nested_true_write: Option<(i64, crate::common_ir::NodeSpan)>,
    method_nested_else_write: Option<(i64, crate::common_ir::NodeSpan)>,
    /// M2：只有原 Checker 真 bool 形式參數能成為方法 if 條件來源。
    method_branch_parameter: Option<DeclarationId>,
    /// 原 Parser 方法中合法的 var int 重賦值群，必在全部局部宣告之後。
    method_int_assignments: Vec<(DeclarationId, PrivateScalarReturnExpr)>,
    method: String,
    method_span: crate::common_ir::NodeSpan,
    call_span: crate::common_ir::NodeSpan,
    loan: BorrowFact,
    /// 原 Lexer／Checker 證明的首個 Copyable int 常值及後續實參；
    /// 不把相同數值當成相同參數，每個 DeclarationId 各自綁定。
    argument_binding: Option<(DeclarationId, i64)>,
    additional_argument_bindings: Vec<(DeclarationId, i64)>,
    /// M2：原 Lexer bool 字面作方法真形參，獨立於 i64 槽，無隱含轉型。
    method_bool_argument: Option<(DeclarationId, bool)>,
    /// main 真已核准 int 宣告；最後一個欄位是已驗證 Copyable
    /// 來源身分（None 表示原 Lexer 整數常值），只依宣告次序求值。
    main_int_locals: Vec<(String, DeclarationId, i64, Option<DeclarationId>)>,
    /// 與 main int 局部宣告一一對應，使用一般函式的同一
    /// Checked int expression HIR 執行樹，不另造初始化求值器。
    main_init_plans: Vec<PrivateScalarReturnExpr>,
    /// 與方法所有實參原來源順序一一對應；None 是 int 字面值。
    argument_sources: Vec<Option<DeclarationId>>,
    /// R02／R04 直接重用一般函式已核准的 int TypedHIR 編譯樹，
    /// 不是第二套文字解析器；實參在真 main 框架內動態求值。
    argument_plans: Vec<PrivateScalarReturnExpr>,
}

impl PreparedPrivateSelfMethod {
    fn prepare(text: &str) -> Result<Self, PrivateSelfMethodPrepareError> {
        use crate::common_ir::{NodeRole, NodeSpan, ScalarType, VerifiedTypedMember};
        let verified = verify_source_with_ability(
            text, crate::ownership_policy::approved_leaf_copy_permission,
        ).map_err(PrivateSelfMethodPrepareError::Check)?;
        if verified.structs.len() != 1 || !(1..=2).contains(&verified.functions.len()) {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "僅支援一個原 Checker 結構、main 和至多一個已核准的純 int helper",
            ));
        }
        let definition = verified.structs.values().next().expect("已驗證只有一結構");
        if definition.fields.len() != 1 || definition.methods.is_empty() {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "僅接通單一 Copyable int 欄位及具原 Checker 簽章的同步方法",
            ));
        }
        let field = &definition.fields[0];
        let member = VerifiedTypedMember::checked_private(
            text, NodeRole::Field, field.declaration_span,
            &field.name, field.name_span,
            &field.ty, field.type_span, !field.immutable,
        ).ok_or(PrivateSelfMethodPrepareError::Unsupported(
            "欄位缺少原 Parser 真 Name／Type 來源身分",
        ))?;
        if field.ty != "int" || verified.plan.members.get(&member.identity) != Some(&member) {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "欄位缺少原 Checker int 成員型別證據",
            ));
        }
        let TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) =
            &field.default.as_ref().ok_or(PrivateSelfMethodPrepareError::Unsupported(
                "此私有子集需要真正已驗證的 int 預設欄位",
            ))?.kind else {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "本期只執行已核准單一獨立 int 欄位預設常值",
            ));
        };
        let field_value = crate::syntax_shared::integer_i64(raw).map_err(|_|
            PrivateSelfMethodPrepareError::Unsupported("欄位 i64 常值無法安全表示"),
        )?;
        // R02／R04：main 以原 Parser 順序建立唯一 struct 接收者，
        // 可再宣告已核准 Copyable int 局部，最後才 return 真方法呼叫。
        // 不能藉此接受尚未接通的賦值、控制流程或任意初始化。
        let main = verified.functions.iter().find(|fun| fun.name == "main")
            .ok_or(PrivateSelfMethodPrepareError::Unsupported(
                "原 Checker 來源缺少唯一 main",
            ))?;
        // 第一個 B4 真混合工作圖：普通函式僅限零參數、單一
        // 已核准 int 常值返回，所有來源身分皆來自同一原 Checker。
        let method_helper = verified.functions.iter().find(|fun| fun.name != "main")
            .map(|helper| {
                if !helper.params.is_empty() || helper.result_ty != "int"
                    || helper.is_async || helper.body.statements.len() != 1 {
                    return Err(PrivateSelfMethodPrepareError::Unsupported(
                        "B4 僅支援原 Checker 零參數純 int helper",
                    ));
                }
                let StmtKind::Return(returned) = &helper.body.statements[0].kind else {
                    return Err(PrivateSelfMethodPrepareError::Unsupported(
                        "B4 普通函式必須具有真正 return 來源",
                    ));
                };
                let plan = PrivateScalarReturnExpr::from_checked_bindings(
                    &returned.expression, &BTreeMap::new(), &verified,
                ).map_err(|_| PrivateSelfMethodPrepareError::Unsupported(
                    "B4 helper 返回缺少原 Checker TypedHIR／Lexer 證明",
                ))?;
                if !matches!(plan, PrivateScalarReturnExpr::Constant(_)) {
                    return Err(PrivateSelfMethodPrepareError::Unsupported(
                        "B4 helper 尚只接通無副作用的純 int 返回",
                    ));
                }
                Ok((helper.name.clone(), plan))
            }).transpose()?;
        let last_main_statement = main.body.statements.last().ok_or(
            PrivateSelfMethodPrepareError::Unsupported("main 缺少真同步方法返回"),
        )?;
        let (called_method, call_arguments) = match &last_main_statement.kind {
            StmtKind::Return(returned) => match &returned.expression.kind {
                TermKind::MethodCall(_, name, args) => (name.as_str(), args.as_slice()),
                _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                    "main 最後 return 須是真同步方法呼叫",
                )),
            },
            _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                "main 最後語句須 return 真同步方法",
            )),
        };
        if main.body.statements.len() < 2 {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "main 需要先建立接收者再 return 真方法",
            ));
        }
        let mut main_int_locals = Vec::<(
            String, DeclarationId, i64, Option<DeclarationId>,
        )>::new();
        let mut main_init_plans = Vec::<PrivateScalarReturnExpr>::new();
        let mut known_int_bindings = BTreeMap::<String, DeclarationId>::new();
        let mut known_int_values = BTreeMap::<DeclarationId, i64>::new();
        for statement in main.body.statements.iter().skip(1)
            .take(main.body.statements.len() - 2) {
            let StmtKind::Declare(local) = &statement.kind else {
                return Err(PrivateSelfMethodPrepareError::Unsupported(
                    "此同步方法子集只接通 main 已驗證 int 局部宣告",
                ));
            };
            let id = DeclarationId(statement.start);
            let initializer = local.initializer.as_ref().ok_or(
                PrivateSelfMethodPrepareError::Unsupported(
                    "main int 局部須由原 Checker 證明已初始化",
                ),
            )?;
            if local.ty != "int"
                || !verified.plan.declarations.matches(id, &local.ty, local.mutable) {
                return Err(PrivateSelfMethodPrepareError::Unsupported(
                    "main int 局部缺少原 Checker 宣告與型別身分",
                ));
            }
            // 與普通函式共用已核准 TypedHIR Expression／Reference／
            // Arithmetic、原 Lexer token 及括號身分，並只允許
            // 先前已正式宣告的 Copyable int 名稱。
            let plan = PrivateScalarReturnExpr::from_checked_bindings(
                initializer, &known_int_bindings, &verified,
            ).map_err(|_| PrivateSelfMethodPrepareError::Unsupported(
                "main int 初始化缺少真 Checker TypedHIR／詞彙與位置",
            ))?;
            let mut calls = Vec::new();
            plan.collect_calls(&mut calls);
            let direct_helper = calls.len() == 1 && matches!(
                &plan,
                PrivateScalarReturnExpr::Call {
                    name, arguments, awaited: false,
                } if arguments.is_empty()
                    && method_helper.as_ref().is_some_and(|(target, _)| target == name)
            );
            if (!calls.is_empty() && !direct_helper)
                || matches!(plan, PrivateScalarReturnExpr::Unreachable) {
                return Err(PrivateSelfMethodPrepareError::Unsupported(
                    "main int 初始化只支援同源 Checker 核准的純整數零參數 helper",
                ));
            }
            // 同一份原 Checker 無副作用、單一常值 helper，可建立
            // 靜態數值快照供來源防偽；真正函式進退只能在 execute
            // 按原 main 執行次序產生，這裡不得預寫動態事件。
            let expected = if direct_helper {
                method_helper.as_ref()
                    .ok_or(PrivateSelfMethodPrepareError::Unsupported(
                        "main helper 缺少原 Checker 唯一純值工作計畫",
                    ))?.1.evaluate(&BTreeMap::new())
            } else { plan.evaluate(&known_int_values) };
            let value = match expected {
                Ok(value) => value,
                // 已核准動態名稱參與的運算可能溢位；prepare 只
                // 封存真計畫，execute 再回報受控溢位且清理已建局部。
                Err(PrivateScalarCallRuntimeError::IntegerOverflow) => 0,
                Err(_) => return Err(PrivateSelfMethodPrepareError::Unsupported(
                    "main int 局部初始化缺少已宣告來源",
                )),
            };
            let source = match &plan {
                PrivateScalarReturnExpr::Parameter(source) => Some(*source),
                _ => None,
            };
            if known_int_bindings.contains_key(&local.name)
                || known_int_values.contains_key(&id) {
                return Err(PrivateSelfMethodPrepareError::Unsupported(
                    "同作用域 Copyable int 局部重複宣告",
                ));
            }
            known_int_bindings.insert(local.name.clone(), id);
            known_int_values.insert(id, value);
            main_int_locals.push((local.name.clone(), id, value, source));
            main_init_plans.push(plan);
        }
        // 同步方法實參與普通函式共用原 Checker→TypedHIR 的
        // Copyable int 表達式編譯；不另造方法專屬的名稱／運算子解析。
        let main_bindings = main_int_locals.iter()
            .map(|(name, id, _, _)| (name.clone(), *id))
            .collect::<BTreeMap<_, _>>();
        let main_prepared_values = main_int_locals.iter()
            .map(|(_, id, value, _)| (*id, *value))
            .collect::<BTreeMap<_, _>>();
        // M2：先由原 Parser 的方法實參與 Lexer 相同半開跨度確認
        // 僅一個 bool 常值；不把 true/false 借用 i64 0/1 表示。
        let bool_argument_literal = if call_arguments.len() == 1 {
            let term = &call_arguments[0];
            match &term.kind {
                TermKind::Scalar(
                    crate::syntax_shared::ScalarLiteral::Boolean(value),
                ) if verified.tokens.iter().any(|token|
                    token.start == term.start && token.end == term.end
                        && matches!(&token.kind, TokenKind::Keyword(word)
                            if word == if *value { "true" } else { "false" })
                ) => Some(*value),
                _ => None,
            }
        } else { None };
        let mut int_arguments = Vec::<i64>::new();
        let mut argument_sources = Vec::<Option<DeclarationId>>::new();
        let mut argument_plans = Vec::<PrivateScalarReturnExpr>::new();
        for arg in call_arguments {
            if bool_argument_literal.is_some() {
                // 這是一個已核准 bool 形參，不是 int HIR 求值。
                break;
            }
            let plan = PrivateScalarReturnExpr::from_checked_bindings(
                arg, &main_bindings, &verified,
            ).map_err(|_| PrivateSelfMethodPrepareError::Unsupported(
                "方法 int 實參缺少原 Checker TypedHIR／Reference／Arithmetic 真來源",
            ))?;
            // 方法子集仍只接受純值加減／括號。未接通普通函式
            // 呼叫嵌入方法實參前，不能用原純值樹偷渡一個額外 Call。
            let mut calls = Vec::new();
            plan.collect_calls(&mut calls);
            if !calls.is_empty() || matches!(plan, PrivateScalarReturnExpr::Unreachable) {
                return Err(PrivateSelfMethodPrepareError::Unsupported(
                    "方法 int 實參不支援未接通的巢狀函式呼叫",
                ));
            }
            // 保留原測試對已知常值的靜態來源快照；若動態加減
            // 確實溢位，必須交由 execute 產生受控執行期錯誤，
            // 不能在 prepare 靜態拒絕原 Checker 合法來源。
            let prepared = match plan.evaluate(&main_prepared_values) {
                Ok(value) => value,
                Err(PrivateScalarCallRuntimeError::IntegerOverflow) => 0,
                Err(_) => return Err(PrivateSelfMethodPrepareError::Unsupported(
                    "方法 int 引數未能由原 main 局部真來源安全求值",
                )),
            };
            let source = match &plan {
                PrivateScalarReturnExpr::Parameter(id) => Some(*id),
                _ => None,
            };
            int_arguments.push(prepared);
            argument_sources.push(source);
            argument_plans.push(plan);
        }
        // 只選與原呼叫數量及已驗證 int 型別完全一致的候選。
        let mut matches = definition.methods_named(called_method)
            .filter(|candidate| {
                if bool_argument_literal.is_some() {
                    candidate.params.len() == 1 && candidate.params[0].ty == "bool"
                } else {
                    candidate.params.len() == int_arguments.len()
                        && candidate.params.iter().all(|param| param.ty == "int")
                }
            });
        let method = matches.next().ok_or(PrivateSelfMethodPrepareError::Unsupported(
            "原 Checker 無符合呼叫名稱與零參數簽章的方法",
        ))?;
        if matches.next().is_some() {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "無法唯一辨識零參數方法簽章",
            ));
        }
        // 唯一性已驗證，先結束候選迭代器的借用，才能移交 verified。
        drop(matches);
        if method.result_ty != "int" || method.body.statements.is_empty() {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "僅執行原 Checker 證明之同步 Copyable int 方法",
            ));
        }
        // 所有參數必須來自**選中的**原 Checker 方法簽章、正式
        // VerifiedTypedMember(Parameter) 與 Lexer 位置，不依數值猜測。
        // 不同於一般函式，方法參數目前未另建 VerifiedDeclarationFacts；
        // 以原 Parser 宣告起點作 DeclarationId 並要求 members 精確一致。
        let mut bindings = Vec::<(DeclarationId, i64)>::new();
        for (param, value) in method.params.iter().zip(int_arguments.iter().copied()) {
            let param_id = DeclarationId(param.declaration_start);
            let member = VerifiedTypedMember::checked_private(
                text, NodeRole::Parameter, param.declaration_span,
                &param.name, param.name_span, &param.ty, param.type_span, false,
            ).ok_or(PrivateSelfMethodPrepareError::Unsupported(
                "方法參數缺少真 Parser／Lexer 位置及型別",
            ))?;
            if verified.plan.members.get(&member.identity) != Some(&member)
                || bindings.iter().any(|(id, _)| id == &param_id) {
                return Err(PrivateSelfMethodPrepareError::Unsupported(
                    "方法參數缺少原 Checker 型別、位置或宣告身分唯一性",
                ));
            }
            bindings.push((param_id, value));
        }
        let argument_binding = bindings.first().copied();
        let additional_argument_bindings = bindings.into_iter().skip(1).collect::<Vec<_>>();
        let method_bool_argument = bool_argument_literal.map(|value| {
            let param = &method.params[0];
            let member = VerifiedTypedMember::checked_private(
                text, NodeRole::Parameter, param.declaration_span,
                &param.name, param.name_span, &param.ty, param.type_span, false,
            ).ok_or(PrivateSelfMethodPrepareError::Unsupported(
                "M2 bool 方法形參缺少真 Parser／Lexer 身分",
            ))?;
            if param.ty != "bool"
                || verified.plan.members.get(&member.identity) != Some(&member) {
                return Err(PrivateSelfMethodPrepareError::Unsupported(
                    "M2 bool 形參未經原 Checker 正式驗證",
                ));
            }
            Ok((DeclarationId(param.declaration_start), value))
        }).transpose()?;
        // 兩語句模式先寫真 Copyable int 欄位，再直接讀取回傳；
        // 唯一是否可寫及 self 自動借用模式來自原 Checker MethodSafety。
        let write_value = if method.body.statements.len() == 2
            && matches!(&method.body.statements[0].kind, StmtKind::FieldAssign { .. })
        {
            let first = &method.body.statements[0];
            let StmtKind::FieldAssign { receiver, field: target, value } = &first.kind else {
                return Err(PrivateSelfMethodPrepareError::Unsupported(
                    "獨占 self 子集僅允許原 Checker 第一語句為欄位賦值",
                ));
            };
            let TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) =
                &value.kind else {
                return Err(PrivateSelfMethodPrepareError::Unsupported(
                    "本期 self 寫入僅限原 Lexer 真 int 常值",
                ));
            };
            if receiver != "self" || target != &field.name
                || !verified.tokens.iter().any(|token|
                    token.start == value.start && token.end == value.end
                        && matches!(&token.kind, TokenKind::Integer(text) if text == raw)
                ) {
                return Err(PrivateSelfMethodPrepareError::Unsupported(
                    "欄位賦值的目標或整數記號未與原 Checker 方法一致",
                ));
            }
            Some(crate::syntax_shared::integer_i64(raw).map_err(|_|
                PrivateSelfMethodPrepareError::Unsupported("寫入值超出 i64 範圍")
            )?)
        } else { None };
        // M2 第二批：保留原已驗證 bool 字面條件，再接通同一
        // Parser/Checker 的 bool 形參名稱。真值由 execute 的
        // 獨立 bool 參數框架讀取，不發明 int 隱含轉型。
        let (method_branch, method_branch_parameter,
            method_branch_else, method_branch_else_write,
            method_branch_else_field_return, method_branch_else_calculation,
            method_branch_write, method_branch_return, method_nested_branch,
            method_nested_branch_parameter, method_nested_else_calculation,
            method_nested_true_write, method_nested_else_write) =
            if method.body.statements.len() == 2 {
                match &method.body.statements[0].kind {
                    StmtKind::If { branch, no } => {
                        if !(1..=2).contains(&branch.body.statements.len()) {
                            return Err(PrivateSelfMethodPrepareError::Unsupported(
                                "M2 if true 葉只可單一 return 或先欄位寫入再 return",
                            ));
                        }
                        let (condition, parameter_source) = match &branch.condition.kind {
                            TermKind::Scalar(
                                crate::syntax_shared::ScalarLiteral::Boolean(value),
                            ) if verified.tokens.iter().any(|token|
                                token.start == branch.condition.start
                                    && token.end == branch.condition.end
                                    && matches!(&token.kind, TokenKind::Keyword(word)
                                        if word == if *value { "true" } else { "false" })
                            ) => (*value, None),
                            TermKind::Name(name) => {
                                let (id, value) = method_bool_argument.ok_or(
                                    PrivateSelfMethodPrepareError::Unsupported(
                                        "M2 方法 bool 條件無合法傳入形參",
                                    ),
                                )?;
                                if method.params.len() != 1
                                    || method.params[0].name != *name
                                    || DeclarationId(method.params[0].declaration_start) != id
                                    || !verified.tokens.iter().any(|token|
                                        token.start == branch.condition.start
                                            && token.end == branch.condition.end
                                            && matches!(&token.kind, TokenKind::Name(found)
                                                if found == name)
                                    )
                                    // 方法 bool 條件的原 Checker 事實由
                                    // method_bool_condition／VerifiedMethodSafety
                                    // 簽發；它不一定進入只供 Copyable int
                                    // 求值的 ScalarHIR，不得誤要求另一權威。
                                {
                                    return Err(PrivateSelfMethodPrepareError::Unsupported(
                                        "M2 bool 參數條件與原 Checker 名稱身分不一致",
                                    ));
                                }
                                (value, Some(id))
                            }
                            _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                                "M2 if 條件尚未有原 Lexer／Checker 證據",
                            )),
                        };
                        // 只有 true 葉可以先寫單一原 Checker var int 欄位；
                        // Lexer 原 token／Parser 原節點身分與 MethodSafety
                        // 的 ExclusiveWrite 證據必須同時成立。
                        let branch_write = if branch.body.statements.len() == 2 {
                            let write = &branch.body.statements[0];
                            let StmtKind::FieldAssign {
                                receiver, field: name, value,
                            } = &write.kind else {
                                return Err(PrivateSelfMethodPrepareError::Unsupported(
                                    "M2 true 葉第一句只能是原 Checker self.int 欄位寫入",
                                ));
                            };
                            let TermKind::Scalar(
                                crate::syntax_shared::ScalarLiteral::Integer(raw),
                            ) = &value.kind else {
                                return Err(PrivateSelfMethodPrepareError::Unsupported(
                                    "M2 true 葉欄位寫入只能是原 Lexer int 字面",
                                ));
                            };
                            if receiver != "self" || name != &field.name
                                || !verified.tokens.iter().any(|token|
                                    token.start == value.start && token.end == value.end
                                        && matches!(&token.kind,
                                            TokenKind::Integer(text) if text == raw)
                                ) {
                                return Err(PrivateSelfMethodPrepareError::Unsupported(
                                    "M2 true 葉欄位來源、型別或詞彙位置不符",
                                ));
                            }
                            let literal = crate::syntax_shared::integer_i64(raw)
                                .map_err(|_|
                                    PrivateSelfMethodPrepareError::Unsupported(
                                        "M2 true 葉欄位值超出原 int 安全範圍",
                                    )
                                )?;
                            Some((literal, write.node_span()))
                        } else { None };
                        // 真巢狀 CFG 最小子集：外層 true 葉只含一筆
                        // 內層 if(bool Lexer 常值)，其 true／else 葉各有
                        // 單一 Copyable int return。內層 false 只讀 self.int，
                        // 不允許未證的動態別名／資源操作。
                        let (early_stmt, nested_branch, nested_parameter_source, nested_else_calculation,
                            nested_true_write, nested_else_write) =
                            if branch.body.statements.len() == 1 {
                                let candidate = &branch.body.statements[0];
                                if let StmtKind::If {
                                    branch: inner, no: Some(otherwise),
                                } = &candidate.kind {
                                    if !(1..=2).contains(&inner.body.statements.len())
                                        || !(1..=2).contains(&otherwise.len()) {
                                        return Err(PrivateSelfMethodPrepareError::Unsupported(
                                            "M2 巢狀 if 各葉只可直接 return 或先寫一筆 self.int",
                                        ));
                                    }
                                    // 與外層 bool Name 共用同一原 Checker 形參
                                    // DeclarationId，不建立第二條不受約束的語意路徑。
                                    let (condition, parameter_source) = match &inner.condition.kind {
                                        TermKind::Scalar(
                                            crate::syntax_shared::ScalarLiteral::Boolean(value),
                                        ) if verified.tokens.iter().any(|token|
                                            token.start == inner.condition.start
                                                && token.end == inner.condition.end
                                                && matches!(&token.kind,
                                                    TokenKind::Keyword(word)
                                                        if word == if *value { "true" } else { "false" })
                                        ) => (*value, None),
                                        TermKind::Name(name) => {
                                            let (id, value) = method_bool_argument.ok_or(
                                                PrivateSelfMethodPrepareError::Unsupported(
                                                    "M2 內層 bool 形參缺少原 Checker 綁定",
                                                ),
                                            )?;
                                            if method.params.len() != 1
                                                || method.params[0].name != *name
                                                || DeclarationId(method.params[0].declaration_start) != id
                                                || !verified.tokens.iter().any(|token|
                                                    token.start == inner.condition.start
                                                        && token.end == inner.condition.end
                                                        && matches!(&token.kind,
                                                            TokenKind::Name(found) if found == name)
                                                )
                                            {
                                                return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                    "M2 內層 bool Name 未匹配原 Lexer/Checker 形參身分",
                                                ));
                                            }
                                            (value, Some(id))
                                        }
                                        _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                                            "M2 內層 if 條件尚無原 Checker 可執行 bool 證據",
                                        )),
                                    };
                                    let true_stmt = inner.body.statements.last().expect(
                                        "原 Parser 已證實內層 true 葉非空",
                                    );
                                    let false_stmt = otherwise.last().expect(
                                        "原 Parser 已證實內層 else 葉非空",
                                    );
                                    if !matches!(&true_stmt.kind, StmtKind::Return(_)) {
                                        return Err(PrivateSelfMethodPrepareError::Unsupported(
                                            "M2 巢狀 true 葉最後一筆必須 return",
                                        ));
                                    }
                                    let StmtKind::Return(returned) = &false_stmt.kind else {
                                        return Err(PrivateSelfMethodPrepareError::Unsupported(
                                            "M2 巢狀 else 葉最後一筆必須 return",
                                        ));
                                    };
                                    // 每葉至多一筆已核准的原 Parser FieldAssign，
                                    // 實際寫入權限由原 Checker method_safety 和真 BorrowTicket 保證。
                                    let writes = [
                                        if inner.body.statements.len() == 2 {
                                            Some(&inner.body.statements[0])
                                        } else { None },
                                        if otherwise.len() == 2 {
                                            Some(&otherwise[0])
                                        } else { None },
                                    ];
                                    let nested_writes = writes.into_iter().map(|candidate|
                                        -> Result<Option<(i64, NodeSpan)>, PrivateSelfMethodPrepareError> {
                                        let Some(write) = candidate else { return Ok(None) };
                                        let StmtKind::FieldAssign { receiver, field: name, value } =
                                            &write.kind else {
                                            return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                "M2 巢狀可寫葉第一句必須是原 Checker self.int 欄位更新",
                                            ));
                                        };
                                        let TermKind::Scalar(
                                            crate::syntax_shared::ScalarLiteral::Integer(raw),
                                        ) = &value.kind else {
                                            return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                "M2 巢狀可寫葉只接受原 Lexer int 常值",
                                            ));
                                        };
                                        if receiver != "self" || name != &field.name
                                            || !verified.tokens.iter().any(|token|
                                                token.start == value.start && token.end == value.end
                                                    && matches!(&token.kind,
                                                        TokenKind::Integer(text) if text == raw)
                                            ) {
                                            return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                "M2 巢狀可寫欄位缺少原 Checker/Lexer 身分",
                                            ));
                                        }
                                        let literal = crate::syntax_shared::integer_i64(raw)
                                            .map_err(|_| PrivateSelfMethodPrepareError::Unsupported(
                                                "M2 巢狀寫入值超過 i64 安全範圍",
                                            ))?;
                                        Ok(Some((literal, write.node_span())))
                                    }).collect::<Result<Vec<_>, _>>()?;
                                    // 重用同一原 Checker FieldRead/BinaryInt 左欄位右 Lexer
                                    // int 精確來源檢核，只在實際選中內層 else 時求值。
                                    let nested_else_calc = match &returned.expression.kind {
                                                                TermKind::Scalar(
                                                                    crate::syntax_shared::ScalarLiteral::Integer(raw),
                                                                ) => {
                                                                    // 原 Checker 已證 int 返回；執行計畫再逐 token 鎖定常值來源。
                                                                    if !verified.tokens.iter().any(|token|
                                                                        token.start == returned.expression.start
                                                                            && token.end == returned.expression.end
                                                                            && matches!(&token.kind,
                                                                                TokenKind::Integer(text) if text == raw)
                                                                    ) {
                                                                        return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                                            "M2 巢狀 else整數常值缺少原 Lexer 精確來源",
                                                                        ));
                                                                    }
                                                                    let literal = crate::syntax_shared::integer_i64(raw).map_err(|_|
                                                                        PrivateSelfMethodPrepareError::Unsupported(
                                                                            "M2 巢狀 else整數常值超出 i64 範圍",
                                                                        )
                                                                    )?;
                                                                    PrivateSelfIntReturn::Constant(literal)
                                                                },
                                                                TermKind::FieldRead(receiver, name)
                                                                    if receiver == "self" && name == &field.name =>
                                                                    PrivateSelfIntReturn::Field,
                                                                TermKind::BinaryInt(binary) => {
                                                                    let TermKind::FieldRead(receiver, name) =
                                                                        &binary.left.kind else {
                                                                        return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                                            "M2 巢狀 else加減左端必須是原 self.int 欄位",
                                                                        ));
                                                                    };
                                                                    let TermKind::Scalar(
                                                                        crate::syntax_shared::ScalarLiteral::Integer(raw),
                                                                    ) = &binary.right.kind else {
                                                                        return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                                            "M2 巢狀 else加減右端必須是原 Lexer int",
                                                                        ));
                                                                    };
                                                                    let left = binary.left.node_span();
                                                                    let right = binary.right.node_span();
                                                                    let operator = verified.source.bytes.get(left.end..right.start)
                                                                        .and_then(|raw| std::str::from_utf8(raw).ok())
                                                                        .map(str::trim);
                                                                    let expected = match binary.operator {
                                                                        '+' => "+",
                                                                        '-' => "-",
                                                                        _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                                            "M2 巢狀 else只能使用已核准 int 加減運算子",
                                                                        )),
                                                                    };
                                                                    if receiver != "self" || name != &field.name
                                                                        || operator != Some(expected)
                                                                        || !verified.tokens.iter().any(|token|
                                                                            token.start == binary.right.start
                                                                                && token.end == binary.right.end
                                                                                && matches!(&token.kind,
                                                                                    TokenKind::Integer(text) if text == raw)
                                                                        ) {
                                                                        return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                                            "M2 巢狀 else算術缺少原 Checker/Lexer 欄位及運算子身分",
                                                                        ));
                                                                    }
                                                                    let literal = crate::syntax_shared::integer_i64(raw).map_err(|_|
                                                                        PrivateSelfMethodPrepareError::Unsupported(
                                                                            "M2 巢狀 else加減值超出 i64",
                                                                        )
                                                                    )?;
                                                                    match binary.operator {
                                                                        '+' => PrivateSelfIntReturn::Add(literal),
                                                                        '-' => PrivateSelfIntReturn::Subtract(literal),
                                                                        _ => unreachable!("運算子已在原 Checker 權限驗證"),
                                                                    }
                                                                }
                                                                _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                                    "M2 巢狀 else僅接通原 Checker int 常值、self.int 或其加減",
                                                                )),
                                                            };
                                    (true_stmt, Some((condition, false_stmt.node_span())),
                                        parameter_source, Some(nested_else_calc),
                                        nested_writes[0].clone(), nested_writes[1].clone())
                                } else { (candidate, None, None, None, None, None) }
                            } else {
                                (branch.body.statements.last().expect(
                                    "原 Parser 已證明 true 葉非空",
                                ), None, None, None, None, None)
                            };
                        let StmtKind::Return(early) = &early_stmt.kind else {
                            return Err(PrivateSelfMethodPrepareError::Unsupported(
                                "M2 bool 選中葉不是已核准提前 return",
                            ));
                        };
                        let true_return_plan = match &early.expression.kind {
                            TermKind::Scalar(
                                crate::syntax_shared::ScalarLiteral::Integer(raw),
                            ) => {
                                // 原 Checker 已證 int 返回；執行計畫再逐 token 鎖定常值來源。
                                if !verified.tokens.iter().any(|token|
                                    token.start == early.expression.start
                                        && token.end == early.expression.end
                                        && matches!(&token.kind,
                                            TokenKind::Integer(text) if text == raw)
                                ) {
                                    return Err(PrivateSelfMethodPrepareError::Unsupported(
                                        "M2 true 返回葉整數常值缺少原 Lexer 精確來源",
                                    ));
                                }
                                let literal = crate::syntax_shared::integer_i64(raw).map_err(|_|
                                    PrivateSelfMethodPrepareError::Unsupported(
                                        "M2 true 返回葉整數常值超出 i64 範圍",
                                    )
                                )?;
                                PrivateSelfIntReturn::Constant(literal)
                            },
                            TermKind::FieldRead(receiver, name)
                                if receiver == "self" && name == &field.name =>
                                PrivateSelfIntReturn::Field,
                            TermKind::BinaryInt(binary) => {
                                let TermKind::FieldRead(receiver, name) =
                                    &binary.left.kind else {
                                    return Err(PrivateSelfMethodPrepareError::Unsupported(
                                        "M2 true 葉加減左端必須是原 self.int 欄位",
                                    ));
                                };
                                let TermKind::Scalar(
                                    crate::syntax_shared::ScalarLiteral::Integer(raw),
                                ) = &binary.right.kind else {
                                    return Err(PrivateSelfMethodPrepareError::Unsupported(
                                        "M2 true 葉加減右端必須是原 Lexer int",
                                    ));
                                };
                                let left = binary.left.node_span();
                                let right = binary.right.node_span();
                                let operator = verified.source.bytes.get(left.end..right.start)
                                    .and_then(|raw| std::str::from_utf8(raw).ok())
                                    .map(str::trim);
                                let expected = match binary.operator {
                                    '+' => "+",
                                    '-' => "-",
                                    _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                                        "M2 true 葉只能使用已核准 int 加減運算子",
                                    )),
                                };
                                if receiver != "self" || name != &field.name
                                    || operator != Some(expected)
                                    || !verified.tokens.iter().any(|token|
                                        token.start == binary.right.start
                                            && token.end == binary.right.end
                                            && matches!(&token.kind,
                                                TokenKind::Integer(text) if text == raw)
                                    ) {
                                    return Err(PrivateSelfMethodPrepareError::Unsupported(
                                        "M2 true 葉算術缺少原 Checker/Lexer 欄位及運算子身分",
                                    ));
                                }
                                let literal = crate::syntax_shared::integer_i64(raw).map_err(|_|
                                    PrivateSelfMethodPrepareError::Unsupported(
                                        "M2 true 葉加減值超出 i64",
                                    )
                                )?;
                                match binary.operator {
                                    '+' => PrivateSelfIntReturn::Add(literal),
                                    '-' => PrivateSelfIntReturn::Subtract(literal),
                                    _ => unreachable!("運算子已在原 Checker 權限驗證"),
                                }
                            }
                            _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                                "M2 true 葉僅接通原 Checker int 常值、self.int 或其加減",
                            )),
                        };
                        // else 僅接通原 Checker 單一 int 常值或 self.int 真返回／加減，
                        // 或一筆真 self.int 寫入後的同欄位返回。
                        // 其他 CFG、Move／Borrow 保守拒絕。
                        let (else_return, else_write, else_field_return,
                            else_calculation) =
                            if let Some(otherwise) = no.as_ref() {
                                let statements = otherwise.as_slice();
                                if statements.len() == 1 {
                                    let stmt = &statements[0];
                                    let StmtKind::Return(returned) = &stmt.kind else {
                                        return Err(PrivateSelfMethodPrepareError::Unsupported(
                                            "M2 else 單語句僅能是原 Checker return",
                                        ));
                                    };
                                    match &returned.expression.kind {
                                        TermKind::Scalar(
                                            crate::syntax_shared::ScalarLiteral::Integer(raw),
                                        ) => {
                                            if !verified.tokens.iter().any(|token|
                                                token.start == returned.expression.start
                                                    && token.end == returned.expression.end
                                                    && matches!(&token.kind,
                                                        TokenKind::Integer(text) if text == raw)
                                            ) {
                                                return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                    "M2 else int 常值來源與 Lexer 不一致",
                                                ));
                                            }
                                            let literal = crate::syntax_shared::integer_i64(raw)
                                                .map_err(|_| PrivateSelfMethodPrepareError::Unsupported(
                                                    "M2 else 返回整數超出 i64",
                                                ))?;
                                            (Some((literal, stmt.node_span())), None, None, None)
                                        }
                                        TermKind::FieldRead(receiver, name)
                                            if receiver == "self" && name == &field.name =>
                                            (None, None, Some(stmt.node_span()),
                                                Some(PrivateSelfIntReturn::Field)),
                                        TermKind::BinaryInt(binary) => {
                                            let TermKind::FieldRead(receiver, name) =
                                                &binary.left.kind else {
                                                return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                    "M2 else 直接加減左端必須為原 self.int",
                                                ));
                                            };
                                            let TermKind::Scalar(
                                                crate::syntax_shared::ScalarLiteral::Integer(raw),
                                            ) = &binary.right.kind else {
                                                return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                    "M2 else 直接加減右端必須為原 Lexer int",
                                                ));
                                            };
                                            let left = binary.left.node_span();
                                            let right = binary.right.node_span();
                                            let operator = verified.source.bytes.get(
                                                left.end..right.start,
                                            ).and_then(|raw| std::str::from_utf8(raw).ok())
                                                .map(str::trim);
                                            let expected = match binary.operator {
                                                '+' => "+",
                                                '-' => "-",
                                                _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                    "M2 else 直接加減只支援已核准運算子",
                                                )),
                                            };
                                            if receiver != "self" || name != &field.name
                                                || operator != Some(expected)
                                                || !verified.tokens.iter().any(|token|
                                                    token.start == binary.right.start
                                                        && token.end == binary.right.end
                                                        && matches!(&token.kind,
                                                            TokenKind::Integer(text) if text == raw)
                                                )
                                            {
                                                return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                    "M2 else 直接加減缺少原 Checker/Lexer 身分",
                                                ));
                                            }
                                            let literal = crate::syntax_shared::integer_i64(raw)
                                                .map_err(|_| PrivateSelfMethodPrepareError::Unsupported(
                                                    "M2 else 直接加減超出 i64",
                                                ))?;
                                            let plan = match binary.operator {
                                                '+' => PrivateSelfIntReturn::Add(literal),
                                                '-' => PrivateSelfIntReturn::Subtract(literal),
                                                _ => unreachable!("運算子已經原來源驗證"),
                                            };
                                            (None, None, Some(stmt.node_span()), Some(plan))
                                        }
                                        _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                                            "M2 else 單 return 只接通 int 常值、self.int 或其加減",
                                        )),
                                    }
                                } else if statements.len() == 2 {
                                    let assignment = &statements[0];
                                    let StmtKind::FieldAssign {
                                        receiver, field: name, value,
                                    } = &assignment.kind else {
                                        return Err(PrivateSelfMethodPrepareError::Unsupported(
                                            "M2 else 第一語句需為真 Checker self.int 寫入",
                                        ));
                                    };
                                    let TermKind::Scalar(
                                        crate::syntax_shared::ScalarLiteral::Integer(raw),
                                    ) = &value.kind else {
                                        return Err(PrivateSelfMethodPrepareError::Unsupported(
                                            "M2 else 欄位只能寫入原 Lexer int 常值",
                                        ));
                                    };
                                    if receiver != "self" || name != &field.name
                                        || !verified.tokens.iter().any(|token|
                                            token.start == value.start
                                                && token.end == value.end
                                                && matches!(&token.kind,
                                                    TokenKind::Integer(text) if text == raw)
                                        )
                                    {
                                        return Err(PrivateSelfMethodPrepareError::Unsupported(
                                            "M2 else 寫入欄位名稱或 Lexer 座標未經證明",
                                        ));
                                    }
                                    let literal = crate::syntax_shared::integer_i64(raw)
                                        .map_err(|_| PrivateSelfMethodPrepareError::Unsupported(
                                            "M2 else 寫入整數超出 i64",
                                        ))?;
                                    let returned_stmt = &statements[1];
                                    let StmtKind::Return(returned) =
                                        &returned_stmt.kind else {
                                        return Err(PrivateSelfMethodPrepareError::Unsupported(
                                            "M2 else 寫入後必須直接 return self.int",
                                        ));
                                    };
                                    let calculation = match &returned.expression.kind {
                                        TermKind::FieldRead(target, name)
                                            if target == "self" && name == &field.name =>
                                            PrivateSelfIntReturn::Field,
                                        TermKind::BinaryInt(binary) => {
                                            let TermKind::FieldRead(target, name) =
                                                &binary.left.kind else {
                                                return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                    "M2 else 加減左端必須是原 self.int 欄位",
                                                ));
                                            };
                                            let TermKind::Scalar(
                                                crate::syntax_shared::ScalarLiteral::Integer(raw),
                                            ) = &binary.right.kind else {
                                                return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                    "M2 else 加減右端必須是原 Lexer int 常值",
                                                ));
                                            };
                                            let left = binary.left.node_span();
                                            let right = binary.right.node_span();
                                            let operator = verified.source.bytes.get(
                                                left.end..right.start,
                                            ).and_then(|raw| std::str::from_utf8(raw).ok())
                                                .map(str::trim);
                                            let expected = match binary.operator {
                                                '+' => "+",
                                                '-' => "-",
                                                _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                    "M2 else 加減只允許原 Checker int 運算子",
                                                )),
                                            };
                                            if target != "self" || name != &field.name
                                                || operator != Some(expected)
                                                || !verified.tokens.iter().any(|token|
                                                    token.start == binary.right.start
                                                        && token.end == binary.right.end
                                                        && matches!(&token.kind,
                                                            TokenKind::Integer(text) if text == raw)
                                                )
                                            {
                                                return Err(PrivateSelfMethodPrepareError::Unsupported(
                                                    "M2 else 返回算式缺少原 Checker/Lexer 真身分",
                                                ));
                                            }
                                            let literal = crate::syntax_shared::integer_i64(raw)
                                                .map_err(|_| PrivateSelfMethodPrepareError::Unsupported(
                                                    "M2 else 返回運算常值超出 i64",
                                                ))?;
                                            match binary.operator {
                                                '+' => PrivateSelfIntReturn::Add(literal),
                                                '-' => PrivateSelfIntReturn::Subtract(literal),
                                                _ => unreachable!("原 Checker 運算子已驗證"),
                                            }
                                        }
                                        _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                                            "M2 else 寫入後只能 return self.int 或其原 Checker 加減",
                                        )),
                                    };
                                    (None, Some((literal, assignment.node_span())),
                                        Some(returned_stmt.node_span()), Some(calculation))
                                } else {
                                    return Err(PrivateSelfMethodPrepareError::Unsupported(
                                        "M2 else 只能真單 return 或一筆寫入後直接 return",
                                    ));
                                }
                            } else { (None, None, None, None) };
                        (Some((condition, early_stmt.node_span())),
                            parameter_source, else_return, else_write, else_field_return,
                            else_calculation, branch_write, Some(true_return_plan),
                            nested_branch, nested_parameter_source, nested_else_calculation,
                            nested_true_write, nested_else_write)
                    }
                    _ => (None, None, None, None, None, None, None, None, None, None, None, None, None),
                }
            } else { (None, None, None, None, None, None, None, None, None, None, None, None, None) };
        // 同步 self 已取得的參數框架沿用普通函式的 TypedHIR
        // expression/reference/arithmetic，從第一筆到 return 前逐一
        // 編譯真 Copyable int 局部；所有宣告身分均須原 Checker
        // 登錄且只接受先前已合法初始化的名稱，拒絕額外 CFG／Move。
        let mut method_bindings = method.params.iter().map(|param|
            (param.name.clone(), DeclarationId(param.declaration_start))
        ).collect::<BTreeMap<_, _>>();
        let mut method_int_locals = Vec::<(DeclarationId, PrivateScalarReturnExpr)>::new();
        let mut method_int_assignments =
            Vec::<(DeclarationId, PrivateScalarReturnExpr)>::new();
        let mut local_permissions =
            BTreeMap::<String, (DeclarationId, bool)>::new();
        let mut seen_assignment = false;
        for (index, statement) in method.body.statements.iter().enumerate()
            .take(method.body.statements.len() - 1) {
            if (write_value.is_some() || method_branch.is_some()) && index == 0 {
                continue;
            }
            match &statement.kind {
                StmtKind::Declare(local) => {
                    if seen_assignment {
                        return Err(PrivateSelfMethodPrepareError::Unsupported(
                            "方法局部須先全部宣告才可在直線路徑進行更新",
                        ));
                    }
                    let id = DeclarationId(statement.start);
                    let initializer = local.initializer.as_ref().ok_or(
                        PrivateSelfMethodPrepareError::Unsupported(
                            "方法 int 局部須已由原 Checker 證明初始化",
                        ),
                    )?;
                    if local.ty != "int"
                        || !verified.plan.declarations.matches(
                            id, &local.ty, local.mutable,
                        )
                        || method_bindings.contains_key(&local.name) {
                        return Err(PrivateSelfMethodPrepareError::Unsupported(
                            "方法局部缺少真 Checker int 宣告或重複名稱",
                        ));
                    }
                    let expr = PrivateScalarReturnExpr::from_checked_bindings(
                        initializer, &method_bindings, &verified,
                    ).map_err(|_| PrivateSelfMethodPrepareError::Unsupported(
                        "方法局部初始化缺少原 Checker TypedHIR／Reference／Arithmetic",
                    ))?;
                    let mut calls = Vec::new();
                    expr.collect_calls(&mut calls);
                    let approved_helper = calls.len() == 1 && matches!(
                        &expr,
                        PrivateScalarReturnExpr::Call { name, arguments, awaited: false }
                            if arguments.is_empty() &&
                            method_helper.as_ref().is_some_and(|(target, _)| target == name)
                    );
                    if (!calls.is_empty() && !approved_helper)
                        || matches!(expr, PrivateScalarReturnExpr::Unreachable) {
                        return Err(PrivateSelfMethodPrepareError::Unsupported(
                            "方法 int 局部不支援未核准的普通函式或不可達來源",
                        ));
                    }
                    method_int_locals.push((id, expr));
                    method_bindings.insert(local.name.clone(), id);
                    local_permissions.insert(local.name.clone(), (id, local.mutable));
                }
                StmtKind::Assign(assignment) => {
                    seen_assignment = true;
                    let (id, writable) = *local_permissions.get(&assignment.name)
                        .ok_or(PrivateSelfMethodPrepareError::Unsupported(
                            "方法 int 重賦值目標不是原 Checker 已初始化局部",
                        ))?;
                    if !writable
                        || verified.plan.scalar_hir.assignment(statement.node_span())
                            .is_none_or(|fact|
                                fact.resolved.declaration != id
                                || fact.resolved.ty != ScalarType::Int)
                        || !verified.tokens.iter().any(|token|
                            token.start == statement.start
                                && matches!(&token.kind, TokenKind::Name(found)
                                    if found == &assignment.name)) {
                        return Err(PrivateSelfMethodPrepareError::Unsupported(
                            "方法局部重賦值缺少原 Checker 可寫授權／TypedHIR／Lexer",
                        ));
                    }
                    let expr = PrivateScalarReturnExpr::from_checked_bindings(
                        &assignment.expression, &method_bindings, &verified,
                    ).map_err(|_| PrivateSelfMethodPrepareError::Unsupported(
                        "方法 var int 更新算式沒有原 Checker TypedHIR 來源",
                    ))?;
                    let mut calls = Vec::new();
                    expr.collect_calls(&mut calls);
                    if !calls.is_empty()
                        || matches!(expr, PrivateScalarReturnExpr::Unreachable) {
                        return Err(PrivateSelfMethodPrepareError::Unsupported(
                            "方法 var int 更新不能偷渡未核准的普通函式呼叫",
                        ));
                    }
                    method_int_assignments.push((id, expr));
                }
                _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                    "方法僅接通純 int 區域宣告、後續直線更新及返回",
                )),
            }
        }
        let last = method.body.statements.last().expect("方法已證明非空");
        let StmtKind::Return(returned) = &last.kind else {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "本期方法必須以已核准 return self.int欄位結束",
            ));
        };
        // 回傳允許原 Checker 已核准的 self.int、單一 int 參數名稱，
        // 或兩者與真正 Lexer int 常值的加／減。方法 Checker 自己
        // 的 MethodSafety.checked_returns 是原權威，不能重簽 HIR。
        // 名稱讀取另以 Parser 參數起點、TypedMember 與原 Lexer
        // 名稱 token 綁定，禁止取錯多載或讀取同名 owner。
        // 方法含真局部時，返回式直接復用同一 CheckedScalar
        // 計畫；目前僅限不讀 self.field 的純 int 子集，避免假造
        // 尚未實作的泛化混合 FieldRead expression evaluator。
        let (return_calculation, self_read) = if !method_int_locals.is_empty() {
            let plan = PrivateScalarReturnExpr::from_checked_bindings(
                &returned.expression, &method_bindings, &verified,
            ).map_err(|_| PrivateSelfMethodPrepareError::Unsupported(
                "局部參與的方法返回缺少原 Checker 純 int TypedHIR",
            ))?;
            let mut calls = Vec::new();
            plan.collect_calls(&mut calls);
            if !calls.is_empty() || matches!(plan, PrivateScalarReturnExpr::Unreachable) {
                return Err(PrivateSelfMethodPrepareError::Unsupported(
                    "局部返回不支援尚未接通的函式 Call 或不可達來源",
                ));
            }
            (PrivateSelfIntReturn::CheckedScalar(plan), None)
        } else {
            match &returned.expression.kind {
            TermKind::FieldRead(receiver, read_field) =>
                (PrivateSelfIntReturn::Field,
                    Some((receiver.as_str(), read_field.as_str()))),
            TermKind::Name(_) => {
                let id = private_self_method_parameter_reference(
                    &verified, method, argument_binding,
                    &additional_argument_bindings, &returned.expression,
                ).ok_or(PrivateSelfMethodPrepareError::Unsupported(
                    "方法返回參數沒有原 Checker 身分及 Lexer 名稱證據",
                ))?;
                (PrivateSelfIntReturn::Parameter(id), None)
            }
            TermKind::BinaryInt(binary) => {
                let left_span = binary.left.node_span();
                let right_span = binary.right.node_span();
                let actual_operator = verified.source.bytes.get(
                    left_span.end..right_span.start,
                ).and_then(|raw| std::str::from_utf8(raw).ok())
                    .map(str::trim);
                if !matches!(binary.operator, '+' | '-')
                    || actual_operator != Some(if binary.operator == '+' { "+" } else { "-" }) {
                    return Err(PrivateSelfMethodPrepareError::Unsupported(
                        "方法返回的加減運算子不符合原 Parser 來源位置",
                    ));
                }
                let (mut self_read, param_id) = match &binary.left.kind {
                    TermKind::FieldRead(receiver, read_field) =>
                        (Some((receiver.as_str(), read_field.as_str())), None),
                    TermKind::Name(_) => {
                        let id = private_self_method_parameter_reference(
                            &verified, method, argument_binding,
                            &additional_argument_bindings, &binary.left,
                        ).ok_or(PrivateSelfMethodPrepareError::Unsupported(
                            "方法加減參數沒有原 Checker 身分及 Lexer 名稱證據",
                        ))?;
                        (None, Some(id))
                    }
                    _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                        "方法加減左端僅接通 self.int 或真正 int 參數",
                    )),
                };
                let operation = match &binary.right.kind {
                    TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) => {
                        if !verified.tokens.iter().any(|token|
                            token.start == binary.right.start && token.end == binary.right.end
                                && matches!(&token.kind, TokenKind::Integer(text) if text == raw)
                        ) {
                            return Err(PrivateSelfMethodPrepareError::Unsupported(
                                "方法加減的整數 token 不屬於原 Parser／Checker 來源",
                            ));
                        }
                        let literal = crate::syntax_shared::integer_i64(raw).map_err(|_|
                            PrivateSelfMethodPrepareError::Unsupported(
                                "方法加減 int 常值超出 i64",
                            ),
                        )?;
                        match (binary.operator, param_id) {
                            ('+', None) => PrivateSelfIntReturn::Add(literal),
                            ('-', None) => PrivateSelfIntReturn::Subtract(literal),
                            ('+', Some(id)) => PrivateSelfIntReturn::ParameterAdd(id, literal),
                            ('-', Some(id)) => PrivateSelfIntReturn::ParameterSubtract(id, literal),
                            _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                                "方法加減運算子不合法",
                            )),
                        }
                    }
                    TermKind::Name(_) => {
                        let right_id = private_self_method_parameter_reference(
                            &verified, method, argument_binding,
                            &additional_argument_bindings, &binary.right,
                        ).ok_or(PrivateSelfMethodPrepareError::Unsupported(
                            "右端 int 參數缺少真 Checker／Lexer 名稱身分",
                        ))?;
                        match (binary.operator, param_id) {
                            ('+', Some(left_id)) =>
                                PrivateSelfIntReturn::ParameterPairAdd(left_id, right_id),
                            ('-', Some(left_id)) =>
                                PrivateSelfIntReturn::ParameterPairSubtract(left_id, right_id),
                            ('+', None) if self_read.is_some() =>
                                PrivateSelfIntReturn::FieldAddParameter(right_id),
                            ('-', None) if self_read.is_some() =>
                                PrivateSelfIntReturn::FieldSubtractParameter(right_id),
                            _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                                "欄位與已核准 int 參數的加減來源不一致",
                            )),
                        }
                    }
                    TermKind::FieldRead(receiver, read_field) => {
                        // 欄位在右端一樣要以原 Checker 的 self 借用
                        // 讀取；不能因可在左端辨識參數就跳過欄位權限。
                        let left_id = param_id.ok_or(
                            PrivateSelfMethodPrepareError::Unsupported(
                                "右端 self 欄位只能搭配已驗證的 int 形式參數",
                            ),
                        )?;
                        if self_read.is_some() {
                            return Err(PrivateSelfMethodPrepareError::Unsupported(
                                "本子集不支援一個算式中多次 self 欄位讀取",
                            ));
                        }
                        self_read = Some((receiver.as_str(), read_field.as_str()));
                        match binary.operator {
                            '+' => PrivateSelfIntReturn::ParameterAddField(left_id),
                            '-' => PrivateSelfIntReturn::ParameterSubtractField(left_id),
                            _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                                "參數與 self 欄位只接受原 Checker 加減運算子",
                            )),
                        }
                    }
                    _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                        "方法加減右端僅接通已驗證 int 常值、參數或 self 欄位",
                    )),
                };
                (operation, self_read)
            }
            _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                "方法返回僅接通原 Checker self.int 或已驗證單 int 參數的真值／加減",
            )),
            }
        };
        let method_span = NodeSpan::new(method.start, method.end);
        let method_fact = verified.plan.method_safety.get(&method.start)
            .ok_or(PrivateSelfMethodPrepareError::Unsupported(
                "缺少正式 Checker 的 VerifiedMethodSafety",
            ))?;
        let required_kind = if write_value.is_some()
            || method_branch_write.is_some() || method_branch_else_write.is_some()
            || method_nested_true_write.is_some() || method_nested_else_write.is_some() {
            BorrowKind::ExclusiveWrite
        } else { BorrowKind::SharedRead };
        if self_read.is_some_and(|(receiver, read_field)|
                receiver != "self" || read_field != field.name.as_str())
            || method_branch.is_some_and(|(_, span)|
                !method_fact.checked_returns.contains(&span))
            || method_branch_else.is_some_and(|(_, span)|
                !method_fact.checked_returns.contains(&span))
            || method_branch_else_field_return.is_some_and(|span|
                !method_fact.checked_returns.contains(&span))
            || (method_branch_else_write.is_some()
                && method_branch_else_field_return.is_none())
            || (method_branch_else.is_some()
                && method_branch_else_field_return.is_some())
            || (method_branch_else_calculation.is_some()
                != method_branch_else_field_return.is_some())
            || (method_branch.is_some() != method_branch_return.is_some())
            || method_nested_branch.is_some_and(|(_, span)|
                !method_fact.checked_returns.contains(&span))
            || (method_nested_branch.is_some() && method_branch.is_none())
            || (method_nested_else_calculation.is_some() != method_nested_branch.is_some())
            || ((method_nested_true_write.is_some() || method_nested_else_write.is_some())
                && method_nested_branch.is_none())
            || (method_nested_branch_parameter.is_some()
                && (method_nested_branch.is_none() || method_bool_argument.is_none()))
            || (method_branch_else.is_some() && method_branch.is_none())
            || (method_branch_parameter.is_some() && method_bool_argument.is_none())
            || method_fact.span != method_span
            || method_fact.self_borrow != required_kind
            || !method_fact.checked_returns.contains(&last.node_span()) {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "返回位置、self 共享模式與原 Checker 真方法證據不一致",
            ));
        }
        if main.name != "main" || main.result_ty != "int"
            || !main.params.is_empty() || main.body.statements.len() < 2 {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "主函式限先建立真正結構再直接返回方法呼叫",
            ));
        }
        let StmtKind::Declare(owner) = &main.body.statements[0].kind else {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "主函式首句必須是唯一持有者宣告",
            ));
        };
        let owner_id = DeclarationId(main.body.statements[0].start);
        if owner.ty != definition_name(&verified)?
            || !verified.plan.declarations.matches(owner_id, &owner.ty, owner.mutable) {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "接收者缺少原 Checker 結構型別與真正 DeclarationId",
            ));
        }
        let constructor = owner.initializer.as_ref().ok_or(
            PrivateSelfMethodPrepareError::Unsupported("結構需真建立式"),
        )?;
        let is_ctor = match &constructor.kind {
            TermKind::UnresolvedZeroArgs(name) => name == &owner.ty,
            TermKind::Construct(name, args) => name == &owner.ty && args.is_empty(),
            _ => false,
        };
        if !is_ctor {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "不得用隱含結構複製或未驗證的其它來源當接收者",
            ));
        }
        let StmtKind::Return(call_return) = &last_main_statement.kind else {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "主函式必須直接 return 真方法呼叫",
            ));
        };
        let TermKind::MethodCall(receiver, called, args) = &call_return.expression.kind else {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "主函式須實際呼叫 S16 instance.method()",
            ));
        };
        let call_span = call_return.expression.node_span();
        if receiver != &owner.name || called != &method.name
            || args.len() != usize::from(argument_binding.is_some())
                + additional_argument_bindings.len()
                + usize::from(method_bool_argument.is_some())
            || verified.plan.scalar_hir.expression(call_span)
                .is_none_or(|fact| fact.ty != ScalarType::Int) {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "方法名稱、真接收者或 int 返回缺少原 Parser／Checker TypedHIR",
            ));
        }
        let facts = verified.plan.declarations.call_borrows_at(call_span)
            .ok_or(PrivateSelfMethodPrepareError::Unsupported(
                "本次方法呼叫缺少正式 Checker call_borrows 自動 self 事實",
            ))?;
        let loan = BorrowFact::new(owner_id, required_kind);
        if (write_value.is_some() || method_branch_write.is_some()
            || method_branch_else_write.is_some()
            || method_nested_true_write.is_some() || method_nested_else_write.is_some())
            && (!owner.mutable || field.immutable) {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "獨占 self 僅能修改 var 接收者及 var int 欄位",
            ));
        }
        if facts != [loan] {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "自動 self 借用並非已核准單一共享唯讀原持有者",
            ));
        }
        // 提前複製 AST 字段，才移交整份已驗證計畫所有權。
        let struct_name = owner.ty.clone();
        let owner_name = owner.name.clone();
        let owner_mutable = owner.mutable;
        let field_name = field.name.clone();
        let field_immutable = field.immutable;
        let method_name = method.name.clone();
        let original = verified.source.bytes.clone().into_boxed_slice();
        Ok(Self {
            verified, original, struct_name, owner: owner_name,
            owner_id, owner_mutable, field: field_name, field_value, field_immutable,
            write_value, return_calculation, method_int_locals,
            method_helper, method_branch, method_branch_else,
            method_branch_else_write, method_branch_else_field_return,
            method_branch_else_calculation, method_branch_write, method_branch_return,
            method_nested_branch, method_nested_branch_parameter,
            method_nested_else_calculation, method_nested_true_write,
            method_nested_else_write, method_branch_parameter, method_int_assignments,
            method: method_name, method_span, call_span, loan,
            argument_binding, additional_argument_bindings,
            method_bool_argument, main_int_locals, main_init_plans, argument_sources, argument_plans,
        })
    }

    fn matches_source(&self, text: &str) -> bool {
        let Ok(fresh) = Self::prepare(text) else { return false; };
        self.original == fresh.original
            && self.struct_name == fresh.struct_name
            && self.owner == fresh.owner && self.owner_id == fresh.owner_id
            && self.owner_mutable == fresh.owner_mutable
            && self.field == fresh.field && self.field_value == fresh.field_value
            && self.field_immutable == fresh.field_immutable
            && self.write_value == fresh.write_value
            && self.return_calculation == fresh.return_calculation
            && self.method_int_locals == fresh.method_int_locals
            && self.method_helper == fresh.method_helper
            && self.method_branch == fresh.method_branch
            && self.method_branch_else == fresh.method_branch_else
            && self.method_branch_else_write == fresh.method_branch_else_write
            && self.method_branch_else_field_return == fresh.method_branch_else_field_return
            && self.method_branch_else_calculation == fresh.method_branch_else_calculation
            && self.method_branch_write == fresh.method_branch_write
            && self.method_branch_return == fresh.method_branch_return
            && self.method_nested_branch == fresh.method_nested_branch
            && self.method_nested_branch_parameter == fresh.method_nested_branch_parameter
            && self.method_nested_else_calculation == fresh.method_nested_else_calculation
            && self.method_nested_true_write == fresh.method_nested_true_write
            && self.method_nested_else_write == fresh.method_nested_else_write
            && self.method_branch_parameter == fresh.method_branch_parameter
            && self.method_int_assignments == fresh.method_int_assignments
            && self.method == fresh.method && self.method_span == fresh.method_span
            && self.call_span == fresh.call_span && self.loan == fresh.loan
            && self.argument_binding == fresh.argument_binding
            && self.additional_argument_bindings == fresh.additional_argument_bindings
            && self.method_bool_argument == fresh.method_bool_argument
            && self.main_int_locals == fresh.main_int_locals
            && self.main_init_plans == fresh.main_init_plans
            && self.argument_sources == fresh.argument_sources
            && self.argument_plans == fresh.argument_plans
            && self.verified.plan.method_safety == fresh.verified.plan.method_safety
            && self.verified.plan.declarations.call_borrows
                == fresh.verified.plan.declarations.call_borrows
    }

    fn execute(&self) -> Result<PrivateSelfMethodOutcome, PrivateSelfMethodPrepareError> {
        self.execute_with_fault(false)
    }

    /// 僅 Rust 測試可注入方法存活期錯誤，不新增 voiles 例外語意。
    fn execute_with_fault(&self, fail_inside_method: bool)
        -> Result<PrivateSelfMethodOutcome, PrivateSelfMethodPrepareError> {
        let Ok(text) = std::str::from_utf8(&self.verified.source.bytes) else {
            return Err(PrivateSelfMethodPrepareError::SourceChanged);
        };
        if self.original.as_ref() != self.verified.source.bytes.as_slice()
            || !self.verified.plan.is_consistent(
                &self.verified.structs, &self.verified.functions, text,
            ) || !self.matches_source(text) {
            return Err(PrivateSelfMethodPrepareError::SourceChanged);
        }
        // 在建立任何 method self BorrowTicket 之前，以 main 真宣告順序
        // 將 Copyable int 局部求值寫入自己的執行期框架。具名複製
        // 必須實際讀取先前槽位，不把 prepare 中的常值當成唯一動態證據。
        if self.main_int_locals.len() != self.main_init_plans.len() {
            return Err(PrivateSelfMethodPrepareError::SourceChanged);
        }
        let mut main_values = BTreeMap::<DeclarationId, i64>::new();
        let mut main_initialization_error = None;
        // 普通函式可以在 self 方法開始前實際完成；與方法本體
        // 及其真正 BorrowTicket 使用同一動態進退帳，不能在
        // prepare 預寫假 FunctionEntered／Returned。
        let mut exit_events = Vec::<PrivateSharedCallExitEvent>::new();
        let mut ordinary_calls_completed = 0usize;
        let mut branch_checked = None::<bool>;
        let mut nested_branch_checked = None::<bool>;
        let mut branch_write_applied = false;
        let mut branch_else_write_applied = false;
        let mut nested_write_applied = false;
        let mut nested_else_write_applied = false;
        for ((_, id, planned, _), initializer) in self.main_int_locals.iter()
            .zip(&self.main_init_plans) {
            let executed = match initializer {
                PrivateScalarReturnExpr::Call {
                    name, arguments, awaited: false,
                } if arguments.is_empty()
                    && self.method_helper.as_ref().is_some_and(|(target, _)| target == name) => {
                    let (_, helper) = self.method_helper.as_ref()
                        .ok_or(PrivateSelfMethodPrepareError::SourceChanged)?;
                    exit_events.push(PrivateSharedCallExitEvent::FunctionEntered);
                    let value = helper.evaluate(&BTreeMap::new());
                    if value.is_ok() {
                        exit_events.push(PrivateSharedCallExitEvent::Returned);
                        exit_events.push(PrivateSharedCallExitEvent::FunctionExited {
                            returned: true,
                        });
                        ordinary_calls_completed += 1;
                    }
                    value
                }
                _ => initializer.evaluate(&main_values),
            };
            let actual = match executed {
                Ok(value) => value,
                Err(PrivateScalarCallRuntimeError::IntegerOverflow) => {
                    main_initialization_error =
                        Some(PrivateSelfMethodRuntimeError::IntegerOverflow);
                    break;
                }
                Err(_) => return Err(PrivateSelfMethodPrepareError::SourceChanged),
            };
            if actual != *planned || main_values.insert(*id, actual).is_some() {
                return Err(PrivateSelfMethodPrepareError::SourceChanged);
            }
        }
        if self.argument_sources.len()
            != usize::from(self.argument_binding.is_some())
                + self.additional_argument_bindings.len()
            || self.argument_plans.len() != self.argument_sources.len() {
            return Err(PrivateSelfMethodPrepareError::SourceChanged);
        }
        use crate::borrow_safety::BorrowSafety;
        use crate::mixed_struct_runtime::{
            MixedStructRuntime, MixedFieldSpec, ScalarKind, SeedValue, Observed,
        };
        let mut runtime = MixedStructRuntime::new();
        let specs = [MixedFieldSpec {
            name: &self.field, kind: ScalarKind::Int,
            default: Some(SeedValue::Int(self.field_value)),
            immutable: self.field_immutable,
        }];
        runtime.construct(&self.owner, &specs, &[], None)
            .map_err(PrivateSelfMethodPrepareError::Cleanup)?;
        let mut tracker = BorrowSafety::new();
        let mut tickets = Vec::new();
        let mut facts = Vec::new();
        let mut method_entered = false;
        let mut method_exited = false;
        let mut method_local_events = Vec::<PrivateSelfMethodLocalEvent>::new();
        let mut initialized_method_locals = Vec::<DeclarationId>::new();
        let mut parameter_frame = std::collections::BTreeMap::<DeclarationId, i64>::new();
        let mut bool_parameter_frame = std::collections::BTreeMap::<DeclarationId, bool>::new();
        let main_locals_complete = main_initialization_error.is_none();
        let result = (|| -> Result<i64, PrivateSelfMethodRuntimeError> {
            // main 複合 int 初始化在任何方法 BorrowTicket 簽發前
            // 就可能受控溢位，尚未進入的方法不能留下假進／退出。
            if let Some(error) = main_initialization_error {
                return Err(error);
            }
            // 完成原 Checker 參數的真值框架綁定；選中來源返回時
            // 僅由同一 DeclarationId 讀取此框架，不重用 owner 值。
            if let Some((id, value)) = self.method_bool_argument {
                if bool_parameter_frame.insert(id, value).is_some() {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
            }
            for ((id, prepared), expression) in self.argument_binding.into_iter()
                .chain(self.additional_argument_bindings.iter().copied())
                .zip(&self.argument_plans) {
                // 按原方法引數次序真正求值；尚未完成全部引數前
                // 不會取得 self BorrowTicket 或偽造進入方法框架。
                let actual = expression.evaluate(&main_values).map_err(|reason| match reason {
                    PrivateScalarCallRuntimeError::IntegerOverflow =>
                        PrivateSelfMethodRuntimeError::IntegerOverflow,
                    _ => PrivateSelfMethodRuntimeError::WrongReceiver,
                })?;
                if actual != prepared || parameter_frame.insert(id, actual).is_some() {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
            }
            let ticket = tracker.acquire(self.loan.source, self.loan.kind)
                .map_err(PrivateSelfMethodRuntimeError::Ticket)?;
            exit_events.push(PrivateSharedCallExitEvent::LoanAcquired(ticket));
            tickets.push(ticket);
            facts.push(self.loan);
            tracker.verify_static_snapshot(facts.iter().copied())
                .map_err(PrivateSelfMethodRuntimeError::Ticket)?;
            match tracker.check_direct_access(self.owner_id) {
                Err(crate::borrow_safety::BorrowSafetyError::SourceIsBorrowed) => {}
                Err(other) => return Err(PrivateSelfMethodRuntimeError::Ticket(other)),
                Ok(()) => return Err(PrivateSelfMethodRuntimeError::WrongReceiver),
            }
            method_entered = true;
            exit_events.push(PrivateSharedCallExitEvent::FunctionEntered);
            if fail_inside_method {
                return Err(PrivateSelfMethodRuntimeError::InjectedFailure);
            }
            // 由已取得的 self 真方法框架按宣告次序執行局部。
            // 不進入任何未達作用域的局部，也不誤用 main 的值槽。
            for (id, initializer) in &self.method_int_locals {
                let calculated = match initializer {
                    PrivateScalarReturnExpr::Call { name, arguments, awaited: false }
                        if arguments.is_empty() &&
                            self.method_helper.as_ref().is_some_and(|(target, _)| target == name) => {
                        // 不是 prepare 階段求值：只有走到方法此局部時，
                        // 由同一原 Checker 的真 helper int 工作計畫執行，
                        // 在 self 的真借用存活期間進／退普通函式框架。
                        let (_, checked) = self.method_helper.as_ref()
                            .ok_or(PrivateSelfMethodRuntimeError::WrongReceiver)?;
                        exit_events.push(PrivateSharedCallExitEvent::FunctionEntered);
                        let value = checked.evaluate(&BTreeMap::new())
                            .map_err(|_| PrivateSelfMethodRuntimeError::WrongReceiver)?;
                        exit_events.push(PrivateSharedCallExitEvent::Returned);
                        exit_events.push(PrivateSharedCallExitEvent::FunctionExited {
                            returned: true,
                        });
                        ordinary_calls_completed += 1;
                        value
                    }
                    _ => initializer.evaluate(&parameter_frame)
                        .map_err(|reason| match reason {
                            PrivateScalarCallRuntimeError::IntegerOverflow =>
                                PrivateSelfMethodRuntimeError::IntegerOverflow,
                            _ => PrivateSelfMethodRuntimeError::WrongReceiver,
                        })?,
                };
                if parameter_frame.insert(*id, calculated).is_some() {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
                initialized_method_locals.push(*id);
                method_local_events.push(PrivateSelfMethodLocalEvent::Initialized(
                    *id, calculated,
                ));
                exit_events.push(PrivateSharedCallExitEvent::LocalInitialized(*id));
            }
            // 只有原 Checker 正式核准的 var int 局部允許依序更新；
            // 先完成 RHS 真值計算再提交目標，故障時保留舊值
            // 直到原有 ScopeEnded 逆序清理。
            for (id, expression) in &self.method_int_assignments {
                if !initialized_method_locals.contains(id) {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
                let value = expression.evaluate(&parameter_frame)
                    .map_err(|reason| match reason {
                        PrivateScalarCallRuntimeError::IntegerOverflow =>
                            PrivateSelfMethodRuntimeError::IntegerOverflow,
                        _ => PrivateSelfMethodRuntimeError::WrongReceiver,
                    })?;
                if parameter_frame.insert(*id, value).is_none() {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
                method_local_events.push(PrivateSelfMethodLocalEvent::Assigned(
                    *id, value,
                ));
                exit_events.push(PrivateSharedCallExitEvent::LocalAssigned(*id));
            }
            // 條件只於真 self 框架進入後，從原 Checker bool 參數
            // 獨立框架或 Lexer 常值取得；支線有寫入時不能先寫後選。
            let selected_branch = if let Some((condition, _)) = self.method_branch {
                let observed = if let Some(id) = self.method_branch_parameter {
                    *bool_parameter_frame.get(&id).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver,
                    )?
                } else { condition };
                if observed != condition {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
                branch_checked = Some(observed);
                Some(observed)
            } else { None };
            // 只有外層 true 已真正抵達時才觀測內層 Checker bool，
            // 並在寫入 self 前確定唯一被選中的巢狀葉。
            if selected_branch == Some(true) {
                if let Some((condition, _)) = self.method_nested_branch {
                    let observed = if let Some(id) = self.method_nested_branch_parameter {
                        *bool_parameter_frame.get(&id).ok_or(
                            PrivateSelfMethodRuntimeError::WrongReceiver,
                        )?
                    } else { condition };
                    if observed != condition {
                        return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                    }
                    nested_branch_checked = Some(observed);
                }
            }
            // 原 Checker 只簽發一張方法入口票據；僅選中葉可真寫。
            let selected_write = self.write_value.or_else(|| {
                match selected_branch {
                    Some(true) if self.method_nested_branch.is_some() =>
                        if nested_branch_checked == Some(true) {
                            self.method_nested_true_write.map(|(value, _)| value)
                        } else {
                            self.method_nested_else_write.map(|(value, _)| value)
                        },
                    Some(true) => self.method_branch_write.map(|(value, _)| value),
                    Some(false) => self.method_branch_else_write.map(|(value, _)| value),
                    None => None,
                }
            });
            if let Some(value) = selected_write {
                let authorized = tracker.check_ticket_access(
                    ticket, BorrowKind::ExclusiveWrite,
                ).map_err(PrivateSelfMethodRuntimeError::Ticket)?;
                if authorized != self.owner_id {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
                runtime.write_int_field(
                    &self.owner, &self.field, value, self.owner_mutable,
                ).map_err(PrivateSelfMethodRuntimeError::Resource)?;
                if selected_branch == Some(true) && self.method_branch_write.is_some() {
                    branch_write_applied = true;
                }
                if selected_branch == Some(false)
                    && self.method_branch_else_write.is_some() {
                    branch_else_write_applied = true;
                }
                if selected_branch == Some(true) && nested_branch_checked == Some(true)
                    && self.method_nested_true_write.is_some() {
                    nested_write_applied = true;
                }
                if selected_branch == Some(true) && nested_branch_checked == Some(false)
                    && self.method_nested_else_write.is_some() {
                    nested_else_write_applied = true;
                }
            }
            // 真方法 self 欄位讀取須以存活 BorrowTicket 授權，不可
            // 僅因原 AST 名稱看似 self 而直接取用所有者。
            let authorized = tracker.check_ticket_access(ticket, BorrowKind::SharedRead)
                .map_err(PrivateSelfMethodRuntimeError::Ticket)?;
            if authorized != self.owner_id {
                return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
            }
            let Observed::Int(value) = runtime.field(&self.owner, &self.field)
                .map_err(PrivateSelfMethodRuntimeError::Resource)? else {
                return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
            };
            let selected_calculation = match selected_branch {
                Some(true) => {
                    if self.method_nested_branch.is_some() {
                        if nested_branch_checked == Some(true) {
                            self.method_branch_return.clone().ok_or(
                                PrivateSelfMethodRuntimeError::WrongReceiver,
                            )?
                        } else {
                            self.method_nested_else_calculation.clone().ok_or(
                                PrivateSelfMethodRuntimeError::WrongReceiver,
                            )?
                        }
                    } else {
                        self.method_branch_return.clone().ok_or(
                            PrivateSelfMethodRuntimeError::WrongReceiver,
                        )?
                    }
                },
                Some(false) => {
                    if let Some(calculation) = &self.method_branch_else_calculation {
                        calculation.clone()
                    } else if let Some((literal, _)) = self.method_branch_else {
                        PrivateSelfIntReturn::Constant(literal)
                    } else {
                        self.return_calculation.clone()
                    }
                }
                None => self.return_calculation.clone(),
            };
            let calculated = match selected_calculation {
                PrivateSelfIntReturn::Field => value,
                PrivateSelfIntReturn::Constant(value) => value,
                PrivateSelfIntReturn::Add(rhs) => value.checked_add(rhs)
                    .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?,
                PrivateSelfIntReturn::Subtract(rhs) => value.checked_sub(rhs)
                    .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?,
                PrivateSelfIntReturn::Parameter(id) =>
                    *parameter_frame.get(&id).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?,
                PrivateSelfIntReturn::ParameterAdd(id, rhs) => {
                    let base = *parameter_frame.get(&id).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    base.checked_add(rhs)
                        .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?
                }
                PrivateSelfIntReturn::ParameterSubtract(id, rhs) => {
                    let base = *parameter_frame.get(&id).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    base.checked_sub(rhs)
                        .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?
                }
                PrivateSelfIntReturn::ParameterPairAdd(left, right) => {
                    let lhs = *parameter_frame.get(&left).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    let rhs = *parameter_frame.get(&right).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    lhs.checked_add(rhs)
                        .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?
                }
                PrivateSelfIntReturn::ParameterPairSubtract(left, right) => {
                    let lhs = *parameter_frame.get(&left).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    let rhs = *parameter_frame.get(&right).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    lhs.checked_sub(rhs)
                        .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?
                }
                PrivateSelfIntReturn::FieldAddParameter(id) => {
                    let rhs = *parameter_frame.get(&id).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    value.checked_add(rhs)
                        .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?
                }
                PrivateSelfIntReturn::FieldSubtractParameter(id) => {
                    let rhs = *parameter_frame.get(&id).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    value.checked_sub(rhs)
                        .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?
                }
                PrivateSelfIntReturn::ParameterAddField(id) => {
                    let lhs = *parameter_frame.get(&id).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    lhs.checked_add(value)
                        .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?
                }
                PrivateSelfIntReturn::ParameterSubtractField(id) => {
                    let lhs = *parameter_frame.get(&id).ok_or(
                        PrivateSelfMethodRuntimeError::WrongReceiver)?;
                    lhs.checked_sub(value)
                        .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?
                }
                PrivateSelfIntReturn::CheckedScalar(ref plan) =>
                    plan.evaluate(&parameter_frame).map_err(|reason| match reason {
                        PrivateScalarCallRuntimeError::IntegerOverflow =>
                            PrivateSelfMethodRuntimeError::IntegerOverflow,
                        _ => PrivateSelfMethodRuntimeError::WrongReceiver,
                    })?,
            };
            method_exited = true;
            exit_events.push(PrivateSharedCallExitEvent::Returned);
            Ok(calculated)
        })();
        if result.is_err() && !exit_events.is_empty() {
            exit_events.push(PrivateSharedCallExitEvent::UnwindStarted);
        }
        // 真局部依逆序離域，無論正常返回、溢位或方法中途故障。
        // 只有實際完成初始化的局部才能留下 ScopeEnded 事件。
        for id in initialized_method_locals.iter().rev() {
            if parameter_frame.remove(id).is_none() {
                return Err(PrivateSelfMethodPrepareError::SourceChanged);
            }
            method_local_events.push(PrivateSelfMethodLocalEvent::ScopeEnded(*id));
            exit_events.push(PrivateSharedCallExitEvent::LocalScopeEnded(*id));
        }
        initialized_method_locals.clear();
        // 不論成功或錯誤，實際參數框架不可洩漏至後續呼叫。
        parameter_frame.clear();
        // 不論成功與否，先按原 Checker 動態 self BorrowFact 原子歸還，
        // 再釋放真正結構資源；失敗不可偽裝成功 return。
        let released = tracker.finish_scope_verified(&tickets, &facts)
            .map_err(PrivateSelfMethodRuntimeError::Ticket);
        if released.is_ok() {
            for ticket in tickets.iter().rev() {
                exit_events.push(PrivateSharedCallExitEvent::LoanReturned(*ticket));
            }
        }
        if method_entered {
            exit_events.push(PrivateSharedCallExitEvent::FunctionExited {
                returned: method_exited,
            });
        }
        let result = match (result, released) {
            (Err(problem), _) => Err(problem),
            (Ok(_), Err(problem)) => Err(problem),
            (Ok(value), Ok(())) => tracker.check_direct_access(self.owner_id)
                .map_err(PrivateSelfMethodRuntimeError::Ticket).map(|()| value),
        };
        let ticket_returned = tracker.check_direct_access(self.owner_id).is_ok();
        let events = runtime.finish().map_err(PrivateSelfMethodPrepareError::Cleanup)?;
        // 與一般函式的 LocalScopeEnded 契約相同：在 main 返回時
        // 刪除全部 Copyable int 局部，且不能殘留到重複執行。
        for (_, id, _, _) in self.main_int_locals.iter().rev() {
            // 若前一個初始化溢位，後面的宣告尚未真正走到，
            // 不能假造其作用域或要求釋放從未配置的純值槽。
            if main_values.remove(id).is_none() && main_locals_complete {
                return Err(PrivateSelfMethodPrepareError::SourceChanged);
            }
        }
        let outcome = PrivateSelfMethodOutcome {
            result, events, method_entered, method_exited, ticket_returned,
            ordinary_calls_completed,
            branch_checked,
            nested_branch_checked,
            nested_write_applied, nested_else_write_applied,
            expected_nested_write: self.method_nested_true_write.is_some(),
            expected_nested_else_write: self.method_nested_else_write.is_some(),
            expected_nested_branch: self.method_nested_branch.map(|(condition, _)| condition),
            branch_write_applied,
            branch_else_write_applied,
            expected_branch_write: self.method_branch_write.is_some(),
            expected_branch_else_write: self.method_branch_else_write.is_some(),
            expected_branch: self.method_branch.map(|(condition, _)| condition),
            main_local_slots_after_exit: main_values.len(),
            method_local_slots_after_exit: initialized_method_locals.len(),
            method_local_events,
            expected_method_local_ids: self.method_int_locals.iter().map(
                |(id, _)| *id,
            ).collect(),
            expected_method_assignment_ids: self.method_int_assignments.iter().map(
                |(id, _)| *id,
            ).collect(),
            checked_loan: self.loan, exit_events,
        };
        if !outcome.has_balanced_self_borrow_trace() {
            return Err(PrivateSelfMethodPrepareError::SourceChanged);
        }
        Ok(outcome)
    }
}


/// 只承認選中方法原參數名稱，且該 int 參數的型別／位置已在
/// prepare() 由原 Checker 封存的 VerifiedTypedMember(Parameter) 核對。
/// 此處重新核對原 Lexer Name token，不用文字比對猜測引用身分。
fn private_self_method_parameter_reference(
    verified: &VerifiedPrivateSource, method: &Function,
    first: Option<(DeclarationId, i64)>,
    additional: &[(DeclarationId, i64)], term: &Term,
) -> Option<DeclarationId> {
    let TermKind::Name(name) = &term.kind else { return None; };
    let parameter = method.params.iter().find(|param| &param.name == name)?;
    let id = DeclarationId(parameter.declaration_start);
    if !first.into_iter().chain(additional.iter().copied())
        .any(|(binding, _)| binding == id) {
        return None;
    }
    verified.tokens.iter().any(|token|
        token.start == term.start && token.end == term.end
            && matches!(&token.kind, TokenKind::Name(actual) if actual == name)
    ).then_some(id)
}

fn definition_name(verified: &VerifiedPrivateSource)
    -> Result<String, PrivateSelfMethodPrepareError> {
    let (name, _) = verified.structs.iter().next()
        .ok_or(PrivateSelfMethodPrepareError::Unsupported("缺少原正式結構宣告"))?;
    Ok(name.clone())
}

#[cfg(test)]
mod g18_private_method_self_tests {
    use super::*;

    fn checked(src: &str) -> PreparedPrivateSelfMethod {
        PreparedPrivateSelfMethod::prepare(src)
            .expect("已核准同步唯讀 self 方法須有正式 Checker 及 BorrowFact")
    }

    fn normal() -> &'static str {
        "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.get()\n}\n"
    }

    #[test]
    fn g18_f_真同步方法_self_使用原_checker_共享票據讀取實際_int_欄位() {
        let plan = checked(normal());
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(7));
        assert!(outcome.method_entered && outcome.method_exited);
        assert!(outcome.ticket_returned);
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn g18_f_合法_const_結構接收者共享_self_不需可寫權限() {
        let src = "struct Counter{\nvar count:int = 5\nfn get()->int{\nreturn self.count\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.get()\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(5));
        assert!(outcome.ticket_returned);
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn g18_f_方法真借用存活期間發生測試故障也要先歸還票據再離域() {
        let result = checked(normal()).execute_with_fault(true).unwrap();
        assert_eq!(result.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(result.method_entered);
        assert!(!result.method_exited);
        assert!(result.ticket_returned);
        assert!(result.events.is_empty());
    }

    #[test]
    fn g18_f_偽造動態_self_來源或借用_kind_必在配置前拒絕() {
        let mut plan = checked(normal());
        plan.loan.source = DeclarationId(0);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(normal());
        plan.loan.kind = BorrowKind::ExclusiveWrite;
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_移除真_checker_方法自動借用或方法證據不能執行() {
        let mut plan = checked(normal());
        plan.verified.plan.declarations.call_borrows.clear();
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(normal());
        plan.verified.plan.method_safety.clear();
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_自動_self_錯誤返回型別與消耗整個_self_由原_checker_拒絕() {
        let wrong_type = "struct Counter{\nvar count:int = 7\nfn get()->bool{\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.get()\n}\n";
        let wrong_move = "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.move()\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.get()\n}\n";
        for source in [wrong_type, wrong_move] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(source),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }

    #[test]
    fn g18_f_獨占_self_真寫入_var_int_並以同張票據返回更新後值() {
        let src = "struct Counter{\nvar count:int = 1\nfn set()->int{\nself.count = 9\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.set()\n}\n";
        let plan = checked(src);
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        assert_eq!(plan.write_value, Some(9));
        let actual = plan.execute().unwrap();
        assert_eq!(actual.result, Ok(9));
        assert!(actual.method_entered && actual.method_exited);
        assert!(actual.ticket_returned);
        assert!(actual.events.is_empty());
    }

    #[test]
    fn g18_f_獨占_self_真寫入後重跑同一已驗證來源不保留前次狀態() {
        let src = "struct Counter{\nvar count:int = 2\nfn set()->int{\nself.count = 17\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.set()\n}\n";
        let plan = checked(src);
        for _ in 0..3 {
            let actual = plan.execute().unwrap();
            assert_eq!(actual.result, Ok(17));
            assert!(actual.ticket_returned && actual.events.is_empty());
        }
    }

    #[test]
    fn g18_f_獨占_self_執行中故障必歸還票據且不報成功返回() {
        let src = "struct Counter{\nvar count:int = 7\nfn set()->int{\nself.count = 8\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.set()\n}\n";
        let plan = checked(src);
        let failure = plan.execute_with_fault(true).unwrap();
        assert_eq!(failure.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(failure.method_entered);
        assert!(!failure.method_exited);
        assert!(failure.ticket_returned);
        assert!(failure.events.is_empty());
        let ok = plan.execute().unwrap();
        assert_eq!(ok.result, Ok(8));
    }

    #[test]
    fn g18_f_const_接收者不能以獨占_self_修改原欄位() {
        let src = "struct Counter{\nvar count:int = 7\nfn set()->int{\nself.count = 8\nreturn self.count\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.set()\n}\n";
        assert!(matches!(PreparedPrivateSelfMethod::prepare(src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn g18_f_const_欄位即使接收者為_var_也不能由方法覆寫() {
        let src = "struct Counter{\nconst count:int = 7\nfn set()->int{\nself.count = 8\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.set()\n}\n";
        assert!(matches!(PreparedPrivateSelfMethod::prepare(src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn g18_f_篡改已驗證的獨占_self_票據或寫入值在動態配置前拒絕() {
        let src = "struct Counter{\nvar count:int = 7\nfn set()->int{\nself.count = 8\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.set()\n}\n";
        let mut changed = checked(src);
        changed.loan.kind = BorrowKind::SharedRead;
        assert!(matches!(changed.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut changed = checked(src);
        changed.write_value = Some(999);
        assert!(matches!(changed.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_獨占_self_寫入禁止非_int_值不藉執行器自行轉型() {
        let src = "struct Counter{\nvar count:int = 7\nfn set()->int{\nself.count = true\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.set()\n}\n";
        assert!(matches!(PreparedPrivateSelfMethod::prepare(src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn g18_f_共享_self_真欄位加法按原_checker_hir_求值() {
        let src = "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.count+3\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.get()\n}\n";
        let plan = checked(src);
        assert_eq!(plan.loan.kind, BorrowKind::SharedRead);
        assert_eq!(plan.return_calculation, PrivateSelfIntReturn::Add(3));
        let actual = plan.execute().unwrap();
        assert_eq!(actual.result, Ok(10));
        assert!(actual.method_entered && actual.method_exited && actual.ticket_returned);
        assert!(actual.events.is_empty());
    }

    #[test]
    fn g18_f_獨占_self_寫入後真欄位減法按最新值計算() {
        let src = "struct Counter{\nvar count:int = 2\nfn change()->int{\nself.count = 20\nreturn self.count-4\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.change()\n}\n";
        let plan = checked(src);
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        assert_eq!(plan.return_calculation, PrivateSelfIntReturn::Subtract(4));
        let actual = plan.execute().unwrap();
        assert_eq!(actual.result, Ok(16));
        assert!(actual.method_entered && actual.method_exited && actual.ticket_returned);
    }

    #[test]
    fn g18_f_共享_self_真欄位加法溢位受控錯誤且歸還票據() {
        let src = "struct Counter{\nvar count:int = 9223372036854775807\nfn get()->int{\nreturn self.count+1\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.get()\n}\n";
        let actual = checked(src).execute().unwrap();
        assert_eq!(actual.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(actual.method_entered && !actual.method_exited && actual.ticket_returned);
        assert!(actual.events.is_empty());
    }

    #[test]
    fn g18_f_獨占_self_真欄位減法溢位亦不得保留借用() {
        let src = "struct Counter{\nvar count:int = 0\nfn change()->int{\nself.count = 0\nreturn self.count-9223372036854775807-2\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.change()\n}\n";
        // 本子集只接通 self.field ± 一個原 Checker int 常值；
        // 巢狀運算不得冒充已執行，必須在配置前拒絕。
        assert!(matches!(PreparedPrivateSelfMethod::prepare(src),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
    }

    #[test]
    fn g18_f_竄改已核准方法算術運算子或常值須在配置前拒絕() {
        let src = "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.count+3\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.get()\n}\n";
        let mut plan = checked(src);
        plan.return_calculation = PrivateSelfIntReturn::Subtract(3);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(src);
        plan.return_calculation = PrivateSelfIntReturn::Add(999);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_錯誤方法加法型別與未知欄位由原_checker_拒絕() {
        for src in [
            "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.count+true\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.get()\n}\n",
            "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.missing+1\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.get()\n}\n",
        ] {
            assert!(matches!(PreparedPrivateSelfMethod::prepare(src),
                Err(PrivateSelfMethodPrepareError::Check(_))));
        }
    }
    fn overloaded_zero_source(reverse: bool, mutable: bool) -> String {
        let read = "fn get()->int{\nreturn self.count+3\n}\n";
        let other = "fn get(n:int)->int{\nself.count = 99\nreturn self.count\n}\n";
        let methods = if reverse { format!("{other}{read}") } else { format!("{read}{other}") };
        let owner = if mutable { "var" } else { "const" };
        format!("struct Counter{{\nvar count:int = 7\n{methods}}}\nfn main()->int{{\n{owner} value:Counter = Counter()\nreturn value.get()\n}}\n")
    }

    #[test]
    fn g18_f_同名方法多載真執行零參數共享_self_不誤選獨占候選() {
        let src = overloaded_zero_source(false, false);
        let plan = checked(&src);
        assert_eq!(plan.loan.kind, BorrowKind::SharedRead);
        assert_eq!(plan.return_calculation, PrivateSelfIntReturn::Add(3));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(10));
        assert!(outcome.method_entered && outcome.method_exited && outcome.ticket_returned);
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn g18_f_同名多載宣告顛倒仍依真呼叫靜態簽章執行() {
        for reverse in [false, true] {
            let plan = checked(&overloaded_zero_source(reverse, true));
            assert_eq!(plan.execute().unwrap().result, Ok(10));
        }
    }

    #[test]
    fn g18_f_零參數多載的獨占_self_真的寫入而非調用具參數共享候選() {
        let src = "struct Counter{\nvar count:int = 7\nfn change(n:int)->int{\nreturn self.count\n}\nfn change()->int{\nself.count = 9\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.change()\n}\n";
        let plan = checked(src);
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        assert_eq!(plan.write_value, Some(9));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(9));
        assert!(outcome.ticket_returned && outcome.events.is_empty());
    }

    #[test]
    fn g18_f_零參數多載方法中途故障仍先歸還原_self_票據() {
        let plan = checked(&overloaded_zero_source(true, true));
        let outcome = plan.execute_with_fault(true).unwrap();
        assert_eq!(outcome.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(outcome.method_entered && !outcome.method_exited && outcome.ticket_returned);
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn g18_f_竄改已選多載的方法來源跨度與借用種類須在配置前拒絕() {
        let src = overloaded_zero_source(true, true);
        let mut plan = checked(&src);
        plan.method_span = crate::common_ir::NodeSpan::new(0, 2);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.loan.kind = BorrowKind::ExclusiveWrite;
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_同一_int_參數左右加法仍取同一真框架值() {
        let src = "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.count\n}\nfn get(n:int)->int{\nreturn n+n\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.get(3)\n}\n";
        let plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(6));
    }

    fn overloaded_int_source(reverse: bool, owner_mutable: bool) -> String {
        let zero = "fn get()->int{\nself.count = 99\nreturn self.count\n}\n";
        let one = "fn get(n:int)->int{\nreturn self.count+2\n}\n";
        let methods = if reverse { format!("{one}{zero}") } else { format!("{zero}{one}") };
        let keyword = if owner_mutable { "var" } else { "const" };
        format!("struct Counter{{\nvar count:int = 7\n{methods}}}\nfn main()->int{{\n{keyword} value:Counter = Counter()\nreturn value.get(4)\n}}\n")
    }

    #[test]
    fn g18_f_同名方法單_int_實參依原_checker_型別選真共享多載() {
        let plan = checked(&overloaded_int_source(false, false));
        assert_eq!(plan.loan.kind, BorrowKind::SharedRead);
        assert_eq!(plan.argument_binding.map(|pair| pair.1), Some(4));
        let result = plan.execute().unwrap();
        assert_eq!(result.result, Ok(9));
        assert!(result.method_entered && result.method_exited && result.ticket_returned);
        assert!(result.events.is_empty());
    }

    #[test]
    fn g18_f_具_int_實參的多載宣告順序不影響真方法選擇() {
        for reverse in [false, true] {
            let plan = checked(&overloaded_int_source(reverse, true));
            assert_eq!(plan.execute().unwrap().result, Ok(9));
        }
    }

    #[test]
    fn g18_f_具_int_實參選中獨占方法_必真更新欄位並歸還票據() {
        let src = "struct Counter{\nvar count:int = 7\nfn set()->int{\nreturn self.count\n}\nfn set(n:int)->int{\nself.count = 11\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.set(3)\n}\n";
        let plan = checked(src);
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        assert_eq!(plan.argument_binding.map(|pair| pair.1), Some(3));
        let result = plan.execute().unwrap();
        assert_eq!(result.result, Ok(11));
        assert!(result.ticket_returned && result.events.is_empty());
    }

    #[test]
    fn g18_f_具_int_實參多載失敗後參數框架與_self_票據均不得存活() {
        let plan = checked(&overloaded_int_source(true, true));
        let failed = plan.execute_with_fault(true).unwrap();
        assert_eq!(failed.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(!failed.method_exited && failed.ticket_returned);
        assert!(failed.events.is_empty());
        assert_eq!(plan.execute().unwrap().result, Ok(9));
    }

    #[test]
    fn g18_f_竄改已驗證_int_方法實參值或宣告身分須在動態執行前拒絕() {
        let src = overloaded_int_source(true, true);
        let mut plan = checked(&src);
        plan.argument_binding = plan.argument_binding.map(|(id, _)| (id, 100));
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.argument_binding = Some((DeclarationId(0), 4));
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.verified.plan.members.clear();
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_已核准_int_參數與_self_欄位可真混算但非_int_實參仍拒絕() {
        let other = "struct Counter{\nvar count:int = 7\nfn get(n:int)->int{\nreturn n+self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.get(3)\n}\n";
        let outcome = checked(other).execute().unwrap();
        assert_eq!(outcome.result, Ok(10));
        assert!(outcome.ticket_returned);
        let bool_arg = "struct Counter{\nvar count:int = 7\nfn get(flag:bool)->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.get(true)\n}\n";
        // 先前這個歷史案例預期尚未接通的 bool 方法為 Unsupported。
        // M2 已核准 bool 形參值槽後，合法 true 原來源必真執行，
        // 但不得轉為 int 或產生假的 UTF-8／借用票據。
        let bool_out = checked(bool_arg).execute().unwrap();
        assert_eq!(bool_out.result, Ok(7));
        assert!(bool_out.ticket_returned);
        assert!(bool_out.has_balanced_self_borrow_trace());
        assert!(bool_out.events.is_empty());
    }

    fn method_param_source(body: &str, argument: &str) -> String {
        format!(
            "struct Counter{{\nvar count:int = 7\nfn use()->int{{\nreturn self.count\n}}\nfn use(n:int)->int{{\n{body}\n}}\n}}\nfn main()->int{{\nconst value:Counter = Counter()\nreturn value.use({argument})\n}}\n"
        )
    }

    #[test]
    fn g18_f_方法真參數返回使用所選_DeclarationId_的框架值() {
        let src = method_param_source("return n", "13");
        let plan = checked(&src);
        let expected = plan.argument_binding.expect("真正單一 int 參數");
        assert_eq!(plan.return_calculation, PrivateSelfIntReturn::Parameter(expected.0));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(13));
        assert!(outcome.method_entered && outcome.method_exited && outcome.ticket_returned);
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn g18_f_方法參數真加法在框架內求值不誤讀_self_欄位() {
        let plan = checked(&method_param_source("return n+3", "13"));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(16));
        assert!(outcome.ticket_returned && outcome.events.is_empty());
    }

    #[test]
    fn g18_f_方法參數真減法與重複呼叫保留同一來源參數值() {
        let plan = checked(&method_param_source("return n-4", "13"));
        for _ in 0..3 {
            let outcome = plan.execute().unwrap();
            assert_eq!(outcome.result, Ok(9));
            assert!(outcome.ticket_returned);
        }
    }

    #[test]
    fn g18_f_方法參數真加法溢位受控錯誤且先歸還_self_票據() {
        let plan = checked(&method_param_source("return n+1", "9223372036854775807"));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(outcome.method_entered && !outcome.method_exited && outcome.ticket_returned);
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn g18_f_方法參數真讀取之前故障不得報告成功返回() {
        let plan = checked(&method_param_source("return n", "13"));
        let failed = plan.execute_with_fault(true).unwrap();
        assert_eq!(failed.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(failed.method_entered && !failed.method_exited && failed.ticket_returned);
        assert_eq!(plan.execute().unwrap().result, Ok(13));
    }

    #[test]
    fn g18_f_篡改已驗證的方法參數返回身分或算術方案必在配置前拒絕() {
        let src = method_param_source("return n+3", "13");
        let mut plan = checked(&src);
        plan.return_calculation = PrivateSelfIntReturn::Parameter(DeclarationId(0));
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        let id = plan.argument_binding.unwrap().0;
        plan.return_calculation = PrivateSelfIntReturn::ParameterSubtract(id, 3);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_方法未定義的參數來源及錯誤型別須先由原_checker_拒絕() {
        let unknown = method_param_source("return missing", "13");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&unknown),
            Err(PrivateSelfMethodPrepareError::Check(_))));
        let invalid = method_param_source("return n+true", "13");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&invalid),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }


    #[test]
    fn g18_r05_同步共享與獨占_self_正常及故障均歸還真票據且零假資源() {
        for source in [
            normal(),
            "struct Counter{\nvar count:int = 1\nfn set()->int{\nself.count = 9\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.set()\n}\n",
        ] {
            let plan = checked(source);
            for fail in [false, true] {
                let out = plan.execute_with_fault(fail).unwrap();
                assert!(out.has_balanced_self_borrow_trace());
                assert!(out.ticket_returned);
                assert_eq!(out.method_exited, !fail);
                assert!(out.events.is_empty());
            }
        }
    }

    #[test]
    fn g18_r05_同步_self_偽造未歸票據或無來源資源事件必被守門拒絕() {
        let plan = checked(normal());
        let mut out = plan.execute().unwrap();
        out.ticket_returned = false;
        assert!(!out.has_balanced_self_borrow_trace());
        let mut out = plan.execute().unwrap();
        out.method_entered = false;
        assert!(!out.has_balanced_self_borrow_trace());
        let mut out = plan.execute().unwrap();
        out.events.push(crate::owned_str_runtime::ResourceEvent::Created(1));
        out.events.push(crate::owned_str_runtime::ResourceEvent::Released(1));
        assert!(!out.has_balanced_self_borrow_trace());
    }

    fn multi_int_method_source(body: &str, a: &str, b: &str) -> String {
        format!(
            "struct Counter{{\nvar count:int = 7\nfn calculate()->int{{\nreturn self.count\n}}\nfn calculate(n:int)->int{{\nreturn n\n}}\nfn calculate(a:int,b:int)->int{{\n{body}\n}}\n}}\nfn main()->int{{\nconst value:Counter = Counter()\nreturn value.calculate({a},{b})\n}}\n"
        )
    }

    #[test]
    fn g18_f_雙_int_方法真依原多載簽章選取並計算兩個獨立參數() {
        let plan = checked(&multi_int_method_source("return a+b", "11", "4"));
        assert_eq!(plan.loan.kind, BorrowKind::SharedRead);
        assert_eq!(plan.argument_binding.map(|pair| pair.1), Some(11));
        assert_eq!(plan.additional_argument_bindings.len(), 1);
        assert_eq!(plan.additional_argument_bindings[0].1, 4);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(15));
        assert!(outcome.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_f_雙_int_方法可只返回第二參數而不錯讀第一或_self_() {
        let plan = checked(&multi_int_method_source("return b", "11", "4"));
        let second = plan.additional_argument_bindings[0].0;
        assert_eq!(plan.return_calculation, PrivateSelfIntReturn::Parameter(second));
        assert_eq!(plan.execute().unwrap().result, Ok(4));
    }

    #[test]
    fn g18_f_雙_int_方法依實參順序正確執行減法() {
        let plan = checked(&multi_int_method_source("return a-b", "4", "11"));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(-7));
        assert!(outcome.ticket_returned && outcome.events.is_empty());
    }

    #[test]
    fn g18_f_雙_int_方法加法溢位仍歸還唯一真_self_票據() {
        let plan = checked(&multi_int_method_source(
            "return a+b", "9223372036854775807", "1",
        ));
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(outcome.method_entered && !outcome.method_exited && outcome.ticket_returned);
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn g18_f_多_int_實參方法故障不殘留形參且重新呼叫不受污染() {
        let plan = checked(&multi_int_method_source("return b", "11", "4"));
        let failed = plan.execute_with_fault(true).unwrap();
        assert_eq!(failed.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(failed.method_entered && !failed.method_exited && failed.ticket_returned);
        assert_eq!(plan.execute().unwrap().result, Ok(4));
    }

    #[test]
    fn g18_f_竄改第二方法實參值或宣告身分均於真資源前拒絕() {
        let src = multi_int_method_source("return a+b", "11", "4");
        let mut wrong = checked(&src);
        wrong.additional_argument_bindings[0].1 = 100;
        assert!(matches!(wrong.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut wrong = checked(&src);
        wrong.additional_argument_bindings[0].0 = wrong.argument_binding.unwrap().0;
        assert!(matches!(wrong.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_三個_int_方法參數按原宣告位置獨立綁定返回第三個() {
        let src = "struct Counter{\nvar count:int = 7\nfn last(a:int,b:int,c:int)->int{\nreturn c\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.last(4,8,12)\n}\n";
        let plan = checked(src);
        assert_eq!(plan.argument_binding.map(|entry| entry.1), Some(4));
        assert_eq!(plan.additional_argument_bindings.len(), 2);
        assert_eq!(plan.additional_argument_bindings.iter().map(|entry| entry.1).collect::<Vec<_>>(), [8, 12]);
        assert_eq!(plan.execute().unwrap().result, Ok(12));
    }

    #[test]
    fn g18_f_多_int_方法來源錯型別與不存在參數須原_checker_或安全準備拒絕() {
        let wrong_name = multi_int_method_source("return missing", "11", "4");
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&wrong_name),
            Err(PrivateSelfMethodPrepareError::Check(_))));
        let mixed = "struct Counter{\nvar count:int = 7\nfn calculate(a:int,b:bool)->int{\nreturn a\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.calculate(11,true)\n}\n";
        assert!(matches!(PreparedPrivateSelfMethod::prepare(mixed),
            Err(PrivateSelfMethodPrepareError::Unsupported(_))));
    }


    fn mixed_self_param_source(body: &str, argument: &str, mutable: bool) -> String {
        let owner = if mutable { "var" } else { "const" };
        format!(
            "struct Counter{{\nvar count:int = 7\nfn calc(n:int)->int{{\n{body}\n}}\n}}\nfn main()->int{{\n{owner} value:Counter = Counter()\nreturn value.calc({argument})\n}}\n"
        )
    }

    #[test]
    fn g18_f_共享_self_欄位加參數讀取真_runtime_與宣告身分() {
        let src = mixed_self_param_source("return self.count+n", "4", false);
        let plan = checked(&src);
        assert_eq!(plan.loan.kind, BorrowKind::SharedRead);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(11));
        assert!(out.method_entered && out.method_exited && out.ticket_returned);
    }

    #[test]
    fn g18_f_共享_self_形式參數減欄位維持左減右次序() {
        let plan = checked(&mixed_self_param_source("return n-self.count", "4", false));
        assert_eq!(plan.execute().unwrap().result, Ok(-3));
        let reverse = checked(&mixed_self_param_source("return self.count-n", "4", false));
        assert_eq!(reverse.execute().unwrap().result, Ok(3));
    }

    #[test]
    fn g18_f_獨占_self_真寫入後再同方法與_int_形式參數混算() {
        let src = mixed_self_param_source("self.count = 11\nreturn self.count+n", "4", true);
        let plan = checked(&src);
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(15));
        assert!(out.ticket_returned && out.events.is_empty());
    }

    #[test]
    fn g18_f_共享_self_欄位加巨大_int_實參溢位仍歸還真借用() {
        let plan = checked(&mixed_self_param_source(
            "return self.count+n", "9223372036854775807", false,
        ));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.method_entered && !out.method_exited && out.ticket_returned);
        assert!(out.events.is_empty());
    }

    #[test]
    fn g18_f_雙_int_方法只讀第二參數與_self_欄位不混用第一個() {
        let src = "struct Counter{\nvar count:int = 7\nfn calc(a:int,b:int)->int{\nreturn self.count+b\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nreturn value.calc(100,4)\n}\n";
        let plan = checked(src);
        let id = plan.additional_argument_bindings[0].0;
        assert_eq!(plan.return_calculation, PrivateSelfIntReturn::FieldAddParameter(id));
        assert_eq!(plan.execute().unwrap().result, Ok(11));
    }

    #[test]
    fn g18_f_篡改_self_混算方向與參數身分須在動態配置前拒絕() {
        let src = mixed_self_param_source("return self.count+n", "4", false);
        let mut plan = checked(&src);
        let id = plan.argument_binding.unwrap().0;
        plan.return_calculation = PrivateSelfIntReturn::FieldSubtractParameter(id);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.return_calculation = PrivateSelfIntReturn::FieldAddParameter(DeclarationId(0));
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_偽造_other_欄位與錯誤接收者不得冒充真正_self_來源() {
        let src = mixed_self_param_source("return other.count+n", "4", false);
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }


    fn main_named_method_source(locals: &str, actuals: &str, body: &str) -> String {
        format!(
            "struct Counter{{\nvar count:int = 7\nfn calc(a:int,b:int)->int{{\n{body}\n}}\n}}\nfn main()->int{{\nconst value:Counter = Counter()\n{locals}\nreturn value.calc({actuals})\n}}\n"
        )
    }

    #[test]
    fn g18_r02_r04_同步方法讀取main具名_const_int_真局部框架() {
        let src = main_named_method_source(
            "const amount:int = 5", "amount,3", "return a+b",
        );
        let plan = checked(&src);
        assert_eq!(plan.main_int_locals.len(), 1);
        assert_eq!(plan.argument_sources[0], Some(plan.main_int_locals[0].1));
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.ticket_returned && out.method_exited);
        assert_eq!(out.main_local_slots_after_exit, 0);
    }

    #[test]
    fn g18_r02_r04_雙具名_copyable_int_按main宣告及原方法參數次序做減法() {
        let src = main_named_method_source(
            "var left:int = 4\nconst right:int = 11", "right,left", "return a-b",
        );
        let plan = checked(&src);
        assert_eq!(plan.main_int_locals.len(), 2);
        assert_eq!(plan.argument_sources,
            [Some(plan.main_int_locals[1].1), Some(plan.main_int_locals[0].1)]);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.main_local_slots_after_exit, 0);
    }

    #[test]
    fn g18_r02_r04_main整數局部由前一已初始化名稱複製再傳入self方法() {
        let src = main_named_method_source(
            "const first:int = 12\nconst second:int = first",
            "second,4", "return a-b",
        );
        let plan = checked(&src);
        assert_eq!(plan.main_int_locals[1].3, Some(plan.main_int_locals[0].1));
        assert_eq!(plan.execute().unwrap().result, Ok(8));
    }

    #[test]
    fn g18_r02_r04_同一具名_copyable_int_可作兩個方法實參不消耗來源() {
        let src = main_named_method_source(
            "const both:int = 6", "both,both", "return a+b",
        );
        let plan = checked(&src);
        assert_eq!(plan.argument_sources, [Some(plan.main_int_locals[0].1); 2]);
        for _ in 0..3 {
            let out = plan.execute().unwrap();
            assert_eq!(out.result, Ok(12));
            assert_eq!(out.main_local_slots_after_exit, 0);
        }
    }

    #[test]
    fn g18_r02_r05_具名main實參與_self_欄位加法溢位仍清除局部及真票據() {
        let src = "struct Counter{\nvar count:int = 7\nfn calc(n:int)->int{\nreturn self.count+n\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nconst amount:int = 9223372036854775807\nreturn value.calc(amount)\n}\n";
        let plan = checked(src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.method_entered && !out.method_exited && out.ticket_returned);
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert!(out.events.is_empty());
    }

    #[test]
    fn g18_r02_r05_同步方法具名實參故障注入仍先歸還借用後清空main局部() {
        let src = main_named_method_source(
            "const amount:int = 11", "amount,4", "return a+b",
        );
        let plan = checked(&src);
        let out = plan.execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(out.method_entered && !out.method_exited && out.ticket_returned);
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert_eq!(plan.execute().unwrap().result, Ok(15));
    }

    #[test]
    fn g18_r02_r04_篡改具名來源局部值身分或方法實參來源皆須零執行拒絕() {
        let src = main_named_method_source(
            "const amount:int = 11", "amount,4", "return a+b",
        );
        let mut plan = checked(&src);
        plan.main_int_locals[0].2 = 99;
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.argument_sources[0] = Some(DeclarationId(0));
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.argument_binding = plan.argument_binding.map(|(id, _)| (id, 100));
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r02_r04_尚未宣告或未初始化main實參不得進入同步self方法() {
        let future = main_named_method_source(
            "var later:int", "later,4", "return a+b",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&future),
            Err(PrivateSelfMethodPrepareError::Check(_))));
        let wrong_type = main_named_method_source(
            "const amount:bool = true", "amount,4", "return a+b",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&wrong_type),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }


    #[test]
    fn g18_r02_r04_同步方法實參共用原_typedhir_具名加法括號與返回() {
        let src = main_named_method_source(
            "const amount:int = 5", "(amount+3),4", "return a+b",
        );
        let plan = checked(&src);
        assert_eq!(plan.argument_binding.map(|(_, value)| value), Some(8));
        assert_eq!(plan.argument_sources[0], None);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(12));
        assert_eq!(out.main_local_slots_after_exit, 0);
        assert!(out.ticket_returned && out.method_exited);
    }

    #[test]
    fn g18_r02_r04_巢狀括號與左右加減依原_checker_樹求值() {
        let src = main_named_method_source(
            "const amount:int = 11", "((amount-(7-3))),((2+4))",
            "return a-b",
        );
        let plan = checked(&src);
        assert_eq!(plan.execute().unwrap().result, Ok(1));
    }

    #[test]
    fn g18_r02_r04_已核准複合整數實參與_self_欄位回傳使用同一真框架() {
        let src = "struct Counter{\nvar count:int = 7\nfn calc(n:int)->int{\nreturn self.count+n\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nconst amount:int = 3\nreturn value.calc(amount+4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(14));
        assert!(out.ticket_returned && out.events.is_empty());
    }

    #[test]
    fn g18_r02_r05_第二個整數複合實參溢位不可提前取得_self_借用或進入方法() {
        let src = main_named_method_source(
            "const amount:int = 1",
            "3,(amount+9223372036854775807)",
            "return a+b",
        );
        let plan = checked(&src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_entered && !out.method_exited);
        assert!(out.ticket_returned && out.events.is_empty());
        assert_eq!(out.main_local_slots_after_exit, 0);
    }

    #[test]
    fn g18_r02_r05_第一個複合實參溢位不得執行後續方法本體() {
        let src = main_named_method_source(
            "const amount:int = 1",
            "(amount+9223372036854775807),4",
            "return a+b",
        );
        let out = checked(&src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_entered && !out.method_exited && out.ticket_returned);
        assert_eq!(out.main_local_slots_after_exit, 0);
    }

    #[test]
    fn g18_r02_r04_偽造已編譯方法實參算術樹或來源位置於執行前拒絕() {
        let src = main_named_method_source(
            "const amount:int = 5", "(amount+3),4", "return a+b",
        );
        let mut plan = checked(&src);
        plan.argument_plans[0] = PrivateScalarReturnExpr::Constant(8);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.argument_sources[0] = Some(plan.main_int_locals[0].1);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r02_r04_非法布林混用與未宣告名稱在方法複合引數前先被_checker_拒絕() {
        let wrong_type = main_named_method_source(
            "const amount:int = 5", "(amount+true),4", "return a+b",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&wrong_type),
            Err(PrivateSelfMethodPrepareError::Check(_))));
        let unknown = main_named_method_source(
            "const amount:int = 5", "(missing+3),4", "return a+b",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&unknown),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }


    #[test]
    fn g18_r02_r04_main整數複合初始化使用同一_typedhir_再傳同步方法() {
        let src = main_named_method_source(
            "const base:int = 5\nconst delta:int = (base+4)",
            "delta,2", "return a+b",
        );
        let plan = checked(&src);
        assert_eq!(plan.main_int_locals.len(), 2);
        assert_eq!(plan.main_init_plans.len(), 2);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(11));
        assert!(out.ticket_returned && out.method_exited);
        assert_eq!(out.main_local_slots_after_exit, 0);
    }

    #[test]
    fn g18_r02_r04_巢狀main局部初始化依原整數減法左右次序傳入方法() {
        let src = main_named_method_source(
            "const first:int = 12\nconst second:int = ((first-4)+2)",
            "second,3", "return a-b",
        );
        let plan = checked(&src);
        assert_eq!(plan.main_int_locals[1].2, 10);
        assert_eq!(plan.execute().unwrap().result, Ok(7));
    }

    #[test]
    fn g18_r02_r04_已計算main局部可再被Copyable名稱引用並與_self_欄位混算() {
        let src = "struct Counter{\nvar count:int = 7\nfn calc(n:int)->int{\nreturn self.count+n\n}\n}\nfn main()->int{\nconst value:Counter = Counter()\nconst first:int = 3\nconst computed:int = first+4\nconst forwarded:int = computed\nreturn value.calc(forwarded)\n}\n";
        let plan = checked(src);
        assert_eq!(plan.main_int_locals.len(), 3);
        assert_eq!(plan.main_int_locals[2].3, Some(plan.main_int_locals[1].1));
        assert_eq!(plan.execute().unwrap().result, Ok(14));
    }

    #[test]
    fn g18_r02_r05_main局部動態加法溢位在方法借用前受控失敗() {
        let src = main_named_method_source(
            "const first:int = 1\nconst overflow:int = first+9223372036854775807",
            "overflow,2", "return a+b",
        );
        let plan = checked(&src);
        for _ in 0..2 {
            let out = plan.execute().unwrap();
            assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
            assert!(!out.method_entered && !out.method_exited);
            assert!(out.ticket_returned && out.events.is_empty());
            assert_eq!(out.main_local_slots_after_exit, 0);
        }
    }

    #[test]
    fn g18_r02_r05_中途初始化溢位不可清理未進入的後續局部也不能假造返回() {
        let src = main_named_method_source(
            "const first:int = 1\nconst overflow:int = first+9223372036854775807\nconst later:int = 5",
            "later,2", "return a+b",
        );
        let plan = checked(&src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_entered && !out.method_exited);
        assert!(out.ticket_returned);
        assert_eq!(out.main_local_slots_after_exit, 0);
    }

    #[test]
    fn g18_r02_r04_篡改已編譯main整數初始化工作樹必在真執行前拒絕() {
        let src = main_named_method_source(
            "const first:int = 3\nconst second:int = first+4",
            "second,2", "return a+b",
        );
        let mut plan = checked(&src);
        plan.main_init_plans[1] = PrivateScalarReturnExpr::Constant(7);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.main_int_locals[1].2 = 99;
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r02_r04_main複合初始化未知或錯型別來源由原_checker_先拒絕() {
        let unknown = main_named_method_source(
            "const amount:int = missing+3", "amount,2", "return a+b",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&unknown),
            Err(PrivateSelfMethodPrepareError::Check(_))));
        let bad = main_named_method_source(
            "const amount:int = true+3", "amount,2", "return a+b",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&bad),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }


    #[test]
    fn g18_r05_同步self真共享借用及成功返回逐筆由共用進退帳驗證() {
        let out = checked(normal()).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        let ticket = match out.exit_events.first() {
            Some(PrivateSharedCallExitEvent::LoanAcquired(ticket)) => *ticket,
            _ => panic!("必須是實際簽發的 self BorrowTicket"),
        };
        assert_eq!(ticket.fact(), out.checked_loan);
        assert_eq!(out.exit_events, [
            PrivateSharedCallExitEvent::LoanAcquired(ticket),
            PrivateSharedCallExitEvent::FunctionEntered,
            PrivateSharedCallExitEvent::Returned,
            PrivateSharedCallExitEvent::LoanReturned(ticket),
            PrivateSharedCallExitEvent::FunctionExited { returned: true },
        ]);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_同步self故障須真回滾後歸還同張票據再退出() {
        let out = checked(normal()).execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        let ticket = match out.exit_events.first() {
            Some(PrivateSharedCallExitEvent::LoanAcquired(ticket)) => *ticket,
            _ => panic!("應有真自動 self 借用"),
        };
        assert_eq!(out.exit_events, [
            PrivateSharedCallExitEvent::LoanAcquired(ticket),
            PrivateSharedCallExitEvent::FunctionEntered,
            PrivateSharedCallExitEvent::UnwindStarted,
            PrivateSharedCallExitEvent::LoanReturned(ticket),
            PrivateSharedCallExitEvent::FunctionExited { returned: false },
        ]);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_方法引數尚未求值成功不得假造self借用或進入退出() {
        let src = main_named_method_source(
            "const amount:int = 1", "3,amount+9223372036854775807",
            "return a+b",
        );
        let out = checked(&src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.exit_events.is_empty());
        assert!(!out.method_entered && !out.method_exited && out.ticket_returned);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_偽造同步self歸還票據缺失或重複均由共用帳簿拒絕() {
        let mut out = checked(normal()).execute().unwrap();
        out.exit_events.retain(|event| !matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        ));
        assert!(!out.has_balanced_self_borrow_trace());
        let mut out = checked(normal()).execute().unwrap();
        let returned = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        let duplicate = out.exit_events[returned].clone();
        out.exit_events.insert(returned, duplicate);
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_同步self刪真返回改退出及竄改票據來源均不可交付() {
        let mut out = checked(normal()).execute().unwrap();
        out.exit_events.retain(|event| !matches!(
            event, PrivateSharedCallExitEvent::Returned
        ));
        assert!(!out.has_balanced_self_borrow_trace());
        let mut out = checked(normal()).execute().unwrap();
        out.checked_loan = BorrowFact::new(DeclarationId(0), BorrowKind::SharedRead);
        assert!(!out.has_balanced_self_borrow_trace());
    }


    #[test]
    fn g18_r05_共用帳簿拒絕同步self在真返回之前提前還票() {
        let mut out = checked(normal()).execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        let at = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        let forged = out.exit_events.remove(at);
        let returned = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::Returned
        )).unwrap();
        out.exit_events.insert(returned, forged);
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_共用帳簿拒絕同步self成功退出後才補還真票據() {
        let mut out = checked(normal()).execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        let at = out.exit_events.iter().position(|event| matches!(
            event, PrivateSharedCallExitEvent::LoanReturned(_)
        )).unwrap();
        let forged = out.exit_events.remove(at);
        out.exit_events.push(forged);
        assert!(!out.has_balanced_self_borrow_trace());
        let failed = checked(normal()).execute_with_fault(true).unwrap();
        assert!(failed.has_balanced_self_borrow_trace());
    }


    fn checked_method_local_source(body: &str, arguments: &str) -> String {
        format!(
            "struct Counter{{\nvar count:int = 7\nfn calc(a:int,b:int)->int{{\n{body}\n}}\n}}\nfn main()->int{{\nconst value:Counter = Counter()\nreturn value.calc({arguments})\n}}\n",
        )
    }

    #[test]
    fn g18_r04_同步self方法int局部以真參數初始化後正確返回() {
        let src = checked_method_local_source(
            "const intermediate:int = a+4\nreturn intermediate", "5,2",
        );
        let plan = checked(&src);
        assert_eq!(plan.method_int_locals.len(), 1);
        let result = plan.execute().unwrap();
        assert_eq!(result.result, Ok(9));
        assert!(result.has_balanced_self_borrow_trace());
        assert_eq!(result.method_local_slots_after_exit, 0);
        assert_eq!(result.method_local_events.len(), 2);
        assert_eq!(result.exit_events.iter().filter(|e|
            matches!(e, PrivateSharedCallExitEvent::Returned)
        ).count(), 1);
    }

    #[test]
    fn g18_r04_同步self連續兩個int局部依原checker宣告先後傳值及逆序離域() {
        let src = checked_method_local_source(
            "const first:int = a+b\nconst second:int = (first-2)\nreturn second+first",
            "11,4",
        );
        let plan = checked(&src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(28));
        let first = plan.method_int_locals[0].0;
        let second = plan.method_int_locals[1].0;
        assert_eq!(out.method_local_events, [
            PrivateSelfMethodLocalEvent::Initialized(first, 15),
            PrivateSelfMethodLocalEvent::Initialized(second, 13),
            PrivateSelfMethodLocalEvent::ScopeEnded(second),
            PrivateSelfMethodLocalEvent::ScopeEnded(first),
        ]);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r04_方法局部Copyable同一來源多次讀取不當成Move或重借() {
        let src = checked_method_local_source(
            "const initial:int = a+1\nconst copied:int = initial\nreturn initial+copied",
            "3,2",
        );
        let out = checked(&src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.ticket_returned && out.events.is_empty());
        assert_eq!(out.method_local_slots_after_exit, 0);
    }

    #[test]
    fn g18_r05_方法第二int局部初始化溢位只離域已初始化第一槽() {
        let src = checked_method_local_source(
            "const first:int = 11\nconst overflow:int = first+9223372036854775807\nreturn overflow",
            "3,4",
        );
        let plan = checked(&src);
        let first = plan.method_int_locals[0].0;
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.method_entered && !out.method_exited && out.ticket_returned);
        assert_eq!(out.method_local_slots_after_exit, 0);
        assert_eq!(out.method_local_events, [
            PrivateSelfMethodLocalEvent::Initialized(first, 11),
            PrivateSelfMethodLocalEvent::ScopeEnded(first),
        ]);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_方法返回算式溢位先逆序清真int局部再歸還self票據() {
        let src = checked_method_local_source(
            "const value:int = 9223372036854775807\nreturn value+1",
            "3,4",
        );
        let plan = checked(&src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_exited && out.ticket_returned);
        assert_eq!(out.method_local_events.len(), 2);
        assert!(out.exit_events.iter().any(|event| matches!(
            event, PrivateSharedCallExitEvent::UnwindStarted
        )));
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_方法存在int局部且方法故障注入前不假造初始化() {
        let src = checked_method_local_source(
            "const value:int = a+4\nreturn value", "5,2",
        );
        let plan = checked(&src);
        let out = plan.execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(out.method_local_events.is_empty());
        assert!(out.ticket_returned && out.has_balanced_self_borrow_trace());
        assert_eq!(plan.execute().unwrap().result, Ok(9));
    }

    #[test]
    fn g18_r04_篡改方法int局部編譯TypedHIR樹及真宣告身分一律執行前拒絕() {
        let src = checked_method_local_source(
            "const value:int = a+4\nreturn value", "5,2",
        );
        let mut plan = checked(&src);
        plan.method_int_locals[0].1 = PrivateScalarReturnExpr::Constant(9);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.method_int_locals[0].0 = DeclarationId(0);
        assert!(matches!(plan.execute(), Err(PrivateSelfMethodPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r04_方法局部未知名稱或bool混入初始化交由原Checker拒絕() {
        let unknown = checked_method_local_source(
            "const value:int = missing+4\nreturn value", "5,2",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&unknown),
            Err(PrivateSelfMethodPrepareError::Check(_))));
        let wrong_type = checked_method_local_source(
            "const value:int = true\nreturn value", "5,2",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&wrong_type),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn g18_r05_偽造方法局部逆序離域或重複初始化皆拒絕() {
        let src = checked_method_local_source(
            "const first:int = a\nconst second:int = b\nreturn first+second", "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        out.method_local_events.swap(2, 3);
        assert!(!out.has_balanced_self_borrow_trace());
        let mut out = checked(&src).execute().unwrap();
        out.method_local_events.insert(1, out.method_local_events[0].clone());
        assert!(!out.has_balanced_self_borrow_trace());
    }


    #[test]
    fn g18_r05_真方法局部初始化_return_離域與還票須同一共用時間序() {
        let src = checked_method_local_source(
            "const first:int = a+2\nreturn first", "3,4",
        );
        let out = checked(&src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_self_borrow_trace());
        let id = match &out.method_local_events[0] {
            PrivateSelfMethodLocalEvent::Initialized(id, 5) => *id,
            _ => panic!("方法局部須有真來源值"),
        };
        assert!(matches!(out.exit_events.as_slice(), [
            PrivateSharedCallExitEvent::LoanAcquired(_),
            PrivateSharedCallExitEvent::FunctionEntered,
            PrivateSharedCallExitEvent::LocalInitialized(actual),
            PrivateSharedCallExitEvent::Returned,
            PrivateSharedCallExitEvent::LocalScopeEnded(exited),
            PrivateSharedCallExitEvent::LoanReturned(_),
            PrivateSharedCallExitEvent::FunctionExited { returned: true },
        ] if *actual == id && *exited == id));
    }

    #[test]
    fn g18_r05_偽造同步方法在真正_return_之後才初始化局部必拒絕() {
        let src = checked_method_local_source(
            "const first:int = a+2\nreturn first", "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        let first = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LocalInitialized(_)
        )).unwrap();
        let forged = out.exit_events.remove(first);
        let returned = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )).unwrap();
        out.exit_events.insert(returned + 1, forged);
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_偽造同步方法退出之後才清理局部雖總數相等仍拒絕() {
        let src = checked_method_local_source(
            "const first:int = a+2\nreturn first", "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        let at = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LocalScopeEnded(_)
        )).unwrap();
        let forged = out.exit_events.remove(at);
        out.exit_events.push(forged);
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_方法局部另一張事件帳漏一筆即使單帳平衡仍禁止交付() {
        let src = checked_method_local_source(
            "const first:int = a+2\nreturn first", "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        out.method_local_events.clear();
        assert!(!out.has_balanced_self_borrow_trace());
        let mut out = checked(&src).execute().unwrap();
        out.exit_events.retain(|e| !matches!(
            e, PrivateSharedCallExitEvent::LocalInitialized(_)
                | PrivateSharedCallExitEvent::LocalScopeEnded(_)
        ));
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_錯誤回滾起點前假清理方法局部不得冒充合法離域() {
        let src = checked_method_local_source(
            "const first:int = 11\nconst overflow:int = first+9223372036854775807\nreturn overflow",
            "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_self_borrow_trace());
        let at = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::LocalScopeEnded(_)
        )).unwrap();
        let cleanup = out.exit_events.remove(at);
        let unwind = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )).unwrap();
        out.exit_events.insert(unwind, cleanup);
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_方法回滾後新建int局部即使補做離域亦不可通過() {
        let src = checked_method_local_source(
            "const first:int = 11\nconst overflow:int = first+9223372036854775807\nreturn overflow",
            "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        let extra = DeclarationId(0);
        let at = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::UnwindStarted
        )).unwrap();
        out.exit_events.insert(at + 1,
            PrivateSharedCallExitEvent::LocalInitialized(extra));
        out.exit_events.insert(at + 2,
            PrivateSharedCallExitEvent::LocalScopeEnded(extra));
        assert!(!out.has_balanced_self_borrow_trace());
    }


    #[test]
    fn g18_r05_合法方法兩個真局部初始化必對應原_checker_宣告順序() {
        let src = checked_method_local_source(
            "const first:int = a+1\nconst second:int = first+b\nreturn second",
            "3,4",
        );
        let plan = checked(&src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert_eq!(out.expected_method_local_ids,
            plan.method_int_locals.iter().map(|(id, _)| *id).collect::<Vec<_>>());
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_偽造成功方法內額外平衡局部即使兩帳一致仍須拒絕() {
        let src = checked_method_local_source(
            "const local:int = a+1\nreturn local", "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        let fake = DeclarationId(0);
        let at = out.exit_events.iter().position(|e| matches!(
            e, PrivateSharedCallExitEvent::Returned
        )).unwrap();
        out.exit_events.insert(at,
            PrivateSharedCallExitEvent::LocalInitialized(fake));
        out.exit_events.insert(at + 2,
            PrivateSharedCallExitEvent::LocalScopeEnded(fake));
        out.method_local_events.insert(1,
            PrivateSelfMethodLocalEvent::Initialized(fake, 7));
        out.method_local_events.insert(2,
            PrivateSelfMethodLocalEvent::ScopeEnded(fake));
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_同步方法即使兩張事件帳同時篡改局部身份也不得冒充checker來源() {
        let src = checked_method_local_source(
            "const local:int = a+1\nreturn local", "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        let fake = DeclarationId(0);
        for event in &mut out.method_local_events {
            match event {
                PrivateSelfMethodLocalEvent::Initialized(id, _)
                    | PrivateSelfMethodLocalEvent::Assigned(id, _)
                    | PrivateSelfMethodLocalEvent::ScopeEnded(id) => *id = fake,
            }
        }
        for event in &mut out.exit_events {
            match event {
                PrivateSharedCallExitEvent::LocalInitialized(id)
                    | PrivateSharedCallExitEvent::LocalScopeEnded(id) => *id = fake,
                _ => {}
            }
        }
        assert!(!out.has_balanced_self_borrow_trace());
        let mut out = checked(&src).execute().unwrap();
        out.expected_method_local_ids.clear();
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_方法框架退出後再追加真值假局部不可由main基底冒充() {
        let src = checked_method_local_source(
            "const local:int = a+1\nreturn local", "3,4",
        );
        let mut out = checked(&src).execute().unwrap();
        let fake = DeclarationId(0);
        out.method_local_events.push(
            PrivateSelfMethodLocalEvent::Initialized(fake, 7));
        out.method_local_events.push(
            PrivateSelfMethodLocalEvent::ScopeEnded(fake));
        out.exit_events.push(
            PrivateSharedCallExitEvent::LocalInitialized(fake));
        out.exit_events.push(
            PrivateSharedCallExitEvent::LocalScopeEnded(fake));
        assert!(!out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r05_方法故障僅允許已進入局部對應原宣告前綴() {
        let src = checked_method_local_source(
            "const first:int = 11\nconst overflow:int = first+9223372036854775807\nreturn overflow",
            "3,4",
        );
        let plan = checked(&src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert_eq!(out.expected_method_local_ids.len(), 2);
        assert_eq!(out.method_local_events, [
            PrivateSelfMethodLocalEvent::Initialized(plan.method_int_locals[0].0, 11),
            PrivateSelfMethodLocalEvent::ScopeEnded(plan.method_int_locals[0].0),
        ]);
        assert!(out.has_balanced_self_borrow_trace());
    }


    #[test]
    fn g18_r04_同步self方法var_int_局部可按原checker賦值後真返回() {
        let src = checked_method_local_source(
            "var amount:int = a\namount = amount+b\nreturn amount", "3,4",
        );
        let plan = checked(&src);
        assert_eq!(plan.method_int_assignments.len(), 1);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_self_borrow_trace() && out.ticket_returned);
        assert_eq!(out.method_local_events, [
            PrivateSelfMethodLocalEvent::Initialized(plan.method_int_locals[0].0, 3),
            PrivateSelfMethodLocalEvent::Assigned(plan.method_int_locals[0].0, 7),
            PrivateSelfMethodLocalEvent::ScopeEnded(plan.method_int_locals[0].0),
        ]);
    }

    #[test]
    fn g18_r04_同步self方法連續兩次_var_int_更新沿用最新動態值() {
        let src = checked_method_local_source(
            "var amount:int = a\namount = amount+b\namount = (amount-2)\nreturn amount",
            "11,4",
        );
        let plan = checked(&src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(13));
        assert_eq!(out.expected_method_assignment_ids,
            [plan.method_int_locals[0].0, plan.method_int_locals[0].0]);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r04_同步self方法第二個var局部可讀取第一個已更新值() {
        let src = checked_method_local_source(
            "var first:int = a\nvar second:int = b\nfirst = first+3\nsecond = first+second\nreturn second",
            "4,5",
        );
        let out = checked(&src).execute().unwrap();
        assert_eq!(out.result, Ok(12));
        assert!(out.has_balanced_self_borrow_trace());
        assert_eq!(out.method_local_slots_after_exit, 0);
    }

    #[test]
    fn g18_r05_同步self第二次更新溢位須保留已提交第一次更新及逆序離域() {
        let src = checked_method_local_source(
            "var amount:int = 2\namount = amount+3\namount = amount+9223372036854775807\nreturn amount",
            "3,4",
        );
        let plan = checked(&src);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateSelfMethodRuntimeError::IntegerOverflow));
        assert!(!out.method_exited && out.ticket_returned);
        assert_eq!(out.method_local_events, [
            PrivateSelfMethodLocalEvent::Initialized(plan.method_int_locals[0].0, 2),
            PrivateSelfMethodLocalEvent::Assigned(plan.method_int_locals[0].0, 5),
            PrivateSelfMethodLocalEvent::ScopeEnded(plan.method_int_locals[0].0),
        ]);
        assert!(out.has_balanced_self_borrow_trace());
    }

    #[test]
    fn g18_r04_方法const_int_重新賦值或參數賦值仍原checker靜態拒絕() {
        let const_src = checked_method_local_source(
            "const amount:int = a\namount = b\nreturn amount", "3,4",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&const_src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
        let param_src = checked_method_local_source(
            "var amount:int = a\na = b\nreturn amount", "3,4",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&param_src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn g18_r04_方法var_int_更新後再宣告尚未證明重排必拒絕() {
        let src = checked_method_local_source(
            "var first:int = a\nfirst = first+b\nconst later:int = first\nreturn later",
            "3,4",
        );
        assert!(matches!(PreparedPrivateSelfMethod::prepare(&src),
            Err(PrivateSelfMethodPrepareError::Check(_))));
    }

    #[test]
    fn g18_r05_竄改方法var_int_賦值計畫或兩張帳中的更新身分須拒絕() {
        let src = checked_method_local_source(
            "var amount:int = a\namount = amount+b\nreturn amount", "3,4",
        );
        let mut plan = checked(&src);
        plan.method_int_assignments[0].1 = PrivateScalarReturnExpr::Constant(7);
        assert!(matches!(plan.execute(),
            Err(PrivateSelfMethodPrepareError::SourceChanged)));
        let mut out = checked(&src).execute().unwrap();
        assert!(out.has_balanced_self_borrow_trace());
        out.expected_method_assignment_ids = vec![DeclarationId(0)];
        assert!(!out.has_balanced_self_borrow_trace());
        let mut out = checked(&src).execute().unwrap();
        for event in &mut out.method_local_events {
            if let PrivateSelfMethodLocalEvent::Assigned(id, _) = event {
                *id = DeclarationId(0);
            }
        }
        assert!(!out.has_balanced_self_borrow_trace());
    }


}
