// G18／P18.1、P18.4：只接通正式私有 Checker 已通過的
// 一個 str 真持有者，搭配多個 Copyable int 參數 -> 原 Checker 普通函式真返回。
// 原 Checker 的 call_borrows_at(NodeSpan) 是唯一的借用授權權威；
// 真 BorrowTicket 僅在執行到呼叫時簽出並在該次函式退出時歸還。
// 不新增參數語法、方法多載、隱含 str 複製或公開執行入口。
#[derive(Debug)]
enum PrivateCallBorrowPrepareError {
    Check(crate::Diagnostic),
    Unsupported(&'static str),
    SourceChanged,
    Cleanup(crate::owned_str_runtime::StrRuntimeError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateCallBorrowRuntimeError {
    Resource(crate::owned_str_runtime::StrRuntimeError),
    Ticket(crate::borrow_safety::BorrowSafetyError),
    InjectedFailure,
    StateMismatch,
    IntegerOverflow,
}

/// 只允許仍活躍的真 BorrowTicket 讀取已由原 Checker 繫結的參數。
/// 此函式不改動 OwnedStrRuntime 所有權，不複製或重新宣告資源。
/// source_id／owner 只能來自 PreparedPrivateCallBorrow 的原核准計畫。
fn read_checked_str_argument(
    tracker: &crate::borrow_safety::BorrowSafety,
    ticket: crate::borrow_safety::BorrowTicket,
    source_id: DeclarationId,
    runtime: &crate::owned_str_runtime::OwnedStrRuntime,
    owner: &str,
) -> Result<usize, PrivateCallBorrowRuntimeError> {
    let authorized = tracker
        .check_ticket_access(ticket, BorrowKind::SharedRead)
        .map_err(PrivateCallBorrowRuntimeError::Ticket)?;
    if authorized != source_id {
        return Err(PrivateCallBorrowRuntimeError::Ticket(
            crate::borrow_safety::BorrowSafetyError::SnapshotMismatch,
        ));
    }
    runtime.read(owner)
        .map(|text| text.len())
        .map_err(PrivateCallBorrowRuntimeError::Resource)
}

struct PreparedPrivateCallBorrow {
    verified: VerifiedPrivateSource,
    original: Box<[u8]>,
    owner: String,
    owner_id: DeclarationId,
    mutable: bool,
    text: String,
    callee_name: String,
    callee_span: crate::common_ir::NodeSpan,
    call_span: crate::common_ir::NodeSpan,
    loan: BorrowFact,
    returned: i64,
    /// 原函式簽章中真 str 參數位置，不靠 runtime 值猜測。
    str_position: usize,
    /// 原 Checker 的第二個 Copyable int Parameter DeclarationId 與實參值。
    int_binding: Option<(DeclarationId, i64)>,
    /// 後續所有已驗證 Copyable int 參數，依原完整函式簽章順序綁定。
    additional_int_bindings: Vec<(DeclarationId, i64)>,
    /// 具名 Copyable 實參的原 main 局部宣告及獨立 int 值。
    int_owner: Option<(DeclarationId, i64)>,
    /// 從當次真正 int 參數槽求值的原 ScalarHIR 加／減／返回樹。
    mixed_return: Option<PrivateScalarReturnExpr>,
}

#[derive(Debug)]
struct PrivateCallBorrowOutcome {
    result: Result<i64, PrivateCallBorrowRuntimeError>,
    observed_utf8_bytes: Option<usize>,
    call_entered: bool,
    call_exited: bool,
    events: Vec<crate::owned_str_runtime::ResourceEvent>,
    /// 多型別呼叫內的真 i64 參數槽在退出後必須為零。
    parameter_slots_after_exit: usize,
    observed_integer: Option<i64>,
}

impl PreparedPrivateCallBorrow {
    fn prepare(source: &str) -> Result<Self, PrivateCallBorrowPrepareError> {
        let verified = verify_source_with_ability(
            source, crate::ownership_policy::approved_leaf_copy_permission,
        ).map_err(PrivateCallBorrowPrepareError::Check)?;
        if !verified.structs.is_empty() || verified.functions.len() != 2 {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "只接通一個已核准普通函式與無參數 main",
            ));
        }
        let main = verified.functions.iter().find(|fun| fun.name == "main")
            .ok_or(PrivateCallBorrowPrepareError::Unsupported("缺少真正 main"))?;
        let callee = verified.functions.iter().find(|fun| fun.name != "main")
            .ok_or(PrivateCallBorrowPrepareError::Unsupported("缺少被呼叫函式"))?;
        if main.result_ty != "int" || !main.params.is_empty()
            || !(2..=3).contains(&main.body.statements.len())
            || callee.result_ty != "int"
            || callee.params.is_empty()
            || callee.body.statements.len() != 1 {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "只執行原 Checker 已核准的單 str 借用與任意 Copyable int 參數",
            ));
        }
        let str_position = callee.params.iter().position(|param| param.ty == "str")
            .ok_or(PrivateCallBorrowPrepareError::Unsupported(
                "缺少原 Parser 真 str 預設借用參數",
            ))?;
        // 靜態來源必須恰好有一個非 Copyable str 與其餘
        // 已核准 Copyable int。多個 str 的借用模式仍待獨立完整驗收，
        // 不可透過本次多純值框架忽略或複製第二個持有者。
        if callee.params.iter().filter(|param| param.ty == "str").count() != 1
            || callee.params.iter().any(|param|
                param.ty != "str" && param.ty != "int") {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "本真借用切片要求唯一 str 借用與任意多個已核准 int 參數",
            ));
        }
        for param in &callee.params {
            let id = DeclarationId(param.declaration_start);
            let member = crate::common_ir::VerifiedTypedMember::checked_private(
                source, crate::common_ir::NodeRole::Parameter,
                param.declaration_span, &param.name, param.name_span,
                &param.ty, param.type_span, false,
            ).ok_or(PrivateCallBorrowPrepareError::Unsupported(
                "混合參數缺少原 Parser／Lexer 真名稱與型別身分",
            ))?;
            if !verified.plan.declarations.matches(id, &param.ty, false)
                || verified.plan.members.get(&member.identity) != Some(&member) {
                return Err(PrivateCallBorrowPrepareError::Unsupported(
                    "混合參數缺少原 Checker DeclarationId／TypedMember 正式事實",
                ));
            }
        }
        let StmtKind::Declare(owner) = &main.body.statements[0].kind else {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "主函式首句須實際建立唯一 str 所有者",
            ));
        };
        let identity = DeclarationId(main.body.statements[0].start);
        if owner.ty != "str"
            || !verified.plan.declarations.matches(identity, "str", owner.mutable) {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "持有者缺少原 Checker 真實 str 型別或 var/const 身分",
            ));
        }
        let init = owner.initializer.as_ref().ok_or(
            PrivateCallBorrowPrepareError::Unsupported("str 持有者須立即初始化"),
        )?;
        let text = private_str_literal(init, &verified.tokens)
            .map_err(|_| PrivateCallBorrowPrepareError::Unsupported(
                "str 只接受真正 Lexer 解碼的獨立常值",
            ))?;
        let StmtKind::Return(caller_return) = &main.body.statements[main.body.statements.len()-1].kind else {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "主函式須在真正 str 所有者建立後執行呼叫並返回",
            ));
        };
        let TermKind::Call(call) = &caller_return.expression.kind else {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "只接通真正 Parser 的有參數普通函式呼叫",
            ));
        };
        if call.name != callee.name || call.arguments.len() != callee.params.len() {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "呼叫的函式身分或參數個數不符合原 Parser",
            ));
        }
        let arg = &call.arguments[str_position];
        let TermKind::Name(arg_name) = &arg.kind else {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "非 Copyable 參數只支援已驗證具名來源的預設借用",
            ));
        };
        if arg_name != &owner.name || !verified.tokens.iter().any(|token|
            token.start == arg.start && token.end == arg.end
                && matches!(&token.kind, TokenKind::Name(name) if name == arg_name)
        ) {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "參數詞彙必須指向原 Lexer 真正的同一持有者",
            ));
        }
        let call_span = caller_return.expression.node_span();
        if verified.plan.scalar_hir.expression(call_span)
            .is_none_or(|fact| fact.ty != crate::common_ir::ScalarType::Int) {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "函式呼叫返回型別缺少正式 Checker int HIR",
            ));
        }
        let facts = verified.plan.declarations.call_borrows_at(call_span)
            .ok_or(PrivateCallBorrowPrepareError::Unsupported(
                "這次呼叫缺少正式 Checker 已簽出 call_borrows",
            ))?;
        let expected_kind = if owner.mutable {
            BorrowKind::ExclusiveWrite
        } else {
            BorrowKind::SharedRead
        };
        if facts.len() != 1 || facts[0] != BorrowFact::new(identity, expected_kind) {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "借用來源 DeclarationId／共享或獨占權限與真 Checker 不同",
            ));
        }
        let mut int_owner = None;
        let mut int_bindings = Vec::<(DeclarationId, i64)>::new();
        for (position, parameter) in callee.params.iter().enumerate() {
            if parameter.ty != "int" { continue; }
            let arg = &call.arguments[position];
            let value = match &arg.kind {
                TermKind::Scalar(
                    crate::syntax_shared::ScalarLiteral::Integer(raw),
                ) => {
                    if verified.plan.scalar_hir.expression(arg.node_span()).is_none_or(
                        |fact| fact.ty != crate::common_ir::ScalarType::Int,
                    ) || !verified.tokens.iter().any(|token|
                        token.start == arg.start && token.end == arg.end
                            && matches!(&token.kind, TokenKind::Integer(found) if found == raw)
                    ) {
                        return Err(PrivateCallBorrowPrepareError::Unsupported(
                            "混合多參數 int 字面值缺少原 Checker／Lexer 型別身分",
                        ));
                    }
                    crate::syntax_shared::integer_i64(raw).map_err(|_|
                        PrivateCallBorrowPrepareError::Unsupported(
                            "混合多參數 int 字面值超出 i64 範圍",
                        ),
                    )?
                }
                TermKind::Name(name) if main.body.statements.len() == 3 => {
                    let int_statement = &main.body.statements[1];
                    let StmtKind::Declare(binding) = &int_statement.kind else {
                        return Err(PrivateCallBorrowPrepareError::Unsupported(
                            "具名 Copyable int 實參須由真局部宣告建立",
                        ));
                    };
                    let id = DeclarationId(int_statement.start);
                    let initializer = binding.initializer.as_ref().ok_or(
                        PrivateCallBorrowPrepareError::Unsupported(
                            "混合 int 具名來源必須於真聲明初始化",
                        ),
                    )?;
                    let TermKind::Scalar(
                        crate::syntax_shared::ScalarLiteral::Integer(raw),
                    ) = &initializer.kind else {
                        return Err(PrivateCallBorrowPrepareError::Unsupported(
                            "具名 int 來源須有原 Checker 已驗證常值初始化",
                        ));
                    };
                    if name != &binding.name || binding.name == owner.name
                        || binding.ty != "int"
                        || !verified.plan.declarations.matches(id, "int", binding.mutable)
                        || verified.plan.scalar_hir.reference(arg.node_span()).is_none_or(
                            |fact| fact.resolved.declaration != id
                                || fact.resolved.ty != crate::common_ir::ScalarType::Int,
                        )
                        || verified.plan.scalar_hir.expression(initializer.node_span()).is_none_or(
                            |fact| fact.ty != crate::common_ir::ScalarType::Int,
                        )
                        || !verified.tokens.iter().any(|token|
                            token.start == arg.start && token.end == arg.end
                                && matches!(&token.kind, TokenKind::Name(found) if found == name)
                        )
                        || !verified.tokens.iter().any(|token|
                            token.start == initializer.start && token.end == initializer.end
                                && matches!(&token.kind, TokenKind::Integer(found) if found == raw)
                        ) {
                        return Err(PrivateCallBorrowPrepareError::Unsupported(
                            "具名 int 實參未對應原 Checker DeclarationId／Reference HIR",
                        ));
                    }
                    let value = crate::syntax_shared::integer_i64(raw).map_err(|_|
                        PrivateCallBorrowPrepareError::Unsupported(
                            "具名 int 初始化超出 i64 範圍",
                        ),
                    )?;
                    if int_owner.is_some_and(|old| old != (id, value)) {
                        return Err(PrivateCallBorrowPrepareError::Unsupported(
                            "此切片禁止忽略另一個尚未執行的具名 int 宣告",
                        ));
                    }
                    int_owner = Some((id, value));
                    value
                }
                _ => return Err(PrivateCallBorrowPrepareError::Unsupported(
                    "混合 Copyable int 僅接受已驗證字面或當次具名來源",
                )),
            };
            int_bindings.push((DeclarationId(parameter.declaration_start), value));
        }
        if main.body.statements.len() == 3 && int_owner.is_none() {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "混合呼叫不能略過 main 已宣告而未使用的 int 持有者",
            ));
        }
        let int_binding = int_bindings.first().copied();
        let additional_int_bindings = int_bindings[usize::from(int_binding.is_some())..].to_vec();
        let StmtKind::Return(callee_return) = &callee.body.statements[0].kind else {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "受限被呼叫函式須直接 return 合法 int 常值",
            ));
        };
        let (returned, mixed_return) = if int_binding.is_some() {
            let expression = PrivateScalarReturnExpr::from_checked(
                &callee_return.expression, &callee.params, &verified,
            ).map_err(|_| PrivateCallBorrowPrepareError::Unsupported(
                "混合函式返回未經原 Checker 完整 int TypedHIR 驗證",
            ))?;
            let mut nested_calls = Vec::new();
            expression.collect_calls(&mut nested_calls);
            if !nested_calls.is_empty() {
                return Err(PrivateCallBorrowPrepareError::Unsupported(
                    "本混合借用子集不接通尚未驗證的巢狀再次借用呼叫",
                ));
            }
            (0, Some(expression))
        } else {
            let TermKind::Scalar(
                crate::syntax_shared::ScalarLiteral::Integer(raw),
            ) = &callee_return.expression.kind else {
                return Err(PrivateCallBorrowPrepareError::Unsupported(
                    "單一 str 借用的舊子集仍要求原 Checker int 常值 return",
                ));
            };
            if verified.plan.scalar_hir.expression(callee_return.expression.node_span())
                .is_none_or(|fact| fact.ty != crate::common_ir::ScalarType::Int) {
                return Err(PrivateCallBorrowPrepareError::Unsupported(
                    "被呼叫者返回值缺少正式 Checker int TypedHIR",
                ));
            }
            (crate::syntax_shared::integer_i64(raw).map_err(|_|
                PrivateCallBorrowPrepareError::Unsupported("返回常值不是合法 i64"),
            )?, None)
        };
        let owner_name = owner.name.clone();
        let mutable = owner.mutable;
        let callee_name = callee.name.clone();
        let callee_span = crate::common_ir::NodeSpan::new(callee.start, callee.end);
        let loan = facts[0];
        let original = verified.source.bytes.clone().into_boxed_slice();
        Ok(Self {
            verified, original, owner: owner_name, owner_id: identity,
            mutable, text, callee_name, callee_span, call_span, loan, returned,
            str_position, int_binding, additional_int_bindings,
            int_owner, mixed_return,
        })
    }

    fn matches_source(&self, source: &str) -> bool {
        let Ok(fresh) = Self::prepare(source) else { return false };
        self.original == fresh.original
            && self.owner == fresh.owner && self.owner_id == fresh.owner_id
            && self.mutable == fresh.mutable && self.text == fresh.text
            && self.callee_name == fresh.callee_name
            && self.callee_span == fresh.callee_span
            && self.call_span == fresh.call_span
            && self.loan == fresh.loan && self.returned == fresh.returned
            && self.str_position == fresh.str_position
            && self.int_binding == fresh.int_binding
            && self.additional_int_bindings == fresh.additional_int_bindings
            && self.int_owner == fresh.int_owner
            && self.mixed_return == fresh.mixed_return
            && self.verified.plan.declarations.call_borrows
                == fresh.verified.plan.declarations.call_borrows
            && self.verified.plan.declarations.by_source
                == fresh.verified.plan.declarations.by_source
    }

    fn execute(&self)
        -> Result<PrivateCallBorrowOutcome, PrivateCallBorrowPrepareError> {
        self.execute_with_fault(false)
    }

    /// 僅在已簽發真 BorrowTicket 且尚未歸還的函式範圍中注入錯誤；
    /// 只供 Rust 測試，絕不變成 voiles 例外／取消語意。
    fn execute_with_fault(&self, fail_inside_callee: bool)
        -> Result<PrivateCallBorrowOutcome, PrivateCallBorrowPrepareError> {
        let Ok(source) = std::str::from_utf8(&self.verified.source.bytes) else {
            return Err(PrivateCallBorrowPrepareError::SourceChanged);
        };
        if self.original.as_ref() != self.verified.source.bytes.as_slice()
            || !self.verified.plan.is_consistent(
                &self.verified.structs, &self.verified.functions, source,
            ) || !self.matches_source(source) {
            return Err(PrivateCallBorrowPrepareError::SourceChanged);
        }
        use crate::borrow_safety::BorrowSafety;
        use crate::owned_str_runtime::OwnedStrRuntime;
        let mut runtime = OwnedStrRuntime::new();
        let mut tracker = BorrowSafety::new();
        let mut tickets = Vec::new();
        let mut original_facts = Vec::new();
        let mut observed_utf8_bytes = None;
        let mut call_entered = false;
        let mut call_exited = false;
        let mut frame = BTreeMap::<DeclarationId, i64>::new();
        let mut observed_integer = None;
        let execution = (|| -> Result<i64, PrivateCallBorrowRuntimeError> {
            runtime.declare(&self.owner, self.mutable, &self.text)
                .map_err(PrivateCallBorrowRuntimeError::Resource)?;
            let ticket = tracker.acquire(self.loan.source, self.loan.kind)
                .map_err(PrivateCallBorrowRuntimeError::Ticket)?;
            tickets.push(ticket);
            original_facts.push(self.loan);
            tracker.verify_static_snapshot(original_facts.iter().copied())
                .map_err(PrivateCallBorrowRuntimeError::Ticket)?;
            match tracker.check_direct_access(self.owner_id) {
                Err(crate::borrow_safety::BorrowSafetyError::SourceIsBorrowed) => {}
                Err(other) => return Err(PrivateCallBorrowRuntimeError::Ticket(other)),
                Ok(()) => return Err(PrivateCallBorrowRuntimeError::Ticket(
                    crate::borrow_safety::BorrowSafetyError::SnapshotMismatch,
                )),
            }
            // 先由真票據的 tracker／序號／種類／來源授權，再觸及
            // OwnedStrRuntime 唯一真 UTF-8；錯誤時不產生讀取事件、
            // 不繞過借用、也不靠 debug assert 當安全條件。
            observed_utf8_bytes = Some(read_checked_str_argument(
                &tracker, ticket, self.owner_id, &runtime, &self.owner,
            )?);
            call_entered = true;
            for (index, (id, value)) in self.int_binding.into_iter()
                .chain(self.additional_int_bindings.iter().copied()).enumerate() {
                if frame.insert(id, value).is_some() {
                    return Err(PrivateCallBorrowRuntimeError::StateMismatch);
                }
                if index == 0 { observed_integer = Some(value); }
            }
            if fail_inside_callee {
                return Err(PrivateCallBorrowRuntimeError::InjectedFailure);
            }
            let value = if let Some(expression) = &self.mixed_return {
                expression.evaluate(&frame).map_err(|error| match error {
                    PrivateScalarCallRuntimeError::IntegerOverflow =>
                        PrivateCallBorrowRuntimeError::IntegerOverflow,
                    _ => PrivateCallBorrowRuntimeError::StateMismatch,
                })?
            } else { self.returned };
            call_exited = true;
            Ok(value)
        })();
        // 真函式參數槽與本次動態 BorrowTicket 同屬這次呼叫；
        // 正常 return／算術溢位／故障都先清空 Copyable slot 再還票據。
        frame.clear();
        // 不論函式正常 return 或於真借用存活時內部測試故障，
        // 都先依**原 Checker** call_borrows 快照把這次真票據
        // 完整原子歸還，絕不把票據當作資源所有權轉移。
        let released = tracker.finish_scope_verified(&tickets, &original_facts)
            .map_err(PrivateCallBorrowRuntimeError::Ticket);
        let execution = match (execution, released) {
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Ok(value), Ok(())) => tracker.check_direct_access(self.owner_id)
                .map_err(PrivateCallBorrowRuntimeError::Ticket).map(|()| value),
        };
        let events = runtime.finish().map_err(PrivateCallBorrowPrepareError::Cleanup)?;
        if !crate::owned_str_runtime::event_trace_balanced(&events) {
            return Err(PrivateCallBorrowPrepareError::SourceChanged);
        }
        Ok(PrivateCallBorrowOutcome {
            result: execution, observed_utf8_bytes, call_entered, call_exited, events,
            parameter_slots_after_exit: frame.len(), observed_integer,
        })
    }
}

