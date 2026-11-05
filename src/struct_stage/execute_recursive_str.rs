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
struct PrivateRecursiveStrCall {
    name: String,
    argument: bool,
    span: crate::common_ir::NodeSpan,
    /// 只有原 Checker call_borrows 真票據才可記錄本次 str 參數借用。
    loan: Option<(String, BorrowFact)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateRecursiveStrFunction {
    name: String,
    span: crate::common_ir::NodeSpan,
    parameter: DeclarationId,
    locals: Vec<PrivateRecursiveStrLiteral>,
    branch_locals: Vec<PrivateRecursiveStrLiteral>,
    next: PrivateRecursiveStrCall,
    base_return: i64,
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
    locals: &[PrivateRecursiveStrLiteral],
) -> Result<PrivateRecursiveStrCall, PrivateRecursiveStrPrepareError> {
    use crate::common_ir::ScalarType;
    let TermKind::Call(call) = &term.kind else {
        return Err(PrivateRecursiveStrPrepareError::Unsupported(
            "遞迴子集只接受原 Parser 真具名函式呼叫",
        ));
    };
    let (argument, extra) = match call.arguments.as_slice() {
        [first] => (first, None),
        [first, second] => (first, Some(second)),
        _ => return Err(PrivateRecursiveStrPrepareError::Unsupported(
            "遞迴子集呼叫須有 bool 與可選單一 str 借用實參",
        )),
    };
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
            "遞迴呼叫參數或 int 返回缺少原 Checker TypedHIR 來源",
        ));
    }
    let facts = verified.plan.declarations.call_borrows_at(term.node_span());
    let loan = match extra {
        None => {
            if facts.is_some_and(|loans| !loans.is_empty()) {
                return Err(PrivateRecursiveStrPrepareError::Unsupported(
                    "僅 bool 實參的遞迴呼叫不得憑空有 BorrowFact",
                ));
            }
            None
        }
        Some(second) => {
            let TermKind::Name(name) = &second.kind else {
                return Err(PrivateRecursiveStrPrepareError::Unsupported(
                    "非 Copyable 遞迴實參只准來自真 str 所有者名稱",
                ));
            };
            let owner = locals.iter().rev().find(|owner| &owner.name == name)
                .ok_or(PrivateRecursiveStrPrepareError::Unsupported(
                    "跨框架借用來源不是已執行詞彙 scope 的 str 所有者",
                ))?;
            let expected = BorrowFact::new(owner.identity,
                if owner.mutable { BorrowKind::ExclusiveWrite }
                else { BorrowKind::SharedRead });
            if !verified.plan.declarations.matches(
                owner.identity, "str", owner.mutable,
            ) || !verified.tokens.iter().any(|token|
                token.start == second.start && token.end == second.end
                    && matches!(&token.kind, TokenKind::Name(found) if found == name)
            ) || facts != Some(&[expected][..]) {
                return Err(PrivateRecursiveStrPrepareError::Unsupported(
                    "真 str 實參缺少原 Checker call_borrows 與 Lexer 身分",
                ));
            }
            Some((name.clone(), expected))
        }
    };
    Ok(PrivateRecursiveStrCall {
        name: call.name.clone(),
        argument: *value,
        span: term.node_span(),
        loan,
    })
}

fn private_recursive_str_return_call(
    statement: &Stmt, verified: &VerifiedPrivateSource,
    locals: &[PrivateRecursiveStrLiteral],
) -> Result<PrivateRecursiveStrCall, PrivateRecursiveStrPrepareError> {
    let StmtKind::Return(ret) = &statement.kind else {
        return Err(PrivateRecursiveStrPrepareError::Unsupported(
            "遞迴呼叫須位於真正 return",
        ));
    };
    private_recursive_str_call(&ret.expression, verified, locals)
}


fn target_needs_str_param(verified: &VerifiedPrivateSource, name: &str) -> bool {
    verified.functions.iter().find(|fun| fun.name == name)
        .is_some_and(|fun| fun.params.len() == 2 && fun.params[1].ty == "str")
}

