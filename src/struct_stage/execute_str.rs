// P17-C：只執行已通過同一份私有 Checker 完整驗證的最小直線 str 子集。
// 與 P01 公開執行器隔離，不解析第二套語法、也不把靜態責任當 ResourceEvent。
#[derive(Debug)]
enum PrivateStrPrepareError {
    Check(crate::Diagnostic),
    Unsupported(&'static str),
    SourceChanged,
    Cleanup(crate::owned_str_runtime::StrRuntimeError),
}
#[derive(Debug)]
enum PrivateStrAction {
    DeclareLiteral { name: String, mutable: bool, text: String },
    DeclareMoved { name: String, from: String },
    AssignLiteral { name: String, text: String },
    /// VA16-03-A：既有 var str 目的地接收來源明確 move，舊資源原子釋放。
    AssignMoved { to: String, from: String },
    /// P18-B：只執行真 Checker 已封閉的 bool 常值分支及局部字串。
    LiteralBranch {
        condition: PrivateBoolCondition,
        when_true: Vec<PrivateStrLocalLiteral>,
        when_false: Vec<PrivateStrLocalLiteral>,
    },
    /// P18-B：保留真 Checker 宣告身份的已初始化 bool 純值。
    DeclareBool { identity: DeclarationId, value: bool },
    Return(i64),
}
/// 具名 bool 必須已在來源中由原 Checker 確認宣告和型別；
/// Runtime 自身以 DeclarationId 讀取，不從同名文字猜測。
#[derive(Debug, Clone, Copy)]
enum PrivateBoolCondition {
    Constant(bool),
    Bound(DeclarationId),
}
#[derive(Debug)]
struct PrivateStrLocalLiteral {
    name: String,
    mutable: bool,
    text: String,
}

/// 唯讀取自正式私有 Parser 的區塊和已驗證宣告身分。
/// 這個首批切片禁止分支中的 return／賦值／巢狀控制轉移，
/// 避免在缺少動態 CleanupPlan 時假裝已支援完整資源控制流。
fn prepare_str_literal_branch(
    block: &crate::syntax_shared::ParsedBlock<Stmt>,
    verified: &VerifiedPrivateSource,
) -> Result<Vec<PrivateStrLocalLiteral>, PrivateStrPrepareError> {
    let mut names = BTreeSet::new();
    let mut declarations = Vec::new();
    for statement in &block.statements {
        let StmtKind::Declare(binding) = &statement.kind else {
            return Err(PrivateStrPrepareError::Unsupported(
                "此批分支僅支援作用域內的已驗證 str 常值宣告",
            ));
        };
        if binding.ty != "str" || !names.insert(binding.name.clone()) {
            return Err(PrivateStrPrepareError::Unsupported(
                "此批分支只允許互不重複的 str 局部宣告",
            ));
        }
        let Some(fact) = verified.plan.declarations.typed_declaration(
            DeclarationId(binding.start),
        ) else {
            return Err(PrivateStrPrepareError::Unsupported(
                "分支局部字串缺少 Checker 宣告身分",
            ));
        };
        if fact.ty != "str" || fact.mutable != binding.mutable {
            return Err(PrivateStrPrepareError::Unsupported(
                "分支局部字串型別或寫入權限與 Checker 不符",
            ));
        }
        let init = binding.initializer.as_ref().ok_or(
            PrivateStrPrepareError::Unsupported("分支局部字串必須立即初始化"),
        )?;
        let text = private_str_literal(init, &verified.tokens)?;
        declarations.push(PrivateStrLocalLiteral {
            name: binding.name.clone(), mutable: binding.mutable, text,
        });
    }
    Ok(declarations)
}

fn execute_str_literal_branch(
    resources: &mut crate::owned_str_runtime::OwnedStrRuntime,
    literals: &[PrivateStrLocalLiteral],
) -> Result<(), crate::owned_str_runtime::StrRuntimeError> {
    resources.enter_scope();
    let outcome = (|| {
        for binding in literals {
            resources.declare(&binding.name, binding.mutable, &binding.text)?;
        }
        Ok(())
    })();
    // 即使建立途中失敗也必須離開真正作用域，依序清理已建立資源。
    let cleanup = resources.leave_scope();
    match (outcome, cleanup) {
        (Err(err), _) => Err(err),
        (Ok(()), Err(err)) => Err(err),
        (Ok(()), Ok(())) => Ok(()),
    }
}

#[derive(Debug)]
struct PrivateStrOutcome {
    result: Result<i64, crate::owned_str_runtime::StrRuntimeError>,
    events: Vec<crate::owned_str_runtime::ResourceEvent>,
}
struct PreparedPrivateStr {
    verified: VerifiedPrivateSource,
    original: Box<[u8]>,
    operations: Vec<PrivateStrAction>,
}

/// 僅從這份真 Parser 已保留的 UTF-8 token 拿到正式 Lexer 解碼值。
/// 不對原始文字 find() 猜測 literal，更不允許推測未知字串語法。
fn private_str_literal(
    term: &Term, tokens: &[Token],
) -> Result<String, PrivateStrPrepareError> {
    if !matches!(&term.kind, TermKind::Literal("str")) {
        return Err(PrivateStrPrepareError::Unsupported("僅處理已核准 str 字面值"));
    }
    match tokens.iter().find(|token|
        token.start == term.start && token.end == term.end
    ).map(|token| &token.kind) {
        Some(TokenKind::StringLiteral(value)) => Ok(value.clone()),
        _ => Err(PrivateStrPrepareError::Unsupported(
            "字串運算式缺少原 Lexer 的唯一 token 來源證據",
        )),
    }
}

impl PreparedPrivateStr {
    fn prepare(text: &str) -> Result<Self, PrivateStrPrepareError> {
        let verified = verify_source_with_ability(
            text, crate::ownership_policy::approved_leaf_copy_permission,
        ).map_err(PrivateStrPrepareError::Check)?;
        // P17-C 第一垂直切片只允許單一 main 的局部 str 直線語句。
        // 先完整預檢每一條語句，再允許任何 Rust String 資源建立。
        if !verified.structs.is_empty() || verified.functions.len() != 1 {
            return Err(PrivateStrPrepareError::Unsupported("不支援此私有結構或多函式執行形狀"));
        }
        let main = &verified.functions[0];
        if main.name != "main" || main.result_ty != "int" || !main.params.is_empty() {
            return Err(PrivateStrPrepareError::Unsupported(
                "此切片只接受無參數 main 回傳 int",
            ));
        }
        let mut bound = BTreeMap::<String, (DeclarationId, bool)>::new();
        let mut bool_bindings = BTreeMap::<String, DeclarationId>::new();
        let mut operations = Vec::new();
        let mut returned = false;
        let mut has_resource = false;
        for statement in &main.body.statements {
            if returned {
                return Err(PrivateStrPrepareError::Unsupported("return 後不執行語句"));
            }
            match &statement.kind {
                StmtKind::Declare(binding) if binding.ty == "bool" => {
                    let identity = DeclarationId(binding.start);
                    let typed = verified.plan.declarations.typed_declaration(identity)
                        .ok_or(PrivateStrPrepareError::Unsupported(
                            "具名 bool 缺少真 Checker 宣告身分",
                        ))?;
                    if typed.ty != "bool" || typed.mutable != binding.mutable
                        || bound.contains_key(&binding.name)
                        || bool_bindings.contains_key(&binding.name) {
                        return Err(PrivateStrPrepareError::Unsupported(
                            "bool 宣告型別、可寫權限或唯一身分不符",
                        ));
                    }
                    let Some(init) = &binding.initializer else {
                        return Err(PrivateStrPrepareError::Unsupported(
                            "此執行切片要求 bool 有直接初始值",
                        ));
                    };
                    let TermKind::Scalar(
                        crate::syntax_shared::ScalarLiteral::Boolean(value),
                    ) = &init.kind else {
                        return Err(PrivateStrPrepareError::Unsupported(
                            "此執行切片只接受原 Lexer 的 bool 常值初始值",
                        ));
                    };
                    let expected = if *value { "true" } else { "false" };
                    if text.get(init.start..init.end) != Some(expected) {
                        return Err(PrivateStrPrepareError::Unsupported(
                            "bool 原始常值與真 Parser 來源不一致",
                        ));
                    }
                    bool_bindings.insert(binding.name.clone(), identity);
                    operations.push(PrivateStrAction::DeclareBool {
                        identity, value: *value,
                    });
                }
                StmtKind::Declare(binding) if binding.ty == "str" => {
                    let identity = DeclarationId(binding.start);
                    let typed = verified.plan.declarations.typed_declaration(identity)
                        .ok_or(PrivateStrPrepareError::Unsupported("缺少真 Checker str 宣告證據"))?;
                    if typed.ty != "str" || typed.mutable != binding.mutable {
                        return Err(PrivateStrPrepareError::Unsupported("宣告型別或寫入權限不一致"));
                    }
                    if bound.contains_key(&binding.name)
                        || bool_bindings.contains_key(&binding.name) {
                        return Err(PrivateStrPrepareError::Unsupported("此切片禁止同名遮蔽"));
                    }
                    let init = binding.initializer.as_ref()
                        .ok_or(PrivateStrPrepareError::Unsupported("此切片需要已初始化 str"))?;
                    let action = match &init.kind {
                        TermKind::Literal("str") => PrivateStrAction::DeclareLiteral {
                            name: binding.name.clone(), mutable: binding.mutable,
                            text: private_str_literal(init, &verified.tokens)?,
                        },
                        TermKind::Move(from) if binding.mutable => {
                            let source = bound.get(from)
                                .ok_or(PrivateStrPrepareError::Unsupported("移動來源尚未宣告"))?.0;
                            if !verified.plan.declarations.direct_moves.contains(&(source, identity)) {
                                return Err(PrivateStrPrepareError::Unsupported(
                                    "移動缺少原 Checker 身分轉移證明",
                                ));
                            }
                            PrivateStrAction::DeclareMoved {
                                name: binding.name.clone(), from: from.clone(),
                            }
                        }
                        _ => return Err(PrivateStrPrepareError::Unsupported(
                            "此切片不能執行未證明的字串建構或移動",
                        )),
                    };
                    bound.insert(binding.name.clone(), (identity, binding.mutable));
                    operations.push(action);
                    has_resource = true;
                }
                StmtKind::Assign(assignment) => {
                    if !bound.get(&assignment.name).is_some_and(|(_, mutable)| *mutable) {
                        return Err(PrivateStrPrepareError::Unsupported("賦值目標不存在或不可變"));
                    }
                    let action = match &assignment.expression.kind {
                        TermKind::Literal("str") => PrivateStrAction::AssignLiteral {
                            name: assignment.name.clone(),
                            text: private_str_literal(
                                &assignment.expression, &verified.tokens,
                            )?,
                        },
                        TermKind::Move(source_name) => {
                            // 正式 Checker 已確認此真來源可移動且無非法借用；
                            // 此處只接入既有核准 VA16-03-A 原子重綁，
                            // 不自行補定其它型別、未知引用或自身覆寫語意。
                            let source = bound.get(source_name)
                                .ok_or(PrivateStrPrepareError::Unsupported(
                                    "整筆移動賦值來源尚未宣告",
                                ))?;
                            let target = bound.get(&assignment.name)
                                .ok_or(PrivateStrPrepareError::Unsupported(
                                    "整筆移動賦值目標尚未宣告",
                                ))?;
                            if source.0 == target.0
                                || verified.plan.declarations.typed_declaration(source.0)
                                    .is_none_or(|fact| fact.ty != "str")
                                || verified.plan.declarations.typed_declaration(target.0)
                                    .is_none_or(|fact| fact.ty != "str" || !fact.mutable) {
                                return Err(PrivateStrPrepareError::Unsupported(
                                    "str 整筆移動賦值缺少合法來源／可寫目標身分",
                                ));
                            }
                            PrivateStrAction::AssignMoved {
                                to: assignment.name.clone(),
                                from: source_name.clone(),
                            }
                        }
                        _ => return Err(PrivateStrPrepareError::Unsupported(
                            "此切片只允許真 str 字面值或已驗證整筆移動賦值",
                        )),
                    };
                    operations.push(action);
                }
                StmtKind::If { branch, no } => {
                    let condition = match &branch.branch.condition.kind {
                        TermKind::Scalar(
                            crate::syntax_shared::ScalarLiteral::Boolean(value),
                        ) => {
                            let expected = if *value { "true" } else { "false" };
                            if text.get(branch.branch.condition.start
                                ..branch.branch.condition.end) != Some(expected) {
                                return Err(PrivateStrPrepareError::Unsupported(
                                    "bool 條件與真來源詞彙不一致",
                                ));
                            }
                            PrivateBoolCondition::Constant(*value)
                        }
                        TermKind::Name(name) => {
                            let identity = bool_bindings.get(name).copied()
                                .ok_or(PrivateStrPrepareError::Unsupported(
                                    "此切片只允許已驗證的同函式 bool 宣告作條件",
                                ))?;
                            if !verified.tokens.iter().any(|token|
                                token.start == branch.branch.condition.start
                                && token.end == branch.branch.condition.end
                                && matches!(&token.kind, TokenKind::Name(lexed) if lexed == name)
                            ) {
                                return Err(PrivateStrPrepareError::Unsupported(
                                    "bool 條件未對應原 Lexer 已解析的具名 token",
                                ));
                            }
                            PrivateBoolCondition::Bound(identity)
                        }
                        _ => return Err(PrivateStrPrepareError::Unsupported(
                            "此切片僅執行 bool 常值或同函式已驗證 bool 名稱",
                        )),
                    };
                    let when_true = prepare_str_literal_branch(
                        &branch.branch.body, &verified,
                    )?;
                    let when_false = match no {
                        None => Vec::new(),
                        Some(crate::syntax_shared::ParsedElse::Block(otherwise)) =>
                            prepare_str_literal_branch(otherwise, &verified)?,
                        Some(_) => return Err(PrivateStrPrepareError::Unsupported(
                            "此批尚未支援 else-if 或非區塊分支的資源執行",
                        )),
                    };
                    if !when_true.is_empty() || !when_false.is_empty() {
                        has_resource = true;
                    }
                    operations.push(PrivateStrAction::LiteralBranch {
                        condition, when_true, when_false,
                    });
                }
                StmtKind::Return(ret) => {
                    let TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(raw)) =
                        &ret.expression.kind else {
                        return Err(PrivateStrPrepareError::Unsupported("此切片只接受整數字面值返回"));
                    };
                    let result = crate::syntax_shared::integer_i64(raw)
                        .map_err(|_| PrivateStrPrepareError::Unsupported("返回的整數字面值無效"))?;
                    operations.push(PrivateStrAction::Return(result));
                    returned = true;
                }
                _ => return Err(PrivateStrPrepareError::Unsupported(
                    "尚未核准這種私有執行期控制流或資源操作",
                )),
            }
        }
        if !returned || !has_resource {
            return Err(PrivateStrPrepareError::Unsupported("需要真 str 資源及明確 return"));
        }
        let original = verified.source.bytes.clone().into_boxed_slice();
        Ok(Self { verified, original, operations })
    }

