// G18／P18.2：從同一份正式私有 Checker 構建可證明終止的
// for 半開常值區間及 while(false)/while(true){ ... break/return } 真執行子集。
// 執行前重核完整真來源，真 UTF-8 ResourceId 僅在實際迭代中建立。
// 不擅定一般無限迴圈的停止策略，不公開此入口。
#[derive(Debug)]
enum PrivateLoopPrepareError {
    Check(crate::Diagnostic),
    Unsupported(&'static str),
    SourceChanged,
    Runtime(crate::owned_str_runtime::StrRuntimeError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateLoopAction {
    DeclareStr { name: String, mutable: bool, text: String },
    /// 真私有 Checker 的 Copyable bool HIR；僅儲存獨立純值。
    DeclareBool { identity: DeclarationId, value: PrivateBoolCondition },
    AssignBool { identity: DeclarationId, value: PrivateBoolCondition },
    ForRange {
        identity: DeclarationId, lower: i64, upper: i64,
        body: Vec<PrivateLoopAction>,
    },
    WhileCondition {
        condition: PrivateBoolCondition,
        body: Vec<PrivateLoopAction>,
    },
    Return(i64),
    /// 僅限原 Checker 已解析的 for 區間迭代器具名返回。
    ReturnIterator(DeclarationId),
    Break,
    Continue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrivateLoopFlow {
    Normal,
    Break,
    Continue,
    Return(i64),
}

struct PreparedPrivateLoop {
    verified: VerifiedPrivateSource,
    original: Box<[u8]>,
    operations: Vec<PrivateLoopAction>,
}

#[derive(Debug)]
struct PrivateLoopOutcome {
    result: Result<i64, crate::owned_str_runtime::StrRuntimeError>,
    events: Vec<crate::owned_str_runtime::ResourceEvent>,
    /// 真正每次 while 檢查的 bool 動態值，不將靜態 CFG 當執行路徑。
    condition_checks: Vec<bool>,
    /// 真正每次 for 迭代設置的 (DeclarationId, i64)，非靜態展開。
    iterations: Vec<(DeclarationId, i64)>,
}

// P18.2：安全執行預算由每次 Rust 內部呼叫配置，不是 voiles
// 文法或通用語言規格的固定半開迴圈上界。
#[derive(Debug, Clone, Copy)]
struct PrivateLoopExecutionBudget {
    max_steps: usize,
}
impl Default for PrivateLoopExecutionBudget {
    fn default() -> Self { Self { max_steps: 10_000 } }
}
struct PrivateLoopStepCounter {
    remaining: usize,
}
impl PrivateLoopStepCounter {
    fn new(budget: PrivateLoopExecutionBudget) -> Self {
        Self { remaining: budget.max_steps }
    }
    fn consume(&mut self) -> Result<(), crate::owned_str_runtime::StrRuntimeError> {
        if self.remaining == 0 {
            return Err(crate::owned_str_runtime::StrRuntimeError::ExecutionBudgetExhausted);
        }
        self.remaining -= 1;
        Ok(())
    }
}

impl PreparedPrivateLoop {

    /// 真 Checker 對已知**恰一次** for 且本體必定 return，會
    /// 直接終止外部 CFG，後續語句的 TypedHIR 可以合法不存在。
    /// 此處只能以已證明的直接 return 或唯一單次返回決定可達性，
    /// break/continue 不得被錯當成功返回。
    fn guaranteed_return(actions: &[PrivateLoopAction]) -> bool {
        for action in actions {
            match action {
                PrivateLoopAction::Return(_)
                | PrivateLoopAction::ReturnIterator(_) => return true,
                PrivateLoopAction::Break | PrivateLoopAction::Continue => return false,
                PrivateLoopAction::ForRange { lower, upper, body, .. }
                    if i128::from(*upper) - i128::from(*lower) == 1
                        && Self::guaranteed_return(body) => return true,
                _ => {}
            }
        }
        false
    }

    fn has_real_return(actions: &[PrivateLoopAction]) -> bool {
        actions.iter().any(|action| match action {
            PrivateLoopAction::Return(_) | PrivateLoopAction::ReturnIterator(_) => true,
            PrivateLoopAction::ForRange { body, .. }
                | PrivateLoopAction::WhileCondition { body, .. } =>
                    Self::has_real_return(body),
            _ => false,
        })
    }

    fn verified_actions(
        statements: &[Stmt], verified: &VerifiedPrivateSource, inside_loop: bool,
        bool_bindings: &mut BTreeMap<String, DeclarationId>,
        iterator_bindings: &mut BTreeMap<String, DeclarationId>,
    ) -> Result<Vec<PrivateLoopAction>, PrivateLoopPrepareError> {
        let mut result = Vec::new();
        for statement in statements {
            // 原 Checker 若已判定唯一迭代直接返回，後續正式
            // AST 仍存在，但沒有可達 HIR，不能要求其求值或
            // 偽造「已驗證」的 expression。且絕不提前觸發資源。
            if Self::guaranteed_return(&result) {
                break;
            }
            let action = match &statement.kind {
                // 根作用域的具名 bool 使用原 DeclarationId；迴圈內
                // 不自行推定新的生命週期、遮蔽或參數型別。
                StmtKind::Declare(decl) if decl.ty == "bool" && !inside_loop => {
                    let identity = DeclarationId(statement.start);
                    if !verified.plan.declarations.matches(
                        identity, "bool", decl.mutable,
                    ) || bool_bindings.contains_key(&decl.name) {
                        return Err(PrivateLoopPrepareError::Unsupported(
                            "具名 bool 宣告缺少唯一真 Checker 型別／身分",
                        ));
                    }
                    let init = decl.initializer.as_ref().ok_or(
                        PrivateLoopPrepareError::Unsupported(
                            "此非公開迴圈切片要求 bool 明確初始化",
                        ),
                    )?;
                    let value = verified_nested_bool(init, verified, bool_bindings)
                        .map_err(|_| PrivateLoopPrepareError::Unsupported(
                            "bool 初始化缺少正式 Lexer／TypedHIR 名稱證據",
                        ))?;
                    bool_bindings.insert(decl.name.clone(), identity);
                    PrivateLoopAction::DeclareBool { identity, value }
                }
                StmtKind::Assign(assign) if bool_bindings.contains_key(&assign.name) => {
                    let identity = bool_bindings[&assign.name];
                    if !verified.plan.declarations.typed_declaration(identity)
                        .is_some_and(|fact| fact.ty == "bool" && fact.mutable)
                        || verified.plan.scalar_hir.assignment(statement.node_span())
                            .is_none_or(|fact|
                                fact.resolved.declaration != identity
                                || fact.resolved.ty != crate::common_ir::ScalarType::Bool)
                    {
                        return Err(PrivateLoopPrepareError::Unsupported(
                            "bool 重新賦值缺少 Checker 可寫宣告與 P10 身分",
                        ));
                    }
                    let value = verified_nested_bool(
                        &assign.expression, verified, bool_bindings,
                    ).map_err(|_| PrivateLoopPrepareError::Unsupported(
                        "bool 重新賦值來源不是已簽發純值 HIR",
                    ))?;
                    PrivateLoopAction::AssignBool { identity, value }
                }
                StmtKind::Declare(decl) if decl.ty == "str" => {
                    let id = DeclarationId(statement.start);
                    if !verified.plan.declarations.matches(id, "str", decl.mutable) {
                        return Err(PrivateLoopPrepareError::Unsupported(
                            "區域字串宣告缺少正式 Checker 原型別和 var/const 證據",
                        ));
                    }
                    let Some(init) = decl.initializer.as_ref() else {
                        return Err(PrivateLoopPrepareError::Unsupported(
                            "此迴圈執行子集要求 str 獨立常值初始化",
                        ));
                    };
                    let value = private_str_literal(init, &verified.tokens).map_err(|_|
                        PrivateLoopPrepareError::Unsupported(
                            "迴圈 str 僅接受原 Lexer 真正解碼的獨立 UTF-8 常值",
                        ))?;
                    PrivateLoopAction::DeclareStr {
                        name: decl.name.clone(), mutable: decl.mutable, text: value,
                    }
                }
                StmtKind::ForRange { iterator, iterator_start, lower, upper, body } => {
                    if !verified.plan.declarations.matches(
                        DeclarationId(*iterator_start), "int", false,
                    ) {
                        return Err(PrivateLoopPrepareError::Unsupported(
                            "for 迭代宣告缺少原 Checker 完整身份",
                        ));
                    }
                    let start = crate::syntax_shared::integer_i64(lower).map_err(|_|
                        PrivateLoopPrepareError::Unsupported("for 左端點無合法 i64 證據"),
                    )?;
                    let end = crate::syntax_shared::integer_i64(upper).map_err(|_|
                        PrivateLoopPrepareError::Unsupported("for 右端點無合法 i64 證據"),
                    )?;
                    let count = i128::from(end).saturating_sub(i128::from(start)).max(0);
                    // 原 Checker 有權接受任何合法 i64 有限半開區間。
                    // 真執行的安全退出交由本次呼叫可配置步數預算，
                    // 不可在準備階段以固定 64 次拒絕合法來源。
                    // 空區間本體根本不會執行；原 Checker 可合法跳過
                    // 它的動態宣告，不能憑 AST 為未簽發的值建立 ResourceId。
                    let identity = DeclarationId(*iterator_start);
                    let compiled = if count == 0 {
                        Vec::new()
                    } else {
                        // 迭代名稱只在本體詞彙範圍內可見；即使內層
                        // for 遮蔽同名，離開時也必須恢復外層來源。
                        let previous = iterator_bindings.insert(iterator.clone(), identity);
                        let prepared = Self::verified_actions(
                            &body.statements, verified, true,
                            bool_bindings, iterator_bindings,
                        );
                        if let Some(shadowed) = previous {
                            iterator_bindings.insert(iterator.clone(), shadowed);
                        } else {
                            iterator_bindings.remove(iterator);
                        }
                        prepared?
                    };
                    PrivateLoopAction::ForRange {
                        identity, lower: start, upper: end, body: compiled,
                    }
                }
                StmtKind::While { condition, body } => {
                    // 已核准 P15：每輪重新求值、非資源副作用的 bool。
                    // 具名來源必須由同一份私有 Checker 的 TypedHIR
                    // Reference、Lexer token、DeclarationId 嚴格證明。
                    let guard = verified_nested_bool(condition, verified, bool_bindings)
                        .map_err(|_| PrivateLoopPrepareError::Unsupported(
                            "while 條件缺少原 Checker 真 bool 型別及名稱證據",
                        ))?;
                    let compiled = if guard == PrivateBoolCondition::Constant(false) {
                        // 永不走到的本體沒有執行期資源或 bool 效果。
                        Vec::new()
                    } else {
                        let actions = Self::verified_actions(
                            &body.statements, verified, true,
                            bool_bindings, iterator_bindings,
                        )?;
                        // 只接受能以來源直接證明至多一次本體後退出：
                        // (1) 直接 break／return；
                        // (2) 具名 var bool 的末句明確改成 false，
                        //     下次條件會真正重讀同一 DeclarationId。
                        // 前方不可有 continue 跳過該退出義務。
                        if actions.iter().take(actions.len().saturating_sub(1))
                            .any(|item| matches!(item, PrivateLoopAction::Continue)) {
                            return Err(PrivateLoopPrepareError::Unsupported(
                                "while 的 continue 不得繞過已證明終止出口",
                            ));
                        }
                        let direct_exit = matches!(actions.last(),
                            Some(PrivateLoopAction::Break | PrivateLoopAction::Return(_)));
                        let falsified_guard = match (guard, actions.last()) {
                            (PrivateBoolCondition::Bound(source),
                                Some(PrivateLoopAction::AssignBool {
                                    identity, value: PrivateBoolCondition::Constant(false),
                                })) => source == *identity,
                            _ => false,
                        };
                        if !direct_exit && !falsified_guard {
                            return Err(PrivateLoopPrepareError::Unsupported(
                                "此私有 while 須有直接 break/return 或原條件明確寫為 false",
                            ));
                        }
                        actions
                    };
                    PrivateLoopAction::WhileCondition { condition: guard, body: compiled }
                }
                StmtKind::Return(ret) => {
                    let span = ret.expression.node_span();
                    // 原 Checker 對純常值用 expression 事實、
                    // 對具名 Iterator 用 reference 事實；兩者都是
                    // 同一份原 TypedHIR 的不同型別證據，不能因
                    // Name 沒有獨立 expression 而改用無驗證回傳。
                    match &ret.expression.kind {
                        TermKind::Scalar(
                            crate::syntax_shared::ScalarLiteral::Integer(raw),
                        ) => {
                            if verified.plan.scalar_hir.expression(span)
                                .is_none_or(|fact|
                                    fact.ty != crate::common_ir::ScalarType::Int) {
                                return Err(PrivateLoopPrepareError::Unsupported(
                                    "return int 常值缺少 Checker Expression HIR",
                                ));
                            }
                            let value = crate::syntax_shared::integer_i64(raw)
                                .map_err(|_| PrivateLoopPrepareError::Unsupported(
                                    "return int 超出 i64 有效範圍",
                                ))?;
                            PrivateLoopAction::Return(value)
                        }
                        TermKind::Name(name) => {
                            let identity = iterator_bindings.get(name).copied().ok_or(
                                PrivateLoopPrepareError::Unsupported(
                                    "此私有 return 具名值僅支援在作用域內的 for 迭代器",
                                ),
                            )?;
                            if verified.plan.scalar_hir.reference(span)
                                .is_none_or(|fact|
                                    fact.resolved.declaration != identity
                                    || fact.resolved.ty != crate::common_ir::ScalarType::Int)
                                || !verified.tokens.iter().any(|token|
                                    token.start == span.start && token.end == span.end
                                        && matches!(&token.kind,
                                            TokenKind::Name(raw) if raw == name)
                                ) {
                                return Err(PrivateLoopPrepareError::Unsupported(
                                    "for 迭代器返回缺少原 Checker Reference 與 Lexer 身分",
                                ));
                            }
                            PrivateLoopAction::ReturnIterator(identity)
                        }
                        _ => return Err(PrivateLoopPrepareError::Unsupported(
                            "此私有迴圈切片不執行未核准的返回式",
                        )),
                    }
                }
                StmtKind::Break if inside_loop => PrivateLoopAction::Break,
                StmtKind::Continue if inside_loop => PrivateLoopAction::Continue,
                _ => return Err(PrivateLoopPrepareError::Unsupported(
                    "此真迴圈子集尚不接通任意寫入、借用、方法、動態條件或未證明控制邊",
                )),
            };
            result.push(action);
        }
        Ok(result)
    }

    fn prepare(source: &str) -> Result<Self, PrivateLoopPrepareError> {
        let verified = verify_source_with_ability(
            source, crate::ownership_policy::approved_leaf_copy_permission,
        ).map_err(PrivateLoopPrepareError::Check)?;
        if !verified.structs.is_empty() || verified.functions.len() != 1 {
            return Err(PrivateLoopPrepareError::Unsupported(
                "真迴圈資源子集只接受單一內部 main 函式",
            ));
        }
        let main = &verified.functions[0];
        if main.name != "main" || main.result_ty != "int" || !main.params.is_empty() {
            return Err(PrivateLoopPrepareError::Unsupported(
                "真迴圈資源子集限無參數 main()->int",
            ));
        }
        let operations = Self::verified_actions(
            &main.body.statements, &verified, false,
            &mut BTreeMap::new(), &mut BTreeMap::new(),
        )?;
        if !operations.iter().any(|op| matches!(op,
            PrivateLoopAction::ForRange {..} | PrivateLoopAction::WhileCondition {..})) {
            return Err(PrivateLoopPrepareError::Unsupported("必須包含原 Checker 核准的迴圈"));
        }
        if !Self::has_real_return(&operations) {
            return Err(PrivateLoopPrepareError::Unsupported("函式須有 int 返回"));
        }
        let original = verified.source.bytes.clone().into_boxed_slice();
        Ok(Self { verified, original, operations })
    }

    fn matches_source(&self, source: &str) -> bool {
        let Ok(fresh) = Self::prepare(source) else { return false };
        self.original == fresh.original && self.operations == fresh.operations
            && self.verified.plan.declarations.by_source
                == fresh.verified.plan.declarations.by_source
            && self.verified.plan.control_edges == fresh.verified.plan.control_edges
            && self.verified.plan.function_exits == fresh.verified.plan.function_exits
    }

    fn bool_value(
        value: &PrivateBoolCondition,
        slots: &BTreeMap<DeclarationId, bool>,
    ) -> Result<bool, crate::owned_str_runtime::StrRuntimeError> {
        match value {
            PrivateBoolCondition::Constant(value) => Ok(*value),
            PrivateBoolCondition::Bound(identity) => slots.get(identity).copied()
                .ok_or(crate::owned_str_runtime::StrRuntimeError::MissingOwner),
        }
    }

    fn run_actions(
        resources: &mut crate::owned_str_runtime::OwnedStrRuntime,
        operations: &[PrivateLoopAction], depth: usize,
        injected_at: Option<usize>, allocations: &mut usize,
        bool_values: &mut BTreeMap<DeclarationId, bool>,
        condition_checks: &mut Vec<bool>,
        iterator_values: &mut BTreeMap<DeclarationId, i64>,
        iterations: &mut Vec<(DeclarationId, i64)>,
        steps: &mut PrivateLoopStepCounter,
    ) -> Result<PrivateLoopFlow, crate::owned_str_runtime::StrRuntimeError> {
        use crate::owned_str_runtime::StrRuntimeError;
        for operation in operations {
            steps.consume()?;
            match operation {
                PrivateLoopAction::DeclareStr { name, mutable, text } => {
                    resources.declare(name, *mutable, text)?;
                    let index = *allocations;
                    *allocations += 1;
                    if injected_at == Some(index) {
                        return Err(StrRuntimeError::EvaluationFailed);
                    }
                }
                PrivateLoopAction::DeclareBool { identity, value } => {
                    let evaluated = Self::bool_value(value, bool_values)?;
                    if bool_values.insert(*identity, evaluated).is_some() {
                        return Err(StrRuntimeError::DuplicateBinding);
                    }
                }
                PrivateLoopAction::AssignBool { identity, value } => {
                    let evaluated = Self::bool_value(value, bool_values)?;
                    let slot = bool_values.get_mut(identity)
                        .ok_or(StrRuntimeError::MissingOwner)?;
                    *slot = evaluated;
                }
                PrivateLoopAction::ForRange { identity, lower, upper, body } => {
                    for value in *lower..*upper {
                        // 每輪真正迭代邊必須消耗預算，包含空本體，
                        // 不能依賴 Rust Range 長度或一次性的 AST 解析。
                        steps.consume()?;
                        // 真動態迭代，每輪建立獨立的 int Copyable
                        // 宣告 slot；同一 DeclarationId 不得在其生命
                        // 週期內被第二個擁有位置重複使用。
                        if iterator_values.insert(*identity, value).is_some() {
                            return Err(StrRuntimeError::DuplicateBinding);
                        }
                        iterations.push((*identity, value));
                        let step = Self::run_scoped(
                            resources, body, depth + 1, injected_at, allocations,
                            bool_values, condition_checks, iterator_values, iterations,
                            steps,
                        );
                        // 即使 return／break／continue 或故障，必須
                        // 在本輪真字串離域清理完成後撤銷迭代器身分。
                        iterator_values.remove(identity);
                        match step? {
                            PrivateLoopFlow::Normal | PrivateLoopFlow::Continue => {}
                            PrivateLoopFlow::Break => break,
                            PrivateLoopFlow::Return(value) =>
                                return Ok(PrivateLoopFlow::Return(value)),
                        }
                    }
                }
                PrivateLoopAction::WhileCondition { condition, body } => {
                    loop {
                        steps.consume()?;
                        // 已核准 P15：每輪重新求值同一真 bool slot。
                        let selected = Self::bool_value(condition, bool_values)?;
                        condition_checks.push(selected);
                        if !selected {
                            break;
                        }
                        let flow = Self::run_scoped(
                            resources, body, depth + 1, injected_at, allocations,
                            bool_values, condition_checks,
                            iterator_values, iterations, steps,
                        )?;
                        match flow {
                            PrivateLoopFlow::Break => break,
                            PrivateLoopFlow::Return(value) =>
                                return Ok(PrivateLoopFlow::Return(value)),
                            PrivateLoopFlow::Continue =>
                                return Err(StrRuntimeError::EvaluationFailed),
                            PrivateLoopFlow::Normal => {
                                // 尾端 guard=false 的靜態保證，動態仍
                                // 核對，避免無界重新進入或假回傳。
                                if Self::bool_value(condition, bool_values)? {
                                    return Err(StrRuntimeError::EvaluationFailed);
                                }
                            }
                        }
                    }
                }
                PrivateLoopAction::Return(value) =>
                    return Ok(PrivateLoopFlow::Return(*value)),
                PrivateLoopAction::ReturnIterator(identity) => {
                    let value = iterator_values.get(identity).copied()
                        .ok_or(StrRuntimeError::MissingOwner)?;
                    return Ok(PrivateLoopFlow::Return(value));
                }
                PrivateLoopAction::Break if depth > 0 =>
                    return Ok(PrivateLoopFlow::Break),
                PrivateLoopAction::Continue if depth > 0 =>
                    return Ok(PrivateLoopFlow::Continue),
                _ => return Err(StrRuntimeError::EvaluationFailed),
            }
        }
        Ok(PrivateLoopFlow::Normal)
    }

    fn run_scoped(
        resources: &mut crate::owned_str_runtime::OwnedStrRuntime,
        operations: &[PrivateLoopAction], depth: usize,
        injected_at: Option<usize>, allocations: &mut usize,
        bool_values: &mut BTreeMap<DeclarationId, bool>,
        condition_checks: &mut Vec<bool>,
        iterator_values: &mut BTreeMap<DeclarationId, i64>,
        iterations: &mut Vec<(DeclarationId, i64)>,
        steps: &mut PrivateLoopStepCounter,
    ) -> Result<PrivateLoopFlow, crate::owned_str_runtime::StrRuntimeError> {
        resources.enter_scope();
        let result = Self::run_actions(
            resources, operations, depth, injected_at, allocations,
            bool_values, condition_checks, iterator_values, iterations, steps,
        );
        // 包含 continue／break／return／真失敗，單次迭代作用域皆
        // 先釋放唯一資源，再讓控制訊號傳往上一層。
        let cleanup = resources.leave_scope();
        match (result, cleanup) {
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Ok(flow), Ok(())) => Ok(flow),
        }
    }

    fn execute(&self) -> Result<PrivateLoopOutcome, PrivateLoopPrepareError> {
        self.execute_with_budget_and_fault(PrivateLoopExecutionBudget::default(), None)
    }

    /// 本次執行明確安全預算，不新增 CLI 參數與語言固定迴圈上界。
    fn execute_with_budget(&self, budget: PrivateLoopExecutionBudget)
        -> Result<PrivateLoopOutcome, PrivateLoopPrepareError> {
        self.execute_with_budget_and_fault(budget, None)
    }

    /// 故障序號只供 Rust 真 ResourceId 清理測試，不是 voiles 例外語法。
    fn execute_with_fault(&self, injected_at: Option<usize>)
        -> Result<PrivateLoopOutcome, PrivateLoopPrepareError> {
        self.execute_with_budget_and_fault(
            PrivateLoopExecutionBudget::default(), injected_at,
        )
    }

    fn execute_with_budget_and_fault(
        &self, budget: PrivateLoopExecutionBudget, injected_at: Option<usize>,
    ) -> Result<PrivateLoopOutcome, PrivateLoopPrepareError> {
        let Ok(text) = std::str::from_utf8(&self.verified.source.bytes) else {
            return Err(PrivateLoopPrepareError::SourceChanged);
        };
        if self.original.as_ref() != self.verified.source.bytes.as_slice()
            || !self.verified.plan.is_consistent(
                &self.verified.structs, &self.verified.functions, text,
            ) || !self.matches_source(text) {
            return Err(PrivateLoopPrepareError::SourceChanged);
        }
        let mut resources = crate::owned_str_runtime::OwnedStrRuntime::new();
        let mut allocations = 0;
        let mut bool_values = BTreeMap::new();
        let mut condition_checks = Vec::new();
        let mut iterator_values = BTreeMap::new();
        let mut iterations = Vec::new();
        let mut steps = PrivateLoopStepCounter::new(budget);
        let result = match Self::run_actions(
            &mut resources, &self.operations, 0, injected_at, &mut allocations,
            &mut bool_values, &mut condition_checks,
            &mut iterator_values, &mut iterations, &mut steps,
        ) {
            Ok(PrivateLoopFlow::Return(value)) => Ok(value),
            Ok(_) => Err(crate::owned_str_runtime::StrRuntimeError::EvaluationFailed),
            Err(err) => Err(err),
        };
        let events = resources.finish().map_err(PrivateLoopPrepareError::Runtime)?;
        if !crate::owned_str_runtime::event_trace_balanced(&events) {
            return Err(PrivateLoopPrepareError::SourceChanged);
        }
        Ok(PrivateLoopOutcome { result, events, condition_checks, iterations })
    }
}

#[cfg(test)]
mod g18_private_source_loop_tests {
    use super::*;
    use crate::owned_str_runtime::{ResourceEvent,StrRuntimeError};

    fn checked(src: &str) -> PreparedPrivateLoop {
        PreparedPrivateLoop::prepare(src)
            .expect("真私有 Checker 已核准的有限迴圈應可準備")
    }

    #[test]
    fn g18_b_真_for_三次每迭代重新建立唯一_utf8_並離域釋放() {
        let source = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 0..3{\nvar inner:str = \"甲🪼\"\n}\nreturn 9\n}\n";
        let result = checked(source).execute().unwrap();
        assert_eq!(result.result, Ok(9));
        assert_eq!(result.events, [
            ResourceEvent::Created(1),
            ResourceEvent::Created(2), ResourceEvent::Released(2),
            ResourceEvent::Created(3), ResourceEvent::Released(3),
            ResourceEvent::Created(4), ResourceEvent::Released(4),
            ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_b_半開空區間不建立任何假迭代資源() {
        let source = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 2..2{\nvar never:str = \"不配置\"\n}\nreturn 3\n}\n";
        let result = checked(source).execute().unwrap();
        assert_eq!(result.result, Ok(3));
        assert_eq!(result.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_b_真_continue_每次先離域釋放再開始下一次() {
        let source = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 1..3{\nvar inner:str = \"乙\"\ncontinue\n}\nreturn 5\n}\n";
        let result = checked(source).execute().unwrap();
        assert_eq!(result.result, Ok(5));
        assert_eq!(result.events, [
            ResourceEvent::Created(1),
            ResourceEvent::Created(2), ResourceEvent::Released(2),
            ResourceEvent::Created(3), ResourceEvent::Released(3),
            ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_b_真_break_只執行首輪且離域釋放() {
        let source = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 0..4{\nvar inner:str = \"乙\"\nbreak\n}\nreturn 5\n}\n";
        let result = checked(source).execute().unwrap();
        assert_eq!(result.result, Ok(5));
        assert_eq!(result.events, [
            ResourceEvent::Created(1),
            ResourceEvent::Created(2), ResourceEvent::Released(2),
            ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_b_真_while_false_不執行未走過的字串建構() {
        let source = "fn main()->int{\nvar outer:str = \"外\"\nwhile(false){\nvar inner:str = \"跳過\"\n}\nreturn 5\n}\n";
        let result = checked(source).execute().unwrap();
        assert_eq!(result.result, Ok(5));
        assert_eq!(result.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_b_while_true_只在直接已驗證_break_出口下執行一次() {
        let source = "fn main()->int{\nvar outer:str = \"外\"\nwhile(true){\nvar inner:str = \"一次\"\nbreak\n}\nreturn 5\n}\n";
        let result = checked(source).execute().unwrap();
        assert_eq!(result.result, Ok(5));
        assert_eq!(result.events, [
            ResourceEvent::Created(1),
            ResourceEvent::Created(2), ResourceEvent::Released(2),
            ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_c_真_for_內_return_提前退出並按動態完成順序清理() {
        let source = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 0..3{\nvar inner:str = \"一次\"\nreturn 7\n}\nreturn 0\n}\n";
        let result = checked(source).execute().unwrap();
        assert_eq!(result.result, Ok(7));
        assert_eq!(result.events, [
            ResourceEvent::Created(1),
            ResourceEvent::Created(2), ResourceEvent::Released(2),
            ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_c_第二次迭代真資源故障必回滾當次及外層持有者() {
        let source = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 0..3{\nvar inner:str = \"甲\"\n}\nreturn 0\n}\n";
        let result = checked(source).execute_with_fault(Some(2)).unwrap();
        assert_eq!(result.result, Err(StrRuntimeError::EvaluationFailed));
        assert_eq!(result.events, [
            ResourceEvent::Created(1),
            ResourceEvent::Created(2), ResourceEvent::Released(2),
            ResourceEvent::Created(3), ResourceEvent::Released(3),
            ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_b_巢狀_for_各迭代作用域獨立釋放且順序正確() {
        let source = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 0..2{\nfor j in 0..2{\nvar leaf:str = \"葉\"\n}\n}\nreturn 0\n}\n";
        let result = checked(source).execute().unwrap();
        assert_eq!(result.result, Ok(0));
        assert_eq!(result.events, [
            ResourceEvent::Created(1),
            ResourceEvent::Created(2), ResourceEvent::Released(2),
            ResourceEvent::Created(3), ResourceEvent::Released(3),
            ResourceEvent::Created(4), ResourceEvent::Released(4),
            ResourceEvent::Created(5), ResourceEvent::Released(5),
            ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_b_非法隱含_str_複製由原_checker_在第一筆資源前拒絕() {
        let source = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 0..2{\nvar copy:str = outer\n}\nreturn 0\n}\n";
        assert!(matches!(
            PreparedPrivateLoop::prepare(source),
            Err(PrivateLoopPrepareError::Check(_)),
        ));
    }

    #[test]
    fn g18_b_六十五輪合法半開_for_不再遭私有固定上界拒絕() {
        let source = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 0..65{\nvar inner:str = \"甲\"\n}\nreturn 0\n}\n";
        let outcome = checked(source).execute().unwrap();
        assert_eq!(outcome.result, Ok(0));
        assert_eq!(outcome.iterations.len(), 65);
        assert_eq!(outcome.events.iter().filter(|event|
            matches!(event, ResourceEvent::Created(_))).count(), 66);
        assert!(crate::owned_str_runtime::event_trace_balanced(&outcome.events));
    }

    #[test]
    fn g18_b_擅改_for_區間或_control_必須在配置任何資源前拒絕() {
        let source = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 0..2{\nvar inner:str = \"甲\"\n}\nreturn 0\n}\n";
        let mut plan = checked(source);
        let PrivateLoopAction::ForRange { upper, .. } = &mut plan.operations[1] else {
            panic!("原 Parser 應簽發真實 for 計畫");
        };
        *upper += 1;
        assert!(matches!(
            plan.execute(), Err(PrivateLoopPrepareError::SourceChanged),
        ));
    }

    #[test]
    fn g18_b_刪除原_checker_控制轉移或改寫來源不可沿用舊執行許可() {
        let source = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 0..3{\nvar inner:str = \"甲\"\ncontinue\n}\nreturn 0\n}\n";
        let mut plan = checked(source);
        plan.verified.plan.control_edges.clear();
        assert!(matches!(
            plan.execute(), Err(PrivateLoopPrepareError::SourceChanged),
        ));
        let mut plan = checked(source);
        plan.verified.source.bytes[0] = b'!';
        assert!(matches!(
            plan.execute(), Err(PrivateLoopPrepareError::SourceChanged),
        ));
    }

    // G18-B 第二批：具名 bool 按原 P15 Checker 條件每輪重新讀取。
    #[test]
    fn g18_b_具名_var_bool_while_真重讀條件且本體改_false_後停止() {
        let source = "fn main()->int{\nvar flag:bool = true\nvar outer:str = \"外\"\nwhile(flag){\nvar inner:str = \"內🪼\"\nflag = false\n}\nreturn 8\n}\n";
        let result = checked(source).execute().unwrap();
        assert_eq!(result.result, Ok(8));
        assert_eq!(result.condition_checks, [true, false]);
        assert_eq!(result.events, [
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_b_具名_false_while_零迭代且僅建外部資源() {
        let source = "fn main()->int{\nvar flag:bool = false\nvar outer:str = \"外\"\nwhile(flag){\nvar inner:str = \"未選\"\nflag = false\n}\nreturn 3\n}\n";
        let result = checked(source).execute().unwrap();
        assert_eq!(result.result, Ok(3));
        assert_eq!(result.condition_checks, [false]);
        assert_eq!(result.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_b_具名_bool_重新賦值_false_到_true_選擇一次_break() {
        let source = "fn main()->int{\nvar flag:bool = false\nflag = true\nvar outer:str = \"外\"\nwhile(flag){\nvar inner:str = \"真\"\nbreak\n}\nreturn 3\n}\n";
        let result = checked(source).execute().unwrap();
        assert_eq!(result.result, Ok(3));
        assert_eq!(result.condition_checks, [true]);
        assert_eq!(result.events, [
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_b_已核准_copyable_bool_名稱複製後與來源變更不構成別名() {
        let source = "fn main()->int{\nvar source:bool = true\nvar flag:bool = source\nsource = false\nvar outer:str = \"外\"\nwhile(flag){\nvar inner:str = \"仍為真\"\nflag = false\n}\nreturn 4\n}\n";
        let result = checked(source).execute().unwrap();
        assert_eq!(result.result, Ok(4));
        assert_eq!(result.condition_checks, [true, false]);
        assert_eq!(result.events, [
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_b_具名_while_中的_early_return_僅清理實際選中本體與根資源() {
        let source = "fn main()->int{\nconst flag:bool = true\nvar outer:str = \"外\"\nwhile(flag){\nvar inner:str = \"內\"\nreturn 11\n}\nreturn 0\n}\n";
        let result = checked(source).execute().unwrap();
        assert_eq!(result.result, Ok(11));
        assert_eq!(result.condition_checks, [true]);
        assert_eq!(result.events, [
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_c_具名_while_動態故障真清理且不補做未到達賦值() {
        let source = "fn main()->int{\nvar flag:bool = true\nvar outer:str = \"外\"\nwhile(flag){\nvar inner:str = \"內\"\nflag = false\n}\nreturn 0\n}\n";
        let result = checked(source).execute_with_fault(Some(1)).unwrap();
        assert_eq!(result.result, Err(StrRuntimeError::EvaluationFailed));
        assert_eq!(result.condition_checks, [true]);
        assert_eq!(result.events, [
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_b_具名_while_條件_DeclarationId_偷換必須零資源拒絕() {
        let source = "fn main()->int{\nvar flag:bool = true\nvar outer:str = \"外\"\nwhile(flag){\nvar inner:str = \"內\"\nflag = false\n}\nreturn 0\n}\n";
        let mut plan = checked(source);
        let PrivateLoopAction::WhileCondition {
            condition: PrivateBoolCondition::Bound(identity), ..
        } = &mut plan.operations[2] else {
            panic!("必須保存原 Checker 確認的具名 bool");
        };
        *identity = DeclarationId(0);
        assert!(matches!(
            plan.execute(), Err(PrivateLoopPrepareError::SourceChanged),
        ));
    }

    #[test]
    fn g18_b_具名_while_沒有可證明終止時保持非公開拒絕() {
        let source = "fn main()->int{\nvar flag:bool = true\nvar outer:str = \"外\"\nwhile(flag){\nvar inner:str = \"不能假定退出\"\n}\nreturn 0\n}\n";
        assert!(matches!(
            PreparedPrivateLoop::prepare(source),
            Err(PrivateLoopPrepareError::Unsupported(_)),
        ));
    }

    #[test]
    fn g18_b_具名_while_continue_不得跳過直接退出義務() {
        let source = "fn main()->int{\nvar flag:bool = true\nvar outer:str = \"外\"\nwhile(flag){\nvar inner:str = \"甲\"\ncontinue\nflag = false\n}\nreturn 0\n}\n";
        assert!(PreparedPrivateLoop::prepare(source).is_err());
    }

    #[test]
    fn g18_b_具名_const_bool_重賦值須在原_checker_先拒絕() {
        let source = "fn main()->int{\nconst flag:bool = true\nvar outer:str = \"外\"\nwhile(flag){\nvar inner:str = \"甲\"\nflag = false\n}\nreturn 0\n}\n";
        assert!(matches!(
            PreparedPrivateLoop::prepare(source),
            Err(PrivateLoopPrepareError::Check(_)),
        ));
    }

    // G18-B：迭代器本身是已被 Checker 簽發的 int Copyable 宣告，
    // 非只依照 for 區間重複執行相同字串本體的假迭代。
    #[test]
    fn g18_b_真_for_iterator_返回目前迭代整數而非固定常值() {
        let src = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 4..5{\nvar local:str = \"內\"\nreturn i\n}\nreturn 0\n}\n";
        let actual = checked(src).execute().unwrap();
        assert_eq!(actual.result, Ok(4));
        assert_eq!(actual.iterations.iter().map(|(_, v)| *v).collect::<Vec<_>>(), [4]);
        assert_eq!(actual.events, [
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_b_真_for_iterator_多輪實際值不含右端點() {
        let src = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 2..5{\nvar inner:str = \"甲\"\n}\nreturn 6\n}\n";
        let actual = checked(src).execute().unwrap();
        assert_eq!(actual.result, Ok(6));
        assert_eq!(actual.iterations.iter().map(|(_, v)| *v).collect::<Vec<_>>(),
            [2, 3, 4]);
        assert_eq!(actual.events, [
            ResourceEvent::Created(1),
            ResourceEvent::Created(2), ResourceEvent::Released(2),
            ResourceEvent::Created(3), ResourceEvent::Released(3),
            ResourceEvent::Created(4), ResourceEvent::Released(4),
            ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_b_巢狀_for_iterator_返回真內層宣告且逐層清理() {
        let src = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 2..3{\nvar local:str = \"局部\"\nfor j in 7..8{\nvar inner:str = \"內\"\nreturn j\n}\n}\nreturn 0\n}\n";
        let actual = checked(src).execute().unwrap();
        assert_eq!(actual.result, Ok(7));
        assert_eq!(actual.iterations.iter().map(|(_, v)| *v).collect::<Vec<_>>(),
            [2, 7]);
        assert_eq!(actual.events, [
            ResourceEvent::Created(1),
            ResourceEvent::Created(2),
            ResourceEvent::Created(3),
            ResourceEvent::Released(3), ResourceEvent::Released(2),
            ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g18_b_巢狀同名_for_iterator_以真宣告身份遮蔽外層而非名稱猜測() {
        let src = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 1..2{\nfor i in 9..10{\nreturn i\n}\n}\nreturn 0\n}\n";
        let actual = checked(src).execute().unwrap();
        assert_eq!(actual.result, Ok(9));
        let ids = actual.iterations.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1], "同名巢狀迭代宣告必須有不同 DeclarationId");
        assert_eq!(actual.iterations.iter().map(|(_, v)| *v).collect::<Vec<_>>(),
            [1, 9]);
        assert_eq!(actual.events,
            [ResourceEvent::Created(1), ResourceEvent::Released(1)]);
    }

    #[test]
    fn g18_b_真_for_iterator_continue_迭代每輪獨立且沒有逃逸() {
        let src = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 1..4{\nvar inner:str = \"內\"\ncontinue\n}\nreturn 2\n}\n";
        let actual = checked(src).execute().unwrap();
        assert_eq!(actual.result, Ok(2));
        assert_eq!(actual.iterations.iter().map(|(_, v)| *v).collect::<Vec<_>>(),
            [1, 2, 3]);
        assert!(crate::owned_str_runtime::event_trace_balanced(&actual.events));
    }

    #[test]
    fn g18_b_空半開_for_不建立迭代器值且不執行_return() {
        let src = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 2..2{\nreturn i\n}\nreturn 6\n}\n";
        let actual = checked(src).execute().unwrap();
        assert_eq!(actual.result, Ok(6));
        assert!(actual.iterations.is_empty());
        assert_eq!(actual.events,
            [ResourceEvent::Created(1), ResourceEvent::Released(1)]);
    }

    #[test]
    fn g18_b_返回已離域_for_iterator_須原_checker_先拒絕() {
        let src = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 0..2{\nvar inner:str = \"內\"\n}\nreturn i\n}\n";
        assert!(matches!(
            PreparedPrivateLoop::prepare(src),
            Err(PrivateLoopPrepareError::Check(_)),
        ));
    }

    #[test]
    fn g18_b_竄改_for_iterator_身分或_return_來源須零資源拒絕() {
        let src = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 4..5{\nreturn i\n}\nreturn 0\n}\n";
        let mut plan = checked(src);
        let PrivateLoopAction::ForRange { identity, .. } =
            &mut plan.operations[1] else { panic!("必須為原 for 計畫") };
        *identity = DeclarationId(0);
        assert!(matches!(
            plan.execute(), Err(PrivateLoopPrepareError::SourceChanged),
        ));
        let mut plan = checked(src);
        let PrivateLoopAction::ForRange { body, .. } =
            &mut plan.operations[1] else { panic!("必須為原 for 計畫") };
        let PrivateLoopAction::ReturnIterator(identity) =
            &mut body[0] else { panic!("必須為原 Checker 的具名 iterator 返回") };
        *identity = DeclarationId(0);
        assert!(matches!(
            plan.execute(), Err(PrivateLoopPrepareError::SourceChanged),
        ));
    }
}