#[cfg(test)]
mod g18_private_call_borrow_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent;

    fn checked(source: &str) -> PreparedPrivateCallBorrow {
        PreparedPrivateCallBorrow::prepare(source)
            .expect("正式 Checker 對合法函式 str 借用應簽發來源事實")
    }

    #[test]
    fn g18_d_正式_checker_const_str_參數共享借用真票據與返回值() {
        let source = "fn measure(item:str)->int{\nreturn 7\n}\nfn main()->int{\nconst owner:str = \"繁體🪼\"\nreturn measure(owner)\n}\n";
        let plan = checked(source);
        for _ in 0..2 {
            let result = plan.execute().unwrap();
            assert_eq!(result.result, Ok(7));
            assert_eq!(result.observed_utf8_bytes, Some("繁體🪼".len()));
            assert!(result.call_entered && result.call_exited);
            assert_eq!(result.events, [
                ResourceEvent::Created(1), ResourceEvent::Released(1),
            ]);
        }
    }

    #[test]
    fn g18_d_正式_checker_var_str_參數獨占借用不複製源_resource_id() {
        let source = "fn measure(item:str)->int{\nreturn 5\n}\nfn main()->int{\nvar owner:str = \"唯一\"\nreturn measure(owner)\n}\n";
        let result = checked(source).execute().unwrap();
        assert_eq!(result.result, Ok(5));
        assert_eq!(result.observed_utf8_bytes, Some("唯一".len()));
        assert_eq!(result.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_c_被呼叫函式真借用存活時故障必先歸還票據再釋放擁有者() {
        let source = "fn measure(item:str)->int{\nreturn 7\n}\nfn main()->int{\nvar owner:str = \"唯一\"\nreturn measure(owner)\n}\n";
        let result = checked(source).execute_with_fault(true).unwrap();
        assert_eq!(result.result, Err(PrivateCallBorrowRuntimeError::InjectedFailure));
        assert!(result.call_entered);
        assert!(!result.call_exited);
        assert_eq!(result.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_d_錯誤實參型別及缺少參數在正式_checker_先拒絕() {
        for source in [
            "fn measure(item:str)->int{\nreturn 1\n}\nfn main()->int{\nvar owner:int = 3\nreturn measure(owner)\n}\n",
            "fn measure(item:str)->int{\nreturn 1\n}\nfn main()->int{\nconst owner:str = \"甲\"\nreturn measure()\n}\n",
        ] {
            assert!(matches!(
                PreparedPrivateCallBorrow::prepare(source),
                Err(PrivateCallBorrowPrepareError::Check(_)),
            ));
        }
    }

    #[test]
    fn g18_d_來源借用別名再作函式實參須由_checker_保守拒絕() {
        let source = "fn measure(item:str)->int{\nreturn 2\n}\nfn main()->int{\nconst owner:str = \"甲\"\nconst alias:str = owner.borrow()\nreturn measure(alias)\n}\n";
        assert!(matches!(
            PreparedPrivateCallBorrow::prepare(source),
            Err(PrivateCallBorrowPrepareError::Check(_)),
        ));
    }

    #[test]
    fn g18_d_偽造函式借用的_source_或_Kind_在資源建立前拒絕() {
        let source = "fn measure(item:str)->int{\nreturn 4\n}\nfn main()->int{\nconst owner:str = \"甲\"\nreturn measure(owner)\n}\n";
        let mut plan = checked(source);
        plan.loan.source = DeclarationId(0);
        assert!(matches!(plan.execute(), Err(PrivateCallBorrowPrepareError::SourceChanged)));
        let mut plan = checked(source);
        plan.loan.kind = BorrowKind::ExclusiveWrite;
        assert!(matches!(plan.execute(), Err(PrivateCallBorrowPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_d_刪除原_checker_已簽發的_call_borrows_不可沿用舊票據() {
        let source = "fn measure(item:str)->int{\nreturn 4\n}\nfn main()->int{\nconst owner:str = \"甲\"\nreturn measure(owner)\n}\n";
        let mut plan = checked(source);
        plan.verified.plan.declarations.call_borrows.clear();
        assert!(matches!(plan.execute(), Err(PrivateCallBorrowPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_d_函式本體返回值或來源_utf8_被改寫必在資源前拒絕() {
        let source = "fn measure(item:str)->int{\nreturn 4\n}\nfn main()->int{\nconst owner:str = \"甲\"\nreturn measure(owner)\n}\n";
        let mut plan = checked(source);
        plan.returned = 8;
        assert!(matches!(plan.execute(), Err(PrivateCallBorrowPrepareError::SourceChanged)));
        let mut plan = checked(source);
        plan.verified.source.bytes[0] = b'x';
        assert!(matches!(plan.execute(), Err(PrivateCallBorrowPrepareError::SourceChanged)));
    }
    #[test]
    fn g18_d_函式借用讀取先驗證動態票據_歸還後禁止重用() {
        use crate::borrow_safety::BorrowSafety;
        use crate::owned_str_runtime::OwnedStrRuntime;
        let owner = DeclarationId(3001);
        let mut runtime = OwnedStrRuntime::new();
        runtime.declare("source", false, "正體🪼").unwrap();
        let mut tracker = BorrowSafety::new();
        let ticket = tracker.acquire(owner, BorrowKind::SharedRead).unwrap();
        assert_eq!(read_checked_str_argument(
            &tracker, ticket, owner, &runtime, "source",
        ), Ok("正體🪼".len()));
        assert_eq!(tracker.check_direct_access(owner),
            Err(crate::borrow_safety::BorrowSafetyError::SourceIsBorrowed));
        tracker.release(ticket).unwrap();
        assert_eq!(read_checked_str_argument(
            &tracker, ticket, owner, &runtime, "source",
        ), Err(PrivateCallBorrowRuntimeError::Ticket(
            crate::borrow_safety::BorrowSafetyError::InvalidRelease,
        )));
        assert_eq!(runtime.finish().unwrap(), [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_d_函式實參的真借用票據不能跨動態_tracker_存取() {
        use crate::borrow_safety::BorrowSafety;
        use crate::owned_str_runtime::OwnedStrRuntime;
        let id = DeclarationId(3002);
        let mut runtime = OwnedStrRuntime::new();
        runtime.declare("source", true, "唯一").unwrap();
        let mut first = BorrowSafety::new();
        let mut other = BorrowSafety::new();
        let ticket = first.acquire(id, BorrowKind::ExclusiveWrite).unwrap();
        assert_eq!(read_checked_str_argument(&other, ticket, id, &runtime, "source"),
            Err(PrivateCallBorrowRuntimeError::Ticket(
                crate::borrow_safety::BorrowSafetyError::InvalidRelease,
            )));
        assert_eq!(read_checked_str_argument(&first, ticket, id, &runtime, "source"),
            Ok("唯一".len()));
        first.release(ticket).unwrap();
        assert_eq!(other.check_direct_access(id), Ok(()));
        assert_eq!(runtime.finish().unwrap(), [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_d_同追蹤器另一持有者的有效票據不得假扮此函式參數() {
        use crate::borrow_safety::BorrowSafety;
        use crate::owned_str_runtime::OwnedStrRuntime;
        let true_owner = DeclarationId(3003);
        let different = DeclarationId(3004);
        let mut runtime = OwnedStrRuntime::new();
        runtime.declare("source", false, "甲🪼").unwrap();
        let mut tracker = BorrowSafety::new();
        let wrong = tracker.acquire(different, BorrowKind::SharedRead).unwrap();
        assert_eq!(read_checked_str_argument(
            &tracker, wrong, true_owner, &runtime, "source",
        ), Err(PrivateCallBorrowRuntimeError::Ticket(
            crate::borrow_safety::BorrowSafetyError::SnapshotMismatch,
        )));
        assert_eq!(tracker.check_direct_access(true_owner), Ok(()));
        assert_eq!(runtime.finish().unwrap(), [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_d_函式共享借用不提升獨占權限且不複製_str_資源() {
        use crate::borrow_safety::BorrowSafety;
        use crate::owned_str_runtime::OwnedStrRuntime;
        let id = DeclarationId(3005);
        let mut runtime = OwnedStrRuntime::new();
        runtime.declare("source", false, "甲").unwrap();
        let mut tracker = BorrowSafety::new();
        let ticket = tracker.acquire(id, BorrowKind::SharedRead).unwrap();
        assert_eq!(tracker.check_ticket_access(ticket, BorrowKind::ExclusiveWrite),
            Err(crate::borrow_safety::BorrowSafetyError::BorrowConflict));
        assert_eq!(read_checked_str_argument(
            &tracker, ticket, id, &runtime, "source",
        ), Ok("甲".len()));
        tracker.release(ticket).unwrap();
        assert_eq!(runtime.finish().unwrap(), [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }
    // G18/P18.4：同一次普通函式真有非 Copyable str 借用與 Copyable
    // int 傳參，必須沿原 Parser 參數順序共同驗證與動態離域。
    fn mixed_forward() -> &'static str {
        "fn measure(payload:str, n:int)->int{\nreturn n+5\n}\nfn main()->int{\nconst owner:str = \"正體🪼\"\nreturn measure(owner,7)\n}\n"
    }

    #[test]
    fn g18_d_混合_const_str_與_copyable_int_同框架真借用及_hir_加法返回() {
        let prepared = checked(mixed_forward());
        let result = prepared.execute().unwrap();
        assert_eq!(result.result, Ok(12));
        assert_eq!(result.observed_utf8_bytes, Some("正體🪼".len()));
        assert_eq!(result.observed_integer, Some(7));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert!(result.call_entered && result.call_exited);
        assert_eq!(result.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_d_混合_int_在前_var_str_在後_仍以真獨占票據與參數身分執行() {
        let src = "fn measure(n:int, payload:str)->int{\nreturn n-4\n}\nfn main()->int{\nvar owner:str = \"唯一\"\nreturn measure(10, owner)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.str_position, 1);
        assert_eq!(prepared.loan.kind, BorrowKind::ExclusiveWrite);
        let result = prepared.execute().unwrap();
        assert_eq!(result.result, Ok(6));
        assert_eq!(result.observed_integer, Some(10));
        assert_eq!(result.observed_utf8_bytes, Some("唯一".len()));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert_eq!(result.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_d_混合函式重複呼叫不消耗_str_持有者也不重用上次_int_參數框架() {
        let prepared = checked(mixed_forward());
        for _ in 0..3 {
            let result = prepared.execute().unwrap();
            assert_eq!(result.result, Ok(12));
            assert_eq!(result.parameter_slots_after_exit, 0);
            assert_eq!(result.events, [
                ResourceEvent::Created(1), ResourceEvent::Released(1),
            ]);
        }
    }

    #[test]
    fn g18_c_混合借用期間_int_加法溢位須歸還票據及真_utf8_資源() {
        let src = "fn measure(payload:str,n:int)->int{\nreturn n+1\n}\nfn main()->int{\nvar owner:str = \"唯一\"\nreturn measure(owner,9223372036854775807)\n}\n";
        let result = checked(src).execute().unwrap();
        assert_eq!(result.result, Err(PrivateCallBorrowRuntimeError::IntegerOverflow));
        assert!(result.call_entered && !result.call_exited);
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert_eq!(result.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_c_混合函式真借用與_int_參數已綁定後故障仍原子歸還清理() {
        let result = checked(mixed_forward()).execute_with_fault(true).unwrap();
        assert_eq!(result.result, Err(PrivateCallBorrowRuntimeError::InjectedFailure));
        assert!(result.call_entered && !result.call_exited);
        assert_eq!(result.observed_integer, Some(7));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert_eq!(result.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_d_篡改混合函式_int_值與_DeclarationId_或_hir_返回必須配置前拒絕() {
        let mut prepared = checked(mixed_forward());
        prepared.int_binding.as_mut().unwrap().1 = 999;
        assert!(matches!(prepared.execute(),
            Err(PrivateCallBorrowPrepareError::SourceChanged)));
        let mut prepared = checked(mixed_forward());
        prepared.int_binding.as_mut().unwrap().0 = DeclarationId(0);
        assert!(matches!(prepared.execute(),
            Err(PrivateCallBorrowPrepareError::SourceChanged)));
        let mut prepared = checked(mixed_forward());
        prepared.mixed_return = Some(PrivateScalarReturnExpr::Constant(100));
        assert!(matches!(prepared.execute(),
            Err(PrivateCallBorrowPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_d_混合函式刪除原_checker_真借用表不得使用舊參數票據() {
        let mut prepared = checked(mixed_forward());
        prepared.verified.plan.declarations.call_borrows.clear();
        assert!(matches!(prepared.execute(),
            Err(PrivateCallBorrowPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_d_混合呼叫錯誤型別個數與多餘未執行宣告必保守拒絕() {
        for src in [
            "fn measure(payload:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"甲\"\nreturn measure(owner,true)\n}\n",
            "fn measure(payload:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"甲\"\nreturn measure(owner)\n}\n",
            "fn measure(payload:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"甲\"\nreturn measure(owner,2,3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateCallBorrow::prepare(src),
                Err(PrivateCallBorrowPrepareError::Check(_))), "{src}");
        }
        let unused = "fn measure(payload:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"甲\"\nconst n:int = 3\nreturn measure(owner,5)\n}\n";
        assert!(matches!(PreparedPrivateCallBorrow::prepare(unused),
            Err(PrivateCallBorrowPrepareError::Unsupported(_))));
    }

    // P18.4：真正 Copyable int 名稱從 main 的局部宣告讀取純值，
    // 與同一 call_borrows 所授權的 str 唯一所有者共存。
    fn named_mixed() -> &'static str {
        "fn measure(payload:str,n:int)->int{\nreturn n+5\n}\nfn main()->int{\nconst owner:str = \"正體🪼\"\nconst value:int = 7\nreturn measure(owner,value)\n}\n"
    }
    #[test]
    fn g18_d_具名_const_int_與_str_借用同框架按原_checker_引用身份返回() {
        let p = checked(named_mixed());
        assert!(p.int_owner.is_some());
        assert_ne!(p.int_owner.unwrap().0, p.int_binding.unwrap().0);
        assert_eq!(p.int_owner.unwrap().1, 7);
        let out = p.execute().unwrap();
        assert_eq!(out.result, Ok(12));
        assert_eq!(out.observed_utf8_bytes, Some("正體🪼".len()));
        assert_eq!(out.observed_integer, Some(7));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events, [ResourceEvent::Created(1), ResourceEvent::Released(1)]);
    }
    #[test]
    fn g18_d_具名_var_int_在前_var_str_在後_真獨占借用及複製求值() {
        let text = "fn measure(n:int,payload:str)->int{\nreturn n-4\n}\nfn main()->int{\nvar owner:str = \"唯一\"\nvar number:int = 10\nreturn measure(number,owner)\n}\n";
        let p = checked(text);
        assert_eq!(p.str_position, 1);
        assert_eq!(p.loan.kind, BorrowKind::ExclusiveWrite);
        assert!(p.int_owner.is_some());
        for _ in 0..2 {
            let out = p.execute().unwrap();
            assert_eq!(out.result, Ok(6));
            assert_eq!(out.observed_integer, Some(10));
            assert_eq!(out.parameter_slots_after_exit, 0);
            assert_eq!(out.events, [ResourceEvent::Created(1), ResourceEvent::Released(1)]);
        }
    }
    #[test]
    fn g18_c_具名_copyable_與_str_借用並存故障仍清空參數及歸還票據() {
        let out = checked(named_mixed()).execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateCallBorrowRuntimeError::InjectedFailure));
        assert!(out.call_entered && !out.call_exited);
        assert_eq!(out.observed_integer, Some(7));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events, [ResourceEvent::Created(1), ResourceEvent::Released(1)]);
    }
    #[test]
    fn g18_d_偽造具名_copyable_所有者_decl_id_或初始值須零資源拒絕() {
        let mut p = checked(named_mixed());
        p.int_owner.as_mut().unwrap().0 = DeclarationId(0);
        assert!(matches!(p.execute(), Err(PrivateCallBorrowPrepareError::SourceChanged)));
        let mut p = checked(named_mixed());
        p.int_owner.as_mut().unwrap().1 = 99;
        assert!(matches!(p.execute(), Err(PrivateCallBorrowPrepareError::SourceChanged)));
        let mut p = checked(named_mixed());
        p.int_binding.as_mut().unwrap().1 = 99;
        assert!(matches!(p.execute(), Err(PrivateCallBorrowPrepareError::SourceChanged)));
    }
    #[test]
    fn g18_d_具名_int_未初始化或實參型別不符由原_checker_先拒絕() {
        let uninitialized = named_mixed().replace("const value:int = 7", "var value:int");
        assert!(matches!(PreparedPrivateCallBorrow::prepare(&uninitialized),
            Err(PrivateCallBorrowPrepareError::Check(_))));
        let type_mismatch = named_mixed().replace(
            "const value:int = 7", "const value:bool = true",
        );
        assert!(matches!(PreparedPrivateCallBorrow::prepare(&type_mismatch),
            Err(PrivateCallBorrowPrepareError::Check(_))));
    }

    // P18.4：同一個真非 Copyable str 借用可與完整 Copyable int
    // 參數序列共存，不能因單參數模型而錯誤拒絕合法 Checker 來源。
    fn many_mixed() -> &'static str {
        "fn combine(payload:str,a:int,b:int,c:int,d:int)->int{\nreturn (a+b)+(c-d)\n}\nfn main()->int{\nconst owner:str = \"繁體🪼\"\nreturn combine(owner,7,9,5,2)\n}\n"
    }
    #[test]
    fn g18_d_唯一_str_借用與四個_copyable_int_同框架真算術返回() {
        let prepared = checked(many_mixed());
        assert_eq!(prepared.additional_int_bindings.len(), 3);
        assert_eq!(prepared.str_position, 0);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(19));
        assert_eq!(out.observed_utf8_bytes, Some("繁體🪼".len()));
        assert_eq!(out.observed_integer, Some(7));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert!(out.call_entered && out.call_exited);
        assert_eq!(out.events, [ResourceEvent::Created(1), ResourceEvent::Released(1)]);
    }

    #[test]
    fn g18_d_參數宣告順序為_int_str_int_int_仍以原_checker_身分真借用() {
        let source = "fn combine(a:int,payload:str,b:int,c:int)->int{\nreturn a+b-c\n}\nfn main()->int{\nvar owner:str = \"獨占\"\nreturn combine(9,owner,2,1)\n}\n";
        let prepared = checked(source);
        assert_eq!(prepared.str_position, 1);
        assert_eq!(prepared.loan.kind, BorrowKind::ExclusiveWrite);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(10));
        assert_eq!(out.observed_utf8_bytes, Some("獨占".len()));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events, [ResourceEvent::Created(1), ResourceEvent::Released(1)]);
    }

    #[test]
    fn g18_d_同一具名_copyable_int_重複使用不消耗_str_借用持有者() {
        let source = "fn combine(payload:str,a:int,b:int,c:int)->int{\nreturn a+b+c\n}\nfn main()->int{\nconst owner:str = \"正體\"\nconst number:int = 4\nreturn combine(owner,number,number,2)\n}\n";
        let prepared = checked(source);
        assert_eq!(prepared.int_owner.unwrap().1, 4);
        assert_eq!(prepared.additional_int_bindings.len(), 2);
        for _ in 0..3 {
            let out = prepared.execute().unwrap();
            assert_eq!(out.result, Ok(10));
            assert_eq!(out.observed_integer, Some(4));
            assert_eq!(out.parameter_slots_after_exit, 0);
            assert_eq!(out.events, [ResourceEvent::Created(1), ResourceEvent::Released(1)]);
        }
    }

    #[test]
    fn g18_c_多個_int_算術溢位須於真_str_borrowticket_歸還前中止成功返回() {
        let source = "fn combine(payload:str,a:int,b:int,c:int)->int{\nreturn a+b+c\n}\nfn main()->int{\nvar owner:str = \"獨占\"\nreturn combine(owner,9223372036854775807,1,1)\n}\n";
        let out = checked(source).execute().unwrap();
        assert_eq!(out.result, Err(PrivateCallBorrowRuntimeError::IntegerOverflow));
        assert!(out.call_entered && !out.call_exited);
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events, [ResourceEvent::Created(1), ResourceEvent::Released(1)]);
    }

    #[test]
    fn g18_c_混合四個_int_與_str_中途故障不得殘留任一動態參數或票據() {
        let out = checked(many_mixed()).execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateCallBorrowRuntimeError::InjectedFailure));
        assert!(out.call_entered && !out.call_exited);
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.observed_integer, Some(7));
        assert_eq!(out.events, [ResourceEvent::Created(1), ResourceEvent::Released(1)]);
    }

    #[test]
    fn g18_d_多個_copyable_int_後續參數被篡改不得建立任何_utf8_資源() {
        let mut p = checked(many_mixed());
        p.additional_int_bindings[1].0 = DeclarationId(0);
        assert!(matches!(p.execute(), Err(PrivateCallBorrowPrepareError::SourceChanged)));
        let mut p = checked(many_mixed());
        p.additional_int_bindings[2].1 = 5000;
        assert!(matches!(p.execute(), Err(PrivateCallBorrowPrepareError::SourceChanged)));
        let mut p = checked(many_mixed());
        p.verified.plan.declarations.call_borrows.clear();
        assert!(matches!(p.execute(), Err(PrivateCallBorrowPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_d_多參數型別錯誤與未使用具名_int_保持原_checker_拒絕() {
        let bad = many_mixed().replace("combine(owner,7,9,5,2)",
            "combine(owner,7,true,5,2)");
        assert!(matches!(PreparedPrivateCallBorrow::prepare(&bad),
            Err(PrivateCallBorrowPrepareError::Check(_))));
        let bad = many_mixed().replace("combine(owner,7,9,5,2)",
            "combine(owner,7,9,5)");
        assert!(matches!(PreparedPrivateCallBorrow::prepare(&bad),
            Err(PrivateCallBorrowPrepareError::Check(_))));
        let unused = many_mixed().replace(
            "return combine(owner,7,9,5,2)",
            "const extra:int = 6\nreturn combine(owner,7,9,5,2)",
        );
        assert!(matches!(PreparedPrivateCallBorrow::prepare(&unused),
            Err(PrivateCallBorrowPrepareError::Unsupported(_))));
    }

}


// G18／P18.3／18.4：原 Checker 明確整筆 .move() 跨函式呼叫的
// 真 OwnedStrRuntime 所有權交接。復用 P16 原資源與原靜態計畫，
// 不把 .move() 改寫成借用或偷偷複製 UTF-8；不修改公開 MVP-A。
#[derive(Debug)]
enum PrivateMoveCallPrepareError {
    Check(crate::Diagnostic),
    Unsupported(&'static str),
    SourceChanged,
    Cleanup(crate::owned_str_runtime::StrRuntimeError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateMoveCallRunError {
    CallBudget,
    StepBudget,
    InjectedFailure,
    MissingFunction,
    Resource(crate::owned_str_runtime::StrRuntimeError),
}

#[derive(Debug, Clone, Copy)]
struct PrivateMoveCallBudget {
    max_call_frames: usize,
    max_steps: usize,
}

impl Default for PrivateMoveCallBudget {
    fn default() -> Self { Self { max_call_frames: 128, max_steps: 10_000 } }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateMoveCallBody {
    /// 原 Checker 已核准的 return next(param.move())，全程使用
    /// 同一真 OwnedUtf8 ResourceId，而非按值複製或重新建立。
    Forward { callee: String },
    /// 可選的真局部整筆 move 綁定，再直接返回經驗證的 int。
    Return { into_local: Option<String>, value: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateMoveCallFunction {
    name: String,
    parameter: String,
    parameter_id: DeclarationId,
    body: PrivateMoveCallBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateMoveCallFrameEvent {
    Entered(String),
    Exited { name: String, returned: bool },
}

#[derive(Debug)]
struct PrivateMoveCallOutcome {
    result: Result<i64, PrivateMoveCallRunError>,
    events: Vec<crate::owned_str_runtime::ResourceEvent>,
    frames: Vec<PrivateMoveCallFrameEvent>,
    /// 實際進入的函式框架數，錯誤／返回後均已歸還作用域。
    slots_after_exit: usize,
}

struct PreparedPrivateMoveCall {
    verified: VerifiedPrivateSource,
    original: Box<[u8]>,
    owner_name: String,
    owner_id: DeclarationId,
    owner_mutable: bool,
    owner_text: String,
    entry: String,
    callees: Vec<PrivateMoveCallFunction>,
}

impl PreparedPrivateMoveCall {
    fn checked_move_name<'a>(
        term: &'a Term,
        verified: &VerifiedPrivateSource,
    ) -> Result<&'a str, PrivateMoveCallPrepareError> {
        let TermKind::Move(name) = &term.kind else {
            return Err(PrivateMoveCallPrepareError::Unsupported(
                "只有原 Parser 真 .move() 才可轉交非 Copyable 所有權",
            ));
        };
        if !verified.tokens.iter().any(|token|
            token.start == term.start
                && matches!(&token.kind, TokenKind::Name(found) if found == name)
        ) {
            return Err(PrivateMoveCallPrepareError::Unsupported(
                ".move() 缺少原 Lexer 的具名來源 token",
            ));
        }
        Ok(name)
    }

    fn checked_forward(
        term: &Term,
        verified: &VerifiedPrivateSource,
        owner: &str,
    ) -> Result<String, PrivateMoveCallPrepareError> {
        use crate::common_ir::ScalarType;
        let TermKind::Call(call) = &term.kind else {
            return Err(PrivateMoveCallPrepareError::Unsupported(
                "所有權移交呼叫必須由原 Parser Call AST 授權",
            ));
        };
        if call.arguments.len() != 1
            || Self::checked_move_name(&call.arguments[0], verified)? != owner
            || verified.plan.scalar_hir.expression(term.node_span())
                .is_none_or(|fact| fact.ty != ScalarType::Int)
            || verified.plan.declarations.call_borrows_at(term.node_span())
                .is_some_and(|facts| !facts.is_empty()) {
            return Err(PrivateMoveCallPrepareError::Unsupported(
                "函式整筆移動實參未經原 Checker／ScalarHIR 授權，或被誤當借用",
            ));
        }
        Ok(call.name.clone())
    }

    fn checked_int_return(
        term: &Term,
        verified: &VerifiedPrivateSource,
    ) -> Result<i64, PrivateMoveCallPrepareError> {
        let TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) =
            &term.kind else {
            return Err(PrivateMoveCallPrepareError::Unsupported(
                "本擁有權移交子集只接受原 Checker int 常值返回",
            ));
        };
        if verified.plan.scalar_hir.expression(term.node_span())
            .is_none_or(|fact| fact.ty != crate::common_ir::ScalarType::Int)
            || !verified.tokens.iter().any(|token|
                token.start == term.start && token.end == term.end
                    && matches!(&token.kind, TokenKind::Integer(found) if found == raw)
            ) {
            return Err(PrivateMoveCallPrepareError::Unsupported(
                "整筆移動後 int 返回缺少原 Checker／Lexer 授權",
            ));
        }
        crate::syntax_shared::integer_i64(raw).map_err(|_|
            PrivateMoveCallPrepareError::Unsupported(
                "整筆移動後 int 返回超出 i64",
            ),
        )
    }

    fn prepare(source: &str) -> Result<Self, PrivateMoveCallPrepareError> {
        use crate::common_ir::{NodeRole, VerifiedTypedMember};
        let verified = verify_source_with_ability(
            source, crate::ownership_policy::approved_leaf_copy_permission,
        ).map_err(PrivateMoveCallPrepareError::Check)?;
        if !verified.structs.is_empty() || verified.functions.len() < 2 {
            return Err(PrivateMoveCallPrepareError::Unsupported(
                "須有原 Checker 已核准 main 和至少一個普通函式",
            ));
        }
        let main = verified.functions.iter().find(|fun| fun.name == "main")
            .ok_or(PrivateMoveCallPrepareError::Unsupported(
                "缺少原 main",
            ))?;
        if main.result_ty != "int" || !main.params.is_empty()
            || main.body.statements.len() != 2 {
            return Err(PrivateMoveCallPrepareError::Unsupported(
                "本子集只接通一筆主函式 str 宣告與真正返回呼叫",
            ));
        }
        let owner_statement = &main.body.statements[0];
        let StmtKind::Declare(owner) = &owner_statement.kind else {
            return Err(PrivateMoveCallPrepareError::Unsupported(
                "必須先建立真正的 main str 唯一所有者",
            ));
        };
        let owner_id = DeclarationId(owner_statement.start);
        let Some(initializer) = owner.initializer.as_ref() else {
            return Err(PrivateMoveCallPrepareError::Unsupported(
                "main str 所有者必須有真正 Lexer 初始化來源",
            ));
        };
        let owner_text = private_str_literal(initializer, &verified.tokens)
            .map_err(|_| PrivateMoveCallPrepareError::Unsupported(
                "main str 所有者不是原 Lexer 合法 UTF-8 字面",
            ))?;
        if owner.ty != "str"
            || !verified.plan.declarations.matches(owner_id, "str", owner.mutable) {
            return Err(PrivateMoveCallPrepareError::Unsupported(
                "main 字串所有者缺少原 Checker DeclarationId",
            ));
        }
        let StmtKind::Return(ret) = &main.body.statements[1].kind else {
            return Err(PrivateMoveCallPrepareError::Unsupported(
                "main 須以真 int Call 返回",
            ));
        };
        let entry = Self::checked_forward(&ret.expression, &verified, &owner.name)?;
        let mut callees = Vec::new();
        for fun in verified.functions.iter().filter(|fun| fun.name != "main") {
            let [parameter] = fun.params.as_slice() else {
                return Err(PrivateMoveCallPrepareError::Unsupported(
                    "整筆移交子集須由單一 str 參數承接真所有權",
                ));
            };
            let parameter_id = DeclarationId(parameter.declaration_start);
            let member = VerifiedTypedMember::checked_private(
                source, NodeRole::Parameter, parameter.declaration_span,
                &parameter.name, parameter.name_span, &parameter.ty,
                parameter.type_span, false,
            ).ok_or(PrivateMoveCallPrepareError::Unsupported(
                "移動函式參數缺少原 Parser／Lexer 型別身分",
            ))?;
            if fun.result_ty != "int" || parameter.ty != "str"
                || !verified.plan.declarations.matches(
                    parameter_id, "str", false,
                ) || verified.plan.members.get(&member.identity) != Some(&member) {
                return Err(PrivateMoveCallPrepareError::Unsupported(
                    "移動函式參數缺少原 Checker TypedMember／DeclarationId",
                ));
            }
            let body = match fun.body.statements.as_slice() {
                [statement] => {
                    let StmtKind::Return(ret) = &statement.kind else {
                        return Err(PrivateMoveCallPrepareError::Unsupported(
                            "單一敘述必須是原 Checker return",
                        ));
                    };
                    match &ret.expression.kind {
                        TermKind::Call(_) => PrivateMoveCallBody::Forward {
                            callee: Self::checked_forward(
                                &ret.expression, &verified, &parameter.name,
                            )?,
                        },
                        _ => PrivateMoveCallBody::Return {
                            into_local: None,
                            value: Self::checked_int_return(&ret.expression, &verified)?,
                        },
                    }
                }
                [statement, last] => {
                    let StmtKind::Declare(local) = &statement.kind else {
                        return Err(PrivateMoveCallPrepareError::Unsupported(
                            "函式內整筆移動只接受真正具名局部宣告",
                        ));
                    };
                    let local_id = DeclarationId(statement.start);
                    let Some(initializer) = &local.initializer else {
                        return Err(PrivateMoveCallPrepareError::Unsupported(
                            "整筆移動的新所有者沒有初值",
                        ));
                    };
                    if local.ty != "str" || local.name == parameter.name
                        || !verified.plan.declarations.matches(
                            local_id, "str", local.mutable,
                        ) || Self::checked_move_name(initializer, &verified)?
                            != parameter.name {
                        return Err(PrivateMoveCallPrepareError::Unsupported(
                            "函式內 .move() 沒有原 Checker 具名來源及目的身分",
                        ));
                    }
                    let StmtKind::Return(ret) = &last.kind else {
                        return Err(PrivateMoveCallPrepareError::Unsupported(
                            "被呼叫者轉交後仍需真實 int return",
                        ));
                    };
                    PrivateMoveCallBody::Return {
                        into_local: Some(local.name.clone()),
                        value: Self::checked_int_return(&ret.expression, &verified)?,
                    }
                }
                _ => return Err(PrivateMoveCallPrepareError::Unsupported(
                    "本批只接通原 Checker 已驗證的直線整筆移動／轉呼叫",
                )),
            };
            callees.push(PrivateMoveCallFunction {
                name: fun.name.clone(), parameter: parameter.name.clone(),
                parameter_id, body,
            });
        }
        if !callees.iter().any(|fun| fun.name == entry)
            || callees.iter().any(|fun| match &fun.body {
                PrivateMoveCallBody::Forward { callee } =>
                    !callees.iter().any(|target| &target.name == callee),
                PrivateMoveCallBody::Return { .. } => false,
            }) {
            return Err(PrivateMoveCallPrepareError::Unsupported(
                "整筆所有權轉交圖缺少原 Checker 對應函式",
            ));
        }
        let original = verified.source.bytes.clone().into_boxed_slice();
        Ok(Self {
            verified, original, owner_name: owner.name.clone(),
            owner_id, owner_mutable: owner.mutable, owner_text, entry, callees,
        })
    }

    fn same_source(&self, source: &str) -> bool {
        let Ok(fresh) = Self::prepare(source) else { return false; };
        self.original == fresh.original
            && self.owner_name == fresh.owner_name
            && self.owner_id == fresh.owner_id
            && self.owner_mutable == fresh.owner_mutable
            && self.owner_text == fresh.owner_text
            && self.entry == fresh.entry
            && self.callees == fresh.callees
            && self.verified.plan.declarations.call_borrows
                == fresh.verified.plan.declarations.call_borrows
    }

    fn execute(
        &self, budget: PrivateMoveCallBudget,
    ) -> Result<PrivateMoveCallOutcome, PrivateMoveCallPrepareError> {
        self.execute_with_fault(budget, None)
    }

    /// 故障序號表示第 N 次已完成整筆 move 後測試注入失敗；
    /// Some(0) 則在 main 真 owner 建立後、尚未任何轉交前故障。
    fn execute_with_fault(
        &self, budget: PrivateMoveCallBudget, fault_after_moves: Option<usize>,
    ) -> Result<PrivateMoveCallOutcome, PrivateMoveCallPrepareError> {
        let Ok(source) = std::str::from_utf8(&self.verified.source.bytes) else {
            return Err(PrivateMoveCallPrepareError::SourceChanged);
        };
        if self.original.as_ref() != self.verified.source.bytes.as_slice()
            || !self.verified.plan.is_consistent(
                &self.verified.structs, &self.verified.functions, source,
            ) || !self.same_source(source) {
            return Err(PrivateMoveCallPrepareError::SourceChanged);
        }
        let mut resources = crate::owned_str_runtime::OwnedStrRuntime::new();
        let mut entered = Vec::<String>::new();
        let mut timeline = Vec::<PrivateMoveCallFrameEvent>::new();
        let mut steps = 0usize;
        let mut move_count = 0usize;
        let mut spend = || -> Result<(), PrivateMoveCallRunError> {
            if steps >= budget.max_steps {
                Err(PrivateMoveCallRunError::StepBudget)
            } else {
                steps += 1;
                Ok(())
            }
        };
        let result = (|| -> Result<i64, PrivateMoveCallRunError> {
            if budget.max_call_frames == 0 || budget.max_steps == 0 {
                return Err(if budget.max_call_frames == 0 {
                    PrivateMoveCallRunError::CallBudget
                } else {
                    PrivateMoveCallRunError::StepBudget
                });
            }
            spend()?;
            resources.declare(
                &self.owner_name, self.owner_mutable, &self.owner_text,
            ).map_err(PrivateMoveCallRunError::Resource)?;
            entered.push("main".to_owned());
            timeline.push(PrivateMoveCallFrameEvent::Entered("main".to_owned()));
            if fault_after_moves == Some(0) {
                return Err(PrivateMoveCallRunError::InjectedFailure);
            }
            let mut target = self.entry.as_str();
            let mut from = self.owner_name.as_str();
            loop {
                if entered.len() >= budget.max_call_frames {
                    return Err(PrivateMoveCallRunError::CallBudget);
                }
                spend()?;
                let callee = self.callees.iter().find(|fun| fun.name == target)
                    .ok_or(PrivateMoveCallRunError::MissingFunction)?;
                resources.enter_scope();
                entered.push(callee.name.clone());
                timeline.push(PrivateMoveCallFrameEvent::Entered(callee.name.clone()));
                resources.move_named(from, &callee.parameter)
                    .map_err(PrivateMoveCallRunError::Resource)?;
                move_count += 1;
                if fault_after_moves == Some(move_count) {
                    return Err(PrivateMoveCallRunError::InjectedFailure);
                }
                match &callee.body {
                    PrivateMoveCallBody::Forward { callee: next } => {
                        spend()?;
                        from = &callee.parameter;
                        target = next;
                    }
                    PrivateMoveCallBody::Return { into_local, value } => {
                        if let Some(local) = into_local {
                            spend()?;
                            resources.move_named(&callee.parameter, local)
                                .map_err(PrivateMoveCallRunError::Resource)?;
                            move_count += 1;
                            if fault_after_moves == Some(move_count) {
                                return Err(PrivateMoveCallRunError::InjectedFailure);
                            }
                        }
                        spend()?;
                        break Ok(*value);
                    }
                }
            }
        })();
        // 每個真正進入的子函式各有一層 P16 UTF-8 作用域。
        // 深層參數最後持有者必須先釋放；已轉交的父槽不釋放。
        while let Some(name) = entered.pop() {
            if name != "main" {
                resources.leave_scope().map_err(
                    PrivateMoveCallPrepareError::Cleanup,
                )?;
            }
            timeline.push(PrivateMoveCallFrameEvent::Exited {
                name, returned: result.is_ok(),
            });
        }
        let events = resources.finish().map_err(PrivateMoveCallPrepareError::Cleanup)?;
        if !crate::owned_str_runtime::event_trace_balanced(&events) {
            return Err(PrivateMoveCallPrepareError::SourceChanged);
        }
        Ok(PrivateMoveCallOutcome {
            result, events, frames: timeline, slots_after_exit: 0,
        })
    }
}

#[cfg(test)]
mod g18_private_owned_move_call_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent;

    fn checked(source: &str) -> PreparedPrivateMoveCall {
        PreparedPrivateMoveCall::prepare(source)
            .expect("原私有 Checker 應核准具名整筆 move 及函式實參轉交")
    }

    fn one_move() -> &'static str {
        "fn consume(payload:str)->int{\nreturn 7\n}\nfn main()->int{\nvar source:str = \"中文🪼\"\nreturn consume(source.move())\n}\n"
    }

    fn consumed_in_callee() -> &'static str {
        "fn consume(payload:str)->int{\nvar owned:str = payload.move()\nreturn 7\n}\nfn main()->int{\nvar source:str = \"中文🪼\"\nreturn consume(source.move())\n}\n"
    }

    fn nested_move() -> &'static str {
        "fn first(p:str)->int{\nreturn second(p.move())\n}\nfn second(q:str)->int{\nreturn third(q.move())\n}\nfn third(r:str)->int{\nvar final:str = r.move()\nreturn 9\n}\nfn main()->int{\nvar source:str = \"中文🪼\"\nreturn first(source.move())\n}\n"
    }

    fn recursive_move() -> &'static str {
        "fn repeat(p:str)->int{\nreturn repeat(p.move())\n}\nfn main()->int{\nvar source:str = \"逐層\"\nreturn repeat(source.move())\n}\n"
    }

    #[test]
    fn g18_d_原_checker_明確_move_普通函式實參轉交同一真_utf8_不複製() {
        let out = checked(one_move()).execute(PrivateMoveCallBudget::default()).unwrap();
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.events, [
            ResourceEvent::Created(1), ResourceEvent::Moved(1), ResourceEvent::Released(1),
        ]);
        assert_eq!(out.slots_after_exit, 0);
        assert_eq!(out.frames, [
            PrivateMoveCallFrameEvent::Entered("main".into()),
            PrivateMoveCallFrameEvent::Entered("consume".into()),
            PrivateMoveCallFrameEvent::Exited { name: "consume".into(), returned: true },
            PrivateMoveCallFrameEvent::Exited { name: "main".into(), returned: true },
        ]);
    }

    #[test]
    fn g18_d_被呼叫者真的再次_move_參數至局部唯一所有者退出才釋放一次() {
        let out = checked(consumed_in_callee())
            .execute(PrivateMoveCallBudget::default()).unwrap();
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.events, [
            ResourceEvent::Created(1), ResourceEvent::Moved(1),
            ResourceEvent::Moved(1), ResourceEvent::Released(1),
        ]);
        assert!(crate::owned_str_runtime::event_trace_balanced(&out.events));
        assert_eq!(out.slots_after_exit, 0);
    }

    #[test]
    fn g18_c_多函式逐層_move_同一_resourceid_末端真正釋放且返回逐框架退出() {
        let out = checked(nested_move())
            .execute(PrivateMoveCallBudget::default()).unwrap();
        assert_eq!(out.result, Ok(9));
        assert_eq!(out.events, [
            ResourceEvent::Created(1), ResourceEvent::Moved(1),
            ResourceEvent::Moved(1), ResourceEvent::Moved(1),
            ResourceEvent::Moved(1), ResourceEvent::Released(1),
        ]);
        assert_eq!(out.frames.iter().filter_map(|ev| match ev {
            PrivateMoveCallFrameEvent::Exited { name, returned: true } => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>(), ["third", "second", "first", "main"]);
    }

    #[test]
    fn g18_c_合法自我遞迴每層轉交唯一_utf8_受框架預算安全耗盡() {
        let out = checked(recursive_move()).execute(
            PrivateMoveCallBudget { max_call_frames: 5, max_steps: 80 },
        ).unwrap();
        assert_eq!(out.result, Err(PrivateMoveCallRunError::CallBudget));
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, ResourceEvent::Created(_))).count(), 1);
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, ResourceEvent::Moved(_))).count(), 4);
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, ResourceEvent::Released(_))).count(), 1);
        assert!(crate::owned_str_runtime::event_trace_balanced(&out.events));
        assert_eq!(out.frames.iter().filter(|e|
            matches!(e, PrivateMoveCallFrameEvent::Exited { returned: false, .. })).count(), 5);
    }

    #[test]
    fn g18_c_跨函式_move_步數預算耗盡不得假成功或雙重釋放() {
        let out = checked(nested_move()).execute(
            PrivateMoveCallBudget { max_call_frames: 10, max_steps: 4 },
        ).unwrap();
        assert_eq!(out.result, Err(PrivateMoveCallRunError::StepBudget));
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, ResourceEvent::Created(_))).count(), 1);
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, ResourceEvent::Released(_))).count(), 1);
        assert!(out.events.iter().any(|e| matches!(e, ResourceEvent::Moved(1))));
        assert_eq!(out.slots_after_exit, 0);
        assert!(out.frames.iter().all(|e| !matches!(e,
            PrivateMoveCallFrameEvent::Exited { returned: true, .. })));
    }

    #[test]
    fn g18_c_真移動之前及連續轉交之後注入故障都由目前所有者清理一次() {
        let prepared = checked(nested_move());
        for at in [0, 1, 2, 3, 4] {
            let out = prepared.execute_with_fault(
                PrivateMoveCallBudget::default(), Some(at),
            ).unwrap();
            assert_eq!(out.result, Err(PrivateMoveCallRunError::InjectedFailure));
            assert_eq!(out.events.iter().filter(|e|
                matches!(e, ResourceEvent::Moved(_))).count(), at);
            assert_eq!(out.events.iter().filter(|e|
                matches!(e, ResourceEvent::Created(_))).count(), 1);
            assert_eq!(out.events.iter().filter(|e|
                matches!(e, ResourceEvent::Released(_))).count(), 1);
            assert!(crate::owned_str_runtime::event_trace_balanced(&out.events));
            assert!(out.frames.iter().all(|e| !matches!(e,
                PrivateMoveCallFrameEvent::Exited { returned: true, .. })));
        }
    }

    #[test]
    fn g18_c_零函式框架或零步數預算在配置真_utf8_前受控拒絕() {
        let prepared = checked(one_move());
        for budget in [
            PrivateMoveCallBudget { max_call_frames: 0, max_steps: 30 },
            PrivateMoveCallBudget { max_call_frames: 3, max_steps: 0 },
        ] {
            let out = prepared.execute(budget).unwrap();
            assert!(out.result.is_err());
            assert!(out.events.is_empty() && out.frames.is_empty());
            assert_eq!(out.slots_after_exit, 0);
        }
    }

    #[test]
    fn g18_d_移動後原來源再次使用與漏掉明確_move_必由原_checker_拒絕() {
        for source in [
            consumed_in_callee().replace("consume(source.move())", "consume(source)"),
            consumed_in_callee().replace(
                "return consume(source.move())",
                "var another:str = source.move()\nreturn consume(source.move())",
            ),
            consumed_in_callee().replace(
                "var owned:str = payload.move()",
                "var owned:str = payload.move()\nvar again:str = payload.move()",
            ),
        ] {
            assert!(matches!(PreparedPrivateMoveCall::prepare(&source),
                Err(PrivateMoveCallPrepareError::Check(_))), "{source}");
        }
    }

    #[test]
    fn g18_d_非法借用別名不能以_move_變成真正所有者() {
        let src = "fn consume(payload:str)->int{\nvar owned:str = payload.move()\nreturn 7\n}\nfn main()->int{\nvar source:str = \"甲\"\nconst alias:str = source.borrow()\nreturn consume(alias.move())\n}\n";
        assert!(matches!(PreparedPrivateMoveCall::prepare(src),
            Err(PrivateMoveCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_d_更改已核准轉交函式或字串原值必須零執行前拒絕() {
        let mut p = checked(nested_move());
        p.callees[1].name = "偽造函式".into();
        assert!(matches!(p.execute(PrivateMoveCallBudget::default()),
            Err(PrivateMoveCallPrepareError::SourceChanged)));
        let mut p = checked(nested_move());
        p.callees[2].body = PrivateMoveCallBody::Return {
            into_local: Some("假所有者".into()), value: 9,
        };
        assert!(matches!(p.execute(PrivateMoveCallBudget::default()),
            Err(PrivateMoveCallPrepareError::SourceChanged)));
        let mut p = checked(nested_move());
        p.owner_text = "偽造字串".into();
        assert!(matches!(p.execute(PrivateMoveCallBudget::default()),
            Err(PrivateMoveCallPrepareError::SourceChanged)));
        assert_eq!(checked(nested_move())
            .execute(PrivateMoveCallBudget::default()).unwrap().result, Ok(9));
    }
}