    fn execute(&self) -> Result<PrivateStrOutcome, PrivateStrPrepareError> {
        self.execute_with_fault(None)
    }

    /// fault 只供內部真資源回滾測試，不是 voiles 錯誤／例外語意。
    fn execute_with_fault(
        &self, fail_assignment_at: Option<usize>,
    ) -> Result<PrivateStrOutcome, PrivateStrPrepareError> {
        let Ok(text) = std::str::from_utf8(&self.verified.source.bytes) else {
            return Err(PrivateStrPrepareError::SourceChanged);
        };
        if self.original.as_ref() != self.verified.source.bytes.as_slice()
            || !self.verified.plan.is_consistent(
                &self.verified.structs, &self.verified.functions, text,
            ) {
            return Err(PrivateStrPrepareError::SourceChanged);
        }
        let mut resources = crate::owned_str_runtime::OwnedStrRuntime::new();
        let result = (|| {
            let mut bool_values = BTreeMap::<DeclarationId, bool>::new();
            let mut assignment_index = 0usize;
            for op in &self.operations {
                match op {
                    PrivateStrAction::DeclareLiteral { name, mutable, text } =>
                        resources.declare(name, *mutable, text)?,
                    PrivateStrAction::DeclareMoved { name, from } =>
                        resources.move_named(from, name)?,
                    PrivateStrAction::AssignLiteral { name, text } => {
                        resources.assign_checked(
                            name, text, fail_assignment_at == Some(assignment_index),
                        )?;
                        assignment_index += 1;
                    }
                    PrivateStrAction::AssignMoved { to, from } =>
                        resources.replace_from(from, to)?,
                    PrivateStrAction::DeclareBool { identity, value } => {
                        if bool_values.insert(*identity, *value).is_some() {
                            return Err(crate::owned_str_runtime::StrRuntimeError::DuplicateBinding);
                        }
                    }
                    PrivateStrAction::LiteralBranch { condition, when_true, when_false } => {
                        let selected = match condition {
                            PrivateBoolCondition::Constant(value) => *value,
                            PrivateBoolCondition::Bound(identity) =>
                                bool_values.get(identity).copied().ok_or(
                                    crate::owned_str_runtime::StrRuntimeError::MissingOwner,
                                )?,
                        };
                        execute_str_literal_branch(
                            &mut resources,
                            if selected { when_true } else { when_false },
                        )?;
                    }
                    PrivateStrAction::Return(value) => return Ok(*value),
                }
            }
            Err(crate::owned_str_runtime::StrRuntimeError::IncompleteReturn)
        })();
        // 無論執行成功與否，唯一真實 ResourceId 由原 P16 Runtime
        // 釋放，不能從靜態 CleanupObligation 推測 Released 事件。
        let events = resources.finish().map_err(PrivateStrPrepareError::Cleanup)?;
        if !crate::owned_str_runtime::event_trace_balanced(&events) {
            return Err(PrivateStrPrepareError::Cleanup(
                crate::owned_str_runtime::StrRuntimeError::OutstandingResource
            ));
        }
        Ok(PrivateStrOutcome { result, events })
    }
}

