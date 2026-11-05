// G18／P18.4、18.6：S20 同名方法唯一靜態簽章之真正同步執行。
// 只接通原私有 Checker 已驗證的兩個一參數 Copyable int/bool
// 同名方法；借用權限、方法來源、返回值和錯誤清理皆以原 P16
// VerifiedMethodSafety／call_borrows_at 為權威，不新增公開入口。
#[derive(Debug)]
enum PrivateOverloadPrepareError {
    Check(crate::Diagnostic),
    Unsupported(&'static str),
    SourceChanged,
    Cleanup(crate::mixed_struct_runtime::MixedError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateOverloadRuntimeError {
    Ticket(crate::borrow_safety::BorrowSafetyError),
    Resource(crate::mixed_struct_runtime::MixedError),
    WrongBinding,
    InjectedFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateOverloadValue {
    Int(i64),
    Bool(bool),
}
impl PrivateOverloadValue {
    fn ty(self) -> &'static str {
        match self { Self::Int(_) => "int", Self::Bool(_) => "bool" }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateOverloadWrite {
    None,
    Parameter,
    Literal(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateOverloadEvent {
    ArgumentEvaluated(PrivateOverloadValue),
    MethodEntered(crate::common_ir::NodeSpan),
    ParameterBound(DeclarationId, PrivateOverloadValue),
    FieldWritten(i64),
    Returned(i64),
    MethodExited { returned: bool },
}

#[derive(Debug)]
struct PrivateOverloadOutcome {
    result: Result<i64, PrivateOverloadRuntimeError>,
    selected_method_span: crate::common_ir::NodeSpan,
    events: Vec<PrivateOverloadEvent>,
    resources: Vec<crate::owned_str_runtime::ResourceEvent>,
    parameter_slots_after_exit: usize,
    ticket_returned: bool,
}

struct PreparedPrivateOverload {
    verified: VerifiedPrivateSource,
    original: Box<[u8]>,
    struct_name: String,
    owner: String,
    owner_id: DeclarationId,
    owner_mutable: bool,
    field: String,
    field_value: i64,
    field_immutable: bool,
    call_span: crate::common_ir::NodeSpan,
    method_span: crate::common_ir::NodeSpan,
    parameter_id: DeclarationId,
    argument: PrivateOverloadValue,
    write: PrivateOverloadWrite,
    loan: BorrowFact,
}

impl PreparedPrivateOverload {
    fn prepare(text: &str) -> Result<Self, PrivateOverloadPrepareError> {
        use crate::common_ir::{
            NodeRole, NodeSpan, ScalarType, VerifiedTypedMember,
        };
        let verified = verify_source_with_ability(
            text, crate::ownership_policy::approved_leaf_copy_permission,
        ).map_err(PrivateOverloadPrepareError::Check)?;
        if verified.structs.len() != 1 || verified.functions.len() != 1 {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "只接受真 Checker 已驗證的一結構與無參數 main",
            ));
        }
        let (struct_name, definition) = verified.structs.iter().next().unwrap();
        if definition.fields.len() != 1 || definition.methods.len() != 2
            || definition.methods[0].name != definition.methods[1].name {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "此子集只接通 S20 兩個同名不同型別方法的唯一解析",
            ));
        }
        let field = &definition.fields[0];
        let field_member = VerifiedTypedMember::checked_private(
            text, NodeRole::Field, field.declaration_span,
            &field.name, field.name_span, &field.ty, field.type_span, !field.immutable,
        ).ok_or(PrivateOverloadPrepareError::Unsupported(
            "缺少真 Parser 欄位 Name／Type 來源跨度",
        ))?;
        if field.ty != "int" || verified.plan.members.get(&field_member.identity)
            != Some(&field_member) {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "欄位缺少原 P16 Checker int TypedMember",
            ));
        }
        let TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) =
            &field.default.as_ref().ok_or(PrivateOverloadPrepareError::Unsupported(
                "此安全子集需已核准 int 預設欄位",
            ))?.kind else {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "欄位預設值只允許原 Lexer 已核准 int 字面值",
            ));
        };
        let field_value = crate::syntax_shared::integer_i64(raw).map_err(|_|
            PrivateOverloadPrepareError::Unsupported("欄位整數超出 i64"),
        )?;
        let main = &verified.functions[0];
        if main.name != "main" || main.result_ty != "int"
            || !main.params.is_empty() || main.body.statements.len() != 2 {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "main 必須先建立唯一結構所有者後直接返回 S20 方法呼叫",
            ));
        }
        let StmtKind::Declare(owner) = &main.body.statements[0].kind else {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "main 首句缺少真結構持有者宣告",
            ));
        };
        let owner_id = DeclarationId(main.body.statements[0].start);
        if owner.ty != *struct_name
            || !verified.plan.declarations.matches(
                owner_id, struct_name, owner.mutable,
            ) {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "接收者不符合正式 Checker DeclarationId／型別",
            ));
        }
        let init = owner.initializer.as_ref().ok_or(
            PrivateOverloadPrepareError::Unsupported("結構須有已驗證建立式"),
        )?;
        if !matches!(&init.kind,
            TermKind::UnresolvedZeroArgs(name) if name == struct_name
        ) && !matches!(&init.kind,
            TermKind::Construct(name, args) if name == struct_name && args.is_empty()
        ) {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "此子集不允許隱含結構複製或未驗證欄位初始化",
            ));
        }
        let StmtKind::Return(returned) = &main.body.statements[1].kind else {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "主函式需直接返回方法執行結果",
            ));
        };
        let TermKind::MethodCall(receiver, name, args) = &returned.expression.kind else {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "回傳必須來自真正的 S16 instance.method(arg)",
            ));
        };
        if receiver != &owner.name || args.len() != 1
            || name != &definition.methods[0].name {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "方法呼叫或真接收者的原 Parser 身分不符合",
            ));
        }
        let call_span = returned.expression.node_span();
        if verified.plan.scalar_hir.expression(call_span)
            .is_none_or(|fact| fact.ty != ScalarType::Int) {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "方法呼叫缺少原 Checker int TypedHIR 返回型別",
            ));
        }
        let arg = &args[0];
        let argument = match &arg.kind {
            TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) => {
                if !verified.tokens.iter().any(|token|
                    token.start == arg.start && token.end == arg.end
                        && matches!(&token.kind, TokenKind::Integer(lexed) if lexed == raw)
                ) {
                    return Err(PrivateOverloadPrepareError::Unsupported(
                        "int 實參不符合原 Lexer 的來源記號",
                    ));
                }
                PrivateOverloadValue::Int(crate::syntax_shared::integer_i64(raw).map_err(|_|
                    PrivateOverloadPrepareError::Unsupported("int 實參超出 i64"),
                )?)
            }
            TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Boolean(value)) => {
                let lexed = if *value { "true" } else { "false" };
                if text.get(arg.start..arg.end) != Some(lexed) {
                    return Err(PrivateOverloadPrepareError::Unsupported(
                        "bool 實參與原 Lexer 字面值不一致",
                    ));
                }
                PrivateOverloadValue::Bool(*value)
            }
            _ => return Err(PrivateOverloadPrepareError::Unsupported(
                "此安全子集限真 Checker 已核准的單一 int／bool 純常值實參",
            )),
        };
        if verified.plan.scalar_hir.expression(arg.node_span())
            .is_none_or(|fact| fact.ty !=
                if argument.ty() == "int" { ScalarType::Int } else { ScalarType::Bool }) {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "實參缺少原 Checker TypedHIR 型別證據",
            ));
        }
        // D18-02-A 只由原已核准完整型別序列消歧，不依
        // 執行期資料值、回傳型別、方法宣告位置猜測。
        let matches: Vec<&Function> = definition.methods_named(name).filter(|method|
            method.params.len() == 1 && method.params[0].ty == argument.ty()
        ).collect();
        let [method] = matches.as_slice() else {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "S20 靜態來源不唯一或無匹配方法，不得執行",
            ));
        };
        let method = *method;
        if method.result_ty != "int"
            || !(method.body.statements.len() == 1 || method.body.statements.len() == 2) {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "此子集只支援合法 Copyable int 返回的同步方法",
            ));
        }
        let param = &method.params[0];
        let parameter_id = DeclarationId(param.declaration_start);
        let member = VerifiedTypedMember::checked_private(
            text, NodeRole::Parameter, param.declaration_span,
            &param.name, param.name_span, &param.ty, param.type_span, false,
        ).ok_or(PrivateOverloadPrepareError::Unsupported(
            "參數缺少真 Parser 的 typed member 來源",
        ))?;
        if !verified.plan.declarations.matches(parameter_id, argument.ty(), false)
            || verified.plan.members.get(&member.identity) != Some(&member) {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "S20 所選方法參數未綁定真正 Checker DeclarationId／TypedMember",
            ));
        }
        let write = if method.body.statements.len() == 2 {
            let StmtKind::FieldAssign { receiver, field: target, value } =
                &method.body.statements[0].kind else {
                return Err(PrivateOverloadPrepareError::Unsupported(
                    "兩語句多載方法首句須為已核准的 self 欄位直接賦值",
                ));
            };
            if receiver != "self" || target != &field.name {
                return Err(PrivateOverloadPrepareError::Unsupported(
                    "方法寫入目標不是原 Checker 唯一 self int 欄位",
                ));
            }
            match &value.kind {
                TermKind::Name(found) if found == &param.name
                    && param.ty == "int"
                    && verified.tokens.iter().any(|token|
                        token.start == value.start && token.end == value.end
                            && matches!(&token.kind, TokenKind::Name(n) if n == found)
                    ) => PrivateOverloadWrite::Parameter,
                TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw))
                    if verified.tokens.iter().any(|token|
                        token.start == value.start && token.end == value.end
                            && matches!(&token.kind, TokenKind::Integer(t) if t == raw)
                    ) => PrivateOverloadWrite::Literal(
                        crate::syntax_shared::integer_i64(raw).map_err(|_|
                            PrivateOverloadPrepareError::Unsupported("方法寫入 int 超出 i64"),
                        )?,
                    ),
                _ => return Err(PrivateOverloadPrepareError::Unsupported(
                    "寫入值只接受已驗證的 int 參數或 int 常值",
                )),
            }
        } else { PrivateOverloadWrite::None };
        let last = method.body.statements.last().unwrap();
        let StmtKind::Return(ret) = &last.kind else {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "方法須直接返回真 self 欄位",
            ));
        };
        if !matches!(&ret.expression.kind,
            TermKind::FieldRead(receiver, found)
                if receiver == "self" && found == &field.name
        ) {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "方法返回非核准 self.int 欄位",
            ));
        }
        let method_span = NodeSpan::new(method.start, method.end);
        let method_fact = verified.plan.method_safety.get(&method.start).ok_or(
            PrivateOverloadPrepareError::Unsupported(
                "缺少原 Checker VerifiedMethodSafety",
            ),
        )?;
        let kind = if write != PrivateOverloadWrite::None {
            BorrowKind::ExclusiveWrite
        } else { BorrowKind::SharedRead };
        if method_fact.span != method_span || method_fact.self_borrow != kind
            || !method_fact.checked_returns.contains(&last.node_span()) {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "S20 真選定方法的返回／self 安全模式和原 Checker 不符",
            ));
        }
        let loan = BorrowFact::new(owner_id, kind);
        if verified.plan.declarations.call_borrows_at(call_span) != Some(&[loan][..]) {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "真方法呼叫缺少與 S20 唯一簽章一致的原 Checker BorrowFact",
            ));
        }
        if write != PrivateOverloadWrite::None
            && (!owner.mutable || field.immutable) {
            return Err(PrivateOverloadPrepareError::Unsupported(
                "獨占方法不能寫入 const 接收者或 const 欄位",
            ));
        }
        let original = verified.source.bytes.clone().into_boxed_slice();
        // 所有借用自真 AST 先複製為獨立值，再移入來源所有權。
        let struct_name = struct_name.clone();
        let owner_name = owner.name.clone();
        let owner_mutable = owner.mutable;
        let field_name = field.name.clone();
        let field_immutable = field.immutable;
        Ok(Self {
            verified, original, struct_name, owner: owner_name, owner_id,
            owner_mutable, field: field_name, field_value, field_immutable,
            call_span, method_span, parameter_id, argument, write, loan,
        })
    }

    fn matches_source(&self, text: &str) -> bool {
        let Ok(fresh) = Self::prepare(text) else { return false };
        self.original == fresh.original
            && self.struct_name == fresh.struct_name
            && self.owner == fresh.owner && self.owner_id == fresh.owner_id
            && self.owner_mutable == fresh.owner_mutable
            && self.field == fresh.field && self.field_value == fresh.field_value
            && self.field_immutable == fresh.field_immutable
            && self.call_span == fresh.call_span
            && self.method_span == fresh.method_span
            && self.parameter_id == fresh.parameter_id
            && self.argument == fresh.argument
            && self.write == fresh.write && self.loan == fresh.loan
            && self.verified.plan.method_safety == fresh.verified.plan.method_safety
            && self.verified.plan.declarations.call_borrows
                == fresh.verified.plan.declarations.call_borrows
    }

    fn execute(&self) -> Result<PrivateOverloadOutcome, PrivateOverloadPrepareError> {
        self.execute_with_fault(false)
    }

    /// 僅供 Rust 測試：方法借用和實參框架存活時注入故障，
    /// 不視為 voiles 已核准例外／取消語意。
    fn execute_with_fault(&self, fail_inside_method: bool)
        -> Result<PrivateOverloadOutcome, PrivateOverloadPrepareError> {
        let Ok(text) = std::str::from_utf8(&self.verified.source.bytes) else {
            return Err(PrivateOverloadPrepareError::SourceChanged);
        };
        if self.original.as_ref() != self.verified.source.bytes.as_slice()
            || !self.verified.plan.is_consistent(
                &self.verified.structs, &self.verified.functions, text,
            ) || !self.matches_source(text) {
            return Err(PrivateOverloadPrepareError::SourceChanged);
        }
        use crate::borrow_safety::BorrowSafety;
        use crate::mixed_struct_runtime::{
            MixedStructRuntime, MixedFieldSpec, ScalarKind, SeedValue, Observed,
        };
        let mut runtime = MixedStructRuntime::new();
        runtime.construct(&self.owner, &[MixedFieldSpec {
            name: &self.field, kind: ScalarKind::Int,
            default: Some(SeedValue::Int(self.field_value)),
            immutable: self.field_immutable,
        }], &[], None).map_err(PrivateOverloadPrepareError::Cleanup)?;
        let mut tracker = BorrowSafety::new();
        let mut tickets = Vec::new();
        let mut facts = Vec::new();
        let mut events = vec![PrivateOverloadEvent::ArgumentEvaluated(self.argument)];
        let mut frame = BTreeMap::<DeclarationId, PrivateOverloadValue>::new();
        let result = (|| -> Result<i64, PrivateOverloadRuntimeError> {
            let ticket = tracker.acquire(self.loan.source, self.loan.kind)
                .map_err(PrivateOverloadRuntimeError::Ticket)?;
            tickets.push(ticket);
            facts.push(self.loan);
            tracker.verify_static_snapshot(facts.iter().copied())
                .map_err(PrivateOverloadRuntimeError::Ticket)?;
            let owner_blocked = tracker.check_direct_access(self.owner_id);
            if owner_blocked != Err(
                crate::borrow_safety::BorrowSafetyError::SourceIsBorrowed,
            ) {
                return Err(PrivateOverloadRuntimeError::WrongBinding);
            }
            events.push(PrivateOverloadEvent::MethodEntered(self.method_span));
            if frame.insert(self.parameter_id, self.argument).is_some() {
                return Err(PrivateOverloadRuntimeError::WrongBinding);
            }
            events.push(PrivateOverloadEvent::ParameterBound(
                self.parameter_id, self.argument,
            ));
            if fail_inside_method {
                return Err(PrivateOverloadRuntimeError::InjectedFailure);
            }
            if self.write != PrivateOverloadWrite::None {
                let authorized = tracker.check_ticket_access(
                    ticket, BorrowKind::ExclusiveWrite,
                ).map_err(PrivateOverloadRuntimeError::Ticket)?;
                if authorized != self.owner_id {
                    return Err(PrivateOverloadRuntimeError::WrongBinding);
                }
                let value = match self.write {
                    PrivateOverloadWrite::None => unreachable!(),
                    PrivateOverloadWrite::Parameter => match frame.get(&self.parameter_id) {
                        Some(PrivateOverloadValue::Int(value)) => *value,
                        _ => return Err(PrivateOverloadRuntimeError::WrongBinding),
                    },
                    PrivateOverloadWrite::Literal(value) => value,
                };
                runtime.write_int_field(
                    &self.owner, &self.field, value, self.owner_mutable,
                ).map_err(PrivateOverloadRuntimeError::Resource)?;
                events.push(PrivateOverloadEvent::FieldWritten(value));
            }
            let authorized = tracker.check_ticket_access(ticket, BorrowKind::SharedRead)
                .map_err(PrivateOverloadRuntimeError::Ticket)?;
            if authorized != self.owner_id {
                return Err(PrivateOverloadRuntimeError::WrongBinding);
            }
            let Observed::Int(value) = runtime.field(&self.owner, &self.field)
                .map_err(PrivateOverloadRuntimeError::Resource)? else {
                return Err(PrivateOverloadRuntimeError::WrongBinding);
            };
            events.push(PrivateOverloadEvent::Returned(value));
            Ok(value)
        })();
        // 先退出真正方法參數框架，再原子歸還原 Checker self
        // BorrowFact，最後釋放 MixedStructRuntime 唯一結構所有者。
        frame.clear();
        events.push(PrivateOverloadEvent::MethodExited {
            returned: result.is_ok(),
        });
        let released = tracker.finish_scope_verified(&tickets, &facts)
            .map_err(PrivateOverloadRuntimeError::Ticket);
        let result = match (result, released) {
            (Err(problem), _) => Err(problem),
            (Ok(_), Err(problem)) => Err(problem),
            (Ok(value), Ok(())) => tracker.check_direct_access(self.owner_id)
                .map_err(PrivateOverloadRuntimeError::Ticket).map(|()| value),
        };
        let ticket_returned = tracker.check_direct_access(self.owner_id).is_ok();
        let resources = runtime.finish().map_err(PrivateOverloadPrepareError::Cleanup)?;
        if !crate::owned_str_runtime::event_trace_balanced(&resources) {
            return Err(PrivateOverloadPrepareError::SourceChanged);
        }
        Ok(PrivateOverloadOutcome {
            result, selected_method_span: self.method_span, events,
            resources, parameter_slots_after_exit: frame.len(), ticket_returned,
        })
    }
}

