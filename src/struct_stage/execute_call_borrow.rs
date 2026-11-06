// G18／P18.1、P18.4：只接通正式私有 Checker 已通過的
// 一個 str 真持有者，搭配可選 Copyable int 參數 -> 原 Checker 普通函式真返回。
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
            || main.body.statements.len() != 2
            || callee.result_ty != "int"
            || !(1..=2).contains(&callee.params.len())
            || callee.body.statements.len() != 1 {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "只執行已核准一個 str 借用與可選第二 Copyable int 參數",
            ));
        }
        let str_position = callee.params.iter().position(|param| param.ty == "str")
            .ok_or(PrivateCallBorrowPrepareError::Unsupported(
                "缺少原 Parser 真 str 預設借用參數",
            ))?;
        let int_position = callee.params.iter().position(|param| param.ty == "int");
        if callee.params.len() == 2 && (int_position.is_none()
            || str_position == int_position.unwrap())
            || callee.params.len() == 1 && int_position.is_some() {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "目前真混合呼叫只接通 str 與可選一個 int 完整簽章",
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
        let StmtKind::Return(caller_return) = &main.body.statements[1].kind else {
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
        let int_binding = if let Some(position) = int_position {
            let arg = &call.arguments[position];
            let TermKind::Scalar(
                crate::syntax_shared::ScalarLiteral::Integer(raw),
            ) = &arg.kind else {
                return Err(PrivateCallBorrowPrepareError::Unsupported(
                    "真混合呼叫的 Copyable int 實參須為原 Lexer 已核准常值",
                ));
            };
            if verified.plan.scalar_hir.expression(arg.node_span()).is_none_or(
                |fact| fact.ty != crate::common_ir::ScalarType::Int,
            ) || !verified.tokens.iter().any(|token|
                token.start == arg.start && token.end == arg.end
                    && matches!(&token.kind, TokenKind::Integer(found) if found == raw)
            ) {
                return Err(PrivateCallBorrowPrepareError::Unsupported(
                    "混合呼叫 int 實參缺少原 Checker／Lexer TypedHIR 來源",
                ));
            }
            let value = crate::syntax_shared::integer_i64(raw).map_err(|_|
                PrivateCallBorrowPrepareError::Unsupported(
                    "混合呼叫 int 實參超過 i64 合法範圍",
                ),
            )?;
            Some((DeclarationId(callee.params[position].declaration_start), value))
        } else { None };
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
            str_position, int_binding, mixed_return,
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
            if let Some((id, value)) = self.int_binding {
                if frame.insert(id, value).is_some() {
                    return Err(PrivateCallBorrowRuntimeError::StateMismatch);
                }
                observed_integer = Some(value);
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
    fn g18_d_混合呼叫錯誤型別個數與具名非授權_int_實參必保守拒絕() {
        for src in [
            "fn measure(payload:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"甲\"\nreturn measure(owner,true)\n}\n",
            "fn measure(payload:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"甲\"\nreturn measure(owner)\n}\n",
            "fn measure(payload:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"甲\"\nreturn measure(owner,2,3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateCallBorrow::prepare(src),
                Err(PrivateCallBorrowPrepareError::Check(_))), "{src}");
        }
        let named = "fn measure(payload:str,n:int)->int{\nreturn n\n}\nfn main()->int{\nconst owner:str = \"甲\"\nconst n:int = 3\nreturn measure(owner,n)\n}\n";
        assert!(matches!(PreparedPrivateCallBorrow::prepare(named),
            Err(PrivateCallBorrowPrepareError::Unsupported(_))));
    }

}