#[cfg(test)]
mod p17_private_str_execution_tests {
    use super::*;
    use crate::owned_str_runtime::{ResourceEvent, StrRuntimeError};


    #[test]
    fn p17_c_真私有_checker_整筆_move_覆寫現存_str_舊資源立即釋放() {
        let code = "fn main()->int{\nvar source:str = \"第一\"\nvar target:str = \"第二\"\ntarget = source.move()\nreturn 0\n}\n";
        let plan = PreparedPrivateStr::prepare(code).expect("VA16-03-A 真 Checker 允許");
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(0));
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Moved(1), ResourceEvent::Released(2),
            ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn p17_c_移動後重新初始化原持有者_按真實完成次序逆序清理() {
        let code = "fn main()->int{\nvar source:str = \"原始\"\nvar target:str = \"預設\"\ntarget = source.move()\nsource = \"復原\"\nreturn 1\n}\n";
        let plan = PreparedPrivateStr::prepare(code).expect("來源明確 move 後 var 可重新初始化");
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(1));
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Moved(1), ResourceEvent::Released(2),
            ResourceEvent::Created(3),
            ResourceEvent::Released(3), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn p17_c_賦值目標不可變與自身移動在_checker_先拒絕() {
        for code in [
            "fn main()->int{\nvar a:str = \"a\"\na = a.move()\nreturn 0\n}\n",
            "fn main()->int{\nvar source:str = \"src\"\nconst target:str = \"old\"\ntarget = source.move()\nreturn 0\n}\n",
        ] {
            assert!(matches!(
                PreparedPrivateStr::prepare(code),
                Err(PrivateStrPrepareError::Check(_)),
            ));
        }
    }

