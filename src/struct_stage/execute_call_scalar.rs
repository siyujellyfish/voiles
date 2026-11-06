// G18／P18.1、P18.4：已核准 Copyable int 真普通函式參數子集。
// 原私有 Lexer／Parser／Checker／TypedHIR 是唯一來源權威；
// 本 evaluator 執行原 Checker 的 Copyable int 參數及已驗證巢狀普通函式返回圖，
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
    /// D18-01：是本次呼叫環境預算耗盡，非語言固定遞迴深度。
    CallBudget,
    StepBudget,
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
    /// 原 Checker 唯一具名函式與已驗證 int 實參；真正呼叫會進入新框架。
    Call { name: String, arguments: Vec<Self> },
}

impl PrivateScalarReturnExpr {
    /// 僅由真正 Checker 已交付的 int TypedHIR 準備，不在執行期
    /// 重新猜測文法或由來源文字推論算式樹。
    fn from_checked(term: &Term, params: &[Parameter], verified: &VerifiedPrivateSource)
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
                let param = params.iter().find(|param| &param.name == name).ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "返回名稱不是本次已核准函式的 Copyable int 參數",
                    ),
                )?;
                let id = DeclarationId(param.declaration_start);
                if verified.plan.scalar_hir.reference(span)
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
                Self::from_checked(inner, params, verified),
            TermKind::Call(call) => {
                // 由原 Parser 真函式宣告／Checker HIR 決定簽章。
                // 不允許用字面來源重新推測或合成隱含轉型。
                let target = verified.functions.iter()
                    .find(|fun| fun.name == call.name)
                    .ok_or(PrivateScalarCallPrepareError::Unsupported(
                        "巢狀呼叫沒有原 Checker 具名宣告",
                    ))?;
                if target.name == "main" || target.result_ty != "int"
                    || target.params.len() != call.arguments.len()
                    || target.params.iter().any(|param| param.ty != "int")
                    || verified.plan.declarations.call_borrows_at(span)
                        .is_some_and(|facts| !facts.is_empty()) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "巢狀呼叫必須使用已核准純 Copyable int 的完整參數簽章",
                    ));
                }
                let mut arguments = Vec::new();
                for argument in &call.arguments {
                    arguments.push(Self::from_checked(argument, params, verified)?);
                }
                Ok(Self::Call { name: call.name.clone(), arguments })
            }
            TermKind::UnresolvedZeroArgs(name) => {
                // A2/F02：裸 name() 在 Parser 無法先判斷是建構式或零參數函式；
                // 只有正式 Checker 分類、完整函式宣告和 int TypedHIR
                // 全部一致時，才可交給原顯式動態框架執行。
                // 此純值切片若存在任一結構已於 prepare 入口拒絕，
                // 不得藉此把 Struct() 擅自改判為普通函式。
                let target = verified.functions.iter()
                    .find(|fun| fun.name == *name && fun.name != "main")
                    .ok_or(PrivateScalarCallPrepareError::Unsupported(
                        "零參數來源缺少原 Checker 唯一普通函式宣告",
                    ))?;
                if target.result_ty != "int" || !target.params.is_empty()
                    || verified.plan.declarations.call_borrows_at(span)
                        .is_some_and(|facts| !facts.is_empty())
                    || !verified.tokens.iter().any(|token|
                        token.start == span.start
                            && matches!(&token.kind, TokenKind::Name(found) if found == name))
                    || !verified.tokens.iter().any(|token|
                        token.end == span.end
                            && matches!(token.kind, TokenKind::Symbol(')'))) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "零參數呼叫缺少原 Parser／Checker 完整簽章或詞彙來源",
                    ));
                }
                Ok(Self::Call { name: name.clone(), arguments: Vec::new() })
            }
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
                let left = Box::new(Self::from_checked(&binary.left, params, verified)?);
                let right = Box::new(Self::from_checked(&binary.right, params, verified)?);
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
            Self::Call { .. } => Err(PrivateScalarCallRuntimeError::StateMismatch),
        }
    }

    fn collect_calls<'a>(&'a self, calls: &mut Vec<&'a str>) {
        match self {
            Self::Call { name, arguments } => {
                calls.push(name);
                for argument in arguments { argument.collect_calls(calls); }
            }
            Self::Addition(left, right) | Self::Subtraction(left, right) => {
                left.collect_calls(calls);
                right.collect_calls(calls);
            }
            Self::Parameter(_) | Self::Constant(_) => {}
        }
    }
}


#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateScalarNestedCallee {
    name: String,
    span: crate::common_ir::NodeSpan,
    parameters: Vec<DeclarationId>,
    result_span: crate::common_ir::NodeSpan,
    result: PrivateScalarReturnExpr,
}

struct PreparedPrivateScalarCall {
    verified: VerifiedPrivateSource,
    original: Box<[u8]>,
    call_span: crate::common_ir::NodeSpan,
    callee_span: crate::common_ir::NodeSpan,
    callee_name: String,
    parameter_name: Option<String>,
    parameter_id: Option<DeclarationId>,
    parameter_return_span: crate::common_ir::NodeSpan,
    return_expr: PrivateScalarReturnExpr,
    owner_id: Option<DeclarationId>,
    argument: Option<i64>,
    /// 其餘參數從同一原 Checker 函式簽章依宣告順序綁定。
    additional_arguments: Vec<(DeclarationId, i64, Option<DeclarationId>)>,
    /// 已在主函式真正按順序建立的 Copyable int 詞彙持有者。
    main_owners: Vec<(DeclarationId, i64)>,
    /// 同一完整來源中原 Parser／Checker 已驗證的其餘參數函式。
    nested_callees: Vec<PrivateScalarNestedCallee>,
}

/// G18／D18-01：每次呼叫的執行環境配置，不是 voiles 的語言上限。
#[derive(Debug, Clone, Copy)]
struct PrivateScalarExecutionBudget {
    max_call_frames: usize,
    max_steps: usize,
}

impl Default for PrivateScalarExecutionBudget {
    fn default() -> Self {
        Self { max_call_frames: 128, max_steps: 100_000 }
    }
}

