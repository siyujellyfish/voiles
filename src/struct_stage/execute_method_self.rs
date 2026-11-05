// G18／P18.4、P18.6：原私有 Checker 已核准的同步零參數唯讀
// struct 方法 self，真正 BorrowTicket 及 MixedStructRuntime Copyable int。
// 不擴張多載、async、自動複製非 Copyable 或公開 CLI。
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateSelfIntReturn {
    Field,
    Add(i64),
    Subtract(i64),
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
    method: String,
    method_span: crate::common_ir::NodeSpan,
    call_span: crate::common_ir::NodeSpan,
    loan: BorrowFact,
}

impl PreparedPrivateSelfMethod {
    fn prepare(text: &str) -> Result<Self, PrivateSelfMethodPrepareError> {
        use crate::common_ir::{NodeRole, NodeSpan, ScalarType, VerifiedTypedMember};
        let verified = verify_source_with_ability(
            text, crate::ownership_policy::approved_leaf_copy_permission,
        ).map_err(PrivateSelfMethodPrepareError::Check)?;
        if verified.structs.len() != 1 || verified.functions.len() != 1 {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "僅支援一個原 Checker 結構和其真正 main",
            ));
        }
        let definition = verified.structs.values().next().expect("已驗證只有一結構");
        if definition.fields.len() != 1 || definition.methods.len() != 1 {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "僅接通單一 Copyable int 欄位和零參數同步方法",
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
        let method = &definition.methods[0];
        if method.result_ty != "int" || !method.params.is_empty()
            || !(method.body.statements.len() == 1 || method.body.statements.len() == 2) {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "僅執行原 Checker 證明之同步零參數 int 方法",
            ));
        }
        // 兩語句模式先寫真 Copyable int 欄位，再直接讀取回傳；
        // 唯一是否可寫及 self 自動借用模式來自原 Checker MethodSafety。
        let write_value = if method.body.statements.len() == 2 {
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
        let last = method.body.statements.last().expect("方法已證明非空");
        let StmtKind::Return(returned) = &last.kind else {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "本期方法必須以已核准 return self.int欄位結束",
            ));
        };
        // 回傳允許原 Checker 已核准的 self.int 讀取，
        // 或 self.int 與一個真正 int 常值的 +／-。
        // 運算結構必須與原 TypedHIR Arithmetic 端點和運算子完全一致。
        let (return_calculation, receiver, read_field) = match &returned.expression.kind {
            TermKind::FieldRead(receiver, read_field) =>
                (PrivateSelfIntReturn::Field, receiver, read_field),
            TermKind::BinaryInt(binary) => {
                // 方法型別檢查使用原 Checker::method_pure_value_type，
                // 與一般函式的 ScalarHIR 簽發路徑不同，不能為了
                // 執行器擅自簽發另一種 P16 Arithmetic 證據。
                // 原 VerifiedMethodSafety.checked_returns 授權整筆
                // 方法返回；原 Parser AST 與來源區間確認真正 +／-。
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
                let TermKind::FieldRead(receiver, read_field) = &binary.left.kind else {
                    return Err(PrivateSelfMethodPrepareError::Unsupported(
                        "方法算術僅接通原 Checker self.int 為左運算元",
                    ));
                };
                let TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) =
                    &binary.right.kind else {
                    return Err(PrivateSelfMethodPrepareError::Unsupported(
                        "方法算術僅接通原 Checker int 常值為右運算元",
                    ));
                };
                if !verified.tokens.iter().any(|token|
                    token.start == binary.right.start && token.end == binary.right.end
                        && matches!(&token.kind, TokenKind::Integer(text) if text == raw)
                ) {
                    return Err(PrivateSelfMethodPrepareError::Unsupported(
                        "方法加減的整數 token 不屬於原 Parser／Checker 來源",
                    ));
                }
                let literal = crate::syntax_shared::integer_i64(raw).map_err(|_|
                    PrivateSelfMethodPrepareError::Unsupported("方法加減 int 常值超出 i64"),
                )?;
                let operation = if fact.operator == '+' {
                    PrivateSelfIntReturn::Add(literal)
                } else {
                    PrivateSelfIntReturn::Subtract(literal)
                };
                (operation, receiver, read_field)
            }
            _ => return Err(PrivateSelfMethodPrepareError::Unsupported(
                "方法返回僅接通原 Checker self.int 或其真加減算術",
            )),
        };
        let method_span = NodeSpan::new(method.start, method.end);
        let method_fact = verified.plan.method_safety.get(&method.start)
            .ok_or(PrivateSelfMethodPrepareError::Unsupported(
                "缺少正式 Checker 的 VerifiedMethodSafety",
            ))?;
        let required_kind = if write_value.is_some() {
            BorrowKind::ExclusiveWrite
        } else { BorrowKind::SharedRead };
        if receiver != "self" || read_field != &field.name
            || method_fact.span != method_span
            || method_fact.self_borrow != required_kind
            || !method_fact.checked_returns.contains(&last.node_span()) {
            return Err(PrivateSelfMethodPrepareError::Unsupported(
                "返回位置、self 共享模式與原 Checker 真方法證據不一致",
            ));
        }
        let main = &verified.functions[0];
        if main.name != "main" || main.result_ty != "int"
            || !main.params.is_empty() || main.body.statements.len() != 2 {
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
        let StmtKind::Return(call_return) = &main.body.statements[1].kind else {
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
        if receiver != &owner.name || called != &method.name || !args.is_empty()
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
        if write_value.is_some() && (!owner.mutable || field.immutable) {
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
            write_value, return_calculation, method: method_name, method_span, call_span, loan,
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
            && self.method == fresh.method && self.method_span == fresh.method_span
            && self.call_span == fresh.call_span && self.loan == fresh.loan
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
        let result = (|| -> Result<i64, PrivateSelfMethodRuntimeError> {
            let ticket = tracker.acquire(self.loan.source, self.loan.kind)
                .map_err(PrivateSelfMethodRuntimeError::Ticket)?;
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
            if fail_inside_method {
                return Err(PrivateSelfMethodRuntimeError::InjectedFailure);
            }
            // 寫入屬已核准之獨占 self：先驗真 BorrowTicket 身分、
            // 來源及權限，再寫真 MixedStructRuntime 的 Copyable int。
            // 共用方法無此路徑；唯一資源持有者與欄位可寫性皆受保護。
            if let Some(value) = self.write_value {
                let authorized = tracker.check_ticket_access(
                    ticket, BorrowKind::ExclusiveWrite,
                ).map_err(PrivateSelfMethodRuntimeError::Ticket)?;
                if authorized != self.owner_id {
                    return Err(PrivateSelfMethodRuntimeError::WrongReceiver);
                }
                runtime.write_int_field(
                    &self.owner, &self.field, value, self.owner_mutable,
                ).map_err(PrivateSelfMethodRuntimeError::Resource)?;
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
            let calculated = match self.return_calculation {
                PrivateSelfIntReturn::Field => value,
                PrivateSelfIntReturn::Add(rhs) => value.checked_add(rhs)
                    .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?,
                PrivateSelfIntReturn::Subtract(rhs) => value.checked_sub(rhs)
                    .ok_or(PrivateSelfMethodRuntimeError::IntegerOverflow)?,
            };
            method_exited = true;
            Ok(calculated)
        })();
        // 不論成功與否，先按原 Checker 動態 self BorrowFact 原子歸還，
        // 再釋放真正結構資源；失敗不可偽裝成功 return。
        let released = tracker.finish_scope_verified(&tickets, &facts)
            .map_err(PrivateSelfMethodRuntimeError::Ticket);
        let result = match (result, released) {
            (Err(problem), _) => Err(problem),
            (Ok(_), Err(problem)) => Err(problem),
            (Ok(value), Ok(())) => tracker.check_direct_access(self.owner_id)
                .map_err(PrivateSelfMethodRuntimeError::Ticket).map(|()| value),
        };
        let ticket_returned = tracker.check_direct_access(self.owner_id).is_ok();
        let events = runtime.finish().map_err(PrivateSelfMethodPrepareError::Cleanup)?;
        if !crate::owned_str_runtime::event_trace_balanced(&events) {
            return Err(PrivateSelfMethodPrepareError::SourceChanged);
        }
        Ok(PrivateSelfMethodOutcome {
            result, events, method_entered, method_exited, ticket_returned,
        })
    }
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
}
