// G18／P18.1、P18.4：已核准 Copyable int 真普通函式參數子集。
// 原私有 Lexer／Parser／Checker／TypedHIR 是唯一來源權威；
// 本 evaluator 執行原 Checker 的 Copyable int 參數及已驗證巢狀普通函式返回圖，
// 不把靜態 Checker 等同已執行、亦不擴充公開 P01 CLI。
#[derive(Debug)]
enum PrivateScalarCallPrepareError {
    Check(crate::Diagnostic),
    Unsupported(&'static str),
    SourceChanged,
    /// 真執行事件與動態框架責任不一致，保守拒絕對外暴露不可信結果。
    RuntimeTraceMismatch,
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
    /// bool 實參獨立標記，不把 Copyable bool 當成 int 隱含轉型。
    BoolArgumentEvaluated(bool),
    FunctionEntered(String),
    ParameterBound(DeclarationId, i64),
    BoolParameterBound(DeclarationId, bool),
    /// 每筆經 Checker 核准的純值區域初始化只在真正寫入框架後記錄。
    LocalInitialized(DeclarationId, i64),
    /// 只有原 Checker 確認為 var int 的真賦值才留下寫入事件。
    LocalAssigned(DeclarationId, i64),
    /// 原 Checker 已核准的 bool Copyable 專用局部事件（非資源事件）。
    BoolInitialized(DeclarationId, bool),
    BoolAssigned(DeclarationId, bool),
    BoolScopeEnded(DeclarationId),
    /// 只有執行時真正到達的 if 條件才可記錄求值結果。
    ConditionalChecked(bool),
    /// 非資源釋放：只表示 int 區域宣告已離開真動態作用域。
    LocalScopeEnded(DeclarationId),
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

impl PrivateScalarCallOutcome {
    /// R05：完全由已真正發出的事件驗證每個動態框架的進／退、
    /// 已完成 Copyable 局部宣告之逆序離域及單次返回。
    /// 這是 int／bool 純值 trace，**不冒充 OwnedStr ResourceEvent**。
    fn has_balanced_frame_trace(&self) -> bool {
        struct TraceFrame {
            name: Option<String>,
            locals: Vec<PrivateScalarLocalBinding>,
            returned: bool,
        }
        let mut frames = vec![TraceFrame {
            name: None, locals: Vec::new(), returned: false,
        }];
        for event in &self.events {
            match event {
                PrivateScalarCallEvent::FunctionEntered(name) => {
                    if frames.last().is_none_or(|f| f.returned) { return false; }
                    frames.push(TraceFrame {
                        name: Some(name.clone()), locals: Vec::new(), returned: false,
                    });
                }
                PrivateScalarCallEvent::ParameterBound(..)
                    | PrivateScalarCallEvent::BoolParameterBound(..) => {
                    if frames.last().is_none_or(|f| f.name.is_none() || f.returned) {
                        return false;
                    }
                }
                PrivateScalarCallEvent::LocalInitialized(id, _) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.returned || frame.locals.iter().any(|entry|
                        matches!(entry, PrivateScalarLocalBinding::Int(old) if old == id)
                    ) { return false; }
                    frame.locals.push(PrivateScalarLocalBinding::Int(*id));
                }
                PrivateScalarCallEvent::BoolInitialized(id, _) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.returned || frame.locals.iter().any(|entry|
                        matches!(entry, PrivateScalarLocalBinding::Bool(old) if old == id)
                    ) { return false; }
                    frame.locals.push(PrivateScalarLocalBinding::Bool(*id));
                }
                PrivateScalarCallEvent::LocalAssigned(id, _) => {
                    if frames.last().is_none_or(|frame| frame.returned
                        || !frame.locals.iter().any(|entry|
                            matches!(entry, PrivateScalarLocalBinding::Int(old) if old == id)
                        )) { return false; }
                }
                PrivateScalarCallEvent::BoolAssigned(id, _) => {
                    if frames.last().is_none_or(|frame| frame.returned
                        || !frame.locals.iter().any(|entry|
                            matches!(entry, PrivateScalarLocalBinding::Bool(old) if old == id)
                        )) { return false; }
                }
                PrivateScalarCallEvent::LocalScopeEnded(id) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if !matches!(frame.locals.pop(),
                        Some(PrivateScalarLocalBinding::Int(previous)) if previous == *id) {
                        return false;
                    }
                }
                PrivateScalarCallEvent::BoolScopeEnded(id) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if !matches!(frame.locals.pop(),
                        Some(PrivateScalarLocalBinding::Bool(previous)) if previous == *id) {
                        return false;
                    }
                }
                PrivateScalarCallEvent::Returned(_) => {
                    let Some(frame) = frames.last_mut() else { return false; };
                    if frame.name.is_none() || frame.returned { return false; }
                    frame.returned = true;
                }
                PrivateScalarCallEvent::FunctionExited { name, returned } => {
                    if frames.len() <= 1 { return false; }
                    let Some(frame) = frames.pop() else { return false; };
                    if frame.name.as_deref() != Some(name.as_str())
                        || frame.returned != *returned || !frame.locals.is_empty() {
                        return false;
                    }
                }
                PrivateScalarCallEvent::ArgumentEvaluated(_)
                    | PrivateScalarCallEvent::BoolArgumentEvaluated(_)
                    | PrivateScalarCallEvent::ConditionalChecked(_) => {}
            }
        }
        frames.len() == 1 && frames[0].locals.is_empty()
            && self.parameter_slots_after_exit == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarReturnExpr {
    /// 原 Checker 已證明所有前置 if 分支均真正 Return 的後方
    /// 不可達 expression。只有遭執行到才回報狀態錯誤，絕不造假 int。
    Unreachable,
    Parameter(DeclarationId),
    Constant(i64),
    Addition(Box<Self>, Box<Self>),
    Subtraction(Box<Self>, Box<Self>),
    /// 原 Checker 唯一具名函式與已驗證 int 實參；真正呼叫會進入新框架。
    Call { name: String, arguments: Vec<PrivateScalarCallArgument> },
}

impl PrivateScalarReturnExpr {
    /// 僅由真正 Checker 已交付的 int TypedHIR 準備，不在執行期
    /// 重新猜測文法或由來源文字推論算式樹。
    fn from_checked(term: &Term, params: &[Parameter], verified: &VerifiedPrivateSource)
        -> Result<Self, PrivateScalarCallPrepareError> {
        let bindings = params.iter().map(|param|
            (param.name.clone(), DeclarationId(param.declaration_start))
        ).collect::<BTreeMap<_, _>>();
        Self::from_checked_bindings(term, &bindings, verified)
    }

    /// 共用原 Checker 的 DeclarationId 編譯函式參數及 main 既有 int 持有者。
    fn from_checked_bindings(
        term: &Term,
        bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
    ) -> Result<Self, PrivateScalarCallPrepareError> {
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
                let id = *bindings.get(name).ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "int 名稱不是本次原 Checker 已核准的 Copyable int 來源",
                    ),
                )?;
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
                if !verified.tokens.iter().any(|token|
                    token.start == span.start && token.end == span.end
                        && matches!(&token.kind, TokenKind::Integer(found) if found == raw)) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "int 常值缺少原 Lexer 來源",
                    ));
                }
                Ok(Self::Constant(crate::syntax_shared::integer_i64(raw)
                    .map_err(|_| PrivateScalarCallPrepareError::Unsupported(
                        "返回 int 常值超出 i64 範圍",
                    ))?))
            }
            TermKind::Group(inner) => {
                if !verified.tokens.iter().any(|token|
                    token.start == span.start
                        && matches!(token.kind, TokenKind::Symbol('(')))
                    || !verified.tokens.iter().any(|token|
                        token.end == span.end
                            && matches!(token.kind, TokenKind::Symbol(')'))) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "int Group 缺少原 Lexer 左右括號來源",
                    ));
                }
                Self::from_checked_bindings(inner, bindings, verified)
            },
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
                    || target.params.iter().any(|param|
                        !matches!(param.ty.as_str(), "int" | "bool"))
                    || verified.plan.declarations.call_borrows_at(span)
                        .is_some_and(|facts| !facts.is_empty()) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "巢狀呼叫只接通原 Checker Copyable int／bool 完整簽章",
                    ));
                }
                let mut arguments = Vec::new();
                for (argument, parameter) in call.arguments.iter().zip(&target.params) {
                    arguments.push(if parameter.ty == "bool" {
                        PrivateScalarCallArgument::Bool(
                            PrivateScalarBoolExpr::from_checked(argument, bindings, verified)?,
                        )
                    } else {
                        PrivateScalarCallArgument::Int(
                            Self::from_checked_bindings(argument, bindings, verified)?,
                        )
                    });
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
                let left = Box::new(Self::from_checked_bindings(
                    &binary.left, bindings, verified,
                )?);
                let right = Box::new(Self::from_checked_bindings(
                    &binary.right, bindings, verified,
                )?);
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

    /// 在已驗證的原 Parser int 表達式中找出實際具名函式來源。
    /// 僅用於選擇本私有 evaluator 的函式資料，不改動 Checker 決議。
    fn first_source_call<'a>(term: &'a Term) -> Option<&'a str> {
        match &term.kind {
            TermKind::Call(call) => Some(call.name.as_str()),
            TermKind::UnresolvedZeroArgs(name) => Some(name.as_str()),
            TermKind::Group(inner) => Self::first_source_call(inner),
            TermKind::BinaryInt(binary) => Self::first_source_call(&binary.left)
                .or_else(|| Self::first_source_call(&binary.right)),
            _ => None,
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
            Self::Call { .. } | Self::Unreachable =>
                Err(PrivateScalarCallRuntimeError::StateMismatch),
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
            Self::Parameter(_) | Self::Constant(_) | Self::Unreachable => {}
        }
    }
}


#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateScalarLocalInit {
    declaration: DeclarationId,
    value: PrivateScalarReturnExpr,
}

/// 每個實參的真型別已由 Checker 決定：bool 不是 int 可隱含轉型。
#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarCallArgument {
    Int(PrivateScalarReturnExpr),
    Bool(PrivateScalarBoolExpr),
}

impl PrivateScalarCallArgument {
    fn collect_calls<'a>(&'a self, output: &mut Vec<&'a str>) {
        if let Self::Int(expr) = self { expr.collect_calls(output); }
    }
}

/// R01／R02：原 Parser 順序不可被「先宣告、後賦值」重排。
/// 此執行計畫保留每一筆 Checker 核准的實際宣告與可寫更新。
#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarBoolExpr {
    Constant(bool),
    Bound(DeclarationId),
}

