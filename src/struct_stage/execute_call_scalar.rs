// G18／P18.1、P18.4：已核准 Copyable int 真普通函式參數子集。
// 原私有 Lexer／Parser／Checker／TypedHIR 是唯一來源權威；
// 本 evaluator 只執行一個真正 int 參數的已核准純 int 返回運算式，
// 不把靜態 Checker 等同已執行、亦不擴充公開 P01 CLI。
#[derive(Debug)]
enum PrivateScalarCallPrepareError {
    Check(crate::Diagnostic),
    Unsupported(&'static str),
    SourceChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateScalarCallRuntimeError {
    StateMismatch,
    InjectedFailure,
    IntegerOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarCallEvent {
    ArgumentEvaluated(i64),
    FunctionEntered(String),
    ParameterBound(DeclarationId, i64),
    Returned(i64),
    FunctionExited { name: String, returned: bool },
}

#[derive(Debug)]
struct PrivateScalarCallOutcome {
    result: Result<i64, PrivateScalarCallRuntimeError>,
    events: Vec<PrivateScalarCallEvent>,
    /// 任何成功、失敗及重複執行都不得保留被呼叫者參數。
    parameter_slots_after_exit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarReturnExpr {
    Parameter(DeclarationId),
    Constant(i64),
    Addition(Box<Self>, Box<Self>),
    Subtraction(Box<Self>, Box<Self>),
}

impl PrivateScalarReturnExpr {
    /// 僅由真正 Checker 已交付的 int TypedHIR 準備，不在執行期
    /// 重新猜測文法或由來源文字推論算式樹。
    fn from_checked(term: &Term, param: &Parameter, verified: &VerifiedPrivateSource)
        -> Result<Self, PrivateScalarCallPrepareError> {
        use crate::common_ir::ScalarType;
        let span = term.node_span();
        if verified.plan.scalar_hir.expression(span)
            .is_none_or(|fact| fact.ty != ScalarType::Int) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "int 返回運算式缺少原 Checker TypedHIR Expression 型別",
            ));
        }
        match &term.kind {
            TermKind::Name(name) => {
                let id = DeclarationId(param.declaration_start);
                if name != &param.name || verified.plan.scalar_hir.reference(span)
                    .is_none_or(|fact|
                        fact.resolved.declaration != id
                        || fact.resolved.ty != ScalarType::Int)
                    || !verified.tokens.iter().any(|token|
                        token.start == span.start && token.end == span.end
                            && matches!(&token.kind, TokenKind::Name(found) if found == name)
                    ) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "返回中具名 int 引用缺少原 Checker 參數身分",
                    ));
                }
                Ok(Self::Parameter(id))
            }
            TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) => {
                Ok(Self::Constant(crate::syntax_shared::integer_i64(raw)
                    .map_err(|_| PrivateScalarCallPrepareError::Unsupported(
                        "返回 int 常值超出 i64 範圍",
                    ))?))
            }
            TermKind::Group(inner) =>
                Self::from_checked(inner, param, verified),
            TermKind::BinaryInt(binary) => {
                let left = binary.left.node_span();
                let right = binary.right.node_span();
                let Some(fact) = verified.plan.scalar_hir.arithmetic(span) else {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "函式內加減缺少原 Checker 位置／運算子／來源 HIR",
                    ));
                };
                if fact.left != left || fact.right != right
                    || fact.operator != binary.operator
                    || !matches!(fact.operator, '+' | '-') {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "函式內加減 HIR 端點或運算子遭偷換",
                    ));
                }
                let left = Box::new(Self::from_checked(&binary.left, param, verified)?);
                let right = Box::new(Self::from_checked(&binary.right, param, verified)?);
                Ok(if fact.operator == '+' {
                    Self::Addition(left, right)
                } else {
                    Self::Subtraction(left, right)
                })
            }
            _ => Err(PrivateScalarCallPrepareError::Unsupported(
                "此私有呼叫只接通原 Checker 已核准的 Copyable int 加減及括號",
            )),
        }
    }

    fn evaluate(&self, frame: &BTreeMap<DeclarationId, i64>)
        -> Result<i64, PrivateScalarCallRuntimeError> {
        match self {
            Self::Parameter(identity) =>
                frame.get(identity).copied().ok_or(PrivateScalarCallRuntimeError::StateMismatch),
            Self::Constant(value) => Ok(*value),
            Self::Addition(left, right) => left.evaluate(frame)?
                .checked_add(right.evaluate(frame)?)
                .ok_or(PrivateScalarCallRuntimeError::IntegerOverflow),
            Self::Subtraction(left, right) => left.evaluate(frame)?
                .checked_sub(right.evaluate(frame)?)
                .ok_or(PrivateScalarCallRuntimeError::IntegerOverflow),
        }
    }
}

