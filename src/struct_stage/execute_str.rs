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
    Return(i64),
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
        let mut operations = Vec::new();
        let mut returned = false;
        let mut has_resource = false;
        for statement in &main.body.statements {
            if returned {
                return Err(PrivateStrPrepareError::Unsupported("return 後不執行語句"));
            }
            match &statement.kind {
                StmtKind::Declare(binding) if binding.ty == "str" => {
                    let identity = DeclarationId(binding.start);
                    let typed = verified.plan.declarations.typed_declaration(identity)
                        .ok_or(PrivateStrPrepareError::Unsupported("缺少真 Checker str 宣告證據"))?;
                    if typed.ty != "str" || typed.mutable != binding.mutable {
                        return Err(PrivateStrPrepareError::Unsupported("宣告型別或寫入權限不一致"));
                    }
                    if bound.contains_key(&binding.name) {
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
                    operations.push(PrivateStrAction::AssignLiteral {
                        name: assignment.name.clone(),
                        text: private_str_literal(&assignment.expression, &verified.tokens)?,
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