#[cfg(test)]
mod g18_private_method_overload_runtime_tests {
    use super::*;

    const READ: &str = "fn choose(flag:bool)->int{\nreturn self.n\n}\n";
    const WRITE: &str = "fn choose(value:int)->int{\nself.n = value\nreturn self.n\n}\n";

    fn source(methods: &str, owner_kind: &str, arg: &str) -> String {
        format!(
            "struct Counter{{\nvar n:int = 1\n{methods}}}\nfn main()->int{{\n{owner_kind} owner:Counter = Counter()\nreturn owner.choose({arg})\n}}\n"
        )
    }
    fn checked(src: &str) -> PreparedPrivateOverload {
        PreparedPrivateOverload::prepare(src)
            .expect("S20 完整唯一型別／原 Checker 自動 self BorrowFact 應支援真正執行")
    }

    #[test]
    fn g18_f_真多載_int_參數由已選獨占方法實際寫入欄位並返回() {
        let src = source(&format!("{READ}{WRITE}"), "var", "37");
        let plan = checked(&src);
        let output = plan.execute().unwrap();
        assert_eq!(output.result, Ok(37));
        assert_eq!(output.parameter_slots_after_exit, 0);
        assert!(output.ticket_returned && output.resources.is_empty());
        assert!(output.events.contains(&PrivateOverloadEvent::FieldWritten(37)));
        assert!(output.events.contains(&PrivateOverloadEvent::Returned(37)));
    }