    #[test]
    fn p17_c_已移動來源再次轉交必須在建立資源之前靜態拒絕() {
        let code = "fn main()->int{\nvar source:str = \"a\"\nvar target:str = \"b\"\ntarget = source.move()\nvar another:str = source.move()\nreturn 0\n}\n";
        assert!(matches!(
            PreparedPrivateStr::prepare(code),
            Err(PrivateStrPrepareError::Check(_)),
        ));
    }

    #[test]
    fn p17_c_真資源整筆重綁前若來源改寫不得產生任何實際事件() {
        let code = "fn main()->int{\nvar from:str = \"a\"\nvar into:str = \"b\"\ninto = from.move()\nreturn 0\n}\n";
        let mut prepared = PreparedPrivateStr::prepare(code).unwrap();
        prepared.verified.source.bytes[0] = b'x';
        assert!(matches!(
            prepared.execute(), Err(PrivateStrPrepareError::SourceChanged),
        ));
    }



    #[test]
    fn p18_b_具名_bool_條件由真宣告身分決定實際資源分支() {
        let code = "fn main()->int{\nconst choose:bool = true\nvar outer:str = \"外\"\nif (choose){\nvar selected:str = \"入\"\n}else{\nvar skipped:str = \"不執行\"\n}\nreturn 2\n}\n";
        let plan = PreparedPrivateStr::prepare(code).expect("具名 bool 必須先完成真 Checker");
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(2));
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn p18_b_具名_bool_false_未選分支永不產生_utf8_資源() {
        let code = "fn main()->int{\nvar choose:bool = false\nvar outer:str = \"外\"\nif (choose){\nvar skipped:str = \"未執行\"\n}\nreturn 0\n}\n";
        let outcome = PreparedPrivateStr::prepare(code).unwrap().execute().unwrap();
        assert_eq!(outcome.result, Ok(0));
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn p18_b_來源為_bool_但是由字串同名冒充條件須事前拒絕() {
        let code = "fn main()->int{\nvar outer:str = \"外\"\nif (outer){\nvar skipped:str = \"拒絕\"\n}\nreturn 0\n}\n";
        assert!(matches!(
            PreparedPrivateStr::prepare(code),
            Err(PrivateStrPrepareError::Check(_)),
        ));
    }

    #[test]
    fn p18_b_未初始化_bool_或未知名稱不得開始資源執行() {
        for code in [
            "fn main()->int{\nvar flag:bool\nvar outer:str = \"a\"\nif (flag){\nvar x:str = \"x\"\n}\nreturn 0\n}\n",
            "fn main()->int{\nvar outer:str = \"a\"\nif (unknown){\nvar x:str = \"x\"\n}\nreturn 0\n}\n",
        ] {
            assert!(matches!(
                PreparedPrivateStr::prepare(code),
                Err(PrivateStrPrepareError::Check(_)),
            ));
        }
    }

    #[test]
    fn p18_b_非純值_bool_初始式即便_checker_通過也不偷渡新求值能力() {
        let code = "fn main()->int{\nvar first:bool = true\nvar second:bool = first\nvar outer:str = \"x\"\nif (second){\nvar x:str = \"x\"\n}\nreturn 0\n}\n";
        assert!(matches!(
            PreparedPrivateStr::prepare(code),
            Err(PrivateStrPrepareError::Unsupported(_)),
        ));
    }

    #[test]
    fn p18_b_真私有_成立分支建立兩筆_str_並在外層返回前逆序清理() {
        let code = "fn main()->int{\nvar outer:str = \"外層\"\nif (true){\nvar first:str = \"一\"\nvar second:str = \"二\"\n}\nreturn 0\n}\n";
        let plan = PreparedPrivateStr::prepare(code).expect("真 Checker 必須允許獨立局部資源");
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(0));
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Created(3), ResourceEvent::Released(3),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn p18_b_不成立分支不可建立資源_真_else_才會配置與離域清理() {
        let code = "fn main()->int{\nvar outer:str = \"外\"\nif (false){\nvar missed:str = \"絕不建立\"\n}else{\nvar chosen:str = \"只建立這筆\"\n}\nreturn 1\n}\n";
        let plan = PreparedPrivateStr::prepare(code).expect("Checker 應驗證兩個真區塊");
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.result, Ok(1));
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn p18_b_條件_false_且無_else_不產生未走到的資源事件() {
        let code = "fn main()->int{\nvar outer:str = \"外\"\nif (false){\nvar never:str = \"不分配\"\n}\nreturn 0\n}\n";
        let plan = PreparedPrivateStr::prepare(code).unwrap();
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn p18_b_局部同名遮蔽在離開真資源作用域後不破壞外層持有者() {
        let code = "fn main()->int{\nvar item:str = \"外\"\nif (true){\nvar item:str = \"內\"\n}\nreturn 0\n}\n";
        let plan = PreparedPrivateStr::prepare(code).unwrap();
        let outcome = plan.execute().unwrap();
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn p18_b_分支局部所有者跨作用域外洩必須在_checker_拒絕() {
        let code = "fn main()->int{\nif (true){\nvar inner:str = \"局部\"\n}\ninner = \"不可越界\"\nreturn 0\n}\n";
        assert!(matches!(
            PreparedPrivateStr::prepare(code),
            Err(PrivateStrPrepareError::Check(_)),
        ));
    }

    #[test]
    fn p18_b_分支內返回或巢狀資源控制尚未驗證必須保守拒絕() {
        let code = "fn main()->int{\nvar outside:str = \"外\"\nif (true){\nvar local:str = \"內\"\nreturn 1\n}\nreturn 0\n}\n";
        assert!(matches!(
            PreparedPrivateStr::prepare(code),
            Err(PrivateStrPrepareError::Unsupported(_)),
        ));
    }

    #[test]
    fn p17_c_真私有_checker_來源驅動字串建立移動再初始化並逆序清理() {
        let code = include_str!("../../tests/fixtures/p13_stage/字串具名整筆移動與重新初始化.voil");
        let prepared = PreparedPrivateStr::prepare(code).expect("既有合法私有資源來源必須通過 Checker");
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(0));
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Moved(1),
            ResourceEvent::Created(2), ResourceEvent::Moved(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn p17_c_真私有_lexer_字串內容含中文及跳脫仍只有一筆真資源() {
        let code = r#"fn main()->int{
var word:str = "繁體中文 🪼\n"
return 1
}
"#;
        let prepared = PreparedPrivateStr::prepare(code).unwrap();
        let outcome = prepared.execute().unwrap();
        assert_eq!(outcome.result, Ok(1));
        assert_eq!(outcome.events, [ResourceEvent::Created(1),ResourceEvent::Released(1)]);
    }

    #[test]
    fn p17_c_非法字串隱含複製在真_checker_階段就拒絕() {
        let code = include_str!("../../tests/fixtures/p13_stage/禁止字串名稱隱含複製.voil");
        assert!(matches!(
            PreparedPrivateStr::prepare(code),
            Err(PrivateStrPrepareError::Check(_)),
        ));
    }

    #[test]
    fn p17_c_尚未接通的結構或分支在資源建立前保守拒絕() {
        let code = include_str!("../../tests/fixtures/p13_stage/包含字串欄位的結構允許整筆移動.voil");
        assert!(matches!(
            PreparedPrivateStr::prepare(code),
            Err(PrivateStrPrepareError::Unsupported(_)),
        ));
    }

    #[test]
    fn p17_c_原始來源遭到替換不得產生動態資源事件() {
        let mut prepared = PreparedPrivateStr::prepare(
            "fn main()->int{\nvar x:str = \"abc\"\nreturn 0\n}\n",
        ).unwrap();
        prepared.verified.source.bytes[0] = b'x';
        assert!(matches!(prepared.execute(), Err(PrivateStrPrepareError::SourceChanged)));
    }

    #[test]
    fn p17_c_真資源覆寫錯誤注入原子回滾並清理原持有者() {
        let code = "fn main()->int{\nvar x:str = \"原本\"\nx = \"新值\"\nreturn 0\n}\n";
        let prepared = PreparedPrivateStr::prepare(code).unwrap();
        let outcome = prepared.execute_with_fault(Some(0)).unwrap();
        assert_eq!(outcome.result, Err(StrRuntimeError::EvaluationFailed));
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
    }
}
