// G18／P18.3：原私有 Checker 的 bool 參數有限遞迴與真 OwnedStrRuntime。
// 每個呼叫／分支保有獨立 scope；僅接通可在靜態來源證明之有限呼叫形狀。
// 明確的工作堆疊不用 Rust 遞迴，預算是每次內部執行設定而非語言常數。
// 不擴張公開 CLI、未核准參數借用跨框架或一般遞迴語意。
#[derive(Debug)]
enum PrivateRecursiveStrPrepareError {
    Check(crate::Diagnostic),
    Unsupported(&'static str),
    SourceChanged,
    Cleanup(crate::owned_str_runtime::StrRuntimeError),
    CleanupTicket(crate::borrow_safety::BorrowSafetyError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateRecursiveStrRunError {
    CallBudget,
    StepBudget,
    InjectedFailure,
    MissingFunction,
    Resource(crate::owned_str_runtime::StrRuntimeError),
    Ticket(crate::borrow_safety::BorrowSafetyError),
    Scalar(PrivateScalarCallRuntimeError),
    StateMismatch,
}

#[derive(Debug, Clone, Copy)]
struct PrivateRecursiveStrBudget {
    max_call_frames: usize,
    max_steps: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateRecursiveStrLiteral {
    identity: DeclarationId,
    name: String,
    mutable: bool,
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateRecursiveIntArgument {
    Constant(i64),
    /// 從這次呼叫者**動態**參數槽讀值，不能用原 Checker 的靜態值冒充。
    ParentParameter(DeclarationId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateRecursiveStrCall {
    name: String,
    argument: bool,
    span: crate::common_ir::NodeSpan,
    /// 只有原 Checker call_borrows 真票據才可記錄本次 str 參數借用。
    loan: Option<(String, BorrowFact)>,
    int_arguments: Vec<PrivateRecursiveIntArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateRecursiveStrFunction {
    name: String,
    span: crate::common_ir::NodeSpan,
    parameter: DeclarationId,
    int_parameters: Vec<DeclarationId>,
    locals: Vec<PrivateRecursiveStrLiteral>,
    branch_locals: Vec<PrivateRecursiveStrLiteral>,
    next: PrivateRecursiveStrCall,
    base_return: PrivateScalarReturnExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateRecursiveFrameEvent {
    Entered(String),
    Exited { name: String, returned: bool },
}

#[derive(Debug)]
struct PrivateRecursiveStrOutcome {
    result: Result<i64, PrivateRecursiveStrRunError>,
    events: Vec<crate::owned_str_runtime::ResourceEvent>,
    frames: Vec<PrivateRecursiveFrameEvent>,
    /// 原動態 tracker 真空集合快照，非自行標記的 bool。
    loans_returned: bool,
    /// 唯讀的原真資源 UTF-8 位元組觀測；不複製 str，也不轉交所有權。
    borrowed_lengths: Vec<usize>,
    /// 實際被呼叫框架的 int slot 值；僅供測試確認跨框架真傳遞。
    bound_int_values: Vec<(String, i64)>,
}


struct PrivateRecursiveActiveFrame {
    name: String,
    branch_open: bool,
    /// 本框架獨立的動態借用追蹤器。DeclarationId 是靜態詞彙位置，
    /// 不得將不同重入實例的同一宣告合併到全域借用表。
    issued: crate::borrow_safety::BorrowSafety,
    /// 本框架進入時由父框架 issued 真簽發的票據；必須還給同一追蹤器。
    ticket: Option<(crate::borrow_safety::BorrowTicket, BorrowFact)>,
    /// 本次動態實例的 Copyable int 參數槽，不可跨重入層共用。
    ints: BTreeMap<DeclarationId, i64>,
}
struct PreparedPrivateRecursiveStr {
    verified: VerifiedPrivateSource,
    original: Box<[u8]>,
    main_span: crate::common_ir::NodeSpan,
    main_locals: Vec<PrivateRecursiveStrLiteral>,
    entry: PrivateRecursiveStrCall,
    callees: Vec<PrivateRecursiveStrFunction>,
}

fn private_recursive_str_locals(
    statements: &[Stmt], verified: &VerifiedPrivateSource,
) -> Result<Vec<PrivateRecursiveStrLiteral>, PrivateRecursiveStrPrepareError> {
    let mut names = std::collections::BTreeSet::new();
    let mut result = Vec::new();
    for statement in statements {
        let StmtKind::Declare(binding) = &statement.kind else {
            return Err(PrivateRecursiveStrPrepareError::Unsupported(
                "遞迴資源子集只接受真正 str 局部宣告",
            ));
        };
        let id = DeclarationId(statement.start);
        if binding.ty != "str" || !names.insert(&binding.name)
            || !verified.plan.declarations.matches(id, "str", binding.mutable) {
            return Err(PrivateRecursiveStrPrepareError::Unsupported(
                "遞迴局部 str 缺少原 Checker 宣告與型別身分",
            ));
        }
        let initializer = binding.initializer.as_ref().ok_or(
            PrivateRecursiveStrPrepareError::Unsupported(
                "遞迴局部 str 須有真實初始化來源",
            ),
        )?;
        let text = private_str_literal(initializer, &verified.tokens).map_err(|_|
            PrivateRecursiveStrPrepareError::Unsupported(
                "遞迴局部 str 不是原 Lexer 真正 UTF-8 字面值",
            ),
        )?;
        result.push(PrivateRecursiveStrLiteral {
            identity: id,
            name: binding.name.clone(),
            mutable: binding.mutable,
            text,
        });
    }
    Ok(result)
}

fn private_recursive_str_call(
    term: &Term, verified: &VerifiedPrivateSource,
    locals: &[PrivateRecursiveStrLiteral], caller_ints: &[&Parameter],
) -> Result<PrivateRecursiveStrCall, PrivateRecursiveStrPrepareError> {
    use crate::common_ir::ScalarType;
    let TermKind::Call(call) = &term.kind else {
        return Err(PrivateRecursiveStrPrepareError::Unsupported(
            "遞迴子集只接受原 Parser 真具名函式呼叫",
        ));
    };
    let target = verified.functions.iter()
        .find(|function| function.name == call.name && function.name != "main")
        .ok_or(PrivateRecursiveStrPrepareError::Unsupported(
            "遞迴實際呼叫目標不是原 Checker 的具名函式",
        ))?;
    if call.arguments.len() != target.params.len() || target.params.is_empty()
        || target.params[0].ty != "bool" {
        return Err(PrivateRecursiveStrPrepareError::Unsupported(
            "遞迴呼叫的參數數量／bool 首參與原 Checker 簽章不同",
        ));
    }
    let argument = &call.arguments[0];
    let TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Boolean(value)) =
        &argument.kind else {
        return Err(PrivateRecursiveStrPrepareError::Unsupported(
            "遞迴子集僅接通原 Lexer bool 常值傳遞",
        ));
    };
    let text = std::str::from_utf8(&verified.source.bytes)
        .map_err(|_| PrivateRecursiveStrPrepareError::SourceChanged)?;
    if text.get(argument.start..argument.end)
        != Some(if *value { "true" } else { "false" })
        || verified.plan.scalar_hir.expression(argument.node_span())
            .is_none_or(|fact| fact.ty != ScalarType::Bool)
        || verified.plan.scalar_hir.expression(term.node_span())
            .is_none_or(|fact| fact.ty != ScalarType::Int) {
        return Err(PrivateRecursiveStrPrepareError::Unsupported(
            "遞迴呼叫 bool／int 返回缺少原 Checker TypedHIR",
        ));
    }
    let mut loan = None;
    let mut int_arguments = Vec::new();
    for (param, actual) in target.params.iter().zip(&call.arguments).skip(1) {
        match param.ty.as_str() {
            "str" if loan.is_none() => {
                let TermKind::Name(name) = &actual.kind else {
                    return Err(PrivateRecursiveStrPrepareError::Unsupported(
                        "非 Copyable 遞迴實參必須是真具名 str 所有者",
                    ));
                };
                let owner = locals.iter().rev().find(|owner| &owner.name == name)
                    .ok_or(PrivateRecursiveStrPrepareError::Unsupported(
                        "跨框架借用來源不是本次作用域可達的 str 所有者",
                    ))?;
                let expected = BorrowFact::new(owner.identity,
                    if owner.mutable { BorrowKind::ExclusiveWrite }
                    else { BorrowKind::SharedRead });
                if !verified.plan.declarations.matches(
                    owner.identity, "str", owner.mutable,
                ) || !verified.tokens.iter().any(|token|
                    token.start == actual.start && token.end == actual.end
                        && matches!(&token.kind, TokenKind::Name(found) if found == name)
                ) || verified.plan.declarations.call_borrows_at(term.node_span())
                    != Some(&[expected][..]) {
                    return Err(PrivateRecursiveStrPrepareError::Unsupported(
                        "真 str 實參缺少原 Checker BorrowFact 或 Lexer 身分",
                    ));
                }
                loan = Some((name.clone(), expected));
            }
            "int" => {
                if verified.plan.scalar_hir.expression(actual.node_span())
                    .is_none_or(|fact| fact.ty != ScalarType::Int) {
                    return Err(PrivateRecursiveStrPrepareError::Unsupported(
                        "遞迴 int 實參缺少原 Checker ScalarHIR 型別",
                    ));
                }
                let value = match &actual.kind {
                    TermKind::Scalar(
                        crate::syntax_shared::ScalarLiteral::Integer(raw),
                    ) => {
                        if !verified.tokens.iter().any(|token|
                            token.start == actual.start && token.end == actual.end
                                && matches!(&token.kind,
                                    TokenKind::Integer(found) if found == raw)
                        ) {
                            return Err(PrivateRecursiveStrPrepareError::Unsupported(
                                "遞迴 int 常值未獲原 Lexer token 授權",
                            ));
                        }
                        PrivateRecursiveIntArgument::Constant(
                            crate::syntax_shared::integer_i64(raw).map_err(|_|
                                PrivateRecursiveStrPrepareError::Unsupported(
                                    "遞迴 int 常值超出 i64",
                                ),
                            )?,
                        )
                    }
                    TermKind::Name(name) => {
                        let source = caller_ints.iter()
                            .copied().find(|param| &param.name == name)
                            .ok_or(PrivateRecursiveStrPrepareError::Unsupported(
                                "main 或其他框架不能冒充不存在的具名 int 參數",
                            ))?;
                        let id = DeclarationId(source.declaration_start);
                        if name != &source.name
                            || !verified.plan.declarations.matches(id, "int", false)
                            || verified.plan.scalar_hir.reference(actual.node_span())
                                .is_none_or(|fact|
                                    fact.resolved.declaration != id
                                    || fact.resolved.ty != ScalarType::Int)
                            || !verified.tokens.iter().any(|token|
                                token.start == actual.start && token.end == actual.end
                                    && matches!(&token.kind,
                                        TokenKind::Name(found) if found == name)
                            ) {
                            return Err(PrivateRecursiveStrPrepareError::Unsupported(
                                "遞迴 int 參數引用缺少原 Checker 宣告／Reference HIR",
                            ));
                        }
                        PrivateRecursiveIntArgument::ParentParameter(id)
                    }
                    _ => return Err(PrivateRecursiveStrPrepareError::Unsupported(
                        "遞迴 Copyable int 僅接通具名父參數或真 int 常值",
                    )),
                };
                int_arguments.push(value);
            }
            _ => return Err(PrivateRecursiveStrPrepareError::Unsupported(
                "混合遞迴只接通 bool、至多一個 str 借用與 Copyable int",
            )),
        }
    }
    if loan.is_none() && verified.plan.declarations.call_borrows_at(term.node_span())
        .is_some_and(|facts| !facts.is_empty()) {
        return Err(PrivateRecursiveStrPrepareError::Unsupported(
            "無 str 的混合遞迴不得憑空保留 BorrowFact",
        ));
    }
    Ok(PrivateRecursiveStrCall {
        name: call.name.clone(),
        argument: *value,
        span: term.node_span(),
        loan, int_arguments,
    })
}

fn private_recursive_str_return_call(
    statement: &Stmt, verified: &VerifiedPrivateSource,
    locals: &[PrivateRecursiveStrLiteral], caller_ints: &[&Parameter],
) -> Result<PrivateRecursiveStrCall, PrivateRecursiveStrPrepareError> {
    let StmtKind::Return(ret) = &statement.kind else {
        return Err(PrivateRecursiveStrPrepareError::Unsupported(
            "遞迴呼叫須位於真正 return",
        ));
    };
    private_recursive_str_call(&ret.expression, verified, locals, caller_ints)
}


impl PreparedPrivateRecursiveStr {
    fn prepare(text: &str) -> Result<Self, PrivateRecursiveStrPrepareError> {
        use crate::common_ir::{NodeRole, NodeSpan, ScalarType, VerifiedTypedMember};
        let verified = verify_source_with_ability(
            text, crate::ownership_policy::approved_leaf_copy_permission,
        ).map_err(PrivateRecursiveStrPrepareError::Check)?;
        // 這是私有來源的限制形狀，不是合法 voiles 迴圈或遞迴的一般性限制。
        if !verified.structs.is_empty() || verified.functions.len() < 2 {
            return Err(PrivateRecursiveStrPrepareError::Unsupported(
                "須有 main 與至少一個原 Checker bool 參數函式",
            ));
        }
        let main = verified.functions.iter().find(|fun| fun.name == "main")
            .ok_or(PrivateRecursiveStrPrepareError::Unsupported("缺少真 main"))?;
        if main.result_ty != "int" || !main.params.is_empty()
            || main.body.statements.is_empty() {
            return Err(PrivateRecursiveStrPrepareError::Unsupported(
                "main 必須為真無參數 int 返回函式",
            ));
        }
        let (main_last, main_prefix) = main.body.statements.split_last()
            .expect("已檢查 main 非空");
        let main_locals = private_recursive_str_locals(main_prefix, &verified)?;
        let entry = private_recursive_str_return_call(main_last, &verified, &main_locals, &[])?;
        let main_span = NodeSpan::new(main.start, main.end);

        let mut callees = Vec::new();
        for fun in verified.functions.iter().filter(|fun| fun.name != "main") {
            let param = fun.params.first().ok_or(
                PrivateRecursiveStrPrepareError::Unsupported(
                    "遞迴函式必須有原 Checker bool 控制參數",
                ),
            )?;
            let str_params: Vec<&Parameter> = fun.params.iter()
                .skip(1).filter(|part| part.ty == "str").collect();
            let int_params: Vec<&Parameter> = fun.params.iter()
                .skip(1).filter(|part| part.ty == "int").collect();
            if str_params.len() > 1
                || fun.params.len() != 1 + str_params.len() + int_params.len() {
                return Err(PrivateRecursiveStrPrepareError::Unsupported(
                    "本真框架只接通 bool、至多單 str 借用與多 Copyable int",
                ));
            }
            let str_param = str_params.first().copied();
            let param_id = DeclarationId(param.declaration_start);
            let member = VerifiedTypedMember::checked_private(
                text, NodeRole::Parameter, param.declaration_span,
                &param.name, param.name_span, &param.ty, param.type_span, false,
            ).ok_or(PrivateRecursiveStrPrepareError::Unsupported(
                "遞迴 bool 參數缺少原 Parser／Lexer 型別來源",
            ))?;
            if fun.result_ty != "int" || param.ty != "bool"
                || !verified.plan.declarations.matches(param_id, "bool", false)
                || verified.plan.members.get(&member.identity) != Some(&member)
                || fun.body.statements.len() < 2 {
                return Err(PrivateRecursiveStrPrepareError::Unsupported(
                    "遞迴函式 bool 參數、返回或型別缺少原 Checker 完整證據",
                ));
            }
            if let Some(str_param) = str_param {
                let member = VerifiedTypedMember::checked_private(
                    text, NodeRole::Parameter, str_param.declaration_span,
                    &str_param.name, str_param.name_span, &str_param.ty,
                    str_param.type_span, false,
                ).ok_or(PrivateRecursiveStrPrepareError::Unsupported(
                    "str 借用參數缺少原 Parser 成員型別來源",
                ))?;
                if str_param.ty != "str"
                    || !verified.plan.declarations.matches(
                        DeclarationId(str_param.declaration_start), "str", false,
                    )
                    || verified.plan.members.get(&member.identity) != Some(&member) {
                    return Err(PrivateRecursiveStrPrepareError::Unsupported(
                        "遞迴借用 str 參數缺少原 Checker 型別與宣告證據",
                    ));
                }
            }
            for int_param in &int_params {
                let member = VerifiedTypedMember::checked_private(
                    text, NodeRole::Parameter, int_param.declaration_span,
                    &int_param.name, int_param.name_span, &int_param.ty,
                    int_param.type_span, false,
                ).ok_or(PrivateRecursiveStrPrepareError::Unsupported(
                    "遞迴 int 參數沒有原 Parser／Lexer 來源",
                ))?;
                if !verified.plan.declarations.matches(
                    DeclarationId(int_param.declaration_start), "int", false,
                ) || verified.plan.members.get(&member.identity) != Some(&member) {
                    return Err(PrivateRecursiveStrPrepareError::Unsupported(
                        "遞迴 int 參數缺少原 Checker DeclarationId／TypedMember",
                    ));
                }
            }
            let statements = &fun.body.statements;
            let len = statements.len();
            let locals = private_recursive_str_locals(&statements[..len-2], &verified)?;
            let StmtKind::If { branch, no: None } = &statements[len-2].kind else {
                return Err(PrivateRecursiveStrPrepareError::Unsupported(
                    "遞迴子集須有原 Checker 單一 if(bool參數) 而非任意控制邊",
                ));
            };
            let condition = &branch.branch.condition;
            let TermKind::Name(name) = &condition.kind else {
                return Err(PrivateRecursiveStrPrepareError::Unsupported(
                    "遞迴條件必須是所選函式真正 bool 參數",
                ));
            };
            let referenced = verified.plan.scalar_hir.reference(condition.node_span())
                .ok_or(PrivateRecursiveStrPrepareError::Unsupported(
                    "遞迴 bool 條件缺少原 Checker HIR 引用",
                ))?;
            if name != &param.name
                || referenced.resolved.declaration != param_id
                || referenced.resolved.ty != ScalarType::Bool
                || !verified.tokens.iter().any(|token|
                    token.start == condition.start && token.end == condition.end
                        && matches!(&token.kind, TokenKind::Name(found) if found == name)
                ) {
                return Err(PrivateRecursiveStrPrepareError::Unsupported(
                    "遞迴 bool 條件來源身分不同於原 Checker 參數",
                ));
            }
            let branch_statements = &branch.branch.body.statements;
            let (branch_last, branch_prefix) = branch_statements.split_last()
                .ok_or(PrivateRecursiveStrPrepareError::Unsupported(
                    "遞迴分支缺少真正 return 呼叫",
                ))?;
            let branch_locals = private_recursive_str_locals(branch_prefix, &verified)?;
            let mut owners = locals.clone();
            owners.extend(branch_locals.clone());
            let next = private_recursive_str_return_call(
                branch_last, &verified, &owners, &int_params,
            )?;
            let StmtKind::Return(base_return) = &statements[len-1].kind else {
                return Err(PrivateRecursiveStrPrepareError::Unsupported(
                    "遞迴函式底層須以真 int 返回結束",
                ));
            };
            let base = PrivateScalarReturnExpr::from_checked(
                &base_return.expression, &fun.params, &verified,
            ).map_err(|_| PrivateRecursiveStrPrepareError::Unsupported(
                "遞迴底層返回缺少原 Checker Copyable int HIR 證據",
            ))?;
            let mut calls = Vec::new();
            base.collect_calls(&mut calls);
            if !calls.is_empty() {
                return Err(PrivateRecursiveStrPrepareError::Unsupported(
                    "底層返回不接受尚未執行的巢狀函式副作用",
                ));
            }
            callees.push(PrivateRecursiveStrFunction {
                name: fun.name.clone(),
                span: NodeSpan::new(fun.start, fun.end),
                parameter: param_id,
                int_parameters: int_params.iter().map(|p| DeclarationId(p.declaration_start)).collect(),
                locals, branch_locals, next, base_return: base,
            });
        }
        if !callees.iter().any(|fun| fun.name == entry.name)
            || callees.iter().any(|fun| !callees.iter()
                .any(|candidate| candidate.name == fun.next.name)) {
            return Err(PrivateRecursiveStrPrepareError::Unsupported(
                "遞迴函式呼叫圖無法對應原 Checker 已宣告函式",
            ));
        }
        // 每條邊由原 Checker 目標簽章決定是否需要真 str
        // BorrowTicket 與動態 int 槽，不可只看來源實參個數猜測。
        if std::iter::once(&entry).chain(callees.iter().map(|fun| &fun.next))
            .any(|call| verified.functions.iter().find(|f| f.name == call.name)
                .is_none_or(|target|
                    target.params.iter().filter(|param| param.ty == "str").count()
                        != usize::from(call.loan.is_some())
                    || target.params.iter().filter(|param| param.ty == "int").count()
                        != call.int_arguments.len())) {
            return Err(PrivateRecursiveStrPrepareError::Unsupported(
                "遞迴呼叫參數數量未與原 Checker 已驗證函式簽章一致",
            ));
        }
        let original = verified.source.bytes.clone().into_boxed_slice();
        Ok(Self { verified, original, main_span, main_locals, entry, callees })
    }

    fn same_prepared_source(&self, text: &str) -> bool {
        let Ok(fresh) = Self::prepare(text) else { return false; };
        self.original == fresh.original
            && self.main_span == fresh.main_span
            && self.main_locals == fresh.main_locals
            && self.entry == fresh.entry
            && self.callees == fresh.callees
            // is_consistent 驗證現存票據皆合法，但若整份 call_borrows
            // 被清空，必須與重新執行原 Checker 的完整授權表比較，
            // 不能因所有實際借用都消失便放行錯誤計畫。
            && self.verified.plan.declarations.call_borrows
                == fresh.verified.plan.declarations.call_borrows
    }

    fn execute(&self, budget: PrivateRecursiveStrBudget)
        -> Result<PrivateRecursiveStrOutcome, PrivateRecursiveStrPrepareError> {
        self.execute_with_fault(budget, None)
    }

    /// 測試故障只在第 N 個真正完成的 UTF-8 資源建立後觸發。
    fn execute_with_fault(
        &self, budget: PrivateRecursiveStrBudget, fault_after_creation: Option<usize>,
    ) -> Result<PrivateRecursiveStrOutcome, PrivateRecursiveStrPrepareError> {
        let Ok(text) = std::str::from_utf8(&self.verified.source.bytes) else {
            return Err(PrivateRecursiveStrPrepareError::SourceChanged);
        };
        if self.original.as_ref() != self.verified.source.bytes.as_slice()
            || !self.verified.plan.is_consistent(
                &self.verified.structs, &self.verified.functions, text,
            ) || !self.same_prepared_source(text) {
            return Err(PrivateRecursiveStrPrepareError::SourceChanged);
        }
        let mut resources = crate::owned_str_runtime::OwnedStrRuntime::new();
        let mut frames: Vec<PrivateRecursiveActiveFrame> = Vec::new();
        let mut timeline = Vec::new();
        let mut borrowed_lengths = Vec::new();
        let mut bound_int_values = Vec::new();
        let mut steps = 0usize;
        let mut allocations = 0usize;
        let result = (|| -> Result<i64, PrivateRecursiveStrRunError> {
            if budget.max_call_frames == 0 || budget.max_steps == 0 {
                return Err(PrivateRecursiveStrRunError::CallBudget);
            }
            let spend = |steps: &mut usize| -> Result<(), PrivateRecursiveStrRunError> {
                if *steps >= budget.max_steps {
                    return Err(PrivateRecursiveStrRunError::StepBudget);
                }
                *steps += 1;
                Ok(())
            };
            let mut declare = |
                locals: &[PrivateRecursiveStrLiteral],
                resources: &mut crate::owned_str_runtime::OwnedStrRuntime,
                steps: &mut usize,
            | -> Result<(), PrivateRecursiveStrRunError> {
                for local in locals {
                    spend(steps)?;
                    resources.declare(&local.name, local.mutable, &local.text)
                        .map_err(PrivateRecursiveStrRunError::Resource)?;
                    allocations += 1;
                    if fault_after_creation == Some(allocations) {
                        return Err(PrivateRecursiveStrRunError::InjectedFailure);
                    }
                }
                Ok(())
            };
            spend(&mut steps)?;
            resources.enter_scope();
            frames.push(PrivateRecursiveActiveFrame {
                name: "main".to_owned(), branch_open: false, ticket: None,
                issued: crate::borrow_safety::BorrowSafety::new(),
                ints: BTreeMap::new(),
            });
            timeline.push(PrivateRecursiveFrameEvent::Entered("main".to_owned()));
            declare(&self.main_locals, &mut resources, &mut steps)?;

            let mut next_call = self.entry.clone();
            loop {
                if frames.len() >= budget.max_call_frames {
                    return Err(PrivateRecursiveStrRunError::CallBudget);
                }
                spend(&mut steps)?;
                // 真正來源 Call 仍需在最初的 Checker 授權快照之中；
                // runtime 不重新以名稱猜測參數數量或資料型別。
                let callee = self.callees.iter().find(|fun| fun.name == next_call.name)
                    .ok_or(PrivateRecursiveStrRunError::MissingFunction)?;
                // 實參從「當次父框架」的實際 Copyable int 值讀取，
                // 多次重入同一 DeclarationId 仍是不同動態參數槽。
                let next_int: Vec<i64> = next_call.int_arguments.iter().map(|arg| {
                    match arg {
                        PrivateRecursiveIntArgument::Constant(value) => Ok(*value),
                        PrivateRecursiveIntArgument::ParentParameter(id) => frames.last()
                            .and_then(|frame| frame.ints.get(id).copied())
                            .ok_or(PrivateRecursiveStrRunError::StateMismatch),
                    }
                }).collect::<Result<_, _>>()?;
                if next_int.len() != callee.int_parameters.len() {
                    return Err(PrivateRecursiveStrRunError::StateMismatch);
                }
                // Call 的借用來源、權限、詞彙持有者都已由
                // 原 Checker call_borrows 精確授權。只有真正走到
                // 此遞迴邊時才簽發票據，不能預先借用未選分支。
                // 只從「實際擁有此 str 的父呼叫實例」借出票據；
                // 不能將相同詞彙 DeclarationId 的不同遞迴框架合併。
                let ticket = next_call.loan.as_ref().map(|(_, fact)|
                    frames.last_mut().expect("父框架存在").issued
                        .acquire(fact.source, fact.kind)
                        .map(|ticket| (ticket, *fact))
                        .map_err(PrivateRecursiveStrRunError::Ticket)
                ).transpose()?;
                // 此形狀的每個框架最多一個已發出的子呼叫；
                // 所有在同一框架中的額外票據必須先經 HC14 衝突檢查。
                frames.last().expect("父框架存在").issued.verify_static_snapshot(
                    next_call.loan.iter().map(|(_, fact)| *fact),
                ).map_err(PrivateRecursiveStrRunError::Ticket)?;
                resources.enter_scope();
                let mut ints = BTreeMap::new();
                for (id, value) in callee.int_parameters.iter().zip(next_int) {
                    if ints.insert(*id, value).is_some() {
                        return Err(PrivateRecursiveStrRunError::StateMismatch);
                    }
                    bound_int_values.push((callee.name.clone(), value));
                }
                frames.push(PrivateRecursiveActiveFrame {
                    name: callee.name.clone(), branch_open: false, ticket,
                    issued: crate::borrow_safety::BorrowSafety::new(), ints,
                });
                timeline.push(PrivateRecursiveFrameEvent::Entered(callee.name.clone()));
                if let Some((name, fact)) = &next_call.loan {
                    let ticket = frames.last().and_then(|frame|
                        frame.ticket.map(|(ticket, _)| ticket)
                    ).ok_or(PrivateRecursiveStrRunError::Ticket(
                        crate::borrow_safety::BorrowSafetyError::SnapshotMismatch,
                    ))?;
                    let parent = &frames[frames.len() - 2];
                    // 借用事實同時匹配父實例的追蹤器與原 Lexer str 名稱；
                    // 不以子框架自己新造的 tracker 冒充既有借用。
                    match read_checked_str_argument(
                        &parent.issued, ticket, fact.source, &resources, name,
                    ) {
                        Ok(length) => borrowed_lengths.push(length),
                        Err(PrivateCallBorrowRuntimeError::Resource(error)) =>
                            return Err(PrivateRecursiveStrRunError::Resource(error)),
                        Err(PrivateCallBorrowRuntimeError::Ticket(error)) =>
                            return Err(PrivateRecursiveStrRunError::Ticket(error)),
                        Err(PrivateCallBorrowRuntimeError::InjectedFailure) =>
                            return Err(PrivateRecursiveStrRunError::InjectedFailure),
                        // 純讀取 helper 本身不會產生整數計算／槽狀態
                        // 錯誤；若共用 enum 日後新增路徑，仍必須保守
                        // 拒絕，不可誤報成功的 str 借用存取。
                        Err(PrivateCallBorrowRuntimeError::StateMismatch
                            | PrivateCallBorrowRuntimeError::IntegerOverflow) =>
                            return Err(PrivateRecursiveStrRunError::Ticket(
                                crate::borrow_safety::BorrowSafetyError::SnapshotMismatch,
                            )),
                    }
                }
                declare(&callee.locals, &mut resources, &mut steps)?;
                spend(&mut steps)?; // 真正執行的 bool 參數條件判斷
                if next_call.argument {
                    resources.enter_scope();
                    frames.last_mut().expect("當前遞迴框架存在").branch_open = true;
                    declare(&callee.branch_locals, &mut resources, &mut steps)?;
                    spend(&mut steps)?; // 真正遞迴 return 語句
                    next_call = callee.next.clone();
                } else {
                    spend(&mut steps)?; // 底層真正 int return
                    break callee.base_return.evaluate(
                        &frames.last().expect("底層函式框架應存在").ints,
                    ).map_err(PrivateRecursiveStrRunError::Scalar);
                }
            }
        })();
        // 每份已進入函式的局部 OwnedUtf8 都必須依真完成順序逆序清理。
        // 如果在子呼叫準備前發生預算耗盡，絕不創造未進入的資源。
        let mut verified_frame_trackers = 0usize;
        while let Some(frame) = frames.pop() {
            // 即使子框架與父框架在原檔具有相同 DeclarationId，
            // 每個動態框架的 issued 必須各自完整歸零。
            frame.issued.verify_static_snapshot(std::iter::empty::<BorrowFact>())
                .map_err(PrivateRecursiveStrPrepareError::CleanupTicket)?;
            verified_frame_trackers += 1;
            if let Some((ticket, fact)) = frame.ticket {
                let parent = frames.last_mut().ok_or(
                    PrivateRecursiveStrPrepareError::CleanupTicket(
                        crate::borrow_safety::BorrowSafetyError::SnapshotMismatch,
                    ),
                )?;
                // 票據的 tracker_id／serial／BorrowKind 必須與原父框架一致，
                // 否則 finish_scope_verified 在動態資源離域前保守拒絕。
                parent.issued.finish_scope_verified(&[ticket], &[fact])
                    .map_err(PrivateRecursiveStrPrepareError::CleanupTicket)?;
                parent.issued.check_direct_access(fact.source)
                    .map_err(PrivateRecursiveStrPrepareError::CleanupTicket)?;
            }
            // Copyable 參數的生命期只屬這次動態呼叫實例；
            // 先清空參數，再依法釋放真 UTF-8 區域資源。
            let mut frame = frame;
            frame.ints.clear();
            if frame.branch_open {
                resources.leave_scope().map_err(
                    PrivateRecursiveStrPrepareError::Cleanup,
                )?;
            }
            resources.leave_scope().map_err(
                PrivateRecursiveStrPrepareError::Cleanup,
            )?;
            timeline.push(PrivateRecursiveFrameEvent::Exited {
                name: frame.name, returned: result.is_ok(),
            });
        }
        let loans_returned = verified_frame_trackers == timeline.iter().filter(
            |event| matches!(event, PrivateRecursiveFrameEvent::Entered(_))
        ).count();
        if !loans_returned {
            return Err(PrivateRecursiveStrPrepareError::CleanupTicket(
                crate::borrow_safety::BorrowSafetyError::SnapshotMismatch,
            ));
        }
        let events = resources.finish().map_err(
            PrivateRecursiveStrPrepareError::Cleanup,
        )?;
        if !crate::owned_str_runtime::event_trace_balanced(&events) {
            return Err(PrivateRecursiveStrPrepareError::SourceChanged);
        }
        Ok(PrivateRecursiveStrOutcome {
            result, events, frames: timeline, loans_returned,
            borrowed_lengths, bound_int_values,
        })
    }
}

#[cfg(test)]
mod g18_private_recursive_str_frames_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent;

    fn direct() -> &'static str {
        "fn descend(flag:bool)->int{\nvar local:str = \"中🪼\"\nif(flag){\nvar nested:str = \"內\"\nreturn descend(false)\n}\nreturn 7\n}\nfn main()->int{\nvar root:str = \"根\"\nreturn descend(true)\n}\n"
    }
    fn mutual() -> &'static str {
        "fn first(flag:bool)->int{\nvar a:str = \"甲\"\nif(flag){\nvar a2:str = \"分支甲\"\nreturn second(true)\n}\nreturn 1\n}\nfn second(flag:bool)->int{\nvar b:str = \"乙\"\nif(flag){\nvar b2:str = \"分支乙\"\nreturn first(false)\n}\nreturn 9\n}\nfn main()->int{\nvar root:str = \"根\"\nreturn first(true)\n}\n"
    }
    fn budget(frames: usize, steps: usize) -> PrivateRecursiveStrBudget {
        PrivateRecursiveStrBudget { max_call_frames: frames, max_steps: steps }
    }
    fn plan(source: &str) -> PreparedPrivateRecursiveStr {
        PreparedPrivateRecursiveStr::prepare(source)
            .expect("原私有 Lexer／Parser／Checker 應允許合法 bool 參數遞迴")
    }
    fn releases(events: &[ResourceEvent]) -> Vec<u64> {
        events.iter().filter_map(|e| match e {
            ResourceEvent::Released(id) => Some(*id),
            _ => None,
        }).collect()
    }
    #[test]
    fn g18_c_真直接遞迴每層_utf8_所有者按呼叫及分支逆序清理() {
        let outcome = plan(direct()).execute(budget(3, 50)).unwrap();
        assert_eq!(outcome.result, Ok(7));
        assert_eq!(releases(&outcome.events), [4, 3, 2, 1]);
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, ResourceEvent::Created(_))).count(), 4);
        assert!(crate::owned_str_runtime::event_trace_balanced(&outcome.events));
        assert_eq!(outcome.frames, [
            PrivateRecursiveFrameEvent::Entered("main".into()),
            PrivateRecursiveFrameEvent::Entered("descend".into()),
            PrivateRecursiveFrameEvent::Entered("descend".into()),
            PrivateRecursiveFrameEvent::Exited { name: "descend".into(), returned: true },
            PrivateRecursiveFrameEvent::Exited { name: "descend".into(), returned: true },
            PrivateRecursiveFrameEvent::Exited { name: "main".into(), returned: true },
        ]);
    }
    #[test]
    fn g18_c_真互相遞迴各層和分支資源跨框架清理不重覆() {
        let outcome = plan(mutual()).execute(budget(4, 100)).unwrap();
        assert_eq!(outcome.result, Ok(1));
        assert_eq!(releases(&outcome.events), [6, 5, 4, 3, 2, 1]);
        assert_eq!(outcome.frames.iter().filter(|event|
            matches!(event, PrivateRecursiveFrameEvent::Entered(_))).count(), 4);
        assert!(outcome.frames.iter().filter(|event|
            matches!(event, PrivateRecursiveFrameEvent::Exited { returned: true, .. })).count() == 4);
    }
    #[test]
    fn g18_c_遞迴框架預算耗盡須釋放已建立_utf8_且不得假成功() {
        let outcome = plan(direct()).execute(budget(2, 50)).unwrap();
        assert_eq!(outcome.result, Err(PrivateRecursiveStrRunError::CallBudget));
        assert_eq!(releases(&outcome.events), [3, 2, 1]);
        assert!(!outcome.frames.iter().any(|e|
            matches!(e, PrivateRecursiveFrameEvent::Exited { returned: true, .. })));
        assert_eq!(outcome.frames.iter().filter(|e|
            matches!(e, PrivateRecursiveFrameEvent::Entered(_))).count(), 2);
    }
    #[test]
    fn g18_c_遞迴步數預算耗盡即使分支開啟仍逐層逆序清理() {
        let outcome = plan(direct()).execute(budget(4, 5)).unwrap();
        assert_eq!(outcome.result, Err(PrivateRecursiveStrRunError::StepBudget));
        assert_eq!(releases(&outcome.events), [2, 1]);
        assert!(crate::owned_str_runtime::event_trace_balanced(&outcome.events));
    }
    #[test]
    fn g18_c_真第二層局部_utf8_建立後注入故障逐層回滾() {
        let outcome = plan(direct()).execute_with_fault(budget(4, 80), Some(4)).unwrap();
        assert_eq!(outcome.result, Err(PrivateRecursiveStrRunError::InjectedFailure));
        assert_eq!(releases(&outcome.events), [4, 3, 2, 1]);
        assert!(outcome.frames.iter().all(|event| !matches!(event,
            PrivateRecursiveFrameEvent::Exited { returned: true, .. })));
    }
    #[test]
    fn g18_c_源碼或預備的巢狀資源被篡改須在零配置前拒絕() {
        let mut prepared = plan(direct());
        prepared.callees[0].branch_locals[0].text = "偽造".into();
        assert!(matches!(prepared.execute(budget(3, 50)),
            Err(PrivateRecursiveStrPrepareError::SourceChanged)));
        let mut prepared = plan(direct());
        prepared.verified.source.bytes[0] = b'x';
        assert!(matches!(prepared.execute(budget(3, 50)),
            Err(PrivateRecursiveStrPrepareError::SourceChanged)));
    }
    #[test]
    fn g18_c_非法遞迴引數及非_copyable_複製由原_checker_先拒絕() {
        let invalid = direct().replace("descend(false)", "descend(\"不能當作bool\")");
        assert!(matches!(PreparedPrivateRecursiveStr::prepare(&invalid),
            Err(PrivateRecursiveStrPrepareError::Check(_))));
        let invalid = direct().replace("return descend(false)", "return missing(false)");
        assert!(matches!(PreparedPrivateRecursiveStr::prepare(&invalid),
            Err(PrivateRecursiveStrPrepareError::Check(_))));
    }
    #[test]
    fn g18_c_空預算不進入任一框架且動態事件為空() {
        for limit in [budget(0, 30), budget(4, 0)] {
            let result = plan(direct()).execute(limit).unwrap();
            assert!(result.result.is_err());
            assert!(result.events.is_empty() && result.frames.is_empty());
        }
    }
    fn borrowed_direct() -> &'static str {
        "fn descend(flag:bool, payload:str)->int{\nvar local:str = \"中🪼\"\nif(flag){\nvar nested:str = \"內\"\nreturn descend(false, nested)\n}\nreturn 7\n}\nfn main()->int{\nvar root:str = \"根\"\nreturn descend(true, root)\n}\n"
    }
    fn borrowed_mutual() -> &'static str {
        "fn first(flag:bool, payload:str)->int{\nvar a:str = \"甲\"\nif(flag){\nvar a2:str = \"分支甲\"\nreturn second(true, a2)\n}\nreturn 1\n}\nfn second(flag:bool, payload:str)->int{\nvar b:str = \"乙\"\nif(flag){\nvar b2:str = \"分支乙\"\nreturn first(false, b2)\n}\nreturn 9\n}\nfn main()->int{\nvar root:str = \"根\"\nreturn first(true, root)\n}\n"
    }
    #[test]
    fn g18_c_直接遞迴真_str_參數兩層_borrowticket_先歸還再清理_utf8() {
        let outcome = plan(borrowed_direct()).execute(budget(3, 80)).unwrap();
        assert_eq!(outcome.result, Ok(7));
        assert!(outcome.loans_returned);
        assert_eq!(releases(&outcome.events), [4, 3, 2, 1]);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, ResourceEvent::Moved(_))));
    }
    #[test]
    fn g18_c_互相遞迴每層真_str_借用票據不重覆也不洩漏() {
        let outcome = plan(borrowed_mutual()).execute(budget(4, 120)).unwrap();
        assert_eq!(outcome.result, Ok(1));
        assert!(outcome.loans_returned);
        assert_eq!(releases(&outcome.events), [6, 5, 4, 3, 2, 1]);
        assert_eq!(outcome.frames.iter().filter(|event|
            matches!(event, PrivateRecursiveFrameEvent::Entered(_))).count(), 4);
    }
    #[test]
    fn g18_c_跨框架借用存活時預算耗盡須先還真票據再釋放來源() {
        let outcome = plan(borrowed_direct()).execute(budget(2, 80)).unwrap();
        assert_eq!(outcome.result, Err(PrivateRecursiveStrRunError::CallBudget));
        assert_eq!(releases(&outcome.events), [3, 2, 1]);
        assert!(outcome.loans_returned);
        assert!(outcome.frames.iter().all(|event| !matches!(event,
            PrivateRecursiveFrameEvent::Exited { returned: true, .. })));
    }
    #[test]
    fn g18_c_第二層借用來源完成配置後注入錯誤仍歸還全部票據() {
        let outcome = plan(borrowed_direct()).execute_with_fault(
            budget(4, 80), Some(4),
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateRecursiveStrRunError::InjectedFailure));
        assert!(outcome.loans_returned);
        assert_eq!(releases(&outcome.events), [4, 3, 2, 1]);
    }
    #[test]
    fn g18_c_竄改已核准呼叫_borrowfact_來源與_kind_在配置前拒絕() {
        let mut prepared = plan(borrowed_direct());
        prepared.entry.loan.as_mut().unwrap().1.source = DeclarationId(0);
        assert!(matches!(prepared.execute(budget(3, 80)),
            Err(PrivateRecursiveStrPrepareError::SourceChanged)));
        let mut prepared = plan(borrowed_direct());
        prepared.callees[0].next.loan.as_mut().unwrap().1.kind = BorrowKind::SharedRead;
        assert!(matches!(prepared.execute(budget(3, 80)),
            Err(PrivateRecursiveStrPrepareError::SourceChanged)));
    }
    #[test]
    fn g18_c_原_checker_缺失呼叫借用票據或非法_str_來源不可真執行() {
        let mut prepared = plan(borrowed_direct());
        prepared.verified.plan.declarations.call_borrows.clear();
        assert!(matches!(prepared.execute(budget(3, 80)),
            Err(PrivateRecursiveStrPrepareError::SourceChanged)));
        let invalid = borrowed_direct().replace("descend(false, nested)",
            "descend(false, payload)");
        assert!(matches!(PreparedPrivateRecursiveStr::prepare(&invalid),
            Err(PrivateRecursiveStrPrepareError::Check(_))
                | Err(PrivateRecursiveStrPrepareError::Unsupported(_))));
    }

    fn repeated_borrowed_reentry() -> &'static str {
        "fn descend(flag:bool, payload:str)->int{\nvar repeated:str = \"同名各框架\"\nif(flag){\nvar nested:str = \"繁體每層\"\nreturn descend(true, nested)\n}\nreturn 7\n}\nfn main()->int{\nvar root:str = \"根\"\nreturn descend(true, root)\n}\n"
    }
    #[test]
    fn g18_c_深層重入同一宣告來源須各有動態_borrow_tracker_且受控耗盡() {
        let outcome = plan(repeated_borrowed_reentry())
            .execute(budget(6, 100)).unwrap();
        assert_eq!(outcome.result, Err(PrivateRecursiveStrRunError::CallBudget));
        assert!(outcome.loans_returned);
        assert_eq!(outcome.borrowed_lengths, [3, 12, 12, 12, 12]);
        assert_eq!(releases(&outcome.events), (1..=11).rev().collect::<Vec<_>>());
        assert!(crate::owned_str_runtime::event_trace_balanced(&outcome.events));
        assert_eq!(outcome.frames.iter().filter(|event|
            matches!(event, PrivateRecursiveFrameEvent::Exited { returned: false, .. }))
            .count(), 6);
    }
    #[test]
    fn g18_c_跨框架相同原宣告位置的唯讀和獨占票據不能誤互斥() {
        let text = repeated_borrowed_reentry()
            .replace("var nested:str", "const nested:str");
        let outcome = plan(&text).execute(budget(6, 100)).unwrap();
        assert_eq!(outcome.result, Err(PrivateRecursiveStrRunError::CallBudget));
        assert!(outcome.loans_returned);
        assert_eq!(releases(&outcome.events), (1..=11).rev().collect::<Vec<_>>());
        assert_eq!(outcome.borrowed_lengths.len(), 5);
    }
    #[test]
    fn g18_c_深層重入步數耗盡各框架票據與所有者全部逆序歸還() {
        let outcome = plan(repeated_borrowed_reentry())
            .execute(budget(12, 16)).unwrap();
        assert_eq!(outcome.result, Err(PrivateRecursiveStrRunError::StepBudget));
        assert!(outcome.loans_returned);
        assert!(outcome.events.iter().any(|event|
            matches!(event, ResourceEvent::Created(_))));
        assert!(crate::owned_str_runtime::event_trace_balanced(&outcome.events));
        assert!(!outcome.frames.iter().any(|event|
            matches!(event, PrivateRecursiveFrameEvent::Exited { returned: true, .. })));
    }
    #[test]
    fn g18_c_重入框架內故障不以共用靜態宣告身份錯誤解鎖() {
        let outcome = plan(repeated_borrowed_reentry())
            .execute_with_fault(budget(12, 100), Some(7)).unwrap();
        assert_eq!(outcome.result, Err(PrivateRecursiveStrRunError::InjectedFailure));
        assert!(outcome.loans_returned);
        assert_eq!(releases(&outcome.events), (1..=7).rev().collect::<Vec<_>>());
        assert!(crate::owned_str_runtime::event_trace_balanced(&outcome.events));
    }

    fn mixed_direct() -> &'static str {
        "fn descend(flag:bool,payload:str,n:int)->int{\nvar local:str = \"中🪼\"\nif(flag){\nvar nested:str = \"內\"\nreturn descend(false,nested,n)\n}\nreturn n+3\n}\nfn main()->int{\nvar root:str = \"根\"\nreturn descend(true,root,7)\n}\n"
    }
    fn mixed_mutual() -> &'static str {
        "fn first(flag:bool,payload:str,n:int)->int{\nvar a:str = \"甲\"\nif(flag){\nvar nested:str = \"分支甲\"\nreturn second(false,n,nested)\n}\nreturn n+1\n}\nfn second(flag:bool,n:int,payload:str)->int{\nvar b:str = \"乙\"\nif(flag){\nvar nested:str = \"分支乙\"\nreturn first(false,nested,n)\n}\nreturn n-2\n}\nfn main()->int{\nvar root:str = \"根\"\nreturn first(true,root,12)\n}\n"
    }
    #[test]
    fn g18_c_直接遞迴_str_真票據與_copyable_int_逐框架傳遞並由_hir_算術返回() {
        let plan = plan(mixed_direct());
        let out = plan.execute(budget(4, 100)).unwrap();
        assert_eq!(out.result, Ok(10));
        assert_eq!(out.bound_int_values, [
            ("descend".to_owned(), 7), ("descend".to_owned(), 7),
        ]);
        assert_eq!(out.borrowed_lengths, ["根".len(), "內".len()]);
        assert_eq!(releases(&out.events), [4, 3, 2, 1]);
        assert!(out.loans_returned);
        assert!(crate::owned_str_runtime::event_trace_balanced(&out.events));
    }

    #[test]
    fn g18_c_互相遞迴_參數順序不同仍保留原_checker_int_與_str_借用身份() {
        let out = plan(mixed_mutual()).execute(budget(5, 120)).unwrap();
        assert_eq!(out.result, Ok(10));
        assert_eq!(out.bound_int_values, [
            ("first".to_owned(), 12), ("second".to_owned(), 12),
        ]);
        assert_eq!(releases(&out.events), [4, 3, 2, 1]);
        assert!(out.loans_returned);
        assert_eq!(out.borrowed_lengths, ["根".len(), "分支甲".len()]);
        assert!(crate::owned_str_runtime::event_trace_balanced(&out.events));
    }

    #[test]
    fn g18_c_無_str_實參的_bool_int_遞迴仍真建立每框架_utf8_並返回動態參數() {
        let text = "fn repeat(flag:bool,n:int)->int{\nvar local:str = \"層\"\nif(flag){\nvar nested:str = \"內\"\nreturn repeat(false,n)\n}\nreturn n+n\n}\nfn main()->int{\nvar root:str = \"根\"\nreturn repeat(true,9)\n}\n";
        let out = plan(text).execute(budget(4, 100)).unwrap();
        assert_eq!(out.result, Ok(18));
        assert_eq!(out.bound_int_values, [
            ("repeat".to_owned(), 9), ("repeat".to_owned(), 9),
        ]);
        assert!(out.borrowed_lengths.is_empty());
        assert!(out.loans_returned);
        assert_eq!(releases(&out.events), [4, 3, 2, 1]);
    }

    #[test]
    fn g18_c_子遞迴_int_常值覆寫父框架值_底層不得錯讀父參數() {
        let text = "fn descend(flag:bool,payload:str,n:int)->int{\nvar local:str = \"中\"\nif(flag){\nvar nested:str = \"內\"\nreturn descend(false,nested,4)\n}\nreturn n+1\n}\nfn main()->int{\nvar root:str = \"根\"\nreturn descend(true,root,100)\n}\n";
        let out = plan(text).execute(budget(4, 100)).unwrap();
        assert_eq!(out.result, Ok(5));
        assert_eq!(out.bound_int_values, [
            ("descend".to_owned(), 100), ("descend".to_owned(), 4),
        ]);
        assert!(out.loans_returned);
        assert_eq!(releases(&out.events), [4, 3, 2, 1]);
    }

    #[test]
    fn g18_c_混合_int_底層算術溢位須失敗並先歸還借用票據再逐層釋放() {
        let text = mixed_direct().replace("return n+3",
            "return n+1").replace("descend(true,root,7)",
            "descend(true,root,9223372036854775807)");
        let out = plan(&text).execute(budget(4, 100)).unwrap();
        assert_eq!(out.result, Err(PrivateRecursiveStrRunError::Scalar(
            PrivateScalarCallRuntimeError::IntegerOverflow,
        )));
        assert_eq!(releases(&out.events), [4, 3, 2, 1]);
        assert!(out.loans_returned);
        assert!(out.frames.iter().all(|event| !matches!(event,
            PrivateRecursiveFrameEvent::Exited { returned: true, .. })));
    }

    #[test]
    fn g18_c_混合_str_int_跨框架呼叫預算耗盡仍保留實際參數值與全部清理() {
        let out = plan(mixed_direct()).execute(budget(2, 100)).unwrap();
        assert_eq!(out.result, Err(PrivateRecursiveStrRunError::CallBudget));
        assert_eq!(out.bound_int_values, [("descend".to_owned(), 7)]);
        assert_eq!(out.borrowed_lengths, ["根".len()]);
        assert!(out.loans_returned);
        assert_eq!(releases(&out.events), [3, 2, 1]);
    }

    #[test]
    fn g18_c_混合遞迴第二層_int_已綁定後故障仍清理所有_utf8_與票據() {
        let out = plan(mixed_direct())
            .execute_with_fault(budget(4, 100), Some(4)).unwrap();
        assert_eq!(out.result, Err(PrivateRecursiveStrRunError::InjectedFailure));
        assert_eq!(out.bound_int_values, [
            ("descend".to_owned(), 7), ("descend".to_owned(), 7),
        ]);
        assert!(out.loans_returned);
        assert_eq!(releases(&out.events), [4, 3, 2, 1]);
    }

    #[test]
    fn g18_c_竄改遞迴_int_實參來源或返回_hir_必須在真資源建立前拒絕() {
        let mut p = plan(mixed_direct());
        p.callees[0].next.int_arguments[0] =
            PrivateRecursiveIntArgument::ParentParameter(DeclarationId(0));
        assert!(matches!(p.execute(budget(4, 100)),
            Err(PrivateRecursiveStrPrepareError::SourceChanged)));
        let mut p = plan(mixed_direct());
        p.entry.int_arguments[0] = PrivateRecursiveIntArgument::Constant(99);
        assert!(matches!(p.execute(budget(4, 100)),
            Err(PrivateRecursiveStrPrepareError::SourceChanged)));
        let mut p = plan(mixed_direct());
        p.callees[0].base_return = PrivateScalarReturnExpr::Constant(111);
        assert!(matches!(p.execute(budget(4, 100)),
            Err(PrivateRecursiveStrPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_c_混合遞迴錯誤_int_型別及漏參數必先由原_checker_拒絕() {
        for invalid in [
            mixed_direct().replace("descend(false,nested,n)",
                "descend(false,nested,true)"),
            mixed_direct().replace("descend(true,root,7)",
                "descend(true,root)"),
            mixed_direct().replace("return n+3", "return payload+3"),
        ] {
            assert!(matches!(PreparedPrivateRecursiveStr::prepare(&invalid),
                Err(PrivateRecursiveStrPrepareError::Check(_))), "{invalid}");
        }
    }

    fn mixed_pair() -> &'static str {
        "fn descend(flag:bool,payload:str,left:int,right:int)->int{\nvar local:str = \"層\"\nif(flag){\nvar nested:str = \"內\"\nreturn descend(false,nested,right,left)\n}\nreturn left-right\n}\nfn main()->int{\nvar root:str = \"根\"\nreturn descend(true,root,10,3)\n}\n"
    }
    #[test]
    fn g18_c_直接遞迴兩個_copyable_int_互換父子參數槽並保護真_str_票據() {
        let out = plan(mixed_pair()).execute(budget(4, 100)).unwrap();
        assert_eq!(out.result, Ok(-7));
        assert_eq!(out.bound_int_values, [
            ("descend".to_owned(), 10), ("descend".to_owned(), 3),
            ("descend".to_owned(), 3), ("descend".to_owned(), 10),
        ]);
        assert_eq!(out.borrowed_lengths, ["根".len(), "內".len()]);
        assert_eq!(releases(&out.events), [4, 3, 2, 1]);
        assert!(out.loans_returned);
    }

    #[test]
    fn g18_c_單張_str_borrowticket_和三個_int_跨遞迴位置交換實際返回() {
        let source = "fn sum(flag:bool,payload:str,a:int,b:int,c:int)->int{\nvar local:str = \"層\"\nif(flag){\nvar nested:str = \"內\"\nreturn sum(false,nested,b,c,a)\n}\nreturn a+b+c\n}\nfn main()->int{\nvar root:str = \"根\"\nreturn sum(true,root,1,2,3)\n}\n";
        let out = plan(source).execute(budget(4, 100)).unwrap();
        assert_eq!(out.result, Ok(6));
        assert_eq!(out.bound_int_values, [
            ("sum".to_owned(), 1), ("sum".to_owned(), 2), ("sum".to_owned(), 3),
            ("sum".to_owned(), 2), ("sum".to_owned(), 3), ("sum".to_owned(), 1),
        ]);
        assert!(out.loans_returned);
        assert_eq!(releases(&out.events), [4, 3, 2, 1]);
    }

    #[test]
    fn g18_c_相同_copyable_int_父實參可給兩個子參數但不可複製真_str() {
        let source = mixed_pair().replace("descend(false,nested,right,left)",
            "descend(false,nested,left,left)");
        let out = plan(&source).execute(budget(4, 100)).unwrap();
        assert_eq!(out.result, Ok(0));
        assert_eq!(out.bound_int_values, [
            ("descend".to_owned(), 10), ("descend".to_owned(), 3),
            ("descend".to_owned(), 10), ("descend".to_owned(), 10),
        ]);
        assert!(out.loans_returned);
        assert_eq!(releases(&out.events), [4, 3, 2, 1]);
        assert!(!out.events.iter().any(|event|
            matches!(event, ResourceEvent::Moved(_))));
    }

    fn long_mixed_chain() -> String {
        let mut source = String::new();
        for idx in 0..5 {
            let call = if idx == 4 {
                "f0(false,nested,a,b)".to_owned()
            } else {
                format!("f{}(true,nested,a,b)", idx + 1)
            };
            source.push_str(&format!(
                "fn f{idx}(flag:bool,payload:str,a:int,b:int)->int{{\nvar local:str = \"層\"\nif(flag){{\nvar nested:str = \"內\"\nreturn {call}\n}}\nreturn a+b\n}}\n"
            ));
        }
        source.push_str("fn main()->int{\nvar root:str = \"根\"\nreturn f0(true,root,10,3)\n}\n");
        source
    }
    #[test]
    fn g18_c_五個互相呼叫函式與六層混合資源_不得被舊兩函式上限拒絕() {
        let source = long_mixed_chain();
        let out = plan(&source).execute(budget(8, 250)).unwrap();
        assert_eq!(out.result, Ok(13));
        assert_eq!(out.frames.iter().filter(|event|
            matches!(event, PrivateRecursiveFrameEvent::Entered(_))).count(), 7);
        assert_eq!(out.bound_int_values.len(), 12);
        assert_eq!(out.borrowed_lengths.len(), 6);
        assert_eq!(releases(&out.events), (1..=12).rev().collect::<Vec<_>>());
        assert!(out.loans_returned);
        assert!(crate::owned_str_runtime::event_trace_balanced(&out.events));
    }

    #[test]
    fn g18_c_多函式混合資源框架預算耗盡須逆序釋放每個已配置_resourceid() {
        let source = long_mixed_chain();
        let out = plan(&source).execute(budget(4, 250)).unwrap();
        assert_eq!(out.result, Err(PrivateRecursiveStrRunError::CallBudget));
        assert_eq!(out.bound_int_values.len(), 6);
        assert_eq!(out.borrowed_lengths.len(), 3);
        assert_eq!(releases(&out.events), (1..=7).rev().collect::<Vec<_>>());
        assert!(out.loans_returned);
        assert!(out.frames.iter().all(|event| !matches!(event,
            PrivateRecursiveFrameEvent::Exited { returned: true, .. })));
    }

    #[test]
    fn g18_c_混合多個_int_底層溢位不得略過真借用歸還及_utf8_釋放() {
        let text = mixed_pair().replace("return left-right", "return left+right")
            .replace("descend(true,root,10,3)",
                "descend(false,root,9223372036854775807,1)");
        let out = plan(&text).execute(budget(4, 100)).unwrap();
        assert_eq!(out.result, Err(PrivateRecursiveStrRunError::Scalar(
            PrivateScalarCallRuntimeError::IntegerOverflow,
        )));
        assert_eq!(out.borrowed_lengths, ["根".len()]);
        assert!(out.loans_returned);
        assert_eq!(releases(&out.events), [2, 1]);
    }

    #[test]
    fn g18_c_混合遞迴第二_int_及多函式來源篡改必在建立資源前拒絕() {
        let mut prepared = plan(mixed_pair());
        prepared.entry.int_arguments[1] = PrivateRecursiveIntArgument::Constant(99);
        assert!(matches!(prepared.execute(budget(4, 100)),
            Err(PrivateRecursiveStrPrepareError::SourceChanged)));
        let mut prepared = plan(mixed_pair());
        prepared.callees[0].next.int_arguments[1] =
            PrivateRecursiveIntArgument::ParentParameter(DeclarationId(0));
        assert!(matches!(prepared.execute(budget(4, 100)),
            Err(PrivateRecursiveStrPrepareError::SourceChanged)));
        let mut prepared = plan(&long_mixed_chain());
        prepared.callees[2].name = "偽造".to_owned();
        assert!(matches!(prepared.execute(budget(8, 250)),
            Err(PrivateRecursiveStrPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_c_混合多_int_錯誤參數型別與個數須由原_checker_拒絕() {
        for text in [
            mixed_pair().replace("descend(true,root,10,3)",
                "descend(true,root,true,3)"),
            mixed_pair().replace("descend(false,nested,right,left)",
                "descend(false,nested,right)"),
            mixed_pair().replace("descend(true,root,10,3)",
                "descend(true,root,10,3,4)"),
        ] {
            assert!(matches!(PreparedPrivateRecursiveStr::prepare(&text),
                Err(PrivateRecursiveStrPrepareError::Check(_))), "{text}");
        }
    }

}
