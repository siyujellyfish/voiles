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
    ForRange { lower: i64, upper: i64, body: Vec<PrivateLoopAction> },
    WhileCondition {
        condition: PrivateBoolCondition,
        body: Vec<PrivateLoopAction>,
    },
    Return(i64),
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
}

impl PreparedPrivateLoop {
    /// 本 private evaluator 為避免任何尚未核准的執行耗盡政策，
    /// 僅承接能在準備階段證明有限且很小的測試區間。這是工程
    /// 接通子集的上界，不是 voiles 語言 for 上限。
    const MAX_PRIVATE_ITERATIONS: i128 = 64;

    fn verified_actions(
        statements: &[Stmt], verified: &VerifiedPrivateSource, inside_loop: bool,
        bool_bindings: &mut BTreeMap<String, DeclarationId>,
    ) -> Result<Vec<PrivateLoopAction>, PrivateLoopPrepareError> {
        let mut result = Vec::new();
        for statement in statements {
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
                StmtKind::ForRange { iterator_start, lower, upper, body, .. } => {
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
                    if count > Self::MAX_PRIVATE_ITERATIONS {
                        return Err(PrivateLoopPrepareError::Unsupported(
                            "此非公開試驗切片只執行可證明的小型有限區間",
                        ));
                    }
                    // 空區間本體根本不會執行；原 Checker 可合法跳過
                    // 它的動態宣告，不能憑 AST 為未簽發的值建立 ResourceId。
                    let compiled = if count == 0 {
                        Vec::new()
                    } else {
                        Self::verified_actions(&body.statements, verified, true, bool_bindings)?
                    };
                    PrivateLoopAction::ForRange {
                        lower: start, upper: end, body: compiled,
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
                            &body.statements, verified, true, bool_bindings,
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
                    let TermKind::Scalar(
                        crate::syntax_shared::ScalarLiteral::Integer(raw),
                    ) = &ret.expression.kind else {
                        return Err(PrivateLoopPrepareError::Unsupported(
                            "迴圈最小真返回只接受已核准的 int 常值",
                        ));
                    };
                    if verified.plan.scalar_hir.expression(ret.expression.node_span())
                        .is_none_or(|e| e.ty != crate::common_ir::ScalarType::Int) {
                        return Err(PrivateLoopPrepareError::Unsupported(
                            "返回常值缺少正式 P10 TypedHIR 型別",
                        ));
                    }
                    let value = crate::syntax_shared::integer_i64(raw).map_err(|_|
                        PrivateLoopPrepareError::Unsupported("return int 超出 i64 有效範圍"),
                    )?;
                    PrivateLoopAction::Return(value)
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
        let operations = Self::verified_actions(&main.body.statements, &verified, false, &mut BTreeMap::new())?;
        if !operations.iter().any(|op| matches!(op,
            PrivateLoopAction::ForRange {..} | PrivateLoopAction::WhileConstant {..})) {
            return Err(PrivateLoopPrepareError::Unsupported("必須包含原 Checker 核准的迴圈"));
        }
        if !operations.iter().any(|op| matches!(op, PrivateLoopAction::Return(_))) {
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

    fn run_actions(
        resources: &mut crate::owned_str_runtime::OwnedStrRuntime,
        operations: &[PrivateLoopAction], depth: usize,
        injected_at: Option<usize>, allocations: &mut usize,
    ) -> Result<PrivateLoopFlow, crate::owned_str_runtime::StrRuntimeError> {
        use crate::owned_str_runtime::StrRuntimeError;
        for operation in operations {
            match operation {
                PrivateLoopAction::DeclareStr { name, mutable, text } => {
                    resources.declare(name, *mutable, text)?;
                    let index = *allocations;
                    *allocations += 1;
                    if injected_at == Some(index) {
                        return Err(StrRuntimeError::EvaluationFailed);
                    }
                }
                PrivateLoopAction::ForRange { lower, upper, body } => {
                    for _ in *lower..*upper {
                        let flow = Self::run_scoped(
                            resources, body, depth + 1, injected_at, allocations,
                        )?;
                        match flow {
                            PrivateLoopFlow::Normal | PrivateLoopFlow::Continue => {}
                            PrivateLoopFlow::Break => break,
                            PrivateLoopFlow::Return(value) =>
                                return Ok(PrivateLoopFlow::Return(value)),
                        }
                    }
                }
                PrivateLoopAction::WhileConstant { value, body } => {
                    if *value {
                        let flow = Self::run_scoped(
                            resources, body, depth + 1, injected_at, allocations,
                        )?;
                        match flow {
                            PrivateLoopFlow::Break => {}
                            PrivateLoopFlow::Return(value) =>
                                return Ok(PrivateLoopFlow::Return(value)),
                            PrivateLoopFlow::Normal | PrivateLoopFlow::Continue =>
                                return Err(StrRuntimeError::EvaluationFailed),
                        }
                    }
                }
                PrivateLoopAction::Return(value) =>
                    return Ok(PrivateLoopFlow::Return(*value)),
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
    ) -> Result<PrivateLoopFlow, crate::owned_str_runtime::StrRuntimeError> {
        resources.enter_scope();
        let result = Self::run_actions(resources, operations, depth, injected_at, allocations);
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
        self.execute_with_fault(None)
    }

    /// 故障序號只供 Rust 真 ResourceId 清理測試，不是 voiles 例外語法。
    fn execute_with_fault(&self, injected_at: Option<usize>)
        -> Result<PrivateLoopOutcome, PrivateLoopPrepareError> {
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
        let result = match Self::run_actions(
            &mut resources, &self.operations, 0, injected_at, &mut allocations,
        ) {
            Ok(PrivateLoopFlow::Return(value)) => Ok(value),
            Ok(_) => Err(crate::owned_str_runtime::StrRuntimeError::EvaluationFailed),
            Err(err) => Err(err),
        };
        let events = resources.finish().map_err(PrivateLoopPrepareError::Runtime)?;
        if !crate::owned_str_runtime::event_trace_balanced(&events) {
            return Err(PrivateLoopPrepareError::SourceChanged);
        }
        Ok(PrivateLoopOutcome { result, events })
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
    fn g18_b_超出私有已證明有限上界只保守拒絕不新增語言錯誤() {
        let source = "fn main()->int{\nvar outer:str = \"外\"\nfor i in 0..65{\nvar inner:str = \"甲\"\n}\nreturn 0\n}\n";
        assert!(matches!(
            PreparedPrivateLoop::prepare(source),
            Err(PrivateLoopPrepareError::Unsupported(_)),
        ));
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
}
