// P17-F：只在完整私有 Checker 封閉之後，將合法單一結構建立式
// 的真 Parser 欄位／來源 token 交給既有 P16 MixedStructRuntime 執行。
// 不推測巢狀建構、方法、其他控制流或新的對外語言能力。
#[derive(Debug)]
enum PrivateStructPrepareError {
    Check(crate::Diagnostic),
    Unsupported(&'static str),
    SourceChanged,
    Runtime(crate::mixed_struct_runtime::MixedError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ApprovedSeed {
    Int(i64),
    Bool(bool),
    Str(String),
}
impl ApprovedSeed {
    fn kind(&self) -> crate::mixed_struct_runtime::ScalarKind {
        use crate::mixed_struct_runtime::ScalarKind;
        match self { Self::Int(_) => ScalarKind::Int,
            Self::Bool(_) => ScalarKind::Bool,
            Self::Str(_) => ScalarKind::Str,
        }
    }
    fn runtime(&self) -> crate::mixed_struct_runtime::SeedValue<'_> {
        use crate::mixed_struct_runtime::SeedValue;
        match self { Self::Int(n) => SeedValue::Int(*n),
            Self::Bool(b) => SeedValue::Bool(*b),
            Self::Str(text) => SeedValue::Str(text.as_str()),
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
struct PreparedField {
    name: String,
    kind: crate::mixed_struct_runtime::ScalarKind,
    default: Option<ApprovedSeed>,
    immutable: bool,
}
#[derive(Debug, PartialEq, Eq)]
struct PreparedNamedField {
    name: String,
    seed: ApprovedSeed,
}
struct PreparedPrivateStruct {
    verified: VerifiedPrivateSource,
    original: Box<[u8]>,
    binding_name: String,
    move_target: Option<(String, DeclarationId, DeclarationId)>,
    fields: Vec<PreparedField>,
    supplied: Vec<PreparedNamedField>,
    returned: i64,
}
#[derive(Debug)]
struct PrivateStructOutcome {
    result: Result<i64, crate::mixed_struct_runtime::MixedError>,
    events: Vec<crate::owned_str_runtime::ResourceEvent>,
}

/// 只能消耗真 Lexer／Parser 和 Checker 同來源之純值字面形式。
fn approved_seed(term: &Term, tokens: &[Token])
    -> Result<ApprovedSeed, PrivateStructPrepareError> {
    match &term.kind {
        TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(n)) =>
            crate::syntax_shared::integer_i64(n)
                .map(ApprovedSeed::Int)
                .map_err(|_| PrivateStructPrepareError::Unsupported("整數值超出已驗證範圍")),
        TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Boolean(b)) =>
            Ok(ApprovedSeed::Bool(*b)),
        TermKind::Literal("str") => private_str_literal(term, tokens)
            .map(ApprovedSeed::Str)
            .map_err(|_| PrivateStructPrepareError::Unsupported(
                "字串建立式缺少真 Lexer token",
            )),
        _ => Err(PrivateStructPrepareError::Unsupported(
            "這個切片只接受已核准純值字面值",
        )),
    }
}

impl PreparedPrivateStruct {
    fn prepare(text: &str) -> Result<Self, PrivateStructPrepareError> {
        let verified = verify_source_with_ability(
            text, crate::ownership_policy::approved_leaf_copy_permission,
        ).map_err(PrivateStructPrepareError::Check)?;
        // 所有欄位、具名引數與返回先封閉驗證；任何錯誤都在
        // Runtime::construct 分配第一筆資源之前退出。
        if verified.structs.len() != 1 || verified.functions.len() != 1 {
            return Err(PrivateStructPrepareError::Unsupported("此切片只接受單一結構與 main"));
        }
        let main = &verified.functions[0];
        if main.name != "main" || main.result_ty != "int" || !main.params.is_empty()
            || !(2..=3).contains(&main.body.statements.len()) {
            return Err(PrivateStructPrepareError::Unsupported(
                "此切片僅支援無參數 main 的一次建立與直接整數返回",
            ));
        }
        let StmtKind::Declare(binding) = &main.body.statements[0].kind else {
            return Err(PrivateStructPrepareError::Unsupported("缺少唯一具名結構宣告"));
        };
        let declaration = DeclarationId(binding.start);
        let fact = verified.plan.declarations.typed_declaration(declaration)
            .ok_or(PrivateStructPrepareError::Unsupported("缺少 Checker 具型別宣告身份"))?;
        if fact.ty != binding.ty || fact.mutable != binding.mutable {
            return Err(PrivateStructPrepareError::Unsupported("宣告身份、型別或可變性不一致"));
        }
        let definition = verified.structs.get(&binding.ty)
            .ok_or(PrivateStructPrepareError::Unsupported("缺少真實已驗證結構型別"))?;
        if !definition.methods.is_empty() {
            return Err(PrivateStructPrepareError::Unsupported("此切片尚未支援結構方法執行"));
        }
        let initializer = binding.initializer.as_ref()
            .ok_or(PrivateStructPrepareError::Unsupported("結構實例必須初始化"))?;
        let named = match &initializer.kind {
            TermKind::Construct(name, args) if name == &binding.ty =>
                args.as_slice(),
            TermKind::UnresolvedZeroArgs(name) if name == &binding.ty => &[],
            _ => return Err(PrivateStructPrepareError::Unsupported(
                "此切片只執行已驗證具名建立式或空建立式",
            )),
        };
        let mut fields = Vec::new();
        for field in &definition.fields {
            let kind = match field.ty.as_str() {
                "int" => crate::mixed_struct_runtime::ScalarKind::Int,
                "bool" => crate::mixed_struct_runtime::ScalarKind::Bool,
                "str" => crate::mixed_struct_runtime::ScalarKind::Str,
                _ => return Err(PrivateStructPrepareError::Unsupported(
                    "此切片尚未支援巢狀結構／float／未知資源型別",
                )),
            };
            let default = field.default.as_ref()
                .map(|term| approved_seed(term, &verified.tokens)).transpose()?;
            if default.as_ref().is_some_and(|seed| seed.kind() != kind) {
                return Err(PrivateStructPrepareError::Unsupported("欄位預設與宣告型別不一致"));
            }
            fields.push(PreparedField {
                name: field.name.clone(), kind, default,
                immutable: field.immutable,
            });
        }
        let mut supplied = Vec::new();
        for (name, expression) in named {
            let field = fields.iter().find(|field| &field.name == name)
                .ok_or(PrivateStructPrepareError::Unsupported("具名欄位不在已驗證結構中"))?;
            let seed = approved_seed(expression, &verified.tokens)?;
            if seed.kind() != field.kind {
                return Err(PrivateStructPrepareError::Unsupported("具名欄位值型別不符"));
            }
            supplied.push(PreparedNamedField { name: name.clone(), seed });
        }
        // 只接通 Checker 逐筆證明的同型別整筆移動；不可用名稱猜測授權。
        let mut move_target = None;
        if main.body.statements.len() == 3 {
            let StmtKind::Declare(target) = &main.body.statements[1].kind else {
                return Err(PrivateStructPrepareError::Unsupported("結構整筆移動必須使用具型別宣告"));
            };
            if target.ty != binding.ty || !target.mutable || target.name == binding.name {
                return Err(PrivateStructPrepareError::Unsupported("結構移動目標型別或權限不合法"));
            }
            let term = target.initializer.as_ref().ok_or(
                PrivateStructPrepareError::Unsupported("移動目標必須立即初始化"),
            )?;
            if !matches!(&term.kind, TermKind::Move(name) if name == &binding.name) {
                return Err(PrivateStructPrepareError::Unsupported("只接受真 Checker 核准的整筆 move()"));
            }
            let target_id = DeclarationId(target.start);
            let target_fact = verified.plan.declarations.typed_declaration(target_id)
                .ok_or(PrivateStructPrepareError::Unsupported("移動目標缺少真宣告證據"))?;
            if target_fact.ty != target.ty || !target_fact.mutable
                || !verified.plan.declarations.direct_moves.contains(&(declaration, target_id))
            {
                return Err(PrivateStructPrepareError::Unsupported(
                    "移動來源與目標缺少同一 Checker 簽發的轉移配對",
                ));
            }
            move_target = Some((target.name.clone(), declaration, target_id));
        }
        let StmtKind::Return(ret) = &main.body.statements[main.body.statements.len() - 1].kind else {
            return Err(PrivateStructPrepareError::Unsupported("缺少最後 return"));
        };
        let TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(value)) =
            &ret.expression.kind else {
            return Err(PrivateStructPrepareError::Unsupported("只接受明確整數字面值返回"));
        };
        let returned = crate::syntax_shared::integer_i64(value)
            .map_err(|_| PrivateStructPrepareError::Unsupported("返回值超出 i64 範圍"))?;
        let binding_name = binding.name.clone();
        let original = verified.source.bytes.clone().into_boxed_slice();
        Ok(Self {
            verified, original, binding_name, move_target,
            fields, supplied, returned,
        })
    }

    /// P17-F 安全補強：每次進入資源執行前，都由同一正式私有 Checker
    /// 對**原始來源**重新取得一次安全投影，核對不可由未經驗證的
    /// Rust 內部計畫改寫任何待建立資源、預設值、具名引數或返回值。
    /// 這會額外重跑 Checker，但不建立第二份 Parser 或改變公開語意；
    /// 後續可改成驗證後封閉不可寫計畫以消除此成本。
    fn is_same_prepared_payload(&self, text: &str) -> bool {
        let Ok(fresh) = Self::prepare(text) else { return false };
        self.binding_name == fresh.binding_name
            && self.move_target == fresh.move_target
            && self.fields == fresh.fields
            && self.supplied == fresh.supplied
            && self.returned == fresh.returned
            && self.original == fresh.original
    }

    fn execute(&self) -> Result<PrivateStructOutcome, PrivateStructPrepareError> {
        self.execute_with_fault(None)
    }

    /// 只供 Rust 真資源回滾測試；不新增 voiles 語言例外。
    fn execute_with_fault(&self, fail_after: Option<&str>)
        -> Result<PrivateStructOutcome, PrivateStructPrepareError> {
        let Ok(text) = std::str::from_utf8(&self.verified.source.bytes) else {
            return Err(PrivateStructPrepareError::SourceChanged);
        };
        if self.original.as_ref() != self.verified.source.bytes.as_slice()
            || !self.verified.plan.is_consistent(
                &self.verified.structs, &self.verified.functions, text,
            )
            || !self.is_same_prepared_payload(text)
        {
            return Err(PrivateStructPrepareError::SourceChanged);
        }
        if self.move_target.as_ref().is_some_and(|(_, source, target)|
            !self.verified.plan.declarations.direct_moves.contains(&(*source, *target))
        ) {
            return Err(PrivateStructPrepareError::SourceChanged);
        }
        let schema: Vec<_> = self.fields.iter().map(|field|
            crate::mixed_struct_runtime::MixedFieldSpec {
                name: &field.name, kind: field.kind,
                default: field.default.as_ref().map(ApprovedSeed::runtime),
                immutable: field.immutable,
            }).collect();
        let named: Vec<_> = self.supplied.iter().map(|value|
            crate::mixed_struct_runtime::NamedMixed {
                field: &value.name, value: value.seed.runtime(),
            }).collect();
        let mut runtime = crate::mixed_struct_runtime::MixedStructRuntime::new();
        let result = (|| {
            runtime.construct(&self.binding_name, &schema, &named, fail_after)?;
            if let Some((target, _, _)) = &self.move_target {
                runtime.move_whole(&self.binding_name, target)?;
            }
            Ok(self.returned)
        })();
        // 所有真 UTF-8 資源均由唯一 P16 Runtime 負責清理；
        // 靜態 CleanupObligation 不可直接製造動態 Released 事件。
        let events = runtime.finish().map_err(PrivateStructPrepareError::Runtime)?;
        if !crate::owned_str_runtime::event_trace_balanced(&events) {
            return Err(PrivateStructPrepareError::Runtime(
                crate::mixed_struct_runtime::MixedError::Resource(
                    crate::owned_str_runtime::StrRuntimeError::OutstandingResource,
                ),
            ));
        }
        Ok(PrivateStructOutcome { result, events })
    }
}

#[cfg(test)]
mod p17_private_struct_execution_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent;

    fn checked(text: &str) -> PreparedPrivateStruct {
        PreparedPrivateStruct::prepare(text).expect("合法真私有結構來源須完成 Checker")
    }


    #[test]
    fn p17_f_結構真原始計畫合法時可重複執行且逐次釋放資源() {
        let code = "struct Note{\nvar label:str = \"繁體\"\nvar active:bool = true\n}\nfn main()->int{\nvar n:Note = Note()\nreturn 3\n}\n";
        let prepared = checked(code);
        for _ in 0..2 {
            let outcome = prepared.execute().unwrap();
            assert_eq!(outcome.result, Ok(3));
            assert_eq!(outcome.events, [
                ResourceEvent::Created(1), ResourceEvent::Released(1),
            ]);
        }
    }

    #[test]
    fn p17_f_已驗證欄位預設值內容改寫不得使用原_checker_授權資源() {
        let code = "struct Note{\nvar label:str = \"原始\"\n}\nfn main()->int{\nvar n:Note = Note()\nreturn 0\n}\n";
        let mut prepared = checked(code);
        let Some(ApprovedSeed::Str(value)) = prepared.fields[0].default.as_mut() else {
            panic!("正式欄位應保存字串預設");
        };
        *value = "偽造值".to_owned();
        assert!(matches!(
            prepared.execute(), Err(PrivateStructPrepareError::SourceChanged),
        ));
    }

    #[test]
    fn p17_f_已驗證具名欄位值與欄位名稱若被偷換必須事前拒絕() {
        let code = "struct Note{\nvar a:str = \"原值\"\nvar b:str = \"原值二\"\n}\nfn main()->int{\nvar n:Note = Note(a=\"實參\")\nreturn 0\n}\n";
        let mut prepared = checked(code);
        prepared.supplied[0].seed = ApprovedSeed::Str("偽造".into());
        assert!(matches!(prepared.execute(),
            Err(PrivateStructPrepareError::SourceChanged)));
        let mut prepared = checked(code);
        prepared.supplied[0].name = "b".into();
        assert!(matches!(prepared.execute(),
            Err(PrivateStructPrepareError::SourceChanged)));
    }

    #[test]
    fn p17_f_欄位種類與不可變權限竄改不得產生結構資源事件() {
        let code = "struct Note{\nvar title:str = \"字串\"\nvar flag:bool = true\n}\nfn main()->int{\nvar n:Note = Note()\nreturn 0\n}\n";
        let mut prepared = checked(code);
        prepared.fields[0].immutable = !prepared.fields[0].immutable;
        assert!(matches!(prepared.execute(),
            Err(PrivateStructPrepareError::SourceChanged)));
        let mut prepared = checked(code);
        prepared.fields[0].kind = crate::mixed_struct_runtime::ScalarKind::Bool;
        assert!(matches!(prepared.execute(),
            Err(PrivateStructPrepareError::SourceChanged)));
    }

    #[test]
    fn p17_f_入口返回整數與實例名稱若被竄改不能沿用原來源() {
        let code = "struct Note{\nvar label:str = \"安全\"\n}\nfn main()->int{\nvar n:Note = Note()\nreturn 2\n}\n";
        let mut prepared = checked(code);
        prepared.returned = 9;
        assert!(matches!(prepared.execute(),
            Err(PrivateStructPrepareError::SourceChanged)));
        let mut prepared = checked(code);
        prepared.binding_name = "other".into();
        assert!(matches!(prepared.execute(),
            Err(PrivateStructPrepareError::SourceChanged)));
    }
    #[test]
    fn p17_f_真私有_checker_空建立式求值預設_utf8_並真正釋放() {
        let text = "struct Note{\nvar title:str = \"繁體中文\"\nvar active:bool = true\n}\nfn main()->int{\nvar note:Note = Note()\nreturn 2\n}\n";
        let outcome = checked(text).execute().unwrap();
        assert_eq!(outcome.result, Ok(2));
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn p17_f_真具名引數先依來源序後評估未覆蓋預設且逆序清理() {
        let text = "struct Note{\nvar first:str = \"預設一\"\nvar second:str = \"預設二\"\n}\nfn main()->int{\nvar note:Note = Note(second=\"具名二\")\nreturn 0\n}\n";
        let outcome = checked(text).execute().unwrap();
        assert_eq!(outcome.result, Ok(0));
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn p17_f_真具名欄位覆蓋預設時不分配多餘_utf8_資源() {
        let text = "struct Memo{\nvar title:str = \"不執行的預設\"\n}\nfn main()->int{\nvar memo:Memo = Memo(title=\"具名內容\")\nreturn 0\n}\n";
        let outcome = checked(text).execute().unwrap();
        assert_eq!(outcome.result, Ok(0));
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn p17_f_真結構預設建構中途失敗回滾並逆序釋放已建資源() {
        let text = "struct Note{\nvar first:str = \"預設一\"\nvar second:str = \"預設二\"\n}\nfn main()->int{\nvar note:Note = Note(second=\"具名二\")\nreturn 0\n}\n";
        let outcome = checked(text).execute_with_fault(Some("first")).unwrap();
        assert!(matches!(outcome.result, Err(crate::mixed_struct_runtime::MixedError::EvaluationFailed(_))));
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn p17_f_不合法未知具名欄位在真私有_checker_階段拒絕() {
        let text = "struct Note{\nvar title:str = \"預設\"\n}\nfn main()->int{\nvar note:Note = Note(missing=\"值\")\nreturn 0\n}\n";
        assert!(matches!(
            PreparedPrivateStruct::prepare(text),
            Err(PrivateStructPrepareError::Check(_)),
        ));
    }

    #[test]
    fn p17_f_巢狀結構或來源替換仍不會執行未驗證資源() {
        let nested = include_str!("../../tests/fixtures/p13_stage/已初始化巢狀具名預設值.voil");
        assert!(matches!(
            PreparedPrivateStruct::prepare(nested),
            Err(PrivateStructPrepareError::Unsupported(_)),
        ));
        let text = "struct Note{\nvar title:str = \"預設\"\n}\nfn main()->int{\nvar note:Note = Note()\nreturn 0\n}\n";
        let mut plan = checked(text);
        plan.verified.source.bytes[0] = b'x';
        assert!(matches!(plan.execute(), Err(PrivateStructPrepareError::SourceChanged)));
    }
    #[test]
    fn p17_c_真私有混合結構整筆移動保留唯一_utf8_resource_id() {
        let source = include_str!("../../tests/fixtures/p13_stage/包含字串欄位的結構允許整筆移動.voil");
        let outcome = checked(source).execute().unwrap();
        assert_eq!(outcome.result, Ok(0));
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Moved(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn p17_c_含字串結構的隱含複製必須在分配前由真_checker_拒絕() {
        let source = include_str!("../../tests/fixtures/p13_stage/包含字串欄位的結構禁止複製.voil");
        assert!(matches!(
            PreparedPrivateStruct::prepare(source),
            Err(PrivateStructPrepareError::Check(_)),
        ));
    }

    #[test]
    fn p17_c_刪除_checker_整筆移動配對不得建立任何真資源() {
        let source = include_str!("../../tests/fixtures/p13_stage/包含字串欄位的結構允許整筆移動.voil");
        let mut prepared = checked(source);
        prepared.verified.plan.declarations.direct_moves.clear();
        assert!(matches!(
            prepared.execute(),
            Err(PrivateStructPrepareError::SourceChanged),
        ));
    }

    #[test]
    fn p17_c_已準備整筆移動目標被篡改即使原_checker_正確仍須拒絕() {
        let source = include_str!("../../tests/fixtures/p13_stage/包含字串欄位的結構允許整筆移動.voil");
        let mut prepared = checked(source);
        prepared.move_target.as_mut().expect("應有真 move 計畫").0 = "偽造目標".into();
        assert!(matches!(
            prepared.execute(),
            Err(PrivateStructPrepareError::SourceChanged),
        ));
    }

    #[test]
    fn p17_f_具名引數尾逗號沿用正式_parser_且只建立一次資源() {
        let source = "struct Note{\nvar label:str = \"預設\"\n}\nfn main()->int{\nvar note:Note = Note(label=\"具名\",)\nreturn 0\n}\n";
        let outcome = checked(source).execute().unwrap();
        assert_eq!(outcome.result, Ok(0));
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn p17_f_重複具名引數須由真_checker_在建資源前拒絕() {
        let source = "struct Note{\nvar label:str = \"預設\"\n}\nfn main()->int{\nvar note:Note = Note(label=\"一\",label=\"二\")\nreturn 0\n}\n";
        assert!(matches!(
            PreparedPrivateStruct::prepare(source),
            Err(PrivateStructPrepareError::Check(_)),
        ));
    }

    #[test]
    fn p17_f_沒有預設的_const_欄位須在配置前拒絕() {
        let source = "struct Note{\nconst label:str\n}\nfn main()->int{\nvar note:Note = Note(label=\"具名\")\nreturn 0\n}\n";
        assert!(matches!(
            PreparedPrivateStruct::prepare(source),
            Err(PrivateStructPrepareError::Check(_)),
        ));
    }

    #[test]
    fn p17_f_未初始化_const_實例必須由真_checker_拒絕() {
        let source = "struct Note{\nvar label:str = \"預設\"\n}\nfn main()->int{\nconst note:Note\nreturn 0\n}\n";
        assert!(matches!(
            PreparedPrivateStruct::prepare(source),
            Err(PrivateStructPrepareError::Check(_)),
        ));
    }

    #[test]
    fn p17_f_已完整初始化_const_實例可以安全建立並離域釋放() {
        let source = "struct Note{\nconst label:str = \"定值\"\n}\nfn main()->int{\nconst note:Note = Note()\nreturn 0\n}\n";
        let outcome = checked(source).execute().unwrap();
        assert_eq!(outcome.result, Ok(0));
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn p17_f_空建立保留未初始化_var_欄位不得虛構資源事件() {
        let source = "struct Note{\nvar label:str\n}\nfn main()->int{\nvar note:Note = Note()\nreturn 0\n}\n";
        let outcome = checked(source).execute().unwrap();
        assert_eq!(outcome.result, Ok(0));
        assert!(outcome.events.is_empty());
    }

}