struct PreparedPrivateScalarCall {
    verified: VerifiedPrivateSource,
    original: Box<[u8]>,
    call_span: crate::common_ir::NodeSpan,
    callee_span: crate::common_ir::NodeSpan,
    callee_name: String,
    parameter_name: String,
    parameter_id: DeclarationId,
    parameter_return_span: crate::common_ir::NodeSpan,
    return_expr: PrivateScalarReturnExpr,
    owner_id: Option<DeclarationId>,
    argument: i64,
}

impl PreparedPrivateScalarCall {
    fn prepare(text: &str) -> Result<Self, PrivateScalarCallPrepareError> {
        use crate::common_ir::{ScalarType, NodeSpan};
        let verified = verify_source_with_ability(
            text, crate::ownership_policy::approved_leaf_copy_permission,
        ).map_err(PrivateScalarCallPrepareError::Check)?;
        if !verified.structs.is_empty() || verified.functions.len() != 2 {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "僅驗證兩個真正普通函式且不含結構方法的安全子集",
            ));
        }
        let main = verified.functions.iter().find(|fun| fun.name == "main")
            .ok_or(PrivateScalarCallPrepareError::Unsupported("缺少 main"))?;
        let callee = verified.functions.iter().find(|fun| fun.name != "main")
            .ok_or(PrivateScalarCallPrepareError::Unsupported("缺少被呼叫函式"))?;
        if main.result_ty != "int" || !main.params.is_empty()
            || callee.result_ty != "int" || callee.params.len() != 1
            || callee.params[0].ty != "int"
            || callee.body.statements.len() != 1
            || !(main.body.statements.len() == 1 || main.body.statements.len() == 2) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "只執行原 Checker 證明的一 int 參數直接具名返回",
            ));
        }
        let param = &callee.params[0];
        let param_id = DeclarationId(param.declaration_start);
        let expected_member = crate::common_ir::VerifiedTypedMember::checked_private(
            text, crate::common_ir::NodeRole::Parameter, param.declaration_span,
            &param.name, param.name_span, &param.ty, param.type_span, false,
        ).ok_or(PrivateScalarCallPrepareError::Unsupported(
            "參數缺少真 Parser 來源位置、名稱／型別 token 身分",
        ))?;
        if !verified.plan.declarations.matches(param_id, "int", false)
            || verified.plan.members.get(&expected_member.identity)
                != Some(&expected_member) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "參數缺少原 Checker int const 宣告及 P16 Member 型別來源證據",
            ));
        }
        let StmtKind::Return(return_param) = &callee.body.statements[0].kind else {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "被呼叫者只能用已簽發的 Copyable int 參數直接返回",
            ));
        };
        let param_return_span = return_param.expression.node_span();
        // 每個子節點都取自**原** Checker 的 TypedHIR；
        // Copyable int 加減使用原正式溢位語意，絕不在準備時
        // 把呼叫參數當成常值計算。
        let return_expr = PrivateScalarReturnExpr::from_checked(
            &return_param.expression, param, &verified,
        )?;
        let main_len = main.body.statements.len();
        let StmtKind::Return(main_return) = &main.body.statements[main_len-1].kind else {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "主函式尾端須為真正普通函式呼叫的返回",
            ));
        };
        let TermKind::Call(call) = &main_return.expression.kind else {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "主函式必須真正呼叫一個已核准的 int 函式",
            ));
        };
        if call.name != callee.name || call.arguments.len() != 1 {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "函式名稱／參數個數不符合正式 Parser 及 Checker",
            ));
        }
        let call_span = main_return.expression.node_span();
        if verified.plan.scalar_hir.expression(call_span)
            .is_none_or(|fact| fact.ty != ScalarType::Int)
            || verified.plan.declarations.call_borrows_at(call_span)
                .is_some_and(|facts| !facts.is_empty()) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "呼叫缺少 int TypedHIR 或 Copyable 實參被誤作非 Copyable 借用",
            ));
        }
        let arg = &call.arguments[0];
        let (argument, owner_id) = match &arg.kind {
            TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) => {
                if verified.plan.scalar_hir.expression(arg.node_span())
                    .is_none_or(|fact| fact.ty != ScalarType::Int) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "int 引數常值缺少原 Checker Expression HIR",
                    ));
                }
                (crate::syntax_shared::integer_i64(raw).map_err(|_|
                    PrivateScalarCallPrepareError::Unsupported("int 引數超出 i64 範圍"),
                )?, None)
            }
            TermKind::Name(name) if main_len == 2 => {
                let first = &main.body.statements[0];
                let StmtKind::Declare(owner) = &first.kind else {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "具名 Copyable 實參的持有者必須為真已核准宣告",
                    ));
                };
                let owner_id = DeclarationId(first.start);
                let Some(initializer) = owner.initializer.as_ref() else {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "int 持有者必須在執行前有合法初始值",
                    ));
                };
                let TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) =
                    &initializer.kind else {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "此私有執行子集僅接通已驗證的獨立 int 常值宣告",
                    ));
                };
                if name != &owner.name || owner.ty != "int"
                    || !verified.plan.declarations.matches(owner_id, "int", owner.mutable)
                    || verified.plan.scalar_hir.reference(arg.node_span())
                        .is_none_or(|fact|
                            fact.resolved.declaration != owner_id
                            || fact.resolved.ty != ScalarType::Int)
                    || verified.plan.scalar_hir.expression(initializer.node_span())
                        .is_none_or(|fact| fact.ty != ScalarType::Int)
                    || !verified.tokens.iter().any(|token|
                        token.start == arg.start && token.end == arg.end
                            && matches!(&token.kind, TokenKind::Name(found) if found == name)
                    ) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "實參沒有與原 Checker int DeclarationId／Reference HIR 綁定",
                    ));
                }
                (crate::syntax_shared::integer_i64(raw).map_err(|_|
                    PrivateScalarCallPrepareError::Unsupported("初始 int 常值超出 i64"),
                )?, Some(owner_id))
            }
            _ => return Err(PrivateScalarCallPrepareError::Unsupported(
                "此私有子集的 int 實參只能是已驗證常值或具名 Copyable int",
            )),
        };
        if main_len == 2 && owner_id.is_none() {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "此子集不允許未執行的額外主函式語句",
            ));
        }
        // 先將原 AST 的已核准身分投影成獨立值，再移交
        // VerifiedPrivateSource；不得保留對其內部函式的懸空借用。
        let callee_span = NodeSpan::new(callee.start, callee.end);
        let callee_name = callee.name.clone();
        let parameter_name = param.name.clone();
        let original = verified.source.bytes.clone().into_boxed_slice();
        Ok(Self {
            verified, original, call_span, callee_span, callee_name,
            parameter_name, parameter_id: param_id,
            parameter_return_span: param_return_span, return_expr, owner_id, argument,
        })
    }

    fn matches_source(&self, source: &str) -> bool {
        let Ok(fresh) = Self::prepare(source) else { return false; };
        self.original == fresh.original
            && self.call_span == fresh.call_span
            && self.callee_span == fresh.callee_span
            && self.callee_name == fresh.callee_name
            && self.parameter_name == fresh.parameter_name
            && self.parameter_id == fresh.parameter_id
            && self.parameter_return_span == fresh.parameter_return_span
            && self.return_expr == fresh.return_expr
            && self.owner_id == fresh.owner_id
            && self.argument == fresh.argument
            && self.verified.plan.declarations.by_source
                == fresh.verified.plan.declarations.by_source
            && self.verified.plan.function_exits == fresh.verified.plan.function_exits
            && self.verified.plan.control_edges == fresh.verified.plan.control_edges
    }

    fn execute(&self) -> Result<PrivateScalarCallOutcome, PrivateScalarCallPrepareError> {
        self.execute_with_fault(false)
    }

    /// 本故障只用於測試真正執行中的參數框架清理，不代表 voiles except 語意。
    fn execute_with_fault(&self, fail_after_binding: bool)
        -> Result<PrivateScalarCallOutcome, PrivateScalarCallPrepareError> {
        let Ok(source) = std::str::from_utf8(&self.verified.source.bytes) else {
            return Err(PrivateScalarCallPrepareError::SourceChanged);
        };
        if self.original.as_ref() != self.verified.source.bytes.as_slice()
            || !self.verified.plan.is_consistent(
                &self.verified.structs, &self.verified.functions, source,
            ) || !self.matches_source(source) {
            return Err(PrivateScalarCallPrepareError::SourceChanged);
        }
        let mut events = vec![
            PrivateScalarCallEvent::ArgumentEvaluated(self.argument),
            PrivateScalarCallEvent::FunctionEntered(self.callee_name.clone()),
        ];
        // 真呼叫框架只以原 Checker 參數 DeclarationId 存放 Copyable
        // i64；不能用引數文字名稱建立另一個假的宣告或借用票據。
        let mut frame = BTreeMap::<DeclarationId, i64>::new();
        let result = if frame.insert(self.parameter_id, self.argument).is_some() {
            Err(PrivateScalarCallRuntimeError::StateMismatch)
        } else {
            events.push(PrivateScalarCallEvent::ParameterBound(
                self.parameter_id, self.argument,
            ));
            if fail_after_binding {
                Err(PrivateScalarCallRuntimeError::InjectedFailure)
            } else {
                // 從本次真參數框架逐項求值；加減每個運算元
                // 實際讀取 Copyable i64，checked_* 溢位時沒有
                // 假 Returned 事件且仍正常執行退出與清理。
                match self.return_expr.evaluate(&frame) {
                    Ok(value) => {
                        events.push(PrivateScalarCallEvent::Returned(value));
                        Ok(value)
                    },
                    Err(error) => Err(error),
                }
            }
        };
        // 經正常 Return 或錯誤分支都要離開此呼叫的參數作用域。
        frame.clear();
        events.push(PrivateScalarCallEvent::FunctionExited {
            name: self.callee_name.clone(), returned: result.is_ok(),
        });
        Ok(PrivateScalarCallOutcome {
            result, events, parameter_slots_after_exit: frame.len(),
        })
    }
}

