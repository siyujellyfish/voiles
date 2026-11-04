// G17／P17.3：只有正式私有 Checker 簽出的直線 .borrow() 才能
// 在已核准 UTF-8 最小執行路徑真正取得／歸還 BorrowTicket。
// 不將靜態 BorrowFact 冒充動態票據；不開放新 CLI 或一般借用逃逸。
#[derive(Debug)]
enum PrivateBorrowPrepareError {
    Check(crate::Diagnostic),
    Unsupported(&'static str),
    SourceChanged,
    Resource(crate::owned_str_runtime::StrRuntimeError),
    Ticket(crate::borrow_safety::BorrowSafetyError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ApprovedDirectBorrow {
    alias: String,
    identity: DeclarationId,
    fact: BorrowFact,
}

struct PreparedPrivateBorrow {
    verified: VerifiedPrivateSource,
    original: Box<[u8]>,
    owner: String,
    owner_id: DeclarationId,
    owner_mutable: bool,
    text: String,
    aliases: Vec<ApprovedDirectBorrow>,
    returned: i64,
}

#[derive(Debug)]
struct PrivateBorrowOutcome {
    result: Result<i64, PrivateBorrowPrepareError>,
    observed_utf8_bytes: Option<usize>,
    events: Vec<crate::owned_str_runtime::ResourceEvent>,
}

impl PreparedPrivateBorrow {
    fn prepare(text: &str) -> Result<Self, PrivateBorrowPrepareError> {
        let verified = verify_source_with_ability(
            text, crate::ownership_policy::approved_leaf_copy_permission,
        ).map_err(PrivateBorrowPrepareError::Check)?;
        if !verified.structs.is_empty() || verified.functions.len() != 1 {
            return Err(PrivateBorrowPrepareError::Unsupported(
                "最小真借用執行只接受不含結構的一份 main",
            ));
        }
        let main = &verified.functions[0];
        if main.name != "main" || main.result_ty != "int" || !main.params.is_empty()
            || main.body.statements.len() < 3
        {
            return Err(PrivateBorrowPrepareError::Unsupported(
                "只執行單一 str 持有者、明確直線借用與整數返回",
            ));
        }
        let StmtKind::Declare(owner) = &main.body.statements[0].kind else {
            return Err(PrivateBorrowPrepareError::Unsupported("缺少具名 str 所有者"));
        };
        if owner.ty != "str" {
            return Err(PrivateBorrowPrepareError::Unsupported("真借用來源必須是 str"));
        }
        let owner_id = DeclarationId(owner.start);
        if !verified.plan.declarations.matches(owner_id, "str", owner.mutable) {
            return Err(PrivateBorrowPrepareError::Unsupported(
                "第一筆 str 持有者缺少正式 Checker 宣告權威",
            ));
        }
        let initializer = owner.initializer.as_ref()
            .ok_or(PrivateBorrowPrepareError::Unsupported("str 所有者需原始初始化"))?;
        let content = private_str_literal(initializer, &verified.tokens)
            .map_err(|_| PrivateBorrowPrepareError::Unsupported(
                "只支援由正式 Lexer 解碼的獨立 str 字面值",
            ))?;
        let mut aliases = Vec::new();
        for statement in &main.body.statements[1..main.body.statements.len()-1] {
            let StmtKind::Declare(binding) = &statement.kind else {
                return Err(PrivateBorrowPrepareError::Unsupported(
                    "直線借用切片中間只能有已驗證別名宣告",
                ));
            };
            if binding.ty != "str" || binding.mutable != owner.mutable {
                return Err(PrivateBorrowPrepareError::Unsupported(
                    "別名型別或 var／const 權限與正式來源不同",
                ));
            }
            let Some(TermKind::Borrow(ref source)) =
                binding.initializer.as_ref().map(|term| &term.kind) else {
                return Err(PrivateBorrowPrepareError::Unsupported(
                    "只接受明確原持有者 .borrow()",
                ));
            };
            if source != &owner.name {
                return Err(PrivateBorrowPrepareError::Unsupported(
                    "借用必須引用真正已驗證的單一所有者",
                ));
            }
            let alias_id = DeclarationId(binding.start);
            if !verified.plan.declarations.matches(alias_id, "str", binding.mutable) {
                return Err(PrivateBorrowPrepareError::Unsupported(
                    "別名缺少 Checker 宣告身分與權限證據",
                ));
            }
            let fact = *verified.plan.declarations.direct_borrows.get(&alias_id)
                .ok_or(PrivateBorrowPrepareError::Unsupported(
                    "缺少正式 Checker 簽發的借用來源事實",
                ))?;
            let kind = if owner.mutable {
                BorrowKind::ExclusiveWrite
            } else {
                BorrowKind::SharedRead
            };
            if fact != BorrowFact::new(owner_id, kind) {
                return Err(PrivateBorrowPrepareError::Unsupported(
                    "借用票據的來源與權限不得由 AST 猜測",
                ));
            }
            aliases.push(ApprovedDirectBorrow {
                alias: binding.name.clone(), identity: alias_id, fact,
            });
        }
        let StmtKind::Return(ret) = &main.body.statements[main.body.statements.len()-1].kind
        else {
            return Err(PrivateBorrowPrepareError::Unsupported("缺少已驗證 int 返回"));
        };
        if !verified.plan.scalar_hir.expression(ret.expression.node_span())
            .is_some_and(|item| item.ty == crate::common_ir::ScalarType::Int)
        {
            return Err(PrivateBorrowPrepareError::Unsupported(
                "整數返回缺少原正式 P10 TypedHIR",
            ));
        }
        let TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(n)) =
            &ret.expression.kind else {
            return Err(PrivateBorrowPrepareError::Unsupported(
                "此借用切片僅可原樣返回合法整數字面值",
            ));
        };
        let returned = crate::syntax_shared::integer_i64(n).map_err(|_|
            PrivateBorrowPrepareError::Unsupported("已核准整數返回超出 i64"),
        )?;
        let original = verified.source.bytes.clone().into_boxed_slice();
        Ok(Self {
            verified, original,
            owner: owner.name.clone(), owner_id, owner_mutable: owner.mutable,
            text: content, aliases, returned,
        })
    }

    fn matches_original(&self, text: &str) -> bool {
        let Ok(fresh) = Self::prepare(text) else { return false };
        self.original == fresh.original
            && self.owner == fresh.owner
            && self.owner_id == fresh.owner_id
            && self.owner_mutable == fresh.owner_mutable
            && self.text == fresh.text
            && self.aliases == fresh.aliases
            && self.returned == fresh.returned
    }

    /// 所有安全驗證前置；真票據在 Runtime 實際執行到借用語句時
    /// 才取得，並與 Checker 的原始 BorrowFact 逐張稽核。
    fn execute(&self) -> Result<PrivateBorrowOutcome, PrivateBorrowPrepareError> {
        let Ok(text) = std::str::from_utf8(&self.verified.source.bytes) else {
            return Err(PrivateBorrowPrepareError::SourceChanged);
        };
        if self.original.as_ref() != self.verified.source.bytes.as_slice()
            || !self.verified.plan.is_consistent(
                &self.verified.structs, &self.verified.functions, text,
            )
            || !self.matches_original(text)
        {
            return Err(PrivateBorrowPrepareError::SourceChanged);
        }
        let mut resources = crate::owned_str_runtime::OwnedStrRuntime::new();
        let mut observed_utf8_bytes = None;
        let result = (|| -> Result<i64, PrivateBorrowPrepareError> {
            use crate::borrow_safety::BorrowSafety;
            resources.declare(&self.owner, self.owner_mutable, &self.text)
                .map_err(PrivateBorrowPrepareError::Resource)?;
            let mut tracker = BorrowSafety::new();
            let mut tickets = Vec::new();
            let mut facts = Vec::new();
            for alias in &self.aliases {
                // 原 Checker 同一來源必須對每個真正會執行的 alias 簽發
                // exact DeclarationId＋BorrowKind；動態票據不能反向簽證。
                if self.verified.plan.declarations.direct_borrows
                    .get(&alias.identity) != Some(&alias.fact)
                {
                    return Err(PrivateBorrowPrepareError::SourceChanged);
                }
                let ticket = tracker.acquire(alias.fact.source, alias.fact.kind)
                    .map_err(PrivateBorrowPrepareError::Ticket)?;
                tickets.push(ticket);
                facts.push(alias.fact);
                tracker.verify_static_snapshot(facts.iter().copied())
                    .map_err(PrivateBorrowPrepareError::Ticket)?;
                // 別名讀取仍由實際持有的 UTF-8 字串提供非擁有視圖。
                // 只有對應票據被追蹤器驗證成功才允許讀取；不複製 String。
                let actual = resources.read(&self.owner)
                    .map_err(PrivateBorrowPrepareError::Resource)?;
                observed_utf8_bytes = Some(actual.len());
                if tracker.check_direct_access(self.owner_id).is_ok() {
                    return Err(PrivateBorrowPrepareError::Ticket(
                        crate::borrow_safety::BorrowSafetyError::SnapshotMismatch,
                    ));
                }
            }
            tracker.finish_scope_verified(&tickets, &facts)
                .map_err(PrivateBorrowPrepareError::Ticket)?;
            tracker.check_direct_access(self.owner_id)
                .map_err(PrivateBorrowPrepareError::Ticket)?;
            Ok(self.returned)
        })();
        // 不管 Runtime 的失敗點為何，擁有者仍由 P16 唯一資源核心
        // 實際 Released 一次；借用永不產生假 Created/Moved/Returned。
        let events = resources.finish().map_err(PrivateBorrowPrepareError::Resource)?;
        if !crate::owned_str_runtime::event_trace_balanced(&events) {
            return Err(PrivateBorrowPrepareError::SourceChanged);
        }
        Ok(PrivateBorrowOutcome { result, observed_utf8_bytes, events })
    }
}

#[cfg(test)]
mod g17_private_source_borrow_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent;

    fn checked(source: &str) -> PreparedPrivateBorrow {
        PreparedPrivateBorrow::prepare(source).expect("真私有 Checker 應核准合法借用")
    }

    #[test]
    fn g17_c_合法_const_str_真共享借用取得和歸還動態票據不複製資源() {
        let code = "fn main()->int{\nconst text:str = \"繁體🪼\"\nconst view:str = text.borrow()\nreturn 7\n}\n";
        let plan = checked(code);
        for _ in 0..2 {
            let outcome = plan.execute().unwrap();
            assert!(matches!(outcome.result, Ok(7)));
            assert_eq!(outcome.observed_utf8_bytes, Some("繁體🪼".len()));
            assert_eq!(outcome.events, [
                ResourceEvent::Created(1), ResourceEvent::Released(1),
            ]);
        }
    }

    #[test]
    fn g17_c_合法_var_str_真獨占票據封鎖持有者直接存取後歸還() {
        let code = "fn main()->int{\nvar text:str = \"唯一\"\nvar view:str = text.borrow()\nreturn 4\n}\n";
        let outcome = checked(code).execute().unwrap();
        assert!(matches!(outcome.result, Ok(4)));
        assert_eq!(outcome.observed_utf8_bytes, Some("唯一".len()));
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g17_c_兩個合法共享別名各有真實票據而同一_utf8_只配置一次() {
        let code = "fn main()->int{\nconst text:str = \"相同來源\"\nconst a:str = text.borrow()\nconst b:str = text.borrow()\nreturn 2\n}\n";
        let outcome = checked(code).execute().unwrap();
        assert!(matches!(outcome.result, Ok(2)));
        assert_eq!(outcome.events, [
            ResourceEvent::Created(1), ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn g17_c_非法借用型別或_var_const_權限不符由真_checker_先拒絕() {
        for code in [
            "fn main()->int{\nconst text:str = \"a\"\nconst view:int = text.borrow()\nreturn 0\n}\n",
            "fn main()->int{\nconst text:str = \"a\"\nvar view:str = text.borrow()\nreturn 0\n}\n",
        ] {
            assert!(matches!(
                PreparedPrivateBorrow::prepare(code), Err(PrivateBorrowPrepareError::Check(_)),
            ));
        }
    }

    #[test]
    fn g17_c_偽造借用宣告_id_不能靠相同名稱在真資源分配前過關() {
        let code = "fn main()->int{\nconst text:str = \"安全\"\nconst view:str = text.borrow()\nreturn 0\n}\n";
        let mut plan = checked(code);
        plan.aliases[0].identity = DeclarationId(plan.aliases[0].identity.0 + 1);
        assert!(matches!(plan.execute(), Err(PrivateBorrowPrepareError::SourceChanged)));
    }

    #[test]
    fn g17_c_偽造共享獨占權限或_source_id_在任何資源分配前拒絕() {
        let code = "fn main()->int{\nconst text:str = \"安全\"\nconst view:str = text.borrow()\nreturn 0\n}\n";
        let mut plan = checked(code);
        plan.aliases[0].fact.kind = BorrowKind::ExclusiveWrite;
        assert!(matches!(plan.execute(), Err(PrivateBorrowPrepareError::SourceChanged)));
        let mut plan = checked(code);
        plan.aliases[0].fact.source = DeclarationId(0);
        assert!(matches!(plan.execute(), Err(PrivateBorrowPrepareError::SourceChanged)));
    }

    #[test]
    fn g17_c_刪去原_checker_直線借用事實不可沿用舊票據授權() {
        let code = "fn main()->int{\nconst text:str = \"安全\"\nconst view:str = text.borrow()\nreturn 0\n}\n";
        let mut plan = checked(code);
        plan.verified.plan.declarations.direct_borrows.clear();
        assert!(matches!(plan.execute(), Err(PrivateBorrowPrepareError::SourceChanged)));
    }

    #[test]
    fn g17_c_篡改已驗證來源位元組不能開始真字串資源執行() {
        let code = "fn main()->int{\nconst text:str = \"安全\"\nconst view:str = text.borrow()\nreturn 0\n}\n";
        let mut plan = checked(code);
        plan.verified.source.bytes[0] = b'x';
        assert!(matches!(plan.execute(), Err(PrivateBorrowPrepareError::SourceChanged)));
    }
}