struct PrivateScalarActiveFrame {
    name: String,
    values: BTreeMap<DeclarationId, i64>,
}

/// 不以 Rust 遞迴執行具名函式；每個原 Checker expression 在工作堆疊
/// 上逐項求值，呼叫時由動態框架保存真正參數值。
enum PrivateScalarTask<'a> {
    Evaluate(&'a PrivateScalarReturnExpr),
    /// 每個真實參值求值完成當下才產生事件，不在 EnterCall 補記。
    RecordArgument,
    Apply(char),
    EnterCall(&'a str, usize),
    ExitCall,
}

impl PreparedPrivateScalarCall {
    /// 顯式 expression 工作堆疊＋真函式框架。合法遞迴亦走同一路徑，
    /// 預算耗盡時從最內層逐一退出；禁止依賴宿主 Rust 呼叫堆疊。
    fn evaluate_graph<'a>(
        &'a self,
        expr: &'a PrivateScalarReturnExpr,
        initial_frame: &BTreeMap<DeclarationId, i64>,
        events: &mut Vec<PrivateScalarCallEvent>,
        budget: PrivateScalarExecutionBudget,
    ) -> Result<i64, PrivateScalarCallRuntimeError> {
        let mut frames = vec![PrivateScalarActiveFrame {
            name: self.callee_name.clone(), values: initial_frame.clone(),
        }];
        let mut tasks = vec![PrivateScalarTask::Evaluate(expr)];
        let mut values = Vec::<i64>::new();
        let mut steps = 0usize;
        let result = (|| -> Result<i64, PrivateScalarCallRuntimeError> {
            while let Some(task) = tasks.pop() {
                // 記錄已完成的實參不是另一個 voiles 運算步驟；
                // 即使下一運算會耗盡預算，仍保留先前已真求值的事件。
                if !matches!(&task, PrivateScalarTask::RecordArgument) {
                    if steps >= budget.max_steps {
                        return Err(PrivateScalarCallRuntimeError::StepBudget);
                    }
                    steps += 1;
                }
                match task {
                    PrivateScalarTask::Evaluate(term) => match term {
                        PrivateScalarReturnExpr::Parameter(id) => {
                            let value = frames.last()
                                .and_then(|frame| frame.values.get(id).copied())
                                .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                            values.push(value);
                        }
                        PrivateScalarReturnExpr::Constant(value) => values.push(*value),
                        PrivateScalarReturnExpr::Addition(left, right) => {
                            tasks.push(PrivateScalarTask::Apply('+'));
                            tasks.push(PrivateScalarTask::Evaluate(right));
                            tasks.push(PrivateScalarTask::Evaluate(left));
                        }
                        PrivateScalarReturnExpr::Subtraction(left, right) => {
                            tasks.push(PrivateScalarTask::Apply('-'));
                            tasks.push(PrivateScalarTask::Evaluate(right));
                            tasks.push(PrivateScalarTask::Evaluate(left));
                        }
                        PrivateScalarReturnExpr::Call { name, arguments } => {
                            // 所有子呼叫實參在父框架從左到右求值；
                            // 其中任何一項錯誤時，尚未 Enter 的函式
                            // 不得產生假的進入或返回事件。
                            tasks.push(PrivateScalarTask::EnterCall(
                                name, arguments.len(),
                            ));
                            for argument in arguments.iter().rev() {
                                tasks.push(PrivateScalarTask::RecordArgument);
                                tasks.push(PrivateScalarTask::Evaluate(argument));
                            }
                        }
                    },
                    PrivateScalarTask::RecordArgument => {
                        let value = *values.last().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        events.push(PrivateScalarCallEvent::ArgumentEvaluated(value));
                    }
                    PrivateScalarTask::Apply(operator) => {
                        let right = values.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let left = values.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let value = match operator {
                            '+' => left.checked_add(right),
                            '-' => left.checked_sub(right),
                            _ => return Err(PrivateScalarCallRuntimeError::StateMismatch),
                        }.ok_or(PrivateScalarCallRuntimeError::IntegerOverflow)?;
                        values.push(value);
                    }
                    PrivateScalarTask::EnterCall(name, argc) => {
                        // 遞迴可呼叫最初被 main 選中的函式；
                        // 它與其他原 Checker 函式均須使用各自參數宣告身分。
                        let mut root_params = Vec::new();
                        let (parameter_ids, result_expr): (Vec<DeclarationId>, &PrivateScalarReturnExpr)
                            = if name == self.callee_name {
                                if let Some(id) = self.parameter_id {
                                    root_params.push(id);
                                }
                                root_params.extend(
                                    self.additional_arguments.iter().map(|(id, _, _)| *id),
                                );
                                (root_params, &self.return_expr)
                            } else {
                                let callee = self.nested_callees.iter()
                                    .find(|fun| fun.name == name)
                                    .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                                (callee.parameters.clone(), &callee.result)
                            };
                        if parameter_ids.len() != argc || values.len() < argc {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        if frames.len() >= budget.max_call_frames {
                            return Err(PrivateScalarCallRuntimeError::CallBudget);
                        }
                        let evaluated = values.split_off(values.len() - argc);
                        events.push(PrivateScalarCallEvent::FunctionEntered(name.to_owned()));
                        let mut locals = BTreeMap::new();
                        for (id, value) in parameter_ids.iter().zip(evaluated) {
                            if locals.insert(*id, value).is_some() {
                                return Err(PrivateScalarCallRuntimeError::StateMismatch);
                            }
                            events.push(PrivateScalarCallEvent::ParameterBound(*id, value));
                        }
                        frames.push(PrivateScalarActiveFrame {
                            name: name.to_owned(), values: locals,
                        });
                        tasks.push(PrivateScalarTask::ExitCall);
                        tasks.push(PrivateScalarTask::Evaluate(result_expr));
                    }
                    PrivateScalarTask::ExitCall => {
                        if frames.len() <= 1 {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        let value = values.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let mut frame = frames.pop().expect("真子框架仍存在");
                        events.push(PrivateScalarCallEvent::Returned(value));
                        frame.values.clear();
                        events.push(PrivateScalarCallEvent::FunctionExited {
                            name: frame.name, returned: true,
                        });
                        values.push(value);
                    }
                }
            }
            if frames.len() != 1 || values.len() != 1 {
                return Err(PrivateScalarCallRuntimeError::StateMismatch);
            }
            Ok(values[0])
        })();
        if result.is_err() {
            // 執行期預算／溢位／狀態錯誤皆不得偽裝成 return。
            // 逐層清除已真正進入的參數框架，不退出未曾進入的函式。
            while frames.len() > 1 {
                let mut frame = frames.pop().expect("已進入的子框架");
                frame.values.clear();
                events.push(PrivateScalarCallEvent::FunctionExited {
                    name: frame.name, returned: false,
                });
            }
        }
        result
    }

    fn prepare(text: &str) -> Result<Self, PrivateScalarCallPrepareError> {
        use crate::common_ir::{ScalarType, NodeSpan};
        let verified = verify_source_with_ability(
            text, crate::ownership_policy::approved_leaf_copy_permission,
        ).map_err(PrivateScalarCallPrepareError::Check)?;
        // 此私有切片只限制已核准的純 int／單 return 執行形狀，
        // 不再以固定函式數量或呼叫圖是否成環，拒絕合法 Checker 來源。
        if !verified.structs.is_empty() || verified.functions.len() < 2 {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "Copyable int 真執行須具備 main 及已核准的普通函式",
            ));
        }
        let main = verified.functions.iter().find(|fun| fun.name == "main")
            .ok_or(PrivateScalarCallPrepareError::Unsupported("缺少 main"))?;
        let last = main.body.statements.last().ok_or(
            PrivateScalarCallPrepareError::Unsupported("main 缺少最後 return"),
        )?;
        let StmtKind::Return(ret) = &last.kind else {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "main 最後語句須真正返回一個具名函式呼叫",
            ));
        };
        // A2/F02：main 的 name() 也由同一原 Parser 保留為待決零引數節點；
        // 仍必須以原 Checker 核准的唯一普通函式宣告消歧。
        let source_name = match &ret.expression.kind {
            TermKind::Call(call) => call.name.as_str(),
            TermKind::UnresolvedZeroArgs(name) => name.as_str(),
            _ => return Err(PrivateScalarCallPrepareError::Unsupported(
                "main return 不是原 Parser 真具名函式呼叫",
            )),
        };
        let callee = verified.functions.iter()
            .find(|fun| fun.name == source_name && fun.name != "main")
            .ok_or(PrivateScalarCallPrepareError::Unsupported(
                "main 呼叫缺少原 Checker 具名函式宣告",
            ))?;
        if main.result_ty != "int" || !main.params.is_empty()
            || callee.result_ty != "int"
            || callee.params.iter().any(|param| param.ty != "int")
            || callee.body.statements.len() != 1
            || main.body.statements.is_empty() {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "只接通已核准 Copyable int 完整簽章與單一返回式",
            ));
        }
        // 對函式簽章每一個 Parameter，不以名稱猜測或忽略後續參數；
        // 原 P16 TypedMember 及 Checker DeclarationId 皆須逐項相符。
        for param in &callee.params {
            let id = DeclarationId(param.declaration_start);
            let member = crate::common_ir::VerifiedTypedMember::checked_private(
                text, crate::common_ir::NodeRole::Parameter,
                param.declaration_span, &param.name, param.name_span,
                &param.ty, param.type_span, false,
            ).ok_or(PrivateScalarCallPrepareError::Unsupported(
                "多參數缺少原 Parser／Lexer 的名稱與 int 型別身分",
            ))?;
            if !verified.plan.declarations.matches(id, "int", false)
                || verified.plan.members.get(&member.identity) != Some(&member) {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "多參數缺少原 Checker const int 宣告及 P16 型別來源事實",
                ));
            }
        }
        let param = callee.params.first();
        let StmtKind::Return(return_param) = &callee.body.statements[0].kind else {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "本子集要求被呼叫函式直接執行原 Checker 核准的返回式",
            ));
        };
        let param_return_span = return_param.expression.node_span();
        let return_expr = PrivateScalarReturnExpr::from_checked(
            &return_param.expression, &callee.params, &verified,
        )?;
        let mut nested_callees = Vec::new();
        for function in verified.functions.iter()
            .filter(|fun| fun.name != "main" && fun.name != callee.name) {
            if function.result_ty != "int"
                || function.params.iter().any(|param| param.ty != "int")
                || function.body.statements.len() != 1 {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "多層函式須為已核准單 return 的純 int 完整簽章",
                ));
            }
            let mut parameter_ids = Vec::new();
            for param in &function.params {
                let id = DeclarationId(param.declaration_start);
                let member = crate::common_ir::VerifiedTypedMember::checked_private(
                    text, crate::common_ir::NodeRole::Parameter,
                    param.declaration_span, &param.name, param.name_span,
                    &param.ty, param.type_span, false,
                ).ok_or(PrivateScalarCallPrepareError::Unsupported(
                    "巢狀函式參數缺少原 Parser/Lexer 型別身分",
                ))?;
                if !verified.plan.declarations.matches(id, "int", false)
                    || verified.plan.members.get(&member.identity) != Some(&member) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "巢狀函式參數缺少原 Checker DeclarationId／P16 TypedMember",
                    ));
                }
                parameter_ids.push(id);
            }
            let StmtKind::Return(ret) = &function.body.statements[0].kind else {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "巢狀函式本體不是原 Checker 唯一返回",
                ));
            };
            nested_callees.push(PrivateScalarNestedCallee {
                name: function.name.clone(),
                span: NodeSpan::new(function.start, function.end),
                parameters: parameter_ids,
                result_span: ret.expression.node_span(),
                result: PrivateScalarReturnExpr::from_checked(
                    &ret.expression, &function.params, &verified,
                )?,
            });
        }
        // 每個可達或不可達的已編譯函式 Call 必須在同一份
        // Checker 真來源中有完整目標；允許已核准直接／互相遞迴。
        // 是否真正執行由來源控制流決定，安全耗盡由 runtime 預算決定。
        let mut graph: Vec<(&str, &PrivateScalarReturnExpr)> =
            vec![(&callee.name, &return_expr)];
        graph.extend(nested_callees.iter().map(|fun| (fun.name.as_str(), &fun.result)));
        for (_, expression) in &graph {
            let mut calls = Vec::new();
            expression.collect_calls(&mut calls);
            if calls.iter().any(|called|
                !graph.iter().any(|(name, _)| name == called)) {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "巢狀具名呼叫缺少原 Checker 真函式宣告或完整簽章",
                ));
            }
        }

        // main 的所有前置語句須真為已核准的 Copyable int 常值初始化。
        // 正常詞彙遮蔽／任意賦值另待 P18.2，不把尚未執行的語句跳過。
        let (main_return, main_prefix) = main.body.statements.split_last()
            .expect("已確認 main 非空");
        let mut owners = BTreeMap::<String, (DeclarationId, i64)>::new();
        let mut main_owners = Vec::new();
        for statement in main_prefix {
            let StmtKind::Declare(owner) = &statement.kind else {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "多參數 main 前置語句僅接受真正 int 初始化宣告",
                ));
            };
            let id = DeclarationId(statement.start);
            let Some(init) = &owner.initializer else {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "main int 詞彙持有者尚未初始化",
                ));
            };
            let TermKind::Scalar(
                crate::syntax_shared::ScalarLiteral::Integer(raw),
            ) = &init.kind else {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "此子集只接通原 Checker 已核准的 int 字面初始化",
                ));
            };
            if owner.ty != "int"
                || !verified.plan.declarations.matches(id, "int", owner.mutable)
                || verified.plan.scalar_hir.expression(init.node_span())
                    .is_none_or(|fact| fact.ty != ScalarType::Int)
                || owners.contains_key(&owner.name)
                || !verified.tokens.iter().any(|token|
                    token.start == init.start && token.end == init.end
                        && matches!(&token.kind, TokenKind::Integer(found) if found == raw)
                ) {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "main 具名 int 初始值缺少唯一 Checker／Lexer／TypedHIR 身分",
                ));
            }
            let value = crate::syntax_shared::integer_i64(raw).map_err(|_|
                PrivateScalarCallPrepareError::Unsupported(
                    "main int 詞彙初始化值超出 i64",
                ),
            )?;
            owners.insert(owner.name.clone(), (id, value));
            main_owners.push((id, value));
        }

        let StmtKind::Return(main_return) = &main_return.kind else {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "main 最後一筆必須是真普通函式呼叫 return",
            ));
        };
        let (call_name, call_arguments): (&str, &[Term]) = match &main_return.expression.kind {
            TermKind::Call(call) => (call.name.as_str(), call.arguments.as_slice()),
            TermKind::UnresolvedZeroArgs(name) => (name.as_str(), &[]),
            _ => return Err(PrivateScalarCallPrepareError::Unsupported(
                "main 最後一筆須真正呼叫已核准的 int 函式",
            )),
        };
        if call_name != callee.name || call_arguments.len() != callee.params.len() {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "原 Parser 呼叫名稱／參數個數與真 Checker 函式簽章不符",
            ));
        }
        let call_span = main_return.expression.node_span();
        if verified.plan.scalar_hir.expression(call_span)
            .is_none_or(|fact| fact.ty != ScalarType::Int)
            || verified.plan.declarations.call_borrows_at(call_span)
                .is_some_and(|facts| !facts.is_empty()) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "呼叫缺少 int TypedHIR 或 Copyable 實參遭誤記為借用",
            ));
        }
        let mut all_arguments = Vec::new();
        let mut used_names = std::collections::BTreeSet::<DeclarationId>::new();
        for (arg, parameter) in call_arguments.iter().zip(&callee.params) {
            let (value, owner_id) = match &arg.kind {
                TermKind::Scalar(
                    crate::syntax_shared::ScalarLiteral::Integer(raw),
                ) => {
                    if verified.plan.scalar_hir.expression(arg.node_span())
                        .is_none_or(|fact| fact.ty != ScalarType::Int)
                        || !verified.tokens.iter().any(|token|
                            token.start == arg.start && token.end == arg.end
                                && matches!(&token.kind,
                                    TokenKind::Integer(found) if found == raw)
                        ) {
                        return Err(PrivateScalarCallPrepareError::Unsupported(
                            "參數 int 常值缺少原 Lexer／Checker TypeHIR",
                        ));
                    }
                    (crate::syntax_shared::integer_i64(raw).map_err(|_|
                        PrivateScalarCallPrepareError::Unsupported(
                            "函式 int 實參超出 i64",
                        ),
                    )?, None)
                }
                TermKind::Name(name) => {
                    let (id, value) = owners.get(name).copied().ok_or(
                        PrivateScalarCallPrepareError::Unsupported(
                            "具名 Copyable int 實參不是原 main 已初始化所有者",
                        ),
                    )?;
                    if verified.plan.scalar_hir.reference(arg.node_span())
                        .is_none_or(|fact|
                            fact.resolved.declaration != id
                            || fact.resolved.ty != ScalarType::Int)
                        || !verified.tokens.iter().any(|token|
                            token.start == arg.start && token.end == arg.end
                                && matches!(&token.kind, TokenKind::Name(found) if found == name)
                        ) {
                        return Err(PrivateScalarCallPrepareError::Unsupported(
                            "多參數實參未繫結原 Checker DeclarationId／Reference HIR",
                        ));
                    }
                    used_names.insert(id);
                    (value, Some(id))
                }
                _ => return Err(PrivateScalarCallPrepareError::Unsupported(
                    "Copyable int 參數僅接通已驗證字面或具名來源",
                )),
            };
            all_arguments.push((
                DeclarationId(parameter.declaration_start), value, owner_id,
            ));
        }
        // 確保每筆 main 宣告在此執行切片真被作為引數取值，
        // 不可悄悄跳過未支援的多餘有作用語句。
        if main_owners.iter().any(|(id, _)| !used_names.contains(id)) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "main 有尚未在本真執行路徑使用的 int 宣告",
            ));
        }
        let (parameter_id, argument, owner_id) = match all_arguments.first().copied() {
            Some((id, value, owner)) => (Some(id), Some(value), owner),
            None => (None, None, None),
        };
        let additional_arguments = all_arguments.iter().skip(1).copied().collect();
        let callee_span = NodeSpan::new(callee.start, callee.end);
        let callee_name = callee.name.clone();
        let parameter_name = param.map(|param| param.name.clone());
        let original = verified.source.bytes.clone().into_boxed_slice();
        Ok(Self {
            verified, original, call_span, callee_span, callee_name,
            parameter_name, parameter_id,
            parameter_return_span: param_return_span, return_expr, owner_id, argument,
            additional_arguments, main_owners, nested_callees,
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
            && self.additional_arguments == fresh.additional_arguments
            && self.main_owners == fresh.main_owners
            && self.nested_callees == fresh.nested_callees
            && self.verified.plan.declarations.call_borrows
                == fresh.verified.plan.declarations.call_borrows
            && self.verified.plan.declarations.by_source
                == fresh.verified.plan.declarations.by_source
            && self.verified.plan.function_exits == fresh.verified.plan.function_exits
            && self.verified.plan.control_edges == fresh.verified.plan.control_edges
    }

    fn execute(&self) -> Result<PrivateScalarCallOutcome, PrivateScalarCallPrepareError> {
        self.execute_with_budget_and_fault(PrivateScalarExecutionBudget::default(), false)
    }

    fn execute_with_budget(&self, budget: PrivateScalarExecutionBudget)
        -> Result<PrivateScalarCallOutcome, PrivateScalarCallPrepareError> {
        self.execute_with_budget_and_fault(budget, false)
    }

    /// 測試故障僅為真參數框架清理驗證，不授權 voiles except 語意。
    fn execute_with_fault(&self, fail_after_binding: bool)
        -> Result<PrivateScalarCallOutcome, PrivateScalarCallPrepareError> {
        self.execute_with_budget_and_fault(
            PrivateScalarExecutionBudget::default(), fail_after_binding,
        )
    }

    fn execute_with_budget_and_fault(
        &self, budget: PrivateScalarExecutionBudget, fail_after_binding: bool,
    ) -> Result<PrivateScalarCallOutcome, PrivateScalarCallPrepareError> {
        let Ok(source) = std::str::from_utf8(&self.verified.source.bytes) else {
            return Err(PrivateScalarCallPrepareError::SourceChanged);
        };
        if self.original.as_ref() != self.verified.source.bytes.as_slice()
            || !self.verified.plan.is_consistent(
                &self.verified.structs, &self.verified.functions, source,
            ) || !self.matches_source(source) {
            return Err(PrivateScalarCallPrepareError::SourceChanged);
        }
        // 即使原 Checker 合法遞迴，零執行預算也不得製造假的函式進入。
        if budget.max_call_frames == 0 || budget.max_steps == 0 {
            return Ok(PrivateScalarCallOutcome {
                result: Err(if budget.max_call_frames == 0 {
                    PrivateScalarCallRuntimeError::CallBudget
                } else {
                    PrivateScalarCallRuntimeError::StepBudget
                }),
                events: Vec::new(), parameter_slots_after_exit: 0,
            });
        }
        // 逐個實際 Copyable int 實參依原 Parser 書寫順序求值；
        // 只有在全部型別與身份準備成功後才建立真正被呼叫者框架。
        let mut events = Vec::new();
        if let Some(argument) = self.argument {
            events.push(PrivateScalarCallEvent::ArgumentEvaluated(argument));
        }
        for (_, argument, _) in &self.additional_arguments {
            events.push(PrivateScalarCallEvent::ArgumentEvaluated(*argument));
        }
        events.push(PrivateScalarCallEvent::FunctionEntered(self.callee_name.clone()));
        let mut frame = BTreeMap::<DeclarationId, i64>::new();
        let mut result = Ok(());
        for (id, argument) in self.parameter_id.zip(self.argument).into_iter()
            .chain(self.additional_arguments.iter().map(|(id, value, _)| (*id, *value))) {
            if frame.insert(id, argument).is_some() {
                result = Err(PrivateScalarCallRuntimeError::StateMismatch);
                break;
            }
            events.push(PrivateScalarCallEvent::ParameterBound(id, argument));
        }
        let result = result.and_then(|()| {
            if fail_after_binding {
                Err(PrivateScalarCallRuntimeError::InjectedFailure)
            } else {
                self.evaluate_graph(&self.return_expr, &frame, &mut events, budget)
            }
        });
        if let Ok(value) = result {
            events.push(PrivateScalarCallEvent::Returned(value));
        }

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
            PrivateScalarCallEvent::ParameterBound(prepared.parameter_id.expect("已驗證唯一 int 參數"), 7),
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
        assert_ne!(prepared.owner_id, prepared.parameter_id);
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
        prepared.parameter_id = Some(DeclarationId(0));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(literal_src());
        prepared.argument = Some(999);
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

    // G18 P18.1 第二批：函式內的算術不是靜態算好的常值，
    // 每個運算元都須從當次合法 Copyable int 參數槽取值。
    #[test]
    fn g18_a_真函式_copyable_int_參數參與加法且回傳動態值() {
        let src = "fn step(value:int)->int{\nreturn value+3\n}\nfn main()->int{\nreturn step(9)\n}\n";
        let plan = checked(src);
        let result = plan.execute().unwrap();
        assert_eq!(result.result, Ok(12));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert!(result.events.contains(&PrivateScalarCallEvent::Returned(12)));
    }

    #[test]
    fn g18_a_真函式_copyable_int_參數多次引用每次都取同一實際值() {
        let src = "fn twice(value:int)->int{\nreturn value+value\n}\nfn main()->int{\nreturn twice(11)\n}\n";
        let result = checked(src).execute().unwrap();
        assert_eq!(result.result, Ok(22));
        assert_eq!(result.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_真函式_群組與左右結合加減_依原_checker_來源_hir_計算() {
        let src = "fn mix(value:int)->int{\nreturn (value+2)-(value-1)\n}\nfn main()->int{\nreturn mix(10)\n}\n";
        let result = checked(src).execute().unwrap();
        assert_eq!(result.result, Ok(3));
    }

    #[test]
    fn g18_a_具名_copyable_int_實參傳入函式後參與真減法() {
        let src = "fn minus(value:int)->int{\nreturn value-3\n}\nfn main()->int{\nconst owner:int = 10\nreturn minus(owner)\n}\n";
        let plan = checked(src);
        assert!(plan.owner_id.is_some());
        let result = plan.execute().unwrap();
        assert_eq!(result.result, Ok(7));
        assert_eq!(result.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_c_函式參數真加法溢位不能假造_return_且確實離域() {
        let src = "fn inc(value:int)->int{\nreturn value+1\n}\nfn main()->int{\nreturn inc(9223372036854775807)\n}\n";
        let result = checked(src).execute().unwrap();
        assert_eq!(result.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert!(!result.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(result.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "inc".into(), returned: false,
        }));
    }

    #[test]
    fn g18_c_函式參數真減法溢位亦不可留下框架或偽造成功事件() {
        let src = "fn minus(value:int)->int{\nreturn value-9223372036854775807-2\n}\nfn main()->int{\nreturn minus(0)\n}\n";
        let result = checked(src).execute().unwrap();
        assert_eq!(result.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert_eq!(result.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))).count(), 0);
    }

    #[test]
    fn g18_a_竄改編譯後_true_arithmetic_運算樹必在執行前拒絕() {
        let src = "fn inc(value:int)->int{\nreturn value+1\n}\nfn main()->int{\nreturn inc(2)\n}\n";
        let mut prepared = checked(src);
        prepared.return_expr = PrivateScalarReturnExpr::Constant(999);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(src);
        prepared.return_expr = PrivateScalarReturnExpr::Subtraction(
            Box::new(PrivateScalarReturnExpr::Parameter(prepared.parameter_id.expect("已驗證 int 參數"))),
            Box::new(PrivateScalarReturnExpr::Constant(1)),
        );
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_非法_bool_參與_int_加法與字面值不足均由原_checker_拒絕() {
        let bad_type = "fn wrong(value:int)->int{\nreturn value+true\n}\nfn main()->int{\nreturn wrong(3)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(bad_type),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let bad_name = "fn wrong(value:int)->int{\nreturn value+missing\n}\nfn main()->int{\nreturn wrong(3)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(bad_name),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }
    // P18.1／18.4：不是新的模型，而是原單參數真正 executor 的
    // 多 Parameter DeclarationId、TypedHIR、依序求值與退出擴充。
    fn multiple_source() -> &'static str {
        "fn combine(a:int,b:int)->int{\nreturn (a+b)-a\n}\nfn main()->int{\nreturn combine(7,9)\n}\n"
    }

    #[test]
    fn g18_a_雙_copyable_int_參數依原_checker_宣告順序逐一綁定() {
        let prepared = checked(multiple_source());
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(9));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events[0], PrivateScalarCallEvent::ArgumentEvaluated(7));
        assert_eq!(outcome.events[1], PrivateScalarCallEvent::ArgumentEvaluated(9));
        assert!(matches!(outcome.events[2], PrivateScalarCallEvent::FunctionEntered(_)));
        assert_eq!(outcome.events[3], PrivateScalarCallEvent::ParameterBound(
            prepared.parameter_id.expect("已驗證 int 參數"), 7,
        ));
        assert_eq!(outcome.events[4], PrivateScalarCallEvent::ParameterBound(
            prepared.additional_arguments[0].0, 9,
        ));
        assert_ne!(prepared.parameter_id.expect("已驗證 int 參數"), prepared.additional_arguments[0].0);
    }

    #[test]
    fn g18_a_三個_int_參數在同一真框架內逐項參與原_hir_算式() {
        let text = "fn blend(a:int,b:int,c:int)->int{\nreturn (a-b)+c\n}\nfn main()->int{\nreturn blend(8,3,5)\n}\n";
        let outcome = checked(text).execute().unwrap();
        assert_eq!(outcome.result, Ok(10));
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ParameterBound(..)
        )).count(), 3);
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_同一個_const_int_持有者作兩個實參屬_copyable_不消耗() {
        let text = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nconst original:int = 21\nreturn sum(original,original)\n}\n";
        let prepared = checked(text);
        assert_eq!(prepared.owner_id, prepared.additional_arguments[0].2);
        for _ in 0..3 {
            let outcome = prepared.execute().unwrap();
            assert_eq!(outcome.result, Ok(42));
            assert_eq!(outcome.parameter_slots_after_exit, 0);
        }
    }

    #[test]
    fn g18_a_多個具名_var_const_int_和常值實參按照原宣告身分傳遞() {
        let text = "fn blend(a:int,b:int,c:int)->int{\nreturn a+b+c\n}\nfn main()->int{\nconst first:int = 4\nvar second:int = 3\nreturn blend(first,1,second)\n}\n";
        let prepared = checked(text);
        assert_eq!(prepared.main_owners.len(), 2);
        assert_eq!(prepared.owner_id, Some(prepared.main_owners[0].0));
        assert_eq!(prepared.additional_arguments[1].2, Some(prepared.main_owners[1].0));
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(8));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_c_多參數整數溢位不可回傳成功且所有動態參數清空() {
        let text = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(9223372036854775807,1)\n}\n";
        let prepared = checked(text);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert!(!outcome.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_)
        )));
        assert_eq!(outcome.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "sum".to_owned(), returned: false,
        }));
    }

    #[test]
    fn g18_c_多參數真框架測試故障不殘留參數也不假冒_return() {
        let prepared = checked(multiple_source());
        let failed = prepared.execute_with_fault(true).unwrap();
        assert_eq!(failed.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(failed.parameter_slots_after_exit, 0);
        assert!(!failed.events.iter().any(|e| matches!(
            e, PrivateScalarCallEvent::Returned(_)
        )));
        assert_eq!(prepared.execute().unwrap().result, Ok(9));
    }

    #[test]
    fn g18_a_擅改第二實參值或第二_parameter_declarationid_先驗後拒絕() {
        let mut prepared = checked(multiple_source());
        prepared.additional_arguments[0].1 = 100;
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(multiple_source());
        prepared.additional_arguments[0].0 = DeclarationId(0);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(multiple_source());
        prepared.main_owners.push((DeclarationId(999), 1));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_多參數錯誤型別缺少引數與多餘未執行語句一律拒絕() {
        for text in [
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1,true)\n}\n",
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1)\n}\n",
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1,2,3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(text),
                Err(PrivateScalarCallPrepareError::Check(_))), "{text}");
        }
        let unused = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nvar extra:int = 5\nreturn sum(1,2)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(unused),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
    }

    // P18.1／P18.3／P18.4：同一原 Checker／Parameter HIR 的真正多層具參數
    // 普通函式圖；每個呼叫邊皆有真正的進／出事件與獨立 int slot。
    fn graph_source() -> &'static str {
        "fn leaf(c:int)->int{\nreturn c+1\n}\nfn middle(a:int,b:int)->int{\nreturn leaf(a+b)\n}\nfn main()->int{\nreturn middle(3,4)\n}\n"
    }

    #[test]
    fn g18_a_前向兩層帶參數呼叫每層建立獨立框架並按返回順序退出() {
        let prepared = checked(graph_source());
        let result = prepared.execute().unwrap();
        assert_eq!(result.result, Ok(8));
        assert_eq!(result.parameter_slots_after_exit, 0);
        assert_eq!(result.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionEntered(_))).count(), 2);
        assert_eq!(result.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionExited { returned: true, .. })).count(), 2);
        assert!(matches!(&result.events[0], PrivateScalarCallEvent::ArgumentEvaluated(3)));
        assert!(matches!(&result.events[1], PrivateScalarCallEvent::ArgumentEvaluated(4)));
        let trace = result.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(format!("+{name}")),
            PrivateScalarCallEvent::FunctionExited { name, returned } if *returned =>
                Some(format!("-{name}")),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(trace, ["+middle", "+leaf", "-leaf", "-middle"]);
    }

    #[test]
    fn g18_a_三層具參數函式圖真正使用前一層返回值而非來源常值() {
        let src = "fn last(x:int)->int{\nreturn x-2\n}\nfn next(y:int)->int{\nreturn last(y+3)\n}\nfn first(z:int)->int{\nreturn next(z+4)\n}\nfn main()->int{\nreturn first(8)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(13));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let enters = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(enters, ["first", "next", "last"]);
        let exits = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionExited { name, returned: true } =>
                Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(exits, ["last", "next", "first"]);
    }

    #[test]
    fn g18_a_同一巢狀葉函式重複真呼叫各自返回且不共用參數槽() {
        let src = "fn leaf(p:int)->int{\nreturn p+1\n}\nfn twice(a:int,b:int)->int{\nreturn leaf(a)+leaf(b)\n}\nfn main()->int{\nreturn twice(2,5)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(9));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let values = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(values, [3, 6, 9]);
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "leaf",
        )).count(), 2);
    }

    #[test]
    fn g18_c_巢狀葉函式溢位不得虛構子層或父層成功返回() {
        let src = "fn leaf(p:int)->int{\nreturn p+1\n}\nfn outer(x:int)->int{\nreturn leaf(x)\n}\nfn main()->int{\nreturn outer(9223372036854775807)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        let exits = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionExited { name, returned: false } =>
                Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(exits, ["leaf", "outer"]);
    }

    #[test]
    fn g18_c_父框架故障時不可進入尚未求值的巢狀子函式() {
        let failed = checked(graph_source()).execute_with_fault(true).unwrap();
        assert_eq!(failed.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(failed.parameter_slots_after_exit, 0);
        assert!(!failed.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(name) if name == "leaf",
        )));
        assert!(!failed.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(failed.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "middle".to_owned(), returned: false,
        }));
    }

    #[test]
    fn g18_c_巢狀函式來源或參數宣告身份被篡改零動態框架拒絕() {
        let mut prepared = checked(graph_source());
        prepared.nested_callees[0].parameters[0] = DeclarationId(0);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(graph_source());
        prepared.nested_callees[0].result = PrivateScalarReturnExpr::Constant(99);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(graph_source());
        prepared.verified.plan.declarations.by_source.clear();
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_c_函式圖合法互相遞迴須受控預算耗盡並逐動態框架退出() {
        let src = "fn alpha(x:int)->int{\nreturn beta(x)\n}\nfn beta(y:int)->int{\nreturn alpha(y)\n}\nfn main()->int{\nreturn alpha(1)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 5, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["alpha", "beta", "alpha", "beta", "alpha"]);
        let exited = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionExited { name, returned: false } =>
                Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(exited, ["alpha", "beta", "alpha", "beta", "alpha"]);
        assert!(!outcome.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::Returned(_))));
    }

    #[test]
    fn g18_a_巢狀呼叫錯誤型別及未知函式仍由原_checker_拒絕() {
        let bad_type = graph_source().replace("leaf(a+b)", "leaf(true)");
        assert!(matches!(PreparedPrivateScalarCall::prepare(&bad_type),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let unknown = graph_source().replace("leaf(a+b)", "missing(a+b)");
        assert!(matches!(PreparedPrivateScalarCall::prepare(&unknown),
            Err(PrivateScalarCallPrepareError::Check(_))));
    }

    #[test]
    fn g18_c_合法直接_int_遞迴不得因圖成環被拒_且無_rust_堆疊溢位() {
        let src = "fn repeat(x:int)->int{\nreturn repeat(x)\n}\nfn main()->int{\nreturn repeat(4)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 8, max_steps: 500 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionEntered(_))).count(), 8);
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionExited { returned: false, .. })).count(), 8);
        assert!(!outcome.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::Returned(_))));
    }

    #[test]
    fn g18_a_合法前向六層以上_copyable_int_函式真逐層返回() {
        let mut src = String::new();
        for i in 0..9 {
            let target = if i == 8 {
                "n+1".to_owned()
            } else {
                format!("f{}(n)", i+1)
            };
            src.push_str(&format!("fn f{i}(n:int)->int{{\nreturn {target}\n}}\n"));
        }
        src.push_str("fn main()->int{\nreturn f0(12)\n}\n");
        let outcome = checked(&src).execute().unwrap();
        assert_eq!(outcome.result, Ok(13));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionEntered(_))).count(), 9);
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionExited { returned: true, .. })).count(), 9);
    }

    #[test]
    fn g18_a_五個_copyable_int_真參數不應被內部三參數上界拒絕() {
        let src = "fn sum(a:int,b:int,c:int,d:int,e:int)->int{\nreturn a+b+c+d+e\n}\nfn main()->int{\nreturn sum(1,2,3,4,5)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(15));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::ParameterBound(..))).count(), 5);
    }

    #[test]
    fn g18_c_純值遞迴步數預算受控耗盡且深層框架逆序退出() {
        let src = "fn repeat(x:int)->int{\nreturn repeat(x)\n}\nfn main()->int{\nreturn repeat(9)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 200, max_steps: 8 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        let entered = outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionEntered(_))).count();
        let exited = outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionExited { returned: false, .. })).count();
        assert!(entered >= 2);
        assert_eq!(entered, exited);
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_c_零框架或零步數預算不得偽造任何真函式進入() {
        let checked = checked(graph_source());
        for budget in [
            PrivateScalarExecutionBudget { max_call_frames: 0, max_steps: 100 },
            PrivateScalarExecutionBudget { max_call_frames: 100, max_steps: 0 },
        ] {
            let outcome = checked.execute_with_budget(budget).unwrap();
            assert!(outcome.result.is_err());
            assert!(outcome.events.is_empty());
            assert_eq!(outcome.parameter_slots_after_exit, 0);
        }
    }

    #[test]
    fn g18_a_未被選取的合法遞迴函式不得造成其他正向函式拒絕() {
        let src = "fn unused(x:int)->int{\nreturn unused(x)\n}\nfn good(y:int)->int{\nreturn y+2\n}\nfn main()->int{\nreturn good(7)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 2, max_steps: 30 },
        ).unwrap();
        assert_eq!(outcome.result, Ok(9));
        assert_eq!(outcome.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::FunctionEntered(name) if name == "unused")).count(), 0);
    }

    #[test]
    fn g18_c_有界步數的兩個巢狀兄弟呼叫僅在實際出現時建立獨立框架() {
        let src = "fn leaf(x:int)->int{\nreturn x+1\n}\nfn both(a:int,b:int)->int{\nreturn leaf(a)+leaf(b)\n}\nfn main()->int{\nreturn both(1,2)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 2, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Ok(5));
        let exits = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionExited { name, returned: true } => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(exits, ["leaf", "leaf", "both"]);
    }


    #[test]
    fn g18_a_零參數前向葉函式與_copyable_int_父框架真返回() {
        let src = "fn plus(n:int)->int{\nreturn n+fixed()\n}\nfn main()->int{\nreturn plus(5)\n}\nfn fixed()->int{\nreturn 7\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(12));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["plus", "fixed"]);
        let returned = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returned, [7, 12]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))).count(), 1);
    }

    #[test]
    fn g18_a_零參數函式重複呼叫不重用動態框架且保留真求值順序() {
        let src = "fn fixed()->int{\nreturn 4\n}\nfn add(n:int)->int{\nreturn fixed()+n+fixed()\n}\nfn main()->int{\nreturn add(5)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(13));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["add", "fixed", "fixed"]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: true
            } if name == "fixed")).count(), 2);
    }

    #[test]
    fn g18_c_零參數自遞迴由真框架預算受控中止不虛構返回() {
        let src = "fn loop()->int{\nreturn loop()\n}\nfn entry(n:int)->int{\nreturn n+loop()\n}\nfn main()->int{\nreturn entry(1)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 4, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 4);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                returned: false, ..
            })).count(), 4);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
    }

    #[test]
    fn g18_a_零參數子函式原宣告來源遭竄改必在動態執行前拒絕() {
        let src = "fn fixed()->int{\nreturn 7\n}\nfn plus(n:int)->int{\nreturn n+fixed()\n}\nfn main()->int{\nreturn plus(5)\n}\n";
        let mut prepared = checked(src);
        let zero = prepared.nested_callees.iter_mut()
            .find(|f| f.name == "fixed").expect("原 Checker 的零參數函式");
        assert!(zero.parameters.is_empty());
        zero.parameters.push(DeclarationId(usize::MAX));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }



    #[test]
    fn g18_a_巢狀兄弟呼叫每個實參求值完成立即紀錄且不重複() {
        let src = "fn leaf(x:int)->int{\nreturn x+1\n}\nfn pair(a:int,b:int)->int{\nreturn a-b\n}\nfn outer(n:int)->int{\nreturn pair(leaf(n),leaf(n+2))\n}\nfn main()->int{\nreturn outer(2)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(-2));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let arguments = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        // main 的 2、leaf(2) 的 2、pair 第一實參 3、
        // leaf(4) 的 4、pair 第二實參 5，必為原 Parser 的左至右順序。
        assert_eq!(arguments, [2, 2, 3, 4, 5]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "leaf")).count(), 2);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "pair")).count(), 1);
    }

    #[test]
    fn g18_c_第一實參完成後第二實參溢位仍保留真事件而不進入父呼叫() {
        let src = "fn first(x:int)->int{\nreturn x+1\n}\nfn bad(x:int)->int{\nreturn x+1\n}\nfn pair(a:int,b:int)->int{\nreturn a+b\n}\nfn outer(n:int)->int{\nreturn pair(first(n),bad(9223372036854775807))\n}\nfn main()->int{\nreturn outer(2)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [2, 2, 3, 9223372036854775807]);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "pair")));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 3);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited { .. })).count(), 3);
        assert!(outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: false
            } if name == "outer")));
    }


}