impl PrivateScalarBoolExpr {
    /// 只從原 Checker 的 bool HIR、同一 Lexer/Parser 真來源編譯條件。
    fn from_checked(
        term: &Term,
        bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
    ) -> Result<Self, PrivateScalarCallPrepareError> {
        let span = term.node_span();
        if verified.plan.scalar_hir.expression(span)
            .is_none_or(|fact| fact.ty != crate::common_ir::ScalarType::Bool) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "bool 條件缺少原 Checker 已核准的 TypedHIR",
            ));
        }
        match &term.kind {
            TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Boolean(value)) => {
                let text = std::str::from_utf8(&verified.source.bytes).map_err(|_|
                    PrivateScalarCallPrepareError::SourceChanged,
                )?;
                let expected = if *value { "true" } else { "false" };
                if text.get(term.start..term.end) != Some(expected)
                    || !verified.tokens.iter().any(|token|
                        token.start == term.start && token.end == term.end
                            && matches!(&token.kind, TokenKind::Keyword(_))) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "bool 常值缺少原 Lexer Token 或文字身分",
                    ));
                }
                Ok(Self::Constant(*value))
            }
            TermKind::Name(name) => {
                let id = *bindings.get(name).ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "bool 名稱缺少先前原 Checker 宣告",
                    ),
                )?;
                if verified.plan.scalar_hir.reference(span)
                    .is_none_or(|fact|
                        fact.resolved.declaration != id
                            || fact.resolved.ty != crate::common_ir::ScalarType::Bool)
                    || !verified.plan.declarations.typed_declaration(id)
                        .is_some_and(|fact| fact.ty == "bool")
                    || !verified.tokens.iter().any(|token|
                        token.start == term.start && token.end == term.end
                            && matches!(&token.kind, TokenKind::Name(found) if found == name)) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "bool 引用缺少真 Checker／Lexer DeclarationId",
                    ));
                }
                Ok(Self::Bound(id))
            }
            TermKind::Group(inner) => {
                if !verified.tokens.iter().any(|token|
                    token.start == span.start
                        && matches!(token.kind, TokenKind::Symbol('(')))
                    || !verified.tokens.iter().any(|token|
                        token.end == span.end
                            && matches!(token.kind, TokenKind::Symbol(')'))) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "bool Group 缺少原 Lexer 左右括號",
                    ));
                }
                Self::from_checked(inner, bindings, verified)
            }
            _ => Err(PrivateScalarCallPrepareError::Unsupported(
                "此子集僅執行原 Checker 已核准 bool 常值或已初始化名稱條件",
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarBranchAction {
    /// 原 Parser 同一 if 葉分支的多筆已核准語句，保留真正執行順序。
    Sequence(Vec<PrivateScalarBranchAction>),
    /// 真 Parser 巢狀 If，沿用相同 Checker bool 條件與作用域安全證據。
    NestedIf(Box<PrivateScalarEarlyIf>),
    Return(PrivateScalarReturnExpr),
    AssignInt { declaration: DeclarationId, value: PrivateScalarReturnExpr },
    AssignBool { declaration: DeclarationId, value: PrivateScalarBoolExpr },
    /// 同一已驗證函式圖的逐輪 bool 判斷；本體仍使用原 Checker 語句計畫。
    While { condition: PrivateScalarBoolExpr, body: Box<PrivateScalarBranchAction> },
    Break,
    Continue,
}

impl PrivateScalarBranchAction {
    fn collect_calls<'a>(&'a self, calls: &mut Vec<&'a str>) {
        match self {
            Self::Sequence(steps) => {
                for action in steps { action.collect_calls(calls); }
            }
            Self::NestedIf(plan) => plan.collect_calls(calls),
            Self::Return(expr) | Self::AssignInt { value: expr, .. } =>
                expr.collect_calls(calls),
            Self::While { body, .. } => body.collect_calls(calls),
            Self::AssignBool { .. } | Self::Break | Self::Continue => {}
        }
    }

    fn always_returns(&self) -> bool {
        match self {
            Self::Return(_) => true,
            Self::Sequence(actions) => actions.last()
                .is_some_and(Self::always_returns),
            Self::NestedIf(plan) => plan.always_returns(),
            Self::AssignInt { .. } | Self::AssignBool { .. }
                | Self::While { .. } | Self::Break | Self::Continue => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateScalarEarlyIf {
    condition: PrivateScalarBoolExpr,
    when_true: PrivateScalarBranchAction,
    when_false: Option<PrivateScalarEarlyElse>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarEarlyElse {
    Branch(PrivateScalarBranchAction),
    ElseIf(Box<PrivateScalarEarlyIf>),
}

impl PrivateScalarEarlyIf {
    fn always_returns(&self) -> bool {
        self.when_true.always_returns()
            && match &self.when_false {
                Some(PrivateScalarEarlyElse::Branch(action)) =>
                    action.always_returns(),
                Some(PrivateScalarEarlyElse::ElseIf(next)) => next.always_returns(),
                _ => false,
            }
    }

    fn collect_calls<'a>(&'a self, out: &mut Vec<&'a str>) {
        self.when_true.collect_calls(out);
        if let Some(otherwise) = &self.when_false {
            match otherwise {
                PrivateScalarEarlyElse::Branch(value) => value.collect_calls(out),
                PrivateScalarEarlyElse::ElseIf(next) => next.collect_calls(out),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateScalarLocalStep {
    Declare(PrivateScalarLocalInit),
    Assign { declaration: DeclarationId, value: PrivateScalarReturnExpr },
    DeclareBool { declaration: DeclarationId, value: PrivateScalarBoolExpr },
    AssignBool { declaration: DeclarationId, value: PrivateScalarBoolExpr },
    EarlyReturn(PrivateScalarEarlyIf),
    Control(PrivateScalarBranchAction),
}

impl PrivateScalarLocalStep {
    fn collect_calls<'a>(&'a self, calls: &mut Vec<&'a str>) {
        match self {
            Self::Declare(init) => init.value.collect_calls(calls),
            Self::Assign { value, .. } => value.collect_calls(calls),
            Self::EarlyReturn(plan) => plan.collect_calls(calls),
            Self::Control(action) => action.collect_calls(calls),
            Self::DeclareBool { .. } | Self::AssignBool { .. } => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateScalarNestedCallee {
    name: String,
    span: crate::common_ir::NodeSpan,
    parameters: Vec<DeclarationId>,
    parameter_is_bool: Vec<bool>,
    result_span: crate::common_ir::NodeSpan,
    result: PrivateScalarReturnExpr,
    local_inits: Vec<PrivateScalarLocalInit>,
    local_steps: Vec<PrivateScalarLocalStep>,
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
    root_local_inits: Vec<PrivateScalarLocalInit>,
    root_local_steps: Vec<PrivateScalarLocalStep>,
    owner_id: Option<DeclarationId>,
    argument: Option<i64>,
    /// 其餘參數從同一原 Checker 函式簽章依宣告順序綁定。
    additional_arguments: Vec<(DeclarationId, i64, Option<DeclarationId>)>,
    /// 已在主函式真正按順序建立的 Copyable int 詞彙持有者。
    main_owners: Vec<(DeclarationId, i64)>,
    /// 同一完整來源中原 Parser／Checker 已驗證的其餘參數函式。
    nested_callees: Vec<PrivateScalarNestedCallee>,
    /// 真 main 實參中含原 Checker int 算式／巢狀呼叫時，
    /// 共用顯式工作堆疊執行，不能先預算出假 int 結果。
    main_dynamic_call: Option<PrivateScalarReturnExpr>,
    /// main 已核准動態 int 初始化須在執行時逐筆求值，不預算函式結果。
    main_dynamic_inits: Vec<PrivateScalarLocalInit>,
    main_dynamic_steps: Vec<PrivateScalarLocalStep>,
    /// 被 main 呼叫的完整函式宣告身分，供正常及遞迴路徑使用。
    callee_parameter_ids: Vec<DeclarationId>,
    callee_parameter_is_bool: Vec<bool>,
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

#[derive(Debug, Clone, Copy)]
enum PrivateScalarLocalBinding {
    Int(DeclarationId),
    Bool(DeclarationId),
}

struct PrivateScalarActiveFrame {
    name: String,
    values: BTreeMap<DeclarationId, i64>,
    bool_values: BTreeMap<DeclarationId, bool>,
    // 按實際寫入順序保留 int／bool，退出按逆序記錄離域。
    initialized_order: Vec<PrivateScalarLocalBinding>,
}

/// 不以 Rust 遞迴執行具名函式；每個原 Checker expression 在工作堆疊
/// 上逐項求值，呼叫時由動態框架保存真正參數值。
enum PrivateScalarTask<'a> {
    Evaluate(&'a PrivateScalarReturnExpr, bool),
    /// 每個真實參值求值完成當下才產生事件，不在 EnterCall 補記。
    RecordArgument,
    EvaluateBoolArgument(&'a PrivateScalarBoolExpr),
    RecordBoolArgument,
    Apply(char),
    BindLocal(DeclarationId),
    AssignLocal(DeclarationId),
    BindBool(DeclarationId, &'a PrivateScalarBoolExpr),
    AssignBool(DeclarationId, &'a PrivateScalarBoolExpr),
    CheckEarlyReturn(&'a PrivateScalarEarlyIf),
    /// 循環的可識別反向邊亦作為 break／continue 的最近動態目標。
    CheckWhile(&'a PrivateScalarBoolExpr, &'a PrivateScalarBranchAction),
    ExecuteBranchAction(&'a PrivateScalarBranchAction),
    EnterCall(&'a str, usize, bool),
    ExitCall,
}

impl PreparedPrivateScalarCall {
    /// 本批先接通單筆直接返回的 if／else-if／else 葉分支；
    /// 分支語句來源及條件均由原 Parser、Checker 審定。
    /// 僅接通分支內一筆原 Checker 已核准的 return 或
    /// 對分支外已存在 var int/bool 的賦值；沒有未證明的作用域逃逸。
    fn compile_early_branch(
        statements: &[Stmt],
        int_bindings: &BTreeMap<String, DeclarationId>,
        bool_bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
        inside_loop: bool,
    ) -> Result<PrivateScalarBranchAction, PrivateScalarCallPrepareError> {
        let [statement] = statements else {
            if statements.is_empty() {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "if 此子集不接受缺少已核准執行證據的空分支",
                ));
            }
            let mut actions = Vec::new();
            for (index, statement) in statements.iter().enumerate() {
                let action = Self::compile_early_branch(
                    std::slice::from_ref(statement),
                    int_bindings, bool_bindings, verified, inside_loop,
                )?;
                if action.always_returns() && index + 1 != statements.len() {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "if 已返回後仍有其他語句；不得造假可達效果",
                    ));
                }
                // Checker 對 break／continue 後的不可達語句可不簽發 HIR；
                // 這些來源不能被再編譯成假執行，也不能當作下一輪效果。
                let stops_loop_body = matches!(
                    action, PrivateScalarBranchAction::Break
                        | PrivateScalarBranchAction::Continue
                );
                actions.push(action);
                if stops_loop_body { break; }
            }
            return Ok(PrivateScalarBranchAction::Sequence(actions));
        };
        match &statement.kind {
            StmtKind::If { .. } =>
                Ok(PrivateScalarBranchAction::NestedIf(Box::new(
                    Self::compile_early_if(
                        statement, int_bindings, bool_bindings, verified, inside_loop,
                    )?,
                ))),
            StmtKind::While { condition, body } => {
                let guard = PrivateScalarBoolExpr::from_checked(
                    condition, bool_bindings, verified,
                )?;
                let steps = if matches!(guard, PrivateScalarBoolExpr::Constant(false))
                    || body.statements.is_empty() {
                    PrivateScalarBranchAction::Sequence(Vec::new())
                } else {
                    Self::compile_early_branch(
                        &body.statements, int_bindings, bool_bindings, verified, true,
                    )?
                };
                Ok(PrivateScalarBranchAction::While {
                    condition: guard, body: Box::new(steps),
                })
            }
            StmtKind::Break if inside_loop => Ok(PrivateScalarBranchAction::Break),
            StmtKind::Continue if inside_loop => Ok(PrivateScalarBranchAction::Continue),
            StmtKind::Return(ret) =>
                Ok(PrivateScalarBranchAction::Return(
                    PrivateScalarReturnExpr::from_checked_bindings(
                        &ret.expression, int_bindings, verified,
                    )?,
                )),
            StmtKind::Assign(assign) => {
                let id = *int_bindings.get(&assign.name).ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "if 純值賦值目標缺少原 Checker 已驗證 DeclarationId",
                    ),
                )?;
                let fact = verified.plan.declarations.typed_declaration(id).ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "if 賦值缺少原 Checker 變數宣告",
                    ),
                )?;
                if !fact.mutable || (fact.ty != "int" && fact.ty != "bool")
                    || verified.plan.scalar_hir.assignment(statement.node_span())
                        .is_none_or(|record|
                            record.resolved.declaration != id
                                || record.resolved.ty !=
                                    (if fact.ty == "bool" {
                                        crate::common_ir::ScalarType::Bool
                                    } else {
                                        crate::common_ir::ScalarType::Int
                                    }))
                    || !verified.tokens.iter().any(|token|
                        token.start == statement.start
                            && matches!(&token.kind, TokenKind::Name(name)
                                if name == &assign.name)) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "if 更新缺少真可寫 Checker HIR、Lexer 或型別來源",
                    ));
                }
                if fact.ty == "bool" {
                    if bool_bindings.get(&assign.name) != Some(&id) {
                        return Err(PrivateScalarCallPrepareError::Unsupported(
                            "if bool 更新目標不是原 Matcher 同一 bool 宣告",
                        ));
                    }
                    Ok(PrivateScalarBranchAction::AssignBool {
                        declaration: id,
                        value: PrivateScalarBoolExpr::from_checked(
                            &assign.expression, bool_bindings, verified,
                        )?,
                    })
                } else {
                    Ok(PrivateScalarBranchAction::AssignInt {
                        declaration: id,
                        value: PrivateScalarReturnExpr::from_checked_bindings(
                            &assign.expression, int_bindings, verified,
                        )?,
                    })
                }
            }
            _ => Err(PrivateScalarCallPrepareError::Unsupported(
                "if 其餘語句待獨立控制流程與區域資源證據完成",
            )),
        }
    }

    fn compile_early_if(
        stmt: &Stmt,
        int_bindings: &BTreeMap<String, DeclarationId>,
        bool_bindings: &BTreeMap<String, DeclarationId>,
        verified: &VerifiedPrivateSource,
        inside_loop: bool,
    ) -> Result<PrivateScalarEarlyIf, PrivateScalarCallPrepareError> {
        let StmtKind::If { branch, no } = &stmt.kind else {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "else-if 必須保持原 Parser 真 If 節點",
            ));
        };
        let condition = PrivateScalarBoolExpr::from_checked(
            &branch.branch.condition, bool_bindings, verified,
        )?;
        let when_true = Self::compile_early_branch(
            &branch.body.statements, int_bindings, bool_bindings, verified, inside_loop,
        )?;
        let when_false = match no.as_ref() {
            None => None,
            Some(crate::syntax_shared::ParsedElse::Block(block)) =>
                Some(PrivateScalarEarlyElse::Branch(
                    Self::compile_early_branch(
                        &block.statements, int_bindings, bool_bindings, verified, inside_loop,
                    )?,
                )),
            Some(crate::syntax_shared::ParsedElse::ElseIf(next)) =>
                Some(PrivateScalarEarlyElse::ElseIf(Box::new(
                    Self::compile_early_if(
                        next, int_bindings, bool_bindings, verified, inside_loop,
                    )?,
                ))),
            #[cfg(test)]
            Some(crate::syntax_shared::ParsedElse::SyntheticTest(_)) =>
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "Rust 合成 else 不是原 Parser 合法執行來源",
                )),
        };
        Ok(PrivateScalarEarlyIf { condition, when_true, when_false })
    }

    /// P18.1：從同一原 Parser／Checker 的真宣告序列，逐筆編譯
    /// 函式內 Copyable int 初始化；一律保留 DeclarationId，
    /// 不以函式名稱／變數字面名稱重新解析安全身份。
    fn prepare_function_locals(
        function: &Function, verified: &VerifiedPrivateSource,
    ) -> Result<(
        Vec<PrivateScalarLocalInit>, Vec<PrivateScalarLocalStep>,
        PrivateScalarReturnExpr, crate::common_ir::NodeSpan,
    ), PrivateScalarCallPrepareError> {
        let (last, prefix) = function.body.statements.split_last().ok_or(
            PrivateScalarCallPrepareError::Unsupported("普通函式缺少返回"),
        )?;
        let StmtKind::Return(ret) = &last.kind else {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "普通函式最後一筆必須是真 Checker 核准的 return",
            ));
        };
        let mut bindings = function.params.iter().map(|param|
            (param.name.clone(), DeclarationId(param.declaration_start))
        ).collect::<BTreeMap<_, _>>();
        let mut local_inits = Vec::new();
        let mut local_steps = Vec::new();
        let mut bool_bindings = function.params.iter()
            .filter(|param| param.ty == "bool")
            .map(|param| (param.name.clone(), DeclarationId(param.declaration_start)))
            .collect::<BTreeMap<_, _>>();
        for statement in prefix {
            if matches!(&statement.kind, StmtKind::If { .. }) {
                let branch = Self::compile_early_if(
                    statement, &bindings, &bool_bindings, verified, false,
                )?;
                local_steps.push(PrivateScalarLocalStep::EarlyReturn(branch));
                continue;
            }
            if matches!(&statement.kind, StmtKind::While { .. }) {
                let action = Self::compile_early_branch(
                    std::slice::from_ref(statement), &bindings, &bool_bindings,
                    verified, false,
                )?;
                local_steps.push(PrivateScalarLocalStep::Control(action));
                continue;
            }
            if let StmtKind::Assign(assign) = &statement.kind {
                if let Some(id) = bool_bindings.get(&assign.name).copied() {
                    if !verified.plan.declarations.typed_declaration(id)
                        .is_some_and(|fact| fact.ty == "bool" && fact.mutable)
                        || verified.plan.scalar_hir.assignment(statement.node_span())
                            .is_none_or(|fact|
                                fact.resolved.declaration != id
                                    || fact.resolved.ty != crate::common_ir::ScalarType::Bool)
                        || !verified.tokens.iter().any(|token|
                            token.start == statement.start
                                && matches!(&token.kind,
                                    TokenKind::Name(found) if found == &assign.name)) {
                        return Err(PrivateScalarCallPrepareError::Unsupported(
                            "bool var 賦值缺少原 Checker 可寫身分與 Token",
                        ));
                    }
                    let value = PrivateScalarBoolExpr::from_checked(
                        &assign.expression, &bool_bindings, verified,
                    )?;
                    local_steps.push(PrivateScalarLocalStep::AssignBool {
                        declaration: id, value,
                    });
                    continue;
                }
                let id = *bindings.get(&assign.name).ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "int 賦值目標不是已驗證的本函式宣告或參數",
                    ),
                )?;
                if !verified.plan.declarations.typed_declaration(id)
                    .is_some_and(|fact| fact.ty == "int" && fact.mutable)
                    || verified.plan.scalar_hir.assignment(statement.node_span())
                        .is_none_or(|fact|
                            fact.resolved.declaration != id
                                || fact.resolved.ty != crate::common_ir::ScalarType::Int)
                    || !verified.tokens.iter().any(|token|
                        token.start == statement.start
                            && matches!(&token.kind,
                                TokenKind::Name(found) if found == &assign.name))
                {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "原 Checker int var 賦值缺少完整可寫 HIR／Token",
                    ));
                }
                let value = PrivateScalarReturnExpr::from_checked_bindings(
                    &assign.expression, &bindings, verified,
                )?;
                local_steps.push(PrivateScalarLocalStep::Assign {
                    declaration: id, value,
                });
                continue;
            }
            let StmtKind::Declare(owner) = &statement.kind else {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "函式前置語句只接通真 Copyable int 宣告與已核准 var 賦值",
                ));
            };
            let declaration = DeclarationId(statement.start);
            if owner.ty == "bool" {
                if !verified.plan.declarations.matches(declaration, "bool", owner.mutable) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "bool 區域宣告缺少原 Checker 唯一宣告身分",
                    ));
                }
                let initializer = owner.initializer.as_ref().ok_or(
                    PrivateScalarCallPrepareError::Unsupported(
                        "bool 純值宣告須具真初始化式",
                    ),
                )?;
                let value = PrivateScalarBoolExpr::from_checked(
                    initializer, &bool_bindings, verified,
                )?;
                local_steps.push(PrivateScalarLocalStep::DeclareBool {
                    declaration, value,
                });
                bool_bindings.insert(owner.name.clone(), declaration);
                // 共用原 Checker DeclarationId 名稱表同時供正式函式
                // bool 實參使用；型別仍由 Reference HIR 唯一核對，
                // 不會將 bool 當成 int 或重新解析來源。
                bindings.insert(owner.name.clone(), declaration);
                continue;
            }
            if owner.ty != "int"
                || !verified.plan.declarations.matches(declaration, "int", owner.mutable)
            {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "函式內 int 區域宣告缺少 Checker 唯一宣告身分",
                ));
            }
            let initializer = owner.initializer.as_ref().ok_or(
                PrivateScalarCallPrepareError::Unsupported(
                    "函式內此子集須有真 int 初始化式",
                ),
            )?;
            let value = PrivateScalarReturnExpr::from_checked_bindings(
                initializer, &bindings, verified,
            )?;
            let init = PrivateScalarLocalInit { declaration, value };
            local_steps.push(PrivateScalarLocalStep::Declare(init.clone()));
            local_inits.push(init);
            bindings.insert(owner.name.clone(), declaration);
        }
        let return_span = ret.expression.node_span();
        // 原 Checker 在所有 if／else-if／else 葉分支皆 Return 時，
        // 合法不為後續不可達的原始 Return 建立 Expression HIR。
        // 僅這個可由已驗證完整分支證明的情況，保留不可達標記，
        // 若執行器錯誤落入該路徑，必須受控 StateMismatch。
        let every_branch_returns = matches!(
            local_steps.last(),
            Some(PrivateScalarLocalStep::EarlyReturn(plan)) if plan.always_returns()
        );
        let result = if every_branch_returns
            && verified.plan.scalar_hir.expression(return_span).is_none() {
            PrivateScalarReturnExpr::Unreachable
        } else {
            PrivateScalarReturnExpr::from_checked_bindings(
                &ret.expression, &bindings, verified,
            )?
        };
        Ok((local_inits, local_steps, result, return_span))
    }

    /// 顯式 expression 工作堆疊＋真函式框架。合法遞迴亦走同一路徑，
    /// 預算耗盡時從最內層逐一退出；禁止依賴宿主 Rust 呼叫堆疊。
    fn evaluate_graph<'a>(
        &'a self,
        expr: &'a PrivateScalarReturnExpr,
        initial_frame: &BTreeMap<DeclarationId, i64>,
        events: &mut Vec<PrivateScalarCallEvent>,
        budget: PrivateScalarExecutionBudget,
        fail_after_root_binding: bool,
        initial_local_steps: &'a [PrivateScalarLocalStep],
    ) -> Result<i64, PrivateScalarCallRuntimeError> {
        let mut frames = vec![PrivateScalarActiveFrame {
            name: self.callee_name.clone(), values: initial_frame.clone(),
            bool_values: BTreeMap::new(),
            initialized_order: Vec::new(),
        }];
        let mut tasks = vec![PrivateScalarTask::Evaluate(expr, true)];
        // 原 Checker 語句順序在 LIFO 上反向推入，執行時保持
        // 宣告、賦值與返回的真實相對次序，不提前求值後續 RHS。
        for step in initial_local_steps.iter().rev() {
            match step {
                PrivateScalarLocalStep::Declare(init) => {
                    tasks.push(PrivateScalarTask::BindLocal(init.declaration));
                    tasks.push(PrivateScalarTask::Evaluate(&init.value, false));
                }
                PrivateScalarLocalStep::Assign { declaration, value } => {
                    tasks.push(PrivateScalarTask::AssignLocal(*declaration));
                    tasks.push(PrivateScalarTask::Evaluate(value, false));
                }
                PrivateScalarLocalStep::DeclareBool { declaration, value } =>
                    tasks.push(PrivateScalarTask::BindBool(*declaration, value)),
                PrivateScalarLocalStep::AssignBool { declaration, value } =>
                    tasks.push(PrivateScalarTask::AssignBool(*declaration, value)),
                PrivateScalarLocalStep::EarlyReturn(plan) =>
                    tasks.push(PrivateScalarTask::CheckEarlyReturn(plan)),
                PrivateScalarLocalStep::Control(action) =>
                    tasks.push(PrivateScalarTask::ExecuteBranchAction(action)),
            }
        }
        let mut values = Vec::<i64>::new();
        let mut steps = 0usize;
        let result = (|| -> Result<i64, PrivateScalarCallRuntimeError> {
            while let Some(task) = tasks.pop() {
                // 記錄已完成的實參不是另一個 voiles 運算步驟；
                // 即使下一運算會耗盡預算，仍保留先前已真求值的事件。
                if !matches!(&task, PrivateScalarTask::RecordArgument
                    | PrivateScalarTask::RecordBoolArgument) {
                    if steps >= budget.max_steps {
                        return Err(PrivateScalarCallRuntimeError::StepBudget);
                    }
                    steps += 1;
                }
                match task {
                    PrivateScalarTask::Evaluate(term, is_root_expression) => match term {
                        PrivateScalarReturnExpr::Parameter(id) => {
                            let value = frames.last()
                                .and_then(|frame| frame.values.get(id).copied())
                                .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                            values.push(value);
                        }
                        PrivateScalarReturnExpr::Constant(value) => values.push(*value),
                        PrivateScalarReturnExpr::Unreachable =>
                            return Err(PrivateScalarCallRuntimeError::StateMismatch),
                        PrivateScalarReturnExpr::Addition(left, right) => {
                            tasks.push(PrivateScalarTask::Apply('+'));
                            tasks.push(PrivateScalarTask::Evaluate(right, false));
                            tasks.push(PrivateScalarTask::Evaluate(left, false));
                        }
                        PrivateScalarReturnExpr::Subtraction(left, right) => {
                            tasks.push(PrivateScalarTask::Apply('-'));
                            tasks.push(PrivateScalarTask::Evaluate(right, false));
                            tasks.push(PrivateScalarTask::Evaluate(left, false));
                        }
                        PrivateScalarReturnExpr::Call { name, arguments } => {
                            // 所有子呼叫實參在父框架從左到右求值；
                            // 其中任何一項錯誤時，尚未 Enter 的函式
                            // 不得產生假的進入或返回事件。
                            tasks.push(PrivateScalarTask::EnterCall(
                                name, arguments.len(), is_root_expression,
                            ));
                            for argument in arguments.iter().rev() {
                                match argument {
                                    PrivateScalarCallArgument::Int(expr) => {
                                        tasks.push(PrivateScalarTask::RecordArgument);
                                        tasks.push(PrivateScalarTask::Evaluate(expr, false));
                                    }
                                    PrivateScalarCallArgument::Bool(expr) => {
                                        tasks.push(PrivateScalarTask::RecordBoolArgument);
                                        tasks.push(PrivateScalarTask::EvaluateBoolArgument(expr));
                                    }
                                }
                            }
                        }
                    },
                    PrivateScalarTask::RecordArgument => {
                        let value = *values.last().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        events.push(PrivateScalarCallEvent::ArgumentEvaluated(value));
                    }
                    PrivateScalarTask::EvaluateBoolArgument(expr) => {
                        let frame = frames.last().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let value = match expr {
                            PrivateScalarBoolExpr::Constant(value) => *value,
                            PrivateScalarBoolExpr::Bound(id) =>
                                *frame.bool_values.get(id).ok_or(
                                    PrivateScalarCallRuntimeError::StateMismatch,
                                )?,
                        };
                        // 0/1 僅為同一顯式工作堆疊中的暫態 bool 編碼；
                        // 正式型別來自原 Checker 參數簽章，不能轉成 int slot。
                        values.push(i64::from(value));
                    }
                    PrivateScalarTask::RecordBoolArgument => {
                        let encoded = *values.last().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        if !matches!(encoded, 0 | 1) {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        events.push(PrivateScalarCallEvent::BoolArgumentEvaluated(encoded == 1));
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
                    PrivateScalarTask::BindLocal(id) => {
                        let value = values.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        // 一筆區域宣告在單次動態框架內只可初始化一次；
                        // 同名遮蔽仍由原 Checker 決定不同 DeclarationId。
                        if frame.values.insert(id, value).is_some() {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.initialized_order.push(PrivateScalarLocalBinding::Int(id));
                        events.push(PrivateScalarCallEvent::LocalInitialized(id, value));
                    }
                    PrivateScalarTask::AssignLocal(id) => {
                        let value = values.pop().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let slot = frame.values.get_mut(&id).ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        *slot = value;
                        events.push(PrivateScalarCallEvent::LocalAssigned(id, value));
                    }
                    PrivateScalarTask::BindBool(id, expr) => {
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let value = match expr {
                            PrivateScalarBoolExpr::Constant(value) => *value,
                            PrivateScalarBoolExpr::Bound(source) =>
                                *frame.bool_values.get(source).ok_or(
                                    PrivateScalarCallRuntimeError::StateMismatch,
                                )?,
                        };
                        if frame.bool_values.insert(id, value).is_some() {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        frame.initialized_order.push(PrivateScalarLocalBinding::Bool(id));
                        events.push(PrivateScalarCallEvent::BoolInitialized(id, value));
                    }
                    PrivateScalarTask::AssignBool(id, expr) => {
                        let frame = frames.last_mut().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let value = match expr {
                            PrivateScalarBoolExpr::Constant(value) => *value,
                            PrivateScalarBoolExpr::Bound(source) =>
                                *frame.bool_values.get(source).ok_or(
                                    PrivateScalarCallRuntimeError::StateMismatch,
                                )?,
                        };
                        let slot = frame.bool_values.get_mut(&id).ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        *slot = value;
                        events.push(PrivateScalarCallEvent::BoolAssigned(id, value));
                    }
                    PrivateScalarTask::CheckEarlyReturn(branch) => {
                        let frame = frames.last().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let mut selected = None;
                        let mut current = branch;
                        loop {
                            let condition = match &current.condition {
                                PrivateScalarBoolExpr::Constant(value) => *value,
                                PrivateScalarBoolExpr::Bound(id) =>
                                    *frame.bool_values.get(id).ok_or(
                                        PrivateScalarCallRuntimeError::StateMismatch,
                                    )?,
                            };
                            events.push(PrivateScalarCallEvent::ConditionalChecked(condition));
                            if condition {
                                selected = Some(&current.when_true);
                                break;
                            }
                            match current.when_false.as_ref() {
                                Some(PrivateScalarEarlyElse::Branch(value)) => {
                                    selected = Some(value);
                                    break;
                                }
                                Some(PrivateScalarEarlyElse::ElseIf(next)) => {
                                    current = next;
                                }
                                None => break,
                            }
                        }
                        if let Some(selected) = selected {
                            tasks.push(PrivateScalarTask::ExecuteBranchAction(selected));
                        }
                    }
                    PrivateScalarTask::CheckWhile(condition, body) => {
                        let frame = frames.last().ok_or(
                            PrivateScalarCallRuntimeError::StateMismatch,
                        )?;
                        let value = match condition {
                            PrivateScalarBoolExpr::Constant(value) => *value,
                            PrivateScalarBoolExpr::Bound(id) =>
                                *frame.bool_values.get(id).ok_or(
                                    PrivateScalarCallRuntimeError::StateMismatch,
                                )?,
                        };
                        events.push(PrivateScalarCallEvent::ConditionalChecked(value));
                        if value {
                            // 本輪真動作在反向邊之前，逐輪重新讀取框架 bool 值。
                            tasks.push(PrivateScalarTask::CheckWhile(condition, body));
                            tasks.push(PrivateScalarTask::ExecuteBranchAction(body));
                        }
                    }
                    PrivateScalarTask::ExecuteBranchAction(action) => match action {
                        PrivateScalarBranchAction::While { condition, body } => {
                            tasks.push(PrivateScalarTask::CheckWhile(condition, body));
                        }
                        PrivateScalarBranchAction::Break | PrivateScalarBranchAction::Continue => {
                            // 僅捨棄本次迭代尚未到達的工作，不裁掉父函式。
                            // 最近的 CheckWhile 是動態最內層循環，外層可繼續。
                            let is_break = matches!(action, PrivateScalarBranchAction::Break);
                            while !matches!(tasks.last(), Some(PrivateScalarTask::CheckWhile(..))) {
                                if matches!(tasks.last(), None | Some(PrivateScalarTask::ExitCall)) {
                                    return Err(PrivateScalarCallRuntimeError::StateMismatch);
                                }
                                tasks.pop();
                            }
                            if is_break { tasks.pop(); }
                        }
                        PrivateScalarBranchAction::NestedIf(plan) => {
                            // 只執行已選真分支中的內層 if；
                            // bool 條件在前面更新完的真動態框架重新讀取。
                            tasks.push(PrivateScalarTask::CheckEarlyReturn(plan));
                        }
                        PrivateScalarBranchAction::Sequence(actions) => {
                            // 原 Parser 分支先後次序必須保持；工作堆疊
                            // LIFO 以反序放入，未選中的分支不排任何工作。
                            for action in actions.iter().rev() {
                                tasks.push(PrivateScalarTask::ExecuteBranchAction(action));
                            }
                        }
                        PrivateScalarBranchAction::Return(returned) => {
                            // 提前 return 只裁掉目前函式尚未執行的語句，
                            // 保留目前 ExitCall 與呼叫者外層工作。
                            while !tasks.is_empty()
                                && !matches!(tasks.last(),
                                    Some(PrivateScalarTask::ExitCall)) {
                                tasks.pop();
                            }
                            tasks.push(PrivateScalarTask::Evaluate(returned, false));
                        }
                        PrivateScalarBranchAction::AssignInt { declaration, value } => {
                            tasks.push(PrivateScalarTask::AssignLocal(*declaration));
                            tasks.push(PrivateScalarTask::Evaluate(value, false));
                        }
                        PrivateScalarBranchAction::AssignBool { declaration, value } => {
                            tasks.push(PrivateScalarTask::AssignBool(*declaration, value));
                        }
                    },
                    PrivateScalarTask::EnterCall(name, argc, is_root_call) => {
                        // 遞迴可呼叫最初被 main 選中的函式；
                        // 它與其他原 Checker 函式均須使用各自參數宣告身分。
                        let (parameter_ids, parameter_is_bool, result_expr, local_steps): (
                            Vec<DeclarationId>,
                            Vec<bool>,
                            &PrivateScalarReturnExpr,
                            &Vec<PrivateScalarLocalStep>,
                        ) = if name == self.callee_name {
                            (
                                self.callee_parameter_ids.clone(),
                                self.callee_parameter_is_bool.clone(),
                                &self.return_expr,
                                &self.root_local_steps,
                            )
                        } else {
                            let callee = self.nested_callees.iter()
                                .find(|fun| fun.name == name)
                                .ok_or(PrivateScalarCallRuntimeError::StateMismatch)?;
                            (
                                callee.parameters.clone(), callee.parameter_is_bool.clone(),
                                &callee.result, &callee.local_steps,
                            )
                        };
                        if parameter_ids.len() != argc || parameter_is_bool.len() != argc
                            || values.len() < argc {
                            return Err(PrivateScalarCallRuntimeError::StateMismatch);
                        }
                        if frames.len() >= budget.max_call_frames {
                            return Err(PrivateScalarCallRuntimeError::CallBudget);
                        }
                        let evaluated = values.split_off(values.len() - argc);
                        events.push(PrivateScalarCallEvent::FunctionEntered(name.to_owned()));
                        let mut locals = BTreeMap::new();
                        let mut bool_locals = BTreeMap::new();
                        for ((id, is_bool), value) in parameter_ids.iter()
                            .zip(parameter_is_bool).zip(evaluated) {
                            if is_bool {
                                if !matches!(value, 0 | 1)
                                    || bool_locals.insert(*id, value == 1).is_some() {
                                    return Err(PrivateScalarCallRuntimeError::StateMismatch);
                                }
                                events.push(PrivateScalarCallEvent::BoolParameterBound(
                                    *id, value == 1,
                                ));
                            } else {
                                if locals.insert(*id, value).is_some() {
                                    return Err(PrivateScalarCallRuntimeError::StateMismatch);
                                }
                                events.push(PrivateScalarCallEvent::ParameterBound(*id, value));
                            }
                        }
                        frames.push(PrivateScalarActiveFrame {
                            name: name.to_owned(), values: locals,
                            bool_values: bool_locals,
                            initialized_order: Vec::new(),
                        });
                        // 只有源自 main return 最外層的真 Call 才允許
                        // 故障注入；先求值的同名子呼叫絕不可誤作根框架。
                        if fail_after_root_binding && is_root_call {
                            return Err(PrivateScalarCallRuntimeError::InjectedFailure);
                        }
                        tasks.push(PrivateScalarTask::ExitCall);
                        tasks.push(PrivateScalarTask::Evaluate(result_expr, false));
                        for step in local_steps.iter().rev() {
                            match step {
                                PrivateScalarLocalStep::Declare(init) => {
                                    tasks.push(PrivateScalarTask::BindLocal(init.declaration));
                                    tasks.push(PrivateScalarTask::Evaluate(&init.value, false));
                                }
                                PrivateScalarLocalStep::Assign { declaration, value } => {
                                    tasks.push(PrivateScalarTask::AssignLocal(*declaration));
                                    tasks.push(PrivateScalarTask::Evaluate(value, false));
                                }
                                PrivateScalarLocalStep::DeclareBool { declaration, value } =>
                                    tasks.push(PrivateScalarTask::BindBool(*declaration, value)),
                                PrivateScalarLocalStep::AssignBool { declaration, value } =>
                                    tasks.push(PrivateScalarTask::AssignBool(*declaration, value)),
                                PrivateScalarLocalStep::EarlyReturn(plan) =>
                                    tasks.push(PrivateScalarTask::CheckEarlyReturn(plan)),
                                PrivateScalarLocalStep::Control(action) =>
                                    tasks.push(PrivateScalarTask::ExecuteBranchAction(action)),
                            }
                        }
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
                        for entry in frame.initialized_order.iter().rev() {
                            match entry {
                                PrivateScalarLocalBinding::Int(id) =>
                                    events.push(PrivateScalarCallEvent::LocalScopeEnded(*id)),
                                PrivateScalarLocalBinding::Bool(id) =>
                                    events.push(PrivateScalarCallEvent::BoolScopeEnded(*id)),
                            }
                        }
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
                for entry in frame.initialized_order.iter().rev() {
                    match entry {
                        PrivateScalarLocalBinding::Int(id) =>
                            events.push(PrivateScalarCallEvent::LocalScopeEnded(*id)),
                        PrivateScalarLocalBinding::Bool(id) =>
                            events.push(PrivateScalarCallEvent::BoolScopeEnded(*id)),
                    }
                }
                frame.values.clear();
                events.push(PrivateScalarCallEvent::FunctionExited {
                    name: frame.name, returned: false,
                });
            }
        }
        // 直接 main→被呼叫者的真框架保存在最底層，亦必須在
        // 正常返回、注入故障或預算耗盡後按完成順序逆序離域。
        if let Some(root) = frames.first_mut() {
            for entry in root.initialized_order.iter().rev() {
                match entry {
                    PrivateScalarLocalBinding::Int(id) =>
                        events.push(PrivateScalarCallEvent::LocalScopeEnded(*id)),
                    PrivateScalarLocalBinding::Bool(id) =>
                        events.push(PrivateScalarCallEvent::BoolScopeEnded(*id)),
                }
            }
            root.initialized_order.clear();
            root.values.clear();
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
        // V1：main 的括號 return 同樣是原 Checker 已核准的 int 表達式。
        // 逐層核對真 TypedHIR、Lexer 左右括號，保留最內層的原 AST
        // 才能將零／多參數函式交給同一個動態呼叫框架，不自行猜測語法。
        let mut return_call = &ret.expression;
        while let TermKind::Group(inner) = &return_call.kind {
            let span = return_call.node_span();
            if verified.plan.scalar_hir.expression(span)
                .is_none_or(|fact| fact.ty != ScalarType::Int)
                || !verified.tokens.iter().any(|token|
                    token.start == span.start
                        && matches!(token.kind, TokenKind::Symbol('(')))
                || !verified.tokens.iter().any(|token|
                    token.end == span.end
                        && matches!(token.kind, TokenKind::Symbol(')'))) {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "main 括號返回缺少真 Checker int HIR 與原 Lexer 詞彙",
                ));
            }
            return_call = inner;
        }
        // A2/F02：零參數 name() 仍以原完整函式宣告分類，
        // 非函式或結構建構式不得混入此受限純值執行器。
        // V1：main 前置已核准 int 初始化也可能包含唯一真函式呼叫；
        // 即使最後只是 return 先前取得的 Copyable 結果，仍可由
        // 原 Parser AST 找到同一已核准函式圖，不猜測字串名稱。
        let source_name = PrivateScalarReturnExpr::first_source_call(return_call)
            .or_else(|| main.body.statements.iter().find_map(|statement| {
                if let StmtKind::Declare(owner) = &statement.kind {
                    owner.initializer.as_ref()
                        .and_then(PrivateScalarReturnExpr::first_source_call)
                } else { None }
            }))
            .ok_or(PrivateScalarCallPrepareError::Unsupported(
                "main 來源缺少原 Parser 真具名函式呼叫",
            ))?;
        let callee = verified.functions.iter()
            .find(|fun| fun.name == source_name && fun.name != "main")
            .ok_or(PrivateScalarCallPrepareError::Unsupported(
                "main 呼叫缺少原 Checker 具名函式宣告",
            ))?;
        if main.result_ty != "int" || !main.params.is_empty()
            || callee.result_ty != "int"
            || callee.params.iter().any(|param|
                !matches!(param.ty.as_str(), "int" | "bool"))
            || callee.body.statements.is_empty()
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
            if !verified.plan.declarations.matches(id, &param.ty, false)
                || verified.plan.members.get(&member.identity) != Some(&member) {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "多參數缺少原 Checker const int 宣告及 P16 型別來源事實",
                ));
            }
        }
        let param = callee.params.first();
        let (root_local_inits, root_local_steps, return_expr, param_return_span) =
            Self::prepare_function_locals(callee, &verified)?;
        let mut nested_callees = Vec::new();
        for function in verified.functions.iter()
            .filter(|fun| fun.name != "main" && fun.name != callee.name) {
            if function.result_ty != "int"
                || function.params.iter().any(|param|
                    !matches!(param.ty.as_str(), "int" | "bool"))
                || function.body.statements.is_empty() {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "多層函式須為已核准單 return 的純 int 完整簽章",
                ));
            }
            let mut parameter_ids = Vec::new();
            let mut parameter_is_bool = Vec::new();
            for param in &function.params {
                let id = DeclarationId(param.declaration_start);
                let member = crate::common_ir::VerifiedTypedMember::checked_private(
                    text, crate::common_ir::NodeRole::Parameter,
                    param.declaration_span, &param.name, param.name_span,
                    &param.ty, param.type_span, false,
                ).ok_or(PrivateScalarCallPrepareError::Unsupported(
                    "巢狀函式參數缺少原 Parser/Lexer 型別身分",
                ))?;
                if !verified.plan.declarations.matches(id, &param.ty, false)
                    || verified.plan.members.get(&member.identity) != Some(&member) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "巢狀函式參數缺少原 Checker DeclarationId／P16 TypedMember",
                    ));
                }
                parameter_ids.push(id);
                parameter_is_bool.push(param.ty == "bool");
            }
            let (local_inits, local_steps, result, result_span) =
                Self::prepare_function_locals(function, &verified)?;
            nested_callees.push(PrivateScalarNestedCallee {
                name: function.name.clone(),
                span: NodeSpan::new(function.start, function.end),
                parameters: parameter_ids,
                parameter_is_bool,
                result_span,
                result,
                local_inits,
                local_steps,
            });
        }
        // 每個可達或不可達的已編譯函式 Call 必須在同一份
        // Checker 真來源中有完整目標；允許已核准直接／互相遞迴。
        // 是否真正執行由來源控制流決定，安全耗盡由 runtime 預算決定。
        let mut graph: Vec<(&str, &PrivateScalarReturnExpr)> =
            vec![(&callee.name, &return_expr)];
        graph.extend(nested_callees.iter().map(|fun| (fun.name.as_str(), &fun.result)));
        for (name, expression) in &graph {
            let mut calls = Vec::new();
            expression.collect_calls(&mut calls);
            let steps = if *name == callee.name {
                &root_local_steps
            } else {
                &nested_callees.iter().find(|fun| fun.name == *name)
                    .expect("已收錄的巢狀真函式").local_steps
            };
            for step in steps {
                step.collect_calls(&mut calls);
            }
            if calls.iter().any(|called|
                !graph.iter().any(|(candidate, _)| candidate == called)) {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "巢狀具名呼叫缺少原 Checker 真函式宣告或完整簽章",
                ));
            }
        }

        // main 的所有前置語句須真為已核准的 Copyable int 常值初始化。
        // 正常詞彙遮蔽／任意賦值另待 P18.2，不把尚未執行的語句跳過。
        let (last_statement, main_prefix) = main.body.statements.split_last()
            .expect("已確認 main 非空");
        let mut owners = BTreeMap::<String, (DeclarationId, i64)>::new();
        let mut main_owners = Vec::new();
        // 非字面／名稱的合法 main int 初始化可能含算術、真函式
        // 及溢位，不可在 Prepare 階段求值，也不能省略整筆初始化。
        let needs_main_local_runtime = main_prefix.iter().any(|statement| {
            if matches!(&statement.kind, StmtKind::Assign(_) | StmtKind::If { .. }) {
                return true;
            }
            if let StmtKind::Declare(owner) = &statement.kind {
                if owner.ty == "bool" { return true; }
            }
            if let StmtKind::Declare(owner) = &statement.kind {
                if let Some(initializer) = &owner.initializer {
                    let mut inner = initializer;
                    while let TermKind::Group(next) = &inner.kind { inner = next; }
                    return !matches!(&inner.kind,
                        TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(_))
                        | TermKind::Name(_));
                }
            }
            false
        }) || PrivateScalarReturnExpr::first_source_call(return_call).is_none();
        if !needs_main_local_runtime {
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
            // Group 常值仍須逐層經原 Lexer／Checker 證明為 int；
            // 只移除無副作用括號，不把任意式子在 prepare 階段預求值。
            let mut actual_init = init;
            while let TermKind::Group(inner) = &actual_init.kind {
                let span = actual_init.node_span();
                if verified.plan.scalar_hir.expression(span)
                    .is_none_or(|fact| fact.ty != ScalarType::Int)
                    || !verified.tokens.iter().any(|token|
                        token.start == span.start
                            && matches!(token.kind, TokenKind::Symbol('(')))
                    || !verified.tokens.iter().any(|token|
                        token.end == span.end
                            && matches!(token.kind, TokenKind::Symbol(')'))) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "main Group 初始化缺少真 Checker int HIR 或括號詞彙",
                    ));
                }
                actual_init = inner;
            }
            if owner.ty != "int"
                || !verified.plan.declarations.matches(id, "int", owner.mutable)
                || verified.plan.scalar_hir.expression(actual_init.node_span())
                    .is_none_or(|fact| fact.ty != ScalarType::Int)
                || owners.contains_key(&owner.name) {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "main 具名 int 初始值缺少唯一 Checker／TypedHIR 宣告身分",
                ));
            }
            let value = match &actual_init.kind {
                TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) => {
                    if !verified.tokens.iter().any(|token|
                        token.start == actual_init.start && token.end == actual_init.end
                            && matches!(&token.kind, TokenKind::Integer(found) if found == raw)) {
                        return Err(PrivateScalarCallPrepareError::Unsupported(
                            "main int 字面初始化缺少原 Lexer Token",
                        ));
                    }
                    crate::syntax_shared::integer_i64(raw).map_err(|_|
                        PrivateScalarCallPrepareError::Unsupported(
                            "main int 詞彙初始化值超出 i64",
                        ),
                    )?
                }
                TermKind::Name(name) => {
                    // 已核准 Copyable int 可由同一 main 先前已初始化持有者取值，
                    // 不產生 Move／BorrowTicket。原 Checker Reference HIR 的
                    // DeclarationId 和 Lexer token 都必須逐一相符。
                    let (source_id, value) = owners.get(name).copied().ok_or(
                        PrivateScalarCallPrepareError::Unsupported(
                            "main Copyable int 初始化來源不是先前已驗證的持有者",
                        ),
                    )?;
                    if verified.plan.scalar_hir.reference(actual_init.node_span())
                        .is_none_or(|fact|
                            fact.resolved.declaration != source_id
                                || fact.resolved.ty != ScalarType::Int)
                        || !verified.tokens.iter().any(|token|
                            token.start == actual_init.start
                                && token.end == actual_init.end
                                && matches!(&token.kind, TokenKind::Name(found) if found == name)) {
                        return Err(PrivateScalarCallPrepareError::Unsupported(
                            "main Copyable int 初始化來源缺少原 Checker Reference HIR",
                        ));
                    }
                    value
                }
                _ => return Err(PrivateScalarCallPrepareError::Unsupported(
                    "main 此子集只接通已核准 Copyable int 字面或具名初始化",
                )),
            };
            owners.insert(owner.name.clone(), (id, value));
            main_owners.push((id, value));
        }
        }

        let StmtKind::Return(_) = &last_statement.kind else {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "main 最後一筆必須是真普通函式呼叫 return",
            ));
        };
        let (call_name, call_arguments, direct_call): (&str, &[Term], bool) =
            match &return_call.kind {
                TermKind::Call(call) =>
                    (call.name.as_str(), call.arguments.as_slice(), true),
                TermKind::UnresolvedZeroArgs(name) => (name.as_str(), &[], true),
                // main 可 return 原 Checker 已核准的多函式 int 算式，
                // 但仍不得把純算術當成已有函式動態證據。
                TermKind::BinaryInt(_) | TermKind::Name(_) if needs_main_local_runtime =>
                    (source_name, &[], false),
                TermKind::BinaryInt(_) => (source_name, &[], false),
                _ => return Err(PrivateScalarCallPrepareError::Unsupported(
                    "main 最後一筆須為已核准 int 函式或函式複合返回式",
                )),
            };
        if direct_call
            && (call_name != callee.name || call_arguments.len() != callee.params.len()) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "原 Parser 呼叫名稱／參數個數與真 Checker 函式簽章不符",
            ));
        }
        let call_span = return_call.node_span();
        if verified.plan.scalar_hir.expression(call_span)
            .is_none_or(|fact| fact.ty != ScalarType::Int)
            || verified.plan.declarations.call_borrows_at(call_span)
                .is_some_and(|facts| !facts.is_empty()) {
            return Err(PrivateScalarCallPrepareError::Unsupported(
                "呼叫缺少 int TypedHIR 或 Copyable 實參遭誤記為借用",
            ));
        }
        // 已核准的 main 動態 int 表達式與巢狀具名函式呼叫不能在
        // Prepare 階段預先求值。完整原 Checker HIR 先建立唯一
        // 表達式工作計畫，執行時才在 main 的真 int 持有者框架求值。
        let needs_dynamic = needs_main_local_runtime || !direct_call
            || callee.params.iter().any(|param| param.ty == "bool")
            || call_arguments.iter().any(|argument| {
            let mut inner = argument;
            while let TermKind::Group(nested) = &inner.kind {
                inner = nested;
            }
            !matches!(&inner.kind,
                TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(_))
                | TermKind::Name(_))
        });
        let mut main_dynamic_inits = Vec::new();
        let mut main_dynamic_steps = Vec::new();
        let main_dynamic_call = if needs_main_local_runtime {
            let (inits, steps, result, _) =
                Self::prepare_function_locals(main, &verified)?;
            main_dynamic_inits = inits;
            main_dynamic_steps = steps;
            Some(result)
        } else if needs_dynamic {
            let bindings = owners.iter().map(|(name, (id, _))|
                (name.clone(), *id)
            ).collect::<BTreeMap<_, _>>();
            if direct_call {
                let arguments = call_arguments.iter().zip(&callee.params).map(
                    |(argument, parameter)| {
                        if parameter.ty == "bool" {
                            Ok(PrivateScalarCallArgument::Bool(
                                PrivateScalarBoolExpr::from_checked(
                                    argument, &bindings, &verified,
                                )?,
                            ))
                        } else {
                            Ok(PrivateScalarCallArgument::Int(
                                PrivateScalarReturnExpr::from_checked_bindings(
                                    argument, &bindings, &verified,
                                )?,
                            ))
                        }
                    }
                ).collect::<Result<Vec<_>, PrivateScalarCallPrepareError>>()?;
                Some(PrivateScalarReturnExpr::Call {
                    name: callee.name.clone(), arguments,
                })
            } else {
                Some(PrivateScalarReturnExpr::from_checked_bindings(
                    return_call, &bindings, &verified,
                )?)
            }
        } else {
            None
        };
        let mut all_arguments = Vec::new();
        if main_dynamic_call.is_none() {
        for (arg, parameter) in call_arguments.iter().zip(&callee.params) {
            // 已核准的 Group 由原 Parser/TypedHIR 提供完整來源，
            // 不額外解釋未核准的表達式或改變 Copyable 權限。
            let mut actual = arg;
            while let TermKind::Group(inner) = &actual.kind {
                if verified.plan.scalar_hir.expression(actual.node_span())
                    .is_none_or(|fact| fact.ty != ScalarType::Int)
                    || !verified.tokens.iter().any(|token|
                        token.start == actual.start
                            && matches!(token.kind, TokenKind::Symbol('(')))
                    || !verified.tokens.iter().any(|token|
                        token.end == actual.end
                            && matches!(token.kind, TokenKind::Symbol(')'))) {
                    return Err(PrivateScalarCallPrepareError::Unsupported(
                        "Group 引數缺少已核准的同源 int 型別與括號詞彙",
                    ));
                }
                actual = inner;
            }
            let arg = actual;
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
        }
        // 前置宣告已逐筆由原 Checker、Lexer 與 int TypedHIR 驗證為
        // 無資源的純整數常值初始化；即使未被用作實參，也不能
        // 將合法宣告誤判成「未執行語句」。其身分與值仍納入防偽。
        let (parameter_id, argument, owner_id) = match all_arguments.first().copied() {
            Some((id, value, owner)) => (Some(id), Some(value), owner),
            None => (None, None, None),
        };
        let additional_arguments = all_arguments.iter().skip(1).copied().collect();
        let callee_span = NodeSpan::new(callee.start, callee.end);
        let callee_name = callee.name.clone();
        let parameter_name = param.map(|param| param.name.clone());
        let callee_parameter_ids = callee.params.iter().map(|param|
            DeclarationId(param.declaration_start)
        ).collect();
        let callee_parameter_is_bool = callee.params.iter()
            .map(|param| param.ty == "bool").collect();
        let original = verified.source.bytes.clone().into_boxed_slice();
        Ok(Self {
            verified, original, call_span, callee_span, callee_name,
            parameter_name, parameter_id,
            parameter_return_span: param_return_span, return_expr,
            root_local_inits, root_local_steps, owner_id, argument,
            additional_arguments, main_owners, nested_callees,
            main_dynamic_call, main_dynamic_inits, main_dynamic_steps,
            callee_parameter_ids, callee_parameter_is_bool,
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
            && self.root_local_inits == fresh.root_local_inits
            && self.root_local_steps == fresh.root_local_steps
            && self.owner_id == fresh.owner_id
            && self.argument == fresh.argument
            && self.additional_arguments == fresh.additional_arguments
            && self.main_owners == fresh.main_owners
            && self.nested_callees == fresh.nested_callees
            && self.main_dynamic_call == fresh.main_dynamic_call
            && self.main_dynamic_inits == fresh.main_dynamic_inits
            && self.main_dynamic_steps == fresh.main_dynamic_steps
            && self.callee_parameter_ids == fresh.callee_parameter_ids
            && self.callee_parameter_is_bool == fresh.callee_parameter_is_bool
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

    fn audit_outcome(
        outcome: PrivateScalarCallOutcome,
    ) -> Result<PrivateScalarCallOutcome, PrivateScalarCallPrepareError> {
        if outcome.has_balanced_frame_trace() {
            Ok(outcome)
        } else {
            Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)
        }
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
            return Self::audit_outcome(PrivateScalarCallOutcome {
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
        if let Some(dynamic_call) = &self.main_dynamic_call {
            // 純 int 複合返回可能有數個兄弟 Call，沒有可供
            //「根函式綁定後」故障注入的唯一目標；此測試介面
            // 不得假造單一根框架或默默忽略故障要求。
            if fail_after_binding
                && !matches!(dynamic_call, PrivateScalarReturnExpr::Call { .. }) {
                return Err(PrivateScalarCallPrepareError::Unsupported(
                    "main 複合 int 返回沒有唯一故障注入根呼叫",
                ));
            }
            let main_frame: BTreeMap<DeclarationId, i64> =
                self.main_owners.iter().copied().collect();
            // 這個額外的 frame 只代表執行器的 main 來源持有者；
            // 不是函式呼叫，不能占用 D18-01 的函式框架額度。
            let adjusted_budget = PrivateScalarExecutionBudget {
                max_call_frames: budget.max_call_frames.saturating_add(1),
                max_steps: budget.max_steps,
            };
            let result = self.evaluate_graph(
                dynamic_call, &main_frame, &mut events,
                adjusted_budget, fail_after_binding, &self.main_dynamic_steps,
            );
            return Self::audit_outcome(PrivateScalarCallOutcome {
                result, events, parameter_slots_after_exit: 0,
            });
        }
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
                self.evaluate_graph(&self.return_expr, &frame, &mut events, budget, false, &self.root_local_steps)
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
        Self::audit_outcome(PrivateScalarCallOutcome {
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
    fn g18_a_多參數錯誤型別引數數量拒絕且未使用純值宣告合法() {
        for text in [
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1,true)\n}\n",
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1)\n}\n",
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1,2,3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(text),
                Err(PrivateScalarCallPrepareError::Check(_))), "{text}");
        }
        let unused = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nvar extra:int = 5\nreturn sum(1,2)\n}\n";
        let prepared = checked(unused);
        assert_eq!(prepared.main_owners.len(), 1);
        assert_eq!(prepared.execute().unwrap().result, Ok(3));
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



    #[test]
    fn g18_a_main_零參數呼叫真正建立空參數框架並返回() {
        let src = "fn value()->int{\nreturn 9\n}\nfn main()->int{\nreturn value()\n}\n";
        let prepared = checked(src);
        assert!(prepared.parameter_id.is_none());
        assert!(prepared.argument.is_none());
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(9));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events, vec![
            PrivateScalarCallEvent::FunctionEntered("value".to_owned()),
            PrivateScalarCallEvent::Returned(9),
            PrivateScalarCallEvent::FunctionExited {
                name: "value".to_owned(), returned: true,
            },
        ]);
    }

    #[test]
    fn g18_a_main_零參數前向函式串接巢狀真返回() {
        let src = "fn first()->int{\nreturn second()\n}\nfn main()->int{\nreturn first()\n}\nfn second()->int{\nreturn 11\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(11));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert!(!outcome.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::ArgumentEvaluated(_)
                | PrivateScalarCallEvent::ParameterBound(..)
        )));
        let enters = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(enters, ["first", "second"]);
        let returns = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returns, [11, 11]);
    }

    #[test]
    fn g18_c_main_零參數根函式直接遞迴預算耗盡及逐框架退出() {
        let src = "fn repeat()->int{\nreturn repeat()\n}\nfn main()->int{\nreturn repeat()\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 4, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(_)
        )).count(), 4);
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited { returned: false, .. }
        )).count(), 4);
        assert!(!outcome.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
                | PrivateScalarCallEvent::ArgumentEvaluated(_)
                | PrivateScalarCallEvent::ParameterBound(..)
        )));
    }

    #[test]
    fn g18_a_main_零參數來源被偽造一個參數必執行前拒絕() {
        let src = "fn value()->int{\nreturn 9\n}\nfn main()->int{\nreturn value()\n}\n";
        let mut prepared = checked(src);
        assert!(prepared.parameter_id.is_none());
        prepared.parameter_id = Some(DeclarationId(usize::MAX));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(src);
        prepared.argument = Some(123);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }



    #[test]
    fn g18_a_main_原_checker_括號常值引數真正進入雙參數框架() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum((5),(7))\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(12));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [5, 7]);
    }

    #[test]
    fn g18_a_main_多層括號具名_copyable_來源仍維持原宣告身分() {
        let src = "fn identity(n:int)->int{\nreturn n\n}\nfn main()->int{\nconst original:int = 11\nreturn identity((((original))))\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.owner_id, Some(prepared.main_owners[0].0));
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(11));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_main_括號布林與未知名稱不可充當合法_int_實參() {
        for src in [
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nreturn id((true))\n}\n",
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nreturn id(((missing)))\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }


    #[test]
    fn g18_a_數個未使用純值宣告與具名實參各自保留來源身分() {
        let src = "fn id(value:int)->int{\nreturn value\n}\nfn main()->int{\nconst unused:int = 8\nvar input:int = 4\nvar another:int = 9\nreturn id(input)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 3);
        assert!(prepared.owner_id.is_some());
        for _ in 0..2 {
            let result = prepared.execute().unwrap();
            assert_eq!(result.result, Ok(4));
            assert_eq!(result.parameter_slots_after_exit, 0);
            assert_eq!(result.events.iter().filter(|event| matches!(
                event, PrivateScalarCallEvent::ArgumentEvaluated(_)
            )).count(), 1);
        }
    }

    #[test]
    fn g18_a_未使用純值宣告遭竄改仍須執行前拒絕() {
        let src = "fn id(value:int)->int{\nreturn value\n}\nfn main()->int{\nconst unused:int = 8\nreturn id(3)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(3));
        prepared.main_owners[0].1 = 999;
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let changed = src.replace("unused:int = 8", "unused:int = 9");
        assert!(matches!(PreparedPrivateScalarCall::prepare(&changed),
            Ok(_)));
        assert_ne!(prepared.original.as_ref(), changed.as_bytes());
    }



    #[test]
    fn g18_a_main_括號返回帶參數普通函式依原_checker_真求值() {
        let src = "fn inc(value:int)->int{\nreturn value+1\n}\nfn main()->int{\nreturn (((inc(7))))\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(8));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let arguments = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(arguments, [7]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 1);
    }

    #[test]
    fn g18_a_main_多層括號零參數前向函式返回不虛構參數() {
        let src = "fn main()->int{\nreturn (((value())))\n}\nfn value()->int{\nreturn 23\n}\n";
        let prepared = checked(src);
        assert!(prepared.parameter_id.is_none());
        assert!(prepared.argument.is_none());
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(23));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ArgumentEvaluated(_)
                | PrivateScalarCallEvent::ParameterBound(..))).count(), 0);
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_main_括號返回的真內層呼叫跨度不得竄改() {
        let src = "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nreturn ((id(6)))\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(6));
        prepared.call_span = crate::common_ir::NodeSpan::new(0, 2);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_main_括號返回錯型別與不存在函式由原_checker_拒絕() {
        for src in [
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nreturn ((id(true)))\n}\n",
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nreturn ((missing(6)))\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }


    #[test]
    fn g18_a_main_具名_copyable_int_多層括號初始化仍真傳入函式() {
        let src = "fn inc(x:int)->int{\nreturn x+1\n}\nfn main()->int{\nconst base:int = ((((6))))\nreturn inc(base)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 1);
        assert_eq!(prepared.main_owners[0].1, 6);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(7));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_main_未使用的合法_group_int_初始化不應阻擋零參數函式() {
        let src = "fn fixed()->int{\nreturn 17\n}\nfn main()->int{\nvar unused:int = ((8))\nreturn fixed()\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 1);
        assert_eq!(prepared.main_owners[0].1, 8);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(17));
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ArgumentEvaluated(_)
                | PrivateScalarCallEvent::ParameterBound(..)
        )).count(), 0);
    }

    #[test]
    fn g18_a_main_括號初始化為布林或缺失名稱仍由原_checker_拒絕() {
        for src in [
            "fn id(x:int)->int{\nreturn x\n}\nfn main()->int{\nconst owner:int = ((true))\nreturn id(owner)\n}\n",
            "fn id(x:int)->int{\nreturn x\n}\nfn main()->int{\nconst owner:int = ((missing))\nreturn id(owner)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_a_main_括號初始化持有者遭竄改不得建立動態框架() {
        let src = "fn id(x:int)->int{\nreturn x\n}\nfn main()->int{\nvar owner:int = (((9)))\nreturn id(owner)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(9));
        prepared.main_owners[0].1 = 90;
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_a_main_已核准_copyable_int_名稱初始化串接真傳參() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst origin:int = 6\nvar copied:int = origin\nconst further:int = copied\nreturn inc(further)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 3);
        assert_eq!(prepared.main_owners.iter().map(|(_, value)| *value).collect::<Vec<_>>(),
            [6, 6, 6]);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(7));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ArgumentEvaluated(6)
        )).count(), 1);
    }

    #[test]
    fn g18_a_main_括號名稱初始化與未使用副本不消耗來源() {
        let src = "fn fixed()->int{\nreturn 13\n}\nfn main()->int{\nconst origin:int = ((9))\nvar unused:int = ((((origin))))\nreturn fixed()\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 2);
        assert_eq!(prepared.main_owners[0].1, 9);
        assert_eq!(prepared.main_owners[1].1, 9);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(13));
        assert_eq!(outcome.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::ArgumentEvaluated(_)
                | PrivateScalarCallEvent::ParameterBound(..)
        )).count(), 0);
    }

    #[test]
    fn g18_a_main_初始化來源錯型別或未宣告及尚未初始化仍靜態拒絕() {
        for src in [
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nconst ok:bool = true\nconst copy:int = ok\nreturn id(copy)\n}\n",
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nconst copy:int = missing\nreturn id(copy)\n}\n",
            "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nvar original:int\nconst copy:int = original\nreturn id(copy)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_a_main_被複製初始化持有者來源身分竄改必先拒絕() {
        let src = "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nconst origin:int = 3\nconst copy:int = origin\nreturn id(copy)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(3));
        prepared.main_owners[1].0 = DeclarationId(usize::MAX);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
        let mut prepared = checked(src);
        prepared.main_owners[1].1 = 31;
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_a_main_原_checker_算術實參於真動態框架依順序求值() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1+2,7-4)\n}\n";
        let prepared = checked(src);
        assert!(prepared.main_dynamic_call.is_some());
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(6));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [3, 3]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "sum")).count(), 1);
    }

    #[test]
    fn g18_a_main_具名_copyable_算術與巢狀普通函式依真來源求值() {
        let src = "fn inc(v:int)->int{\nreturn v+1\n}\nfn combine(a:int,b:int)->int{\nreturn a-b\n}\nfn main()->int{\nconst origin:int = 5\nreturn combine(inc(origin),inc(2+1))\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_owners.len(), 1);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(2));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let enters = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(enters, ["inc", "inc", "combine"]);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [5, 6, 3, 4]);
    }

    #[test]
    fn g18_a_main_零參數葉函式作為複合實參須先真返回再綁定目標() {
        let src = "fn fixed()->int{\nreturn 4\n}\nfn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(fixed(),6)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Ok(10));
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["fixed", "sum"]);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [4, 6]);
    }

    #[test]
    fn g18_c_main_第二算術實參溢位保留前一完成事件且不可進入目標() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nconst high:int = 9223372036854775807\nreturn sum(5+1,high+1)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [6]);
        assert!(!outcome.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::FunctionEntered(_)
                | PrivateScalarCallEvent::Returned(_)
                | PrivateScalarCallEvent::ParameterBound(..)
        )));
    }

    #[test]
    fn g18_c_main_算術實參步數預算耗盡不會補寫未完成參數事件() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1+2,3+4)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 5, max_steps: 6 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [3]);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))));
    }

    #[test]
    fn g18_c_main_動態實參綁定後故障須先退出真根框架() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1+2,4)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute_with_fault(true).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))).count(), 2);
        assert_eq!(outcome.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "sum".to_owned(), returned: false,
        }));
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(prepared.execute().unwrap().result, Ok(7));
    }

    #[test]
    fn g18_a_main_動態算式已驗證工作計畫遭竄改必在執行前拒絕() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1+2,3)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(6));
        prepared.main_dynamic_call = Some(PrivateScalarReturnExpr::Constant(99));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_main_動態實參非法型別或未知名稱仍須由原_checker_拒絕() {
        for src in [
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(true+2,3)\n}\n",
            "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(2+missing,3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }


    #[test]
    fn g18_c_main_同名巢狀實參故障只能中斷真正外層函式框架() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(sum(1,2),3)\n}\n";
        let prepared = checked(src);
        let outcome = prepared.execute_with_fault(true).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let enters = outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "sum")).count();
        assert_eq!(enters, 2);
        let successes = outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: true
            } if name == "sum")).count();
        let failures = outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: false
            } if name == "sum")).count();
        assert_eq!((successes, failures), (1, 1));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(3))).count(), 1);
        assert_eq!(prepared.execute().unwrap().result, Ok(6));
    }

    #[test]
    fn g18_c_main_不同名子函式先成功後故障只注入根函式綁定() {
        let src = "fn inc(x:int)->int{\nreturn x+1\n}\nfn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(inc(4),2)\n}\n";
        let outcome = checked(src).execute_with_fault(true).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        let exits = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionExited { name, returned } =>
                Some((name.as_str(), *returned)),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(exits, [("inc", true), ("sum", false)]);
        assert!(outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(5))));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_a_main_動態實參的來源框架不占用唯一使用者函式預算() {
        let src = "fn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(1+2,3+4)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 1, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Ok(10));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 1);
    }

    #[test]
    fn g18_a_main_巢狀實參先返回後可合法復用唯一框架額度() {
        let src = "fn inc(x:int)->int{\nreturn x+1\n}\nfn sum(a:int,b:int)->int{\nreturn a+b\n}\nfn main()->int{\nreturn sum(inc(1),3)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 1, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Ok(5));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["inc", "sum"]);
        let args = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [1, 2, 3]);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: true
            } if name == "inc")).count(), 1);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {
                name, returned: true
            } if name == "sum")).count(), 1);
    }


    #[test]
    fn g18_c_main_真正被呼叫者本體巢狀超單框架預算不可虛構葉函式進入() {
        let src = "fn leaf(x:int)->int{\nreturn x+1\n}\nfn outer(x:int)->int{\nreturn leaf(x)\n}\nfn main()->int{\nreturn outer(1+2)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 1, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["outer"]);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(outcome.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "outer".into(), returned: false,
        }));
    }


    #[test]
    fn g18_a_main_普通函式返回值與常值相加由同一真動態工作堆疊完成() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(5)+2\n}\n";
        let prepared = checked(src);
        assert!(prepared.main_dynamic_call.is_some());
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(6))).count(), 1);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "inc")).count(), 1);
    }

    #[test]
    fn g18_a_main_兩個兄弟函式合成返回必照原_checker_左右順序執行() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(2)+inc(4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert_eq!(out.parameter_slots_after_exit, 0);
        let args = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ArgumentEvaluated(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(args, [2, 4]);
        let returns = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returns, [3, 5]);
    }

    #[test]
    fn g18_a_main_多層括號包含函式複合返回保持原_parser_int_來源() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn ((inc(3)+inc(4)))\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(9));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 2);
    }

    #[test]
    fn g18_c_main_子函式已真返回後最外層算術溢位不得冒充成功() {
        let src = "fn id(n:int)->int{\nreturn n\n}\nfn main()->int{\nconst high:int = 9223372036854775807\nreturn id(high)+1\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::Returned(9223372036854775807)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::FunctionExited {
                name, returned: true
            } if name == "id"
        )).count(), 1);
    }

    #[test]
    fn g18_a_main_複合返回工作計畫被竄改必在任何函式執行前拒絕() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(2)+3\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(6));
        prepared.main_dynamic_call = Some(PrivateScalarReturnExpr::Constant(999));
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_main_複合函式返回與非法型別或未知名稱仍由原_checker_拒絕() {
        for src in [
            "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(1)+true\n}\n",
            "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(1)+missing\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_c_main_多兄弟複合返回不得假造單一根函式故障注入() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nreturn inc(2)+inc(4)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(8));
        assert!(matches!(prepared.execute_with_fault(true),
            Err(PrivateScalarCallPrepareError::Unsupported(_))));
    }


    #[test]
    fn g18_a_函式參數與多個純值區域初始化依_checker_順序真返回() {
        let src = "fn calc(n:int)->int{\nconst one:int = 1\nvar next:int = n+one\nreturn next+2\n}\nfn main()->int{\nreturn calc(5)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.root_local_inits.len(), 2);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(8));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))).count(), 1);
    }

    #[test]
    fn g18_a_巢狀函式各自區域宣告不與父框架參數混用() {
        let src = "fn outer(n:int)->int{\nconst result:int = leaf(n+1)\nreturn result+1\n}\nfn leaf(x:int)->int{\nconst y:int = x+2\nreturn y\n}\nfn main()->int{\nreturn outer(3)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.root_local_inits.len(), 1);
        assert_eq!(prepared.nested_callees.iter().find(|f| f.name == "leaf").unwrap().local_inits.len(), 1);
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(7));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let names = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(names, ["outer", "leaf"]);
    }

    #[test]
    fn g18_a_真動態_main_實參進入帶區域宣告函式仍唯一返回() {
        let src = "fn calc(n:int)->int{\nconst extra:int = n+2\nreturn extra\n}\nfn main()->int{\nreturn calc(2+3)\n}\n";
        let prepared = checked(src);
        assert!(prepared.main_dynamic_call.is_some());
        assert_eq!(prepared.execute().unwrap().result, Ok(7));
    }

    #[test]
    fn g18_c_函式區域算術溢位必退出而不得虛構返回() {
        let src = "fn fail(n:int)->int{\nconst too_big:int = n+1\nreturn too_big\n}\nfn main()->int{\nreturn fail(9223372036854775807)\n}\n";
        let outcome = checked(src).execute().unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))).count(), 0);
        assert_eq!(outcome.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "fail".into(), returned: false,
        }));
    }

    #[test]
    fn g18_c_局部整數宣告跨多個遞迴框架不得共用儲存位置() {
        let src = "fn repeat(n:int)->int{\nconst next:int = n+1\nreturn repeat(next)\n}\nfn main()->int{\nreturn repeat(1)\n}\n";
        let outcome = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 4, max_steps: 100 },
        ).unwrap();
        assert_eq!(outcome.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        let entered = outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count();
        let exited = outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionExited {returned: false, ..})).count();
        assert_eq!((entered, exited), (4, 4));
    }

    #[test]
    fn g18_a_函式區域宣告工作計畫遭偽造必在執行前拒絕() {
        let src = "fn calc(n:int)->int{\nconst next:int = n+1\nreturn next\n}\nfn main()->int{\nreturn calc(2)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(3));
        prepared.root_local_inits[0].value = PrivateScalarReturnExpr::Constant(99);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_函式區域初始化錯誤型別與未知參照由原_checker_拒絕() {
        for src in [
            "fn calc(n:int)->int{\nconst x:int = true\nreturn n\n}\nfn main()->int{\nreturn calc(2)\n}\n",
            "fn calc(n:int)->int{\nconst x:int = missing\nreturn x\n}\nfn main()->int{\nreturn calc(2)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }


    #[test]
    fn g18_a_兩筆真區域初始化與逆序離域事件保持同源宣告身分() {
        let src = "fn calc(n:int)->int{\nconst first:int = n+1\nvar second:int = first+2\nreturn second\n}\nfn main()->int{\nreturn calc(3)\n}\n";
        let prepared = checked(src);
        let ids = prepared.root_local_inits.iter()
            .map(|init| init.declaration).collect::<Vec<_>>();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(6));
        let created = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalInitialized(id, value) => Some((*id, *value)),
            _ => None,
        }).collect::<Vec<_>>();
        let ended = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalScopeEnded(id) => Some(*id),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(created, [(ids[0], 4), (ids[1], 6)]);
        assert_eq!(ended, [ids[1], ids[0]]);
        assert_eq!(out.parameter_slots_after_exit, 0);
    }

    #[test]
    fn g18_c_第二局部初始化運算溢位只釋放已完成宣告() {
        let src = "fn calc(n:int)->int{\nconst first:int = n+1\nconst second:int = n+9223372036854775807\nreturn second\n}\nfn main()->int{\nreturn calc(1)\n}\n";
        let prepared = checked(src);
        let first_id = prepared.root_local_inits[0].declaration;
        let second_id = prepared.root_local_inits[1].declaration;
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalInitialized(first_id, 2)));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalScopeEnded(first_id)));
        assert!(!out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::LocalInitialized(id, _)
                | PrivateScalarCallEvent::LocalScopeEnded(id) if *id == second_id
        )));
        assert_eq!(out.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "calc".into(), returned: false,
        }));
    }

    #[test]
    fn g18_c_遞迴中相同靜態局部宣告每個動態框架均獨立初始化及離域() {
        let src = "fn again(n:int)->int{\nconst local:int = n+1\nreturn again(local)\n}\nfn main()->int{\nreturn again(1)\n}\n";
        let prepared = checked(src);
        let id = prepared.root_local_inits[0].declaration;
        let out = prepared.execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 3, max_steps: 100 },
        ).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert_eq!(out.parameter_slots_after_exit, 0);
        let values = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalInitialized(actual, value) if *actual == id => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(values, [2, 3, 4]);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::LocalScopeEnded(actual) if *actual == id)
        ).count(), 3);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))).count(), 0);
    }


    #[test]
    fn g18_a_main_動態整數初始化應真執行後再傳入普通函式() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst base:int = 2+3\nreturn inc(base)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_dynamic_inits.len(), 1);
        assert!(prepared.main_owners.is_empty());
        let id = prepared.main_dynamic_inits[0].declaration;
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalInitialized(id, 5)));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalScopeEnded(id)));
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "inc")).count(), 1);
    }

    #[test]
    fn g18_a_main_函式結果可先存入區域再用純值算術返回() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst result:int = inc(4)\nreturn result+1\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_dynamic_inits.len(), 1);
        let out = prepared.execute().unwrap();
        // inc(4) = 5，main 再加 1，真正結果為 6。
        assert_eq!(out.result, Ok(6));
        assert_eq!(out.parameter_slots_after_exit, 0);
        let local = prepared.main_dynamic_inits[0].declaration;
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalInitialized(local, 5)));
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(5))).count(), 1);
    }

    #[test]
    fn g18_a_main_前向零參數結果可作為初始化再跨局部複製() {
        let src = "fn main()->int{\nconst first:int = fixed()\nvar copy:int = first\nreturn inc(copy)\n}\nfn inc(n:int)->int{\nreturn n+1\n}\nfn fixed()->int{\nreturn 8\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_dynamic_inits.len(), 2);
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Ok(9));
        let inits = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalInitialized(id, value) => Some((*id, *value)),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(inits.len(), 2);
        assert_eq!(inits.iter().map(|(_, value)| *value).collect::<Vec<_>>(), [8, 8]);
    }

    #[test]
    fn g18_c_main_動態初始化中後筆溢位不得跳過前筆真離域() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst one:int = inc(1)\nconst too_big:int = 9223372036854775807+one\nreturn inc(one)\n}\n";
        let prepared = checked(src);
        assert_eq!(prepared.main_dynamic_inits.len(), 2);
        let first = prepared.main_dynamic_inits[0].declaration;
        let second = prepared.main_dynamic_inits[1].declaration;
        let out = prepared.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalInitialized(first, 2)));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalScopeEnded(first)));
        assert!(!out.events.iter().any(|event| matches!(event,
            PrivateScalarCallEvent::LocalInitialized(id, _)
                | PrivateScalarCallEvent::LocalScopeEnded(id) if *id == second
        )));
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(name) if name == "inc")).count(), 1);
    }

    #[test]
    fn g18_a_main_真動態初始化計畫被篡改必在執行前拒絕() {
        let src = "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst base:int = 2+3\nreturn inc(base)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(6));
        prepared.main_dynamic_inits[0].value = PrivateScalarReturnExpr::Constant(500);
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_a_main_非法動態初始化型別及未宣告來源仍交由原_checker_拒絕() {
        for src in [
            "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst base:int = true+3\nreturn inc(base)\n}\n",
            "fn inc(n:int)->int{\nreturn n+1\n}\nfn main()->int{\nconst base:int = missing+3\nreturn inc(base)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }


    #[test]
    fn g18_r01_r02_函式內多筆可寫純值賦值必依原_checker_語句次序() {
        let source = "fn calculate(n:int)->int{\nvar current:int = n+1\ncurrent = current+2\ncurrent = current+3\nreturn current\n}\nfn main()->int{\nreturn calculate(5)\n}\n";
        let checked = checked(source);
        assert_eq!(checked.root_local_inits.len(), 1);
        assert_eq!(checked.root_local_steps.len(), 3);
        let id = checked.root_local_inits[0].declaration;
        let result = checked.execute().unwrap();
        assert_eq!(result.result, Ok(11));
        assert_eq!(result.parameter_slots_after_exit, 0);
        let changes = result.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalAssigned(key, value) if *key == id => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(changes, [8, 11]);
        assert!(result.events.contains(&PrivateScalarCallEvent::LocalInitialized(id, 6)));
        assert_eq!(result.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::LocalScopeEnded(key) if *key == id)).count(), 1);
    }

    #[test]
    fn g18_r01_r02_賦值右值真呼叫先完成後才寫入持有者() {
        let source = "fn bump(n:int)->int{\nreturn n+1\n}\nfn work(n:int)->int{\nvar value:int = n\nvalue = bump(value)\nreturn value+2\n}\nfn main()->int{\nreturn work(3)\n}\n";
        let checked = checked(source);
        let id = checked.root_local_inits[0].declaration;
        let result = checked.execute().unwrap();
        assert_eq!(result.result, Ok(6));
        let returned = result.events.iter().position(|event|
            matches!(event, PrivateScalarCallEvent::Returned(4))).unwrap();
        let assigned = result.events.iter().position(|event|
            matches!(event, PrivateScalarCallEvent::LocalAssigned(key, 4) if *key == id)).unwrap();
        assert!(returned < assigned);
    }

    #[test]
    fn g18_r01_r02_main_具名_copyable_int_多次更新後才進入普通函式() {
        let source = "fn identity(n:int)->int{\nreturn n\n}\nfn main()->int{\nvar value:int = 2\nvalue = value+3\nreturn identity(value)\n}\n";
        let checked = checked(source);
        assert_eq!(checked.main_dynamic_steps.len(), 2);
        let id = checked.main_dynamic_inits[0].declaration;
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalAssigned(id, 5)));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalScopeEnded(id)));
    }

    #[test]
    fn g18_r01_r02_真巢狀函式可寫宣告不能跨動態框架共用() {
        let source = "fn work(n:int)->int{\nvar current:int = n\ncurrent = current+1\nreturn leaf(current)\n}\nfn leaf(x:int)->int{\nvar tmp:int = x+1\ntmp = tmp+1\nreturn tmp\n}\nfn main()->int{\nreturn work(1)\n}\n";
        let checked = checked(source);
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Ok(4));
        let writes = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalAssigned(id, v) => Some((*id, *v)),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(writes.len(), 2);
        assert_ne!(writes[0].0, writes[1].0);
        assert_eq!((writes[0].1, writes[1].1), (2, 4));
    }

    #[test]
    fn g18_r01_r02_賦值右值溢位不得補記後續賦值或假返回() {
        let source = "fn work(n:int)->int{\nvar current:int = n\ncurrent = current+1\ncurrent = current+9223372036854775807\nreturn current\n}\nfn main()->int{\nreturn work(3)\n}\n";
        let checked = checked(source);
        let id = checked.root_local_inits[0].declaration;
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::LocalAssigned(actual, 4) if *actual == id)).count(), 1);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::LocalAssigned(..))).count(), 1);
        assert!(!out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert!(out.events.contains(&PrivateScalarCallEvent::LocalScopeEnded(id)));
    }

    #[test]
    fn g18_r01_r02_const_重賦值與錯型別必由真_checker_先拒絕() {
        for source in [
            "fn work(n:int)->int{\nconst current:int = n\ncurrent = current+1\nreturn current\n}\nfn main()->int{\nreturn work(3)\n}\n",
            "fn work(n:int)->int{\nvar current:int = n\ncurrent = true\nreturn current\n}\nfn main()->int{\nreturn work(3)\n}\n",
            "fn work(n:int)->int{\nmissing = n+1\nreturn n\n}\nfn main()->int{\nreturn work(3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(source),
                Err(PrivateScalarCallPrepareError::Check(_))), "{source}");
        }
    }

    #[test]
    fn g18_r01_r02_已編譯賦值的_checker_身分遭竄改在執行前拒絕() {
        let source = "fn work(n:int)->int{\nvar value:int = n\nvalue = value+2\nreturn value\n}\nfn main()->int{\nreturn work(3)\n}\n";
        let mut checked = checked(source);
        assert_eq!(checked.execute().unwrap().result, Ok(5));
        if let PrivateScalarLocalStep::Assign { value, .. } = &mut checked.root_local_steps[1] {
            *value = PrivateScalarReturnExpr::Constant(99);
        } else {
            panic!("真 Checker 有第二筆可寫賦值");
        }
        assert!(matches!(checked.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_r03_具名_bool_true_提前_return_不可再執行未達返回路徑() {
        let source = "fn choose(n:int)->int{\nvar flag:bool = true\nif(flag){\nreturn n+1\n}\nreturn n+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let plan = checked(source);
        assert_eq!(plan.root_local_steps.len(), 2);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(4));
        assert_eq!(outcome.parameter_slots_after_exit, 0);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ConditionalChecked(true))).count(), 1);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(4))).count(), 1);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(5))));
    }

    #[test]
    fn g18_r03_具名_bool_false_不走提前_return_而正常執行後段() {
        let source = "fn choose(n:int)->int{\nconst flag:bool = false\nif(flag){\nreturn n+1\n}\nreturn n+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let plan = checked(source);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(5));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ConditionalChecked(false))).count(), 1);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(4))));
    }

    #[test]
    fn g18_r03_已核准_var_bool_更新後真_if_重讀而非使用宣告舊值() {
        let source = "fn choose(n:int)->int{\nvar flag:bool = false\nflag = true\nif(flag){\nreturn n+1\n}\nreturn n+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let plan = checked(source);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(4));
        assert!(outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::BoolAssigned(_, true))));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ConditionalChecked(true))).count(), 1);
    }

    #[test]
    fn g18_r03_else_if_依真_checker_條件順序選擇唯一提前返回() {
        let source = "fn choose(n:int)->int{\nvar first:bool = false\nvar second:bool = true\nif(first){\nreturn n+1\n}else if(second){\nreturn n+2\n}else{\nreturn n+3\n}\nreturn n+4\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let plan = checked(source);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(5));
        let checks = outcome.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ConditionalChecked(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [false, true]);
        assert!(!outcome.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(6 | 7))));
    }

    #[test]
    fn g18_r03_巢狀子函式提前返回只退出該函式不跳過父框架返回() {
        let source = "fn leaf(n:int)->int{\nvar flag:bool = true\nif(flag){\nreturn n+1\n}\nreturn n+2\n}\nfn outer(n:int)->int{\nconst result:int = leaf(n)\nreturn result+3\n}\nfn main()->int{\nreturn outer(4)\n}\n";
        let plan = checked(source);
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(8));
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::FunctionEntered(_))).count(), 2);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::Returned(5 | 8))).count(), 2);
    }

    #[test]
    fn g18_r03_提前返回分支溢位只退出真的已初始化作用域() {
        let source = "fn choose(n:int)->int{\nvar flag:bool = true\nvar temp:int = n+1\nif(flag){\nreturn temp+9223372036854775807\n}\nreturn 0\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let plan = checked(source);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert!(out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::BoolScopeEnded(_))));
        assert!(out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::LocalScopeEnded(_))));
        assert!(!out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(out.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "choose".into(), returned: false,
        }));
    }

    #[test]
    fn g18_r03_未宣告_bool_及_const_bool_重綁由_checker_靜態拒絕() {
        for source in [
            "fn choose(n:int)->int{\nconst flag:bool = false\nflag = true\nif(flag){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(3)\n}\n",
            "fn choose(n:int)->int{\nif(missing){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(3)\n}\n",
            "fn choose(n:int)->int{\nif(n){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(source),
                Err(PrivateScalarCallPrepareError::Check(_))), "{source}");
        }
    }

    #[test]
    fn g18_r03_篡改已編譯分支條件前須拒絕任何真框架() {
        let source = "fn choose(n:int)->int{\nvar flag:bool = true\nif(flag){return n+1}\nreturn n+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let mut plan = checked(source);
        assert_eq!(plan.execute().unwrap().result, Ok(4));
        if let PrivateScalarLocalStep::EarlyReturn(branch) = &mut plan.root_local_steps[1] {
            branch.condition = PrivateScalarBoolExpr::Constant(false);
        } else {
            panic!("原 Checker 提供合法 if Return 計畫");
        }
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_r04_已核准_bool_true_函式實參與_int_返回共用真框架() {
        let source = "fn choose(flag:bool)->int{\nif(flag){return 4}\nreturn 7\n}\nfn main()->int{\nreturn choose(true)\n}\n";
        let checked = checked(source);
        assert_eq!(checked.callee_parameter_is_bool, [true]);
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::BoolArgumentEvaluated(true))).count(), 1);
        assert!(out.events.contains(&PrivateScalarCallEvent::BoolParameterBound(
            checked.callee_parameter_ids[0], true,
        )));
        assert!(!out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))));
    }

    #[test]
    fn g18_r04_已核准_bool_false_int_混合參數保持宣告順序和型別() {
        let source = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+1}\nreturn n-1\n}\nfn main()->int{\nreturn choose(false,8)\n}\n";
        let checked = checked(source);
        assert_eq!(checked.callee_parameter_is_bool, [true, false]);
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        let order = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::BoolArgumentEvaluated(false) => Some("bool"),
            PrivateScalarCallEvent::ArgumentEvaluated(8) => Some("int"),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(order, ["bool", "int"]);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::BoolParameterBound(..))).count(), 1);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(..))).count(), 1);
    }

    #[test]
    fn g18_r04_巢狀普通函式真_bool_參數不被誤讀成父整數槽() {
        let source = "fn inner(flag:bool,n:int)->int{\nif(flag){return n+1}\nreturn n-1\n}\nfn outer(flag:bool,n:int)->int{\nreturn inner(flag,n+2)\n}\nfn main()->int{\nreturn outer(true,4)\n}\n";
        let plan = checked(source);
        let out = plan.execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::BoolParameterBound(_,true))).count(), 2);
        let entered = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::FunctionEntered(name) => Some(name.as_str()), _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(entered, ["outer", "inner"]);
    }

    #[test]
    fn g18_r04_main_變動具名_bool_正值先更新再真正當實參() {
        let source = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+3}\nreturn n+4\n}\nfn main()->int{\nvar switched:bool = false\nswitched = true\nreturn choose(switched,5)\n}\n";
        let checked = checked(source);
        assert_eq!(checked.main_dynamic_steps.len(), 2);
        let out = checked.execute().unwrap();
        assert_eq!(out.result, Ok(8));
        assert!(out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::BoolAssigned(_,true))));
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::BoolArgumentEvaluated(true))).count(), 1);
    }

    #[test]
    fn g18_r04_bool_混合參數進入後故障只退出本次真框架() {
        let source = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+1}\nreturn n+2\n}\nfn main()->int{\nreturn choose(true,5)\n}\n";
        let checked = checked(source);
        let out = checked.execute_with_fault(true).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::InjectedFailure));
        assert_eq!(out.parameter_slots_after_exit, 0);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::BoolParameterBound(_,true))).count(), 1);
        assert_eq!(out.events.iter().filter(|event|
            matches!(event, PrivateScalarCallEvent::ParameterBound(_,5))).count(), 1);
        assert!(!out.events.iter().any(|event|
            matches!(event, PrivateScalarCallEvent::Returned(_))));
        assert_eq!(out.events.last(), Some(&PrivateScalarCallEvent::FunctionExited {
            name: "choose".into(), returned: false,
        }));
    }

    #[test]
    fn g18_r04_bool_參數不是_int_隱含轉型_原_checker_型別優先拒絕() {
        for source in [
            "fn choose(flag:bool,n:int)->int{\nif(flag){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(1,2)\n}\n",
            "fn choose(flag:bool,n:int)->int{\nif(flag){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(true,false)\n}\n",
            "fn choose(flag:bool,n:int)->int{\nif(flag){return n}\nreturn 0\n}\nfn main()->int{\nreturn choose(true)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(source),
                Err(PrivateScalarCallPrepareError::Check(_))), "{source}");
        }
    }

    #[test]
    fn g18_r04_bool_參數綁定靜態型別被竄改前必拒絕() {
        let source = "fn choose(flag:bool,n:int)->int{\nif(flag){return n+1}\nreturn n+2\n}\nfn main()->int{\nreturn choose(true,5)\n}\n";
        let mut checked = checked(source);
        assert_eq!(checked.execute().unwrap().result, Ok(6));
        checked.callee_parameter_is_bool[0] = false;
        assert!(matches!(checked.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_r05_混合布林整數與巢狀提前返回動態事件帳簿必平衡() {
        let src = "fn leaf(n:int)->int{\nvar flag:bool = true\nvar x:int = n+1\nif(flag){return x}\nreturn n\n}\nfn parent(n:int,flag:bool)->int{\nvar other:int = leaf(n)\nif(flag){return other+2}\nreturn other+3\n}\nfn main()->int{\nreturn parent(3,true)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|ev| matches!(
            ev, PrivateScalarCallEvent::FunctionEntered(_)
        )).count(), 2);
        assert_eq!(out.events.iter().filter(|ev| matches!(
            ev, PrivateScalarCallEvent::FunctionExited {returned: true,..}
        )).count(), 2);
    }

    #[test]
    fn g18_r05_動態整數溢位與真失敗退出仍保留合法平衡事件() {
        let src = "fn fail(n:int)->int{\nvar x:int = n\nx = x+1\nreturn x\n}\nfn main()->int{\nreturn fail(9223372036854775807)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|ev|
            matches!(ev, PrivateScalarCallEvent::Returned(_))));
    }

    #[test]
    fn g18_r05_遞迴重入相同靜態宣告身分不應誤判為重複清理() {
        let src = "fn repeat(n:int)->int{\nvar x:int = n\nx = x+1\nreturn repeat(x)\n}\nfn main()->int{\nreturn repeat(1)\n}\n";
        let out = checked(src).execute_with_budget(
            PrivateScalarExecutionBudget { max_call_frames: 3, max_steps: 100 },
        ).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::CallBudget));
        assert!(out.has_balanced_frame_trace());
    }

    #[test]
    fn g18_r05_偽造未建立區域的離域與錯順序均遭事件帳簿拒絕() {
        let x = DeclarationId(99);
        let y = DeclarationId(100);
        for events in [
            vec![PrivateScalarCallEvent::LocalScopeEnded(x)],
            vec![PrivateScalarCallEvent::LocalInitialized(x, 1)],
            vec![
                PrivateScalarCallEvent::LocalInitialized(x, 1),
                PrivateScalarCallEvent::LocalInitialized(y, 2),
                PrivateScalarCallEvent::LocalScopeEnded(x),
                PrivateScalarCallEvent::LocalScopeEnded(y),
            ],
            vec![
                PrivateScalarCallEvent::LocalInitialized(x, 1),
                PrivateScalarCallEvent::LocalInitialized(x, 2),
                PrivateScalarCallEvent::LocalScopeEnded(x),
            ],
            vec![PrivateScalarCallEvent::LocalAssigned(x, 5)],
        ] {
            let outcome = PrivateScalarCallOutcome {
                result: Ok(1), events, parameter_slots_after_exit: 0,
            };
            assert!(!outcome.has_balanced_frame_trace());
            assert!(matches!(PreparedPrivateScalarCall::audit_outcome(outcome),
                Err(PrivateScalarCallPrepareError::RuntimeTraceMismatch)));
        }
    }

    #[test]
    fn g18_r05_虛構函式返回和錯誤退出標記不得通過一致性稽核() {
        let cases = [
            vec![PrivateScalarCallEvent::Returned(1)],
            vec![PrivateScalarCallEvent::FunctionExited {
                name: "leaf".into(), returned: false,
            }],
            vec![
                PrivateScalarCallEvent::FunctionEntered("leaf".into()),
                PrivateScalarCallEvent::Returned(1),
                PrivateScalarCallEvent::FunctionExited {
                    name: "leaf".into(), returned: false,
                },
            ],
            vec![
                PrivateScalarCallEvent::FunctionEntered("leaf".into()),
                PrivateScalarCallEvent::FunctionExited {
                    name: "other".into(), returned: false,
                },
            ],
            vec![
                PrivateScalarCallEvent::FunctionEntered("leaf".into()),
                PrivateScalarCallEvent::Returned(1),
                PrivateScalarCallEvent::Returned(1),
                PrivateScalarCallEvent::FunctionExited {
                    name: "leaf".into(), returned: true,
                },
            ],
        ];
        for events in cases {
            assert!(!PrivateScalarCallOutcome {
                result: Ok(1), events, parameter_slots_after_exit: 0,
            }.has_balanced_frame_trace());
        }
    }

    #[test]
    fn g18_r05_布林與整數反向離域跨型別錯序不可冒充真清理() {
        let x = DeclarationId(100);
        let b = DeclarationId(200);
        let valid = vec![
            PrivateScalarCallEvent::FunctionEntered("fn".into()),
            PrivateScalarCallEvent::LocalInitialized(x, 3),
            PrivateScalarCallEvent::BoolInitialized(b, true),
            PrivateScalarCallEvent::Returned(3),
            PrivateScalarCallEvent::BoolScopeEnded(b),
            PrivateScalarCallEvent::LocalScopeEnded(x),
            PrivateScalarCallEvent::FunctionExited {
                name: "fn".into(), returned: true,
            },
        ];
        assert!(PrivateScalarCallOutcome {
            result: Ok(3), events: valid.clone(), parameter_slots_after_exit: 0,
        }.has_balanced_frame_trace());
        let mut invalid = valid.clone();
        invalid.swap(4, 5);
        assert!(!PrivateScalarCallOutcome {
            result: Ok(3), events: invalid, parameter_slots_after_exit: 0,
        }.has_balanced_frame_trace());
        let mut missing = valid;
        missing.remove(4);
        assert!(!PrivateScalarCallOutcome {
            result: Ok(3), events: missing, parameter_slots_after_exit: 0,
        }.has_balanced_frame_trace());
    }

    #[test]
    fn g18_r05_純值事件帳簿不得將錯誤參數綁定或布林寫入當作合法框架() {
        for events in [
            vec![PrivateScalarCallEvent::BoolParameterBound(DeclarationId(1), true)],
            vec![PrivateScalarCallEvent::BoolAssigned(DeclarationId(1), false)],
            vec![
                PrivateScalarCallEvent::FunctionEntered("f".into()),
                PrivateScalarCallEvent::BoolAssigned(DeclarationId(1), true),
                PrivateScalarCallEvent::FunctionExited {name:"f".into(),returned:false},
            ],
        ] {
            assert!(!PrivateScalarCallOutcome {
                result: Err(PrivateScalarCallRuntimeError::StateMismatch),
                events, parameter_slots_after_exit: 0,
            }.has_balanced_frame_trace());
        }
    }


    #[test]
    fn g18_r03_if_已核准_int_可寫分支後仍執行真正尾端返回() {
        let src = "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+2\n}\nreturn value+1\n}\nfn main()->int{\nreturn choose(true,3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::LocalAssigned(_, 5))).count(), 1);
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::Returned(6))).count(), 1);
    }

    #[test]
    fn g18_r03_if_false_未選中分支不得寫入而仍由原值返回() {
        let src = "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+2\n}\nreturn value+1\n}\nfn main()->int{\nreturn choose(false,3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|ev|
            matches!(ev, PrivateScalarCallEvent::LocalAssigned(..))));
    }

    #[test]
    fn g18_r03_else_if_具名_bool_分支更新與順序後續函式呼叫() {
        let src = "fn id(n:int)->int{\nreturn n\n}\nfn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+1\n}else if(false){\nvalue = value+100\n}else{\nvalue = value+2\n}\nreturn id(value)\n}\nfn main()->int{\nreturn choose(false,3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let checks = out.events.iter().filter_map(|ev| match ev {
            PrivateScalarCallEvent::ConditionalChecked(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [false, false]);
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::LocalAssigned(_, 5))).count(), 1);
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::FunctionEntered(name) if name == "id")).count(), 1);
    }

    #[test]
    fn g18_r03_if_變更_bool_使後續獨立_if_重新讀取真動態值() {
        let src = "fn choose(n:int)->int{\nvar flag:bool = false\nif(true){\nflag = true\n}\nif(flag){\nreturn n+1\n}\nreturn n+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::BoolAssigned(_, true))).count(), 1);
        assert_eq!(out.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::ConditionalChecked(true))).count(), 2);
    }

    #[test]
    fn g18_r03_if_分支呼叫溢位不允許後續真賦值或假返回() {
        let src = "fn bump(n:int)->int{\nreturn n+1\n}\nfn choose(n:int)->int{\nvar value:int = n\nif(true){\nvalue = bump(value)\n}\nreturn value+1\n}\nfn main()->int{\nreturn choose(9223372036854775807)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|ev|
            matches!(ev, PrivateScalarCallEvent::LocalAssigned(..)
                | PrivateScalarCallEvent::Returned(_))));
    }

    #[test]
    fn g18_r03_if_分支不能更改_const_或偷換型別_原_checker_拒絕() {
        for src in [
            "fn choose(n:int)->int{\nconst value:int = n\nif(true){value = value+2}\nreturn value\n}\nfn main()->int{\nreturn choose(3)\n}\n",
            "fn choose(n:int)->int{\nvar value:int = n\nif(true){value = false}\nreturn value\n}\nfn main()->int{\nreturn choose(3)\n}\n",
        ] {
            assert!(matches!(PreparedPrivateScalarCall::prepare(src),
                Err(PrivateScalarCallPrepareError::Check(_))), "{src}");
        }
    }

    #[test]
    fn g18_r03_if_分支原_checker_賦值工作計畫篡改於執行前拒絕() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){value = value+2}\nreturn value\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let mut plan = checked(src);
        assert_eq!(plan.execute().unwrap().result, Ok(5));
        if let PrivateScalarLocalStep::EarlyReturn(if_plan) = &mut plan.root_local_steps[1] {
            if let PrivateScalarBranchAction::AssignInt { value, .. } = &mut if_plan.when_true {
                *value = PrivateScalarReturnExpr::Constant(99);
            } else { panic!("真 Parser int 更新分支遺失"); }
        } else { panic!("真 Parser if 分支遺失"); }
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_r03_同一真_if_分支兩次_var_int_更新須照原_parser_先後返回() {
        for (flag, expected, updates) in [("true", 9, vec![5, 8]), ("false", 4, vec![])] {
            let src = format!(
                "fn choose(flag:bool,n:int)->int{{\nvar value:int = n\nif(flag){{\nvalue = value+2\nvalue = value+3\n}}\nreturn value+1\n}}\nfn main()->int{{\nreturn choose({},3)\n}}\n",
                flag,
            );
            let out = checked(&src).execute().unwrap();
            assert_eq!(out.result, Ok(expected));
            assert!(out.has_balanced_frame_trace());
            let values = out.events.iter().filter_map(|event| match event {
                PrivateScalarCallEvent::LocalAssigned(_, value) => Some(*value),
                _ => None,
            }).collect::<Vec<_>>();
            assert_eq!(values, updates);
        }
    }

    #[test]
    fn g18_r03_else_if_選中分支多筆更新不可執行其他兄弟分支() {
        let src = "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+100\n}else if(true){\nvalue = value+1\nvalue = value+2\n}else{\nvalue = value+50\n}\nreturn value\n}\nfn main()->int{\nreturn choose(false,3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        let values = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalAssigned(_, value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(values, [4, 6]);
        let checks = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ConditionalChecked(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [false, true]);
    }

    #[test]
    fn g18_r03_分支先更新_int_bool_後下個_if_必讀動態最新值() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nvar flag:bool = false\nif(true){\nvalue = value+2\nflag = true\n}\nif(flag){\nreturn value+1\n}\nreturn value+2\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(6));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::BoolAssigned(_, true)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalAssigned(_, 5)
        )).count(), 1);
    }

    #[test]
    fn g18_r03_分支內先變更再提前_return_不得執行不可達尾端() {
        let src = "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+1\nreturn value+2\n}\nreturn value+10\n}\nfn main()->int{\nreturn choose(true,4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::Returned(7)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(15)
        )));
    }

    #[test]
    fn g18_r03_同分支第二更新溢位只保存第一個真更新並失敗退出() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){\nvalue = value+1\nvalue = value+9223372036854775807\n}\nreturn value\n}\nfn main()->int{\nreturn choose(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalAssigned(_, 2)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r03_分支內常值錯誤賦值與真工作計畫竄改均須零額外執行() {
        let invalid = "fn choose(n:int)->int{\nconst x:int = n\nif(true){\nx = x+1\nx = x+2\n}\nreturn x\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let source = "fn choose(n:int)->int{\nvar x:int = n\nif(true){\nx = x+1\nx = x+2\n}\nreturn x\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let mut plan = checked(source);
        assert_eq!(plan.execute().unwrap().result, Ok(6));
        if let PrivateScalarLocalStep::EarlyReturn(conditional) = &mut plan.root_local_steps[1] {
            if let PrivateScalarBranchAction::Sequence(actions) = &mut conditional.when_true {
                actions[1] = PrivateScalarBranchAction::Return(
                    PrivateScalarReturnExpr::Constant(999),
                );
            } else { panic!("原 Parser 多筆分支序列未收錄"); }
        } else { panic!("原 Parser if 節點未收錄"); }
        assert!(matches!(plan.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }


    #[test]
    fn g18_r03_真巢狀_if_葉提前返回保留雙層條件順序() {
        let src = "fn choose(n:int)->int{\nvar flag:bool = true\nif(true){\nif(flag){\nreturn n+1\n}else{\nreturn n+2\n}\n}\nreturn n+3\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(4));
        assert!(out.has_balanced_frame_trace());
        let checks = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ConditionalChecked(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [true, true]);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(6)
        )));
    }

    #[test]
    fn g18_r03_外層先更新_bool_後內層_if_須重讀最新值再寫入_int() {
        let src = "fn choose(n:int)->int{\nvar flag:bool = false\nvar value:int = n\nif(true){\nflag = true\nif(flag){\nvalue = value+2\n}else{\nvalue = value+99\n}\n}\nreturn value\n}\nfn main()->int{\nreturn choose(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let checks = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ConditionalChecked(value) => Some(*value),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [true, true]);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::BoolAssigned(_, true)
        )).count(), 1);
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalAssigned(_, 5)
        )).count(), 1);
    }

    #[test]
    fn g18_r03_外層_false_路徑內的_else_巢狀_if_只執行選中的葉() {
        let src = "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+100\n}else{\nif(false){\nvalue = value+20\n}else{\nvalue = value+3\n}\n}\nreturn value\n}\nfn main()->int{\nreturn choose(false,4)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(7));
        assert!(out.has_balanced_frame_trace());
        let changes = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::LocalAssigned(_, v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(changes, [7]);
        let checks = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::ConditionalChecked(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(checks, [false, false]);
    }

    #[test]
    fn g18_r03_巢狀提前_return_只退出內函式不截斷父函式算術() {
        let src = "fn leaf(n:int)->int{\nif(true){\nif(false){\nreturn n+10\n}else{\nreturn n+1\n}\n}\nreturn n+3\n}\nfn outer(n:int)->int{\nreturn leaf(n)+2\n}\nfn main()->int{\nreturn outer(2)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        let returned = out.events.iter().filter_map(|event| match event {
            PrivateScalarCallEvent::Returned(v) => Some(*v),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(returned, [3, 5]);
    }

    #[test]
    fn g18_r03_巢狀分支第二次_int_更新溢位真框架不補寫後續結果() {
        let src = "fn choose(n:int)->int{\nvar value:int = n\nif(true){\nif(true){\nvalue = value+1\nvalue = value+9223372036854775807\n}\n}\nreturn value\n}\nfn main()->int{\nreturn choose(1)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|event| matches!(
            event, PrivateScalarCallEvent::LocalAssigned(_, 2)
        )).count(), 1);
        assert!(!out.events.iter().any(|event| matches!(
            event, PrivateScalarCallEvent::Returned(_)
        )));
    }

    #[test]
    fn g18_r03_巢狀條件非法名稱與工作計畫遭竄改必先拒絕() {
        let invalid = "fn choose(n:int)->int{\nif(true){\nif(missing){\nreturn n\n}\n}\nreturn n\n}\nfn main()->int{\nreturn choose(1)\n}\n";
        assert!(matches!(PreparedPrivateScalarCall::prepare(invalid),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let valid = "fn choose(n:int)->int{\nif(true){\nif(true){\nreturn n+1\n}\n}\nreturn n+2\n}\nfn main()->int{\nreturn choose(1)\n}\n";
        let mut prepared = checked(valid);
        assert_eq!(prepared.execute().unwrap().result, Ok(2));
        if let PrivateScalarLocalStep::EarlyReturn(plan) = &mut prepared.root_local_steps[0] {
            if let PrivateScalarBranchAction::NestedIf(inner) = &mut plan.when_true {
                inner.when_true = PrivateScalarBranchAction::Return(
                    PrivateScalarReturnExpr::Constant(50),
                );
            } else { panic!("真巢狀 If 原來源丟失"); }
        } else { panic!("原 If 工作計畫丟失"); }
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

    #[test]
    fn g18_r01_普通函式_while_false_零迭代仍真正檢查且返回() {
        let src = "fn count(n:int)->int{\nvar value:int = n\nwhile(false){\nvalue = value+1\n}\nreturn value\n}\nfn main()->int{\nreturn count(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(3));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::ConditionalChecked(false))).count(), 1);
        assert!(!out.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::LocalAssigned(..))));
    }

    #[test]
    fn g18_r03_普通函式_while_逐輪重讀_bool_並僅更新到達的_int() {
        let src = "fn count(n:int)->int{\nvar value:int = n\nvar flag:bool = true\nwhile(flag){\nvalue = value+2\nflag = false\n}\nreturn value\n}\nfn main()->int{\nreturn count(3)\n}\n";
        let out = checked(src).execute().unwrap();
        assert_eq!(out.result, Ok(5));
        assert!(out.has_balanced_frame_trace());
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::ConditionalChecked(true))).count(), 1);
        assert_eq!(out.events.iter().filter(|e|
            matches!(e, PrivateScalarCallEvent::ConditionalChecked(false))).count(), 1);
    }

    #[test]
    fn g18_r03_普通函式_while_內_break_不執行後續寫入() {
        let src = "fn count(n:int)->int{\nvar value:int = n\nwhile(true){\nvalue = value+1\nbreak\nvalue = value+20\n}\nreturn value\n}\nfn main()->int{\nreturn count(1)\n}\n";
        // 只接受原 Checker 核准的可達性，不自行放寬 break 後語句。
        match PreparedPrivateScalarCall::prepare(src) {
            Ok(plan) => {
                let out = plan.execute().unwrap();
                assert_eq!(out.result, Ok(2));
                assert!(out.has_balanced_frame_trace());
            }
            Err(PrivateScalarCallPrepareError::Check(_)) => {}
            Err(other) => panic!("原 Checker 接受但 while 執行器拒絕：{other:?}"),
        }
    }

    #[test]
    fn g18_r03_普通函式_while_步數預算耗盡不產生假返回() {
        let src = "fn count(n:int)->int{\nwhile(true){\ncontinue\n}\nreturn n\n}\nfn main()->int{\nreturn count(1)\n}\n";
        let prepared = checked(src);
        let out = prepared.execute_with_budget(PrivateScalarExecutionBudget {
            max_call_frames: 8, max_steps: 30,
        }).unwrap();
        assert_eq!(out.result, Err(PrivateScalarCallRuntimeError::StepBudget));
        assert!(out.has_balanced_frame_trace());
        assert!(!out.events.iter().any(|e|
            matches!(e, PrivateScalarCallEvent::Returned(..))));
    }

    #[test]
    fn g18_r03_普通函式_while_真控制計畫篡改須執行前拒絕() {
        let src = "fn count(n:int)->int{\nvar flag:bool = true\nwhile(flag){\nflag = false\n}\nreturn n\n}\nfn main()->int{\nreturn count(1)\n}\n";
        let mut prepared = checked(src);
        assert_eq!(prepared.execute().unwrap().result, Ok(1));
        if let PrivateScalarLocalStep::Control(PrivateScalarBranchAction::While {
            condition, ..
        }) = &mut prepared.root_local_steps[1] {
            *condition = PrivateScalarBoolExpr::Constant(false);
        } else { panic!("原 while 來源控制計畫未保留"); }
        assert!(matches!(prepared.execute(),
            Err(PrivateScalarCallPrepareError::SourceChanged)));
    }

}


