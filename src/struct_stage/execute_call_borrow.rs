// G18／P18.1、P18.4：只接通正式私有 Checker 已通過的
// 單一 str 具名來源 -> 一參數普通函式預設借用 -> int 返回。
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
}

#[derive(Debug)]
struct PrivateCallBorrowOutcome {
    result: Result<i64, PrivateCallBorrowRuntimeError>,
    observed_utf8_bytes: Option<usize>,
    call_entered: bool,
    call_exited: bool,
    events: Vec<crate::owned_str_runtime::ResourceEvent>,
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
            || callee.result_ty != "int" || callee.params.len() != 1
            || callee.params[0].ty != "str" || callee.body.statements.len() != 1 {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "只接受原 Checker 已核准的一個 str 參數、常值整數返回",
            ));
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
        if call.name != callee.name || call.arguments.len() != 1 {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "呼叫的函式身分或參數個數不符合原 Parser",
            ));
        }
        let arg = &call.arguments[0];
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
        let StmtKind::Return(callee_return) = &callee.body.statements[0].kind else {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "受限被呼叫函式須直接 return 合法 int 常值",
            ));
        };
        let TermKind::Scalar(
            crate::syntax_shared::ScalarLiteral::Integer(raw),
        ) = &callee_return.expression.kind else {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "此切片不在未驗證的函式本體執行任意 expression",
            ));
        };
        if verified.plan.scalar_hir.expression(callee_return.expression.node_span())
            .is_none_or(|fact| fact.ty != crate::common_ir::ScalarType::Int) {
            return Err(PrivateCallBorrowPrepareError::Unsupported(
                "被呼叫者返回值缺少正式 Checker int TypedHIR",
            ));
        }
        let returned = crate::syntax_shared::integer_i64(raw).map_err(|_|
            PrivateCallBorrowPrepareError::Unsupported("返回常值不是合法 i64"),
        )?;
        let owner_name = owner.name.clone();
        let mutable = owner.mutable;
        let callee_name = callee.name.clone();
        let callee_span = crate::common_ir::NodeSpan::new(callee.start, callee.end);
        let loan = facts[0];
        let original = verified.source.bytes.clone().into_boxed_slice();
        Ok(Self {
            verified, original, owner: owner_name, owner_id: identity,
            mutable, text, callee_name, callee_span, call_span, loan, returned,
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
        let execution = (|| -> Result<i64, PrivateCallBorrowRuntimeError> {
            runtime.declare(&self.owner, self.mutable, &self.text)
                .map_err(PrivateCallBorrowRuntimeError::Resource)?;
            let ticket = tracker.acquire(self.loan.source, self.loan.kind)
                .map_err(PrivateCallBorrowRuntimeError::Ticket)?;
            tickets.push(ticket);
            original_facts.push(self.loan);
            tracker.verify_static_snapshot(original_facts.iter().copied())
                .map_err(PrivateCallBorrowRuntimeError::Ticket)?;
            tracker.check_direct_access(self.owner_id)
                .expect_err("真函式借用期間持有者直接操作必須被封鎖");
            // str 參數是已簽發票據的非擁有視圖。此處只讀真 UTF-8
            // 資源位元組長度；不作新的 String 克隆或虛構 Created。
            observed_utf8_bytes = Some(runtime.read(&self.owner)
                .map_err(PrivateCallBorrowRuntimeError::Resource)?.len());
            call_entered = true;
            if fail_inside_callee {
                return Err(PrivateCallBorrowRuntimeError::InjectedFailure);
            }
            let value = self.returned;
            call_exited = true;
            Ok(value)
        })();
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
}