#[cfg(test)]
mod g18_private_scalar_call_tests {
    use super::*;
    use crate::common_ir::NodeSpan;

    fn checked(src: &str) -> PreparedPrivateScalarCall {
        PreparedPrivateScalarCall::prepare(src)
            .expect("真私有 Checker 已核准 Copyable int 參數應能進入動態框架")
    }

    fn literal_src() -> &'static str {
        "fn main()->int{\nreturn forward(7)\n}\nfn forward(value:int)->int{\nreturn value\n}\n"
    }

    #[test]
    fn g18_a_前向函式真_copyable_int_參數直接返回並保留動態事件順序() {
        let prepared = checked(literal_src());
        let result = prepared.execute().unwrap();
        assert_eq!(result.result, Ok(7));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert_eq!(result.events, [
            PrivateScalarCallEvent::ArgumentEvaluated(7),
            PrivateScalarCallEvent::FunctionEntered("forward".into()),
            PrivateScalarCallEvent::ParameterBound(prepared.parameter_id, 7),
            PrivateScalarCallEvent::Returned(7),
            PrivateScalarCallEvent::FunctionExited {
                name: "forward".into(), returned: true,
            },
        ]);
    }

    #[test]
    fn g18_a_具名_const_int_實參以原_checker_引用身分綁定函式參數() {
        let src = "fn identity(item:int)->int{\nreturn item\n}\nfn main()->int{\nconst n:int = 8\nreturn identity(n)\n}\n";
        let prepared = checked(src);
        assert!(prepared.owner_id.is_some());
        assert_ne!(prepared.owner_id, Some(prepared.parameter_id));
        let result = prepared.execute().unwrap();
        assert_eq!(result.result, Ok(8));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert_eq!(result.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))).count(), 1);
    }

    #[test]
    fn g18_a_具名_var_int_合法_copyable_不因函式返回消耗持有者() {
        let src = "fn identity(item:int)->int{\nreturn item\n}\nfn main()->int{\nvar owner:int = 42\nreturn identity(owner)\n}\n";
        let prepared = checked(src);
        for _ in 0..3 {
            let result = prepared.execute().unwrap();
            assert_eq!(result.result, Ok(42));
            assert_eq!(result.parameter_slots_after_exit, 0);
        }
    }

    #[test]
    fn g18_c_真正參數框架中途故障必清空且不冒充成功返回() {
        let prepared = checked(literal_src());
        let failure = prepared.execute_with_fault(true).unwrap();
        assert_eq!(failure.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(failure.parameter_slots_after_exit, 0);
        assert!(!failure.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(failure.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "forward".into(), returned: false,
        }));
        let success = prepared.execute().unwrap();
        assert_eq!(success.result, Ok(7));
        assert_eq!(success.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_錯誤_copyable_int_實參型別及引數個數由原_checker_拒絕() {
        for src in [
            "fn forward(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn forward(true)\n}\n",
            "fn forward(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn forward(1,2)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_)),), "{src}");
        }
    }

    #[test]
    fn g18_a_具名未初始化_int_無法假裝有函式實參值() {
        let src = "fn forward(value:int)->int{\nreturn value\n}\nfn main()->int{\nvar owner:int\nreturn forward(owner)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(src),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_a_竄改已驗證參數_declarationid_和返回值必須零執行拒絕() {
        let mut prepared = checked(literal_src());
        prepared.parameter_id = DeclarationId(0);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(literal_src());
        prepared.argument = 999;
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_竄改主函式呼叫跨度或原來源位元組不得建立任何參數框架() {
        let mut prepared = checked(literal_src());
        prepared.call_span = NodeSpan::new(0, 2);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(literal_src());
        prepared.verified.source.bytes[0] = b'x';
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }
}