    #[test]
    fn g18_f_真多載_bool_參數由原_checker_唯一選中共享方法不寫入() {
        let src = source(&format!("{READ}{WRITE}"), "const", "true");
        let output = checked(&src).execute().unwrap();
        assert_eq!(output.result, Ok(1));
        assert!(output.ticket_returned && output.resources.is_empty());
        assert_eq!(output.events.iter().filter(|e|
            matches!(e, PrivateOverloadEvent::FieldWritten(_))).count(), 0);
    }

    #[test]
    fn g18_f_真多載方法宣告顛倒仍以完整型別選擇正確動態結果() {
        let src = source(&format!("{WRITE}{READ}"), "var", "41");
        let plan = checked(&src);
        assert_eq!(plan.loan.kind, BorrowKind::ExclusiveWrite);
        assert_eq!(plan.execute().unwrap().result, Ok(41));
        let src = source(&format!("{WRITE}{READ}"), "var", "false");
        let plan = checked(&src);
        assert_eq!(plan.loan.kind, BorrowKind::SharedRead);
        assert_eq!(plan.execute().unwrap().result, Ok(1));
    }

    #[test]
    fn g18_f_const_多載選到獨占方法時由原_checker_禁止寫入() {
        let src = source(&format!("{READ}{WRITE}"), "const", "9");
        let error = PreparedPrivateOverload::prepare(&src)
            .err().expect("const 實例不可調用獨占寫方法");
        assert!(matches!(error, PrivateOverloadPrepareError::Check(_)));
    }