impl PreparedPrivateRecursiveStr {
    fn prepare(text: &str) -> Result<Self, PrivateRecursiveStrPrepareError> {
        use crate::common_ir::{NodeRole, NodeSpan, ScalarType, VerifiedTypedMember};
        let verified = verify_source_with_ability(
            text, crate::ownership_policy::approved_leaf_copy_permission,
        ).map_err(PrivateRecursiveStrPrepareError::Check)?;
        // 這是私有來源的限制形狀，不是合法 voiles 迴圈或遞迴的一般性限制。
        if !verified.structs.is_empty() || !(2..=3).contains(&verified.functions.len()) {
            return Err(PrivateRecursiveStrPrepareError::Unsupported(
                "僅接通 main 與一或兩個原 Checker bool 參數遞迴函式",
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
        let entry = private_recursive_str_return_call(main_last, &verified, &main_locals)?;
        let main_span = NodeSpan::new(main.start, main.end);

        let mut callees = Vec::new();
        for fun in verified.functions.iter().filter(|fun| fun.name != "main") {
            let (param, str_param) = match fun.params.as_slice() {
                [flag] => (flag, None),
                [flag, payload] => (flag, Some(payload)),
                _ => return Err(PrivateRecursiveStrPrepareError::Unsupported(
                    "遞迴函式只支援一 bool 加可選單一 str 借用參數",
                )),
            };
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
                branch_last, &verified, &owners,
            )?;
            let StmtKind::Return(base_return) = &statements[len-1].kind else {
                return Err(PrivateRecursiveStrPrepareError::Unsupported(
                    "遞迴函式底層須以真 int 返回結束",
                ));
            };
            let TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) =
                &base_return.expression.kind else {
                return Err(PrivateRecursiveStrPrepareError::Unsupported(
                    "本批底層只接通原 Checker int 常值返回",
                ));
            };
            let base_return_span = base_return.expression.node_span();
            if verified.plan.scalar_hir.expression(base_return_span)
                .is_none_or(|fact| fact.ty != ScalarType::Int)
                || !verified.tokens.iter().any(|token|
                    token.start == base_return.expression.start
                        && token.end == base_return.expression.end
                        && matches!(&token.kind, TokenKind::Integer(found) if found == raw)
                ) {
                return Err(PrivateRecursiveStrPrepareError::Unsupported(
                    "遞迴底層返回缺少原 Checker int TypedHIR／Lexer token",
                ));
            }
            let base = crate::syntax_shared::integer_i64(raw).map_err(|_|
                PrivateRecursiveStrPrepareError::Unsupported(
                    "遞迴底層 int 返回超過 i64",
                ),
            )?;
            callees.push(PrivateRecursiveStrFunction {
                name: fun.name.clone(),
                span: NodeSpan::new(fun.start, fun.end),
                parameter: param_id,
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
        // 呼叫所選簽章必須與原 Checker 的一或二個參數相同；
        // 絕不能讓無借用路徑跳過真正 str 參數的 BorrowTicket。
        if std::iter::once(&entry).chain(callees.iter().map(|fun| &fun.next))
            .any(|call| callees.iter().find(|fun| fun.name == call.name)
                .is_none_or(|target| target_needs_str_param(&verified, &target.name)
                    != call.loan.is_some())) {
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
        let mut frames: Vec<(String, bool)> = Vec::new();
        let mut timeline = Vec::new();
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
            frames.push(("main".to_owned(), false));
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
                resources.enter_scope();
                frames.push((callee.name.clone(), false));
                timeline.push(PrivateRecursiveFrameEvent::Entered(callee.name.clone()));
                declare(&callee.locals, &mut resources, &mut steps)?;
                spend(&mut steps)?; // 真正執行的 bool 參數條件判斷
                if next_call.argument {
                    resources.enter_scope();
                    frames.last_mut().expect("當前遞迴框架存在").1 = true;
                    declare(&callee.branch_locals, &mut resources, &mut steps)?;
                    spend(&mut steps)?; // 真正遞迴 return 語句
                    next_call = callee.next.clone();
                } else {
                    spend(&mut steps)?; // 底層真正 int return
                    break Ok(callee.base_return);
                }
            }
        })();
        // 每份已進入函式的局部 OwnedUtf8 都必須依真完成順序逆序清理。
        // 如果在子呼叫準備前發生預算耗盡，絕不創造未進入的資源。
        while let Some((name, branch_open)) = frames.pop() {
            if branch_open {
                resources.leave_scope().map_err(
                    PrivateRecursiveStrPrepareError::Cleanup,
                )?;
            }
            resources.leave_scope().map_err(
                PrivateRecursiveStrPrepareError::Cleanup,
            )?;
            timeline.push(PrivateRecursiveFrameEvent::Exited {
                name, returned: result.is_ok(),
            });
        }
        let events = resources.finish().map_err(
            PrivateRecursiveStrPrepareError::Cleanup,
        )?;
        if !crate::owned_str_runtime::event_trace_balanced(&events) {
            return Err(PrivateRecursiveStrPrepareError::SourceChanged);
        }
        Ok(PrivateRecursiveStrOutcome { result, events, frames: timeline })
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
}