/// R06：跨各個原 Checker 已核准執行器的**真動態結果**整合回歸。
/// 不假設各路徑已共享同一 runtime，亦不把 int 離域等同 OwnedStr Release。
#[cfg(test)]
mod g18_r06_cross_runtime_tests {
    use super::*;
    use crate::owned_str_runtime::{self, ResourceEvent};

    fn scalar() -> &'static str {
        "fn choose(flag:bool,n:int)->int{\nvar value:int = n\nif(flag){\nvalue = value+2\n}\nreturn value+1\n}\nfn main()->int{\nreturn choose(true,3)\n}\n"
    }

    fn borrow() -> &'static str {
        "fn measure(item:str)->int{\nreturn 7\n}\nfn main()->int{\nconst owner:str = \"來源🪼\"\nreturn measure(owner)\n}\n"
    }

    fn move_borrow() -> &'static str {
        "fn consume(owned:str,watch:str)->int{\nvar local:str = owned.move()\nreturn 11\n}\nfn main()->int{\nvar source:str = \"唯一🪼\"\nconst observer:str = \"借用\"\nreturn consume(source.move(),observer)\n}\n"
    }

    fn method() -> &'static str {
        "struct Counter{\nvar count:int = 7\nfn get()->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.get()\n}\n"
    }

    fn recursive() -> &'static str {
        "fn descend(flag:bool,payload:str)->int{\nvar local:str = \"層🪼\"\nif(flag){\nvar nested:str = \"內\"\nreturn descend(false,nested)\n}\nreturn 7\n}\nfn main()->int{\nvar root:str = \"根\"\nreturn descend(true,root)\n}\n"
    }

    #[test]
    fn g18_r06_混合布林整數與真借用成功返回各守門須同時成立() {
        let pure = PreparedPrivateScalarCall::prepare(scalar()).unwrap().execute().unwrap();
        assert_eq!(pure.result, Ok(6));
        assert!(pure.has_balanced_frame_trace());
        assert_eq!(pure.events.iter().filter(|ev|
            matches!(ev, PrivateScalarCallEvent::LocalAssigned(_, 5))).count(), 1);
        let borrowed = PreparedPrivateCallBorrow::prepare(borrow()).unwrap().execute().unwrap();
        assert_eq!(borrowed.result, Ok(7));
        assert!(borrowed.has_balanced_borrow_resource_trace());
        assert_eq!(borrowed.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
        // int/bool ScopeEvent 不會也不能憑空創造 OwnedStr 事件。
        assert!(!pure.events.is_empty());
    }

    #[test]
    fn g18_r06_同_call_整筆移動借用和同步_self_不混淆資源責任() {
        let mixed = PreparedPrivateMixedOwnershipCall::prepare(move_borrow())
            .unwrap().execute().unwrap();
        assert_eq!(mixed.result, Ok(11));
        assert!(mixed.has_balanced_mixed_resource_trace());
        assert_eq!(mixed.events.iter().filter(|e|
            matches!(e, ResourceEvent::Created(_))).count(), 2);
        let own = PreparedPrivateSelfMethod::prepare(method())
            .unwrap().execute().unwrap();
        assert_eq!(own.result, Ok(7));
        assert!(own.has_balanced_self_borrow_trace());
        assert!(own.events.is_empty());
        assert!(mixed.ticket_returned && own.ticket_returned);
    }

    #[test]
    fn g18_r06_合法遞迴真資源與普通借用在預算耗盡時皆不得假返回() {
        let plan = PreparedPrivateRecursiveStr::prepare(recursive()).unwrap();
        let exhausted = plan.execute(PrivateRecursiveStrBudget {
            max_call_frames: 2, max_steps: 80,
        }).unwrap();
        assert_eq!(exhausted.result, Err(PrivateRecursiveStrRunError::CallBudget));
        assert!(exhausted.has_balanced_resource_frame_trace());
        assert!(exhausted.loans_returned);
        assert!(owned_str_runtime::event_trace_balanced(&exhausted.events));
        assert!(exhausted.frames.iter().all(|event| !matches!(
            event, PrivateRecursiveFrameEvent::Exited { returned: true, .. }
        )));
        let borrowed = PreparedPrivateCallBorrow::prepare(borrow()).unwrap()
            .execute_with_fault(true).unwrap();
        assert_eq!(borrowed.result, Err(PrivateCallBorrowRuntimeError::InjectedFailure));
        assert!(borrowed.has_balanced_borrow_resource_trace());
        assert!(!borrowed.call_exited);
    }

    #[test]
    fn g18_r06_不同執行器的正常失敗不得丟失已建資源或複製假返回() {
        let pure = PreparedPrivateScalarCall::prepare(
            "fn inc(n:int)->int{\nvar x:int = n\nx = x+1\nreturn x\n}\nfn main()->int{\nreturn inc(9223372036854775807)\n}\n"
        ).unwrap().execute().unwrap();
        assert_eq!(pure.result, Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert!(pure.has_balanced_frame_trace());
        assert!(!pure.events.iter().any(|ev| matches!(
            ev, PrivateScalarCallEvent::Returned(_)
        )));
        let method = PreparedPrivateSelfMethod::prepare(method()).unwrap()
            .execute_with_fault(true).unwrap();
        assert_eq!(method.result, Err(PrivateSelfMethodRuntimeError::InjectedFailure));
        assert!(method.has_balanced_self_borrow_trace());
        assert!(!method.method_exited);
        let mixed = PreparedPrivateMixedOwnershipCall::prepare(move_borrow()).unwrap()
            .execute_with_fault(Some(3)).unwrap();
        assert_eq!(mixed.result, Err(PrivateMixedOwnershipRunError::InjectedFailure));
        assert!(mixed.has_balanced_mixed_resource_trace());
        assert!(mixed.ticket_returned);
    }

    #[test]
    fn g18_r06_跨模型偽造離域資源票據任一項必在交付稽核失敗() {
        let mut pure = PreparedPrivateScalarCall::prepare(scalar()).unwrap()
            .execute().unwrap();
        pure.events.push(PrivateScalarCallEvent::LocalScopeEnded(DeclarationId(0)));
        assert!(!pure.has_balanced_frame_trace());
        let mut borrowed = PreparedPrivateCallBorrow::prepare(borrow()).unwrap()
            .execute().unwrap();
        borrowed.ticket_returned = false;
        assert!(!borrowed.has_balanced_borrow_resource_trace());
        let mut mixed = PreparedPrivateMixedOwnershipCall::prepare(move_borrow())
            .unwrap().execute().unwrap();
        mixed.events.clear();
        assert!(!mixed.has_balanced_mixed_resource_trace());
        let mut self_method = PreparedPrivateSelfMethod::prepare(method()).unwrap()
            .execute().unwrap();
        self_method.events.extend([
            ResourceEvent::Created(77), ResourceEvent::Released(77),
        ]);
        assert!(!self_method.has_balanced_self_borrow_trace());
        let mut nested = PreparedPrivateRecursiveStr::prepare(recursive()).unwrap()
            .execute(PrivateRecursiveStrBudget {
                max_call_frames: 4, max_steps: 100,
            }).unwrap();
        assert!(nested.has_balanced_resource_frame_trace());
        nested.loans_returned = false;
        assert!(!nested.has_balanced_resource_frame_trace());
    }
}