    #[test]
    fn g18_f_真多載借用存活中失敗必歸還票據和局部參數() {
        let src = source(&format!("{READ}{WRITE}"), "var", "7");
        let plan = checked(&src);
        let result = plan.execute_with_fault(true).unwrap();
        assert_eq!(result.result, Err(PrivateOverloadRuntimeError::InjectedFailure));
        assert!(result.ticket_returned);
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert!(result.resources.is_empty());
        assert!(!result.events.iter().any(|e|
            matches!(e, PrivateOverloadEvent::Returned(_))));
        assert_eq!(plan.execute().unwrap().result, Ok(7));
    }

    #[test]
    fn g18_f_竄改已選多載方法跨度或來源值須在執行前拒絕() {
        let src = source(&format!("{READ}{WRITE}"), "var", "7");
        let mut plan = checked(&src);
        plan.method_span = crate::common_ir::NodeSpan::new(0, 1);
        assert!(matches!(plan.execute(), Err(PrivateOverloadPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.argument = PrivateOverloadValue::Int(8);
        assert!(matches!(plan.execute(), Err(PrivateOverloadPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_竄改原_checker_call_borrows_不得改用另一方法票據() {
        let src = source(&format!("{READ}{WRITE}"), "var", "7");
        let mut plan = checked(&src);
        plan.loan.kind = BorrowKind::SharedRead;
        assert!(matches!(plan.execute(), Err(PrivateOverloadPrepareError::SourceChanged)));
        let mut plan = checked(&src);
        plan.verified.plan.declarations.call_borrows.clear();
        assert!(matches!(plan.execute(), Err(PrivateOverloadPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_f_多載不合法的參數型別與相同參數簽章必由原_checker_拒絕() {
        let bad_type = source(&format!("{READ}{WRITE}"), "var", "\"字串\"");
        assert!(matches!(PreparedPrivateOverload::prepare(&bad_type),
            Err(PrivateOverloadPrepareError::Check(_))));
        let duplicate = source(
            "fn choose(x:int)->int{\nreturn self.n\n}\nfn choose(y:int)->int{\nreturn self.n\n}\n",
            "var", "5",
        );
        assert!(matches!(PreparedPrivateOverload::prepare(&duplicate),
            Err(PrivateOverloadPrepareError::Check(_))));
    }
}
