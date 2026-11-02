// P16.6-R03：以單一已驗證來源，核對真實 UTF-8 事件與抽象所有權／資源帳簿。
// 此處僅覆蓋已核准的直線 str 建立、具名整筆移交與最終清理，非一般 P17 執行器。

#[cfg(test)]
mod p16_6_r03_來源身分與資源帳簿交叉驗證 {
    use super::*;
    use crate::resource_borrow_pipeline::SafetyPipeline;
    use crate::owned_str_runtime::ResourceEvent;

    #[test]
    fn f11_完整_checker_hir_必須與真實宣告型別及_const_var_權限逐筆相符() {
        use crate::common_ir::{NodeSpan, ResolvedUse, ScalarType, TypedUse, VerifiedScalarHir};
        let code = "fn main()->int{\nvar n:int = 1\nconst label:int = 2\nvar alias:int = n\nn = alias\nreturn n\n}\n";
        let checked = verified_private_source(code).expect("合法來源應交付完整 Checker HIR");
        let facts = &checked.plan.declarations;
        assert!(facts.scalar_hir_is_consistent(&checked.plan.scalar_hir, code));
        let owner = DeclarationId(code.find("var n:int").unwrap());
        let constant = DeclarationId(code.find("const label:int").unwrap());
        let span = NodeSpan::new(
            code.find("return n").unwrap() + "return ".len(),
            code.find("return n").unwrap() + "return n".len(),
        );
        let mut hir = VerifiedScalarHir::default();
        hir.record_reference(TypedUse::new(span, ResolvedUse::new(
            owner, ScalarType::Int)));
        hir.record_expression(span, ScalarType::Int);
        assert!(facts.scalar_hir_is_consistent(&hir, code));
        // 相同來源節點的第二種型別、來源身分或賦值跨度必須留下
        // 不可恢復的衝突，不能以首次有效值仍在索引裡就宣告安全。
        hir.record_expression(span, ScalarType::Bool);
        assert!(!facts.scalar_hir_is_consistent(&hir, code));
        hir.record_expression(span, ScalarType::Int);
        assert!(!facts.scalar_hir_is_consistent(&hir, code));

        let mut forged_reference = VerifiedScalarHir::default();
        forged_reference.record_expression(span, ScalarType::Int);
        forged_reference.record_reference(TypedUse::new(
            span, ResolvedUse::new(owner, ScalarType::Int)));
        forged_reference.record_reference(TypedUse::new(
            span, ResolvedUse::new(constant, ScalarType::Int)));
        assert!(!facts.scalar_hir_is_consistent(&forged_reference, code),
            "引用身分衝突即使其第一筆權威有效仍不能交付 Checker 計畫");

        let mut forged_assignment = VerifiedScalarHir::default();
        forged_assignment.record_assignment(TypedUse::new(
            span, ResolvedUse::new(owner, ScalarType::Int)));
        forged_assignment.record_assignment(TypedUse::new(
            NodeSpan::new(span.start, span.end + 1),
            ResolvedUse::new(owner, ScalarType::Int)));
        assert!(!facts.scalar_hir_is_consistent(&forged_assignment, code),
            "同一賦值位置的不同來源跨度不得由最後一筆掩蓋");
        let mut unclosed = VerifiedScalarHir::default();
        unclosed.record_reference(TypedUse::new(span, ResolvedUse::new(
            owner, ScalarType::Int)));
        assert!(!facts.scalar_hir_is_consistent(&unclosed, code),
            "同來源身分的具名引用沒有 P10／Checker 運算式型別事實不得簽發");
        let mut wrong_source = VerifiedScalarHir::default();
        wrong_source.record_reference(TypedUse::new(span, ResolvedUse::new(
            DeclarationId(code.len() + 20), ScalarType::Int)));
        assert!(!facts.scalar_hir_is_consistent(&wrong_source, code),
            "沒有 Checker 真實宣告的身分不可進入安全計畫");
        let mut wrong_type = VerifiedScalarHir::default();
        wrong_type.record_reference(TypedUse::new(span, ResolvedUse::new(
            owner, ScalarType::Bool)));
        assert!(!facts.scalar_hir_is_consistent(&wrong_type, code),
            "不能將來源 int 偷換為 bool");
        let mut immutable_write = VerifiedScalarHir::default();
        immutable_write.record_assignment(TypedUse::new(span, ResolvedUse::new(
            constant, ScalarType::Int)));
        assert!(!facts.scalar_hir_is_consistent(&immutable_write, code),
            "const 不能產生合法的 var 賦值型別 HIR");
        let mut out_of_bounds = VerifiedScalarHir::default();
        out_of_bounds.record_expression(NodeSpan::new(code.len() + 1, code.len() + 3),
            ScalarType::Int);
        assert!(!facts.scalar_hir_is_consistent(&out_of_bounds, code),
            "未經原始 UTF-8 範圍驗證的位置不可冒充型別證據");
    }

    #[test]
    fn f11_正式計畫整體驗證禁止以另一份_hir_偷換_checker_來源證據() {
        let src = "fn main()->int{\nvar n:int = 1\nreturn n\n}\n";
        let checked = verified_private_source(src).expect("合法 Checker 應生成完整計畫");
        assert!(checked.plan.is_consistent(&checked.structs, src));
        let mut forged = VerifiedSafetyPlan::from_checker(
            VerifiedDeclarationFacts::from_checker(
                checked.plan.declarations.by_source.clone(),
                checked.plan.declarations.direct_moves.clone(),
                checked.plan.declarations.named_returns.clone(),
                checked.plan.declarations.field_writes.clone(),
            ),
            crate::common_ir::VerifiedScalarHir::default(),
        );
        forged.declarations.call_borrows = checked.plan.declarations.call_borrows.clone();
        // 不可在已建立的正式計畫內偷換空 HIR，仍使用 Checker
        // 交付時封存的原始運算式／引用／賦值位置義務驗證。
        forged.expression_spans = checked.plan.expression_spans.clone();
        forged.reference_spans = checked.plan.reference_spans.clone();
        forged.assignment_spans = checked.plan.assignment_spans.clone();
        assert!(checked.plan.scalar_hir.verified_expressions().count() > 0);
        assert!(!forged.is_consistent(&checked.structs, src),
            "不得刪除正式 Checker 已驗證的 HIR 證據後仍宣稱計畫閉合");
    }

    #[test]
    fn f11_真正函式呼叫借用計畫與宣告身分_共享重數_呼叫位置閉合() {
        let code = "fn view(a:str,b:str)->int{\nreturn 2\n}\nfn main()->int{\nconst text:str = \"資源🪼\"\nvar answer:int = view(text,text)\nreturn answer\n}\n";
        let checked = verified_private_source(code).expect("已核准真實函式來源");
        let start = code.find("view(text,text)").unwrap();
        let span = crate::common_ir::NodeSpan::new(start, start + "view(text,text)".len());
        let source = DeclarationId(code.find("const text:str").unwrap());
        let call = checked.plan.declarations.call_borrows_at(span).expect("必須交付真實呼叫借用來源");
        assert_eq!(call, [
            BorrowFact::new(source, BorrowKind::SharedRead),
            BorrowFact::new(source, BorrowKind::SharedRead),
        ]);
        assert!(checked.plan.declarations.evidence_is_consistent(&checked.structs, code));
        assert!(checked.plan.declarations.call_borrows_at(
            crate::common_ir::NodeSpan::new(span.start, span.end + 1)).is_none());

        let facts = &checked.plan.declarations;
        let mut fake = VerifiedDeclarationFacts::from_checker(
            facts.by_source.clone(), facts.direct_moves.clone(),
            facts.named_returns.clone(), facts.field_writes.clone(),
        );
        fake.call_borrows = facts.call_borrows.clone();
        fake.call_borrows.get_mut(&start).unwrap().facts.push(
            BorrowFact::new(DeclarationId(code.len() + 5), BorrowKind::SharedRead));
        assert!(!fake.evidence_is_consistent(&checked.structs, code),
            "不能為未經 Checker 宣告來源的借用偽造真實呼叫證據");

        let mut forged = VerifiedDeclarationFacts::from_checker(
            facts.by_source.clone(), facts.direct_moves.clone(),
            facts.named_returns.clone(), facts.field_writes.clone(),
        );
        forged.call_borrows = facts.call_borrows.clone();
        forged.call_borrows.get_mut(&start).unwrap().facts.push(
            BorrowFact::new(source, BorrowKind::ExclusiveWrite));
        assert!(!forged.evidence_is_consistent(&checked.structs, code),
            "共享與獨占的活躍集合不可冒充單次有效呼叫票據計畫");
    }

    #[test]
    fn f11_不能將_utf8_字元中間的位元組冒充合法已驗證_hir_來源() {
        use crate::common_ir::{NodeSpan, ScalarType, VerifiedScalarHir};
        let code = "fn main()->int{\nvar text:str = \"海洋🪼\"\nreturn 0\n}\n";
        let checked = verified_private_source(code).expect("合法 UTF-8 私有來源應被接受");
        assert!(checked.plan.declarations.scalar_hir_is_consistent(&checked.plan.scalar_hir, code));
        let first = code.find('海').expect("必須有多位元組 UTF-8 字元");
        assert!(!code.is_char_boundary(first + 1));
        assert!(!code.is_char_boundary(first + 2));
        let mut fake = VerifiedScalarHir::default();
        fake.record_expression(NodeSpan::new(first + 1, first + 2),
            ScalarType::Int);
        assert!(!checked.plan.declarations.scalar_hir_is_consistent(&fake, code),
            "即使在原始碼長度內，字元中間的範圍也不是合法來源跨度");
        let mut fake_end = VerifiedScalarHir::default();
        fake_end.record_expression(NodeSpan::new(first, first + 2), ScalarType::Int);
        assert!(!checked.plan.declarations.scalar_hir_is_consistent(&fake_end, code));
    }

    #[test]
    fn f11_資源安全證據必須在型別來源和欄位權限上完全閉合() {
        let code = "struct Holder{\nvar title:str = \"舊\"\nconst id:str = \"固定\"\n}\nfn main()->int{\nvar owner:Holder = Holder()\nowner.title = \"新\"\nvar transferred:Holder = owner.move()\nreturn 0\n}\n";
        let checked = verified_private_source(code).expect("合法結構整筆資源來源");
        let main = checked.functions.iter().find(|fun| fun.name == "main").unwrap();
        let owner = DeclarationId(main.body[0].start);
        let write = main.body[1].start;
        let target = DeclarationId(main.body[2].start);
        let facts = &checked.plan.declarations;
        assert!(facts.evidence_is_consistent(&checked.structs, code));
        assert!(facts.verified_direct_move(owner, target, "Holder").is_some());
        assert!(facts.verified_field_write(owner, write, "Holder", "title").is_some());

        // 真實 AST 只有一個 target／欄位寫入語句；製造另一個表面合法、
        // 位於 UTF-8 邊界內的來源，確認不能將兩個持有者或寫入權威
        // 冒充同一個資源轉移與清理事件。
        let fake_owner = DeclarationId(owner.0 + 1);
        assert!(code.is_char_boundary(fake_owner.0));
        let mut duplicated_target = VerifiedDeclarationFacts::from_checker(
            facts.by_source.clone(), facts.direct_moves.clone(),
            facts.named_returns.clone(), facts.field_writes.clone(),
        );
        duplicated_target.by_source.insert(fake_owner,
            crate::common_ir::TypedDeclaration::new(
                fake_owner, "Holder".to_owned(), true));
        duplicated_target.direct_moves.insert((fake_owner, target));
        assert!(!duplicated_target.evidence_is_consistent(&checked.structs, code),
            "即使兩個來源都有相同型別，一個宣告目標不能接收兩次整筆資源");
        assert!(duplicated_target.verified_direct_move(owner, target, "Holder").is_none(),
            "單筆橋接不能忽略整張證據表中的競爭持有者");

        let mut duplicated_write = VerifiedDeclarationFacts::from_checker(
            facts.by_source.clone(), facts.direct_moves.clone(),
            facts.named_returns.clone(), facts.field_writes.clone(),
        );
        duplicated_write.by_source.insert(fake_owner,
            crate::common_ir::TypedDeclaration::new(
                fake_owner, "Holder".to_owned(), true));
        duplicated_write.field_writes.insert((fake_owner, write, "title".to_owned()));
        assert!(!duplicated_write.evidence_is_consistent(&checked.structs, code),
            "同一筆欄位寫入語句不可由兩個不同持有者認領");
        assert!(duplicated_write.verified_field_write(
            owner, write, "Holder", "title").is_none(),
            "欄位事件查詢亦不可讓相衝突的證據繞過整體稽核");

        let mut forged = VerifiedDeclarationFacts::from_checker(
            facts.by_source.clone(), facts.direct_moves.clone(),
            facts.named_returns.clone(), facts.field_writes.clone(),
        );
        forged.direct_moves.insert((owner, owner));
        assert!(!forged.evidence_is_consistent(&checked.structs, code),
            "來源與目的不得共用唯一擁有者");

        let mut forged = VerifiedDeclarationFacts::from_checker(
            facts.by_source.clone(), facts.direct_moves.clone(),
            facts.named_returns.clone(), facts.field_writes.clone(),
        );
        forged.direct_moves.insert((target, owner));
        assert!(!forged.evidence_is_consistent(&checked.structs, code),
            "具名直線移交不得將後方宣告偽裝成先前的來源");
        assert!(forged.verified_direct_move(target, owner, "Holder").is_none(),
            "資源橋接的直接證據查詢亦不得返回逆向移交");

        let mut forged = VerifiedDeclarationFacts::from_checker(
            facts.by_source.clone(), facts.direct_moves.clone(),
            facts.named_returns.clone(), facts.field_writes.clone(),
        );
        let invalid_owner = DeclarationId(code.len() + 1);
        forged.by_source.insert(invalid_owner,
            crate::common_ir::TypedDeclaration::new(
                invalid_owner, "Holder".to_owned(), true));
        assert!(!forged.evidence_is_consistent(&checked.structs, code),
            "即使沒有被任何轉移引用，超出 EOF 的宣告也不可簽入計畫");

        let mut forged = VerifiedDeclarationFacts::from_checker(
            facts.by_source.clone(), facts.direct_moves.clone(),
            facts.named_returns.clone(), facts.field_writes.clone(),
        );
        let char_start = code.find('舊').expect("須含 UTF-8 多位元組字元");
        assert!(!code.is_char_boundary(char_start + 1));
        forged.field_writes.insert((owner, char_start + 1, "title".to_owned()));
        assert!(!forged.evidence_is_consistent(&checked.structs, code),
            "欄位寫入不能以中文字元中間的位元組當作已驗證原始位置");

        let mut forged = VerifiedDeclarationFacts::from_checker(
            facts.by_source.clone(), facts.direct_moves.clone(),
            facts.named_returns.clone(), facts.field_writes.clone(),
        );
        forged.by_source.insert(target,
            crate::common_ir::TypedDeclaration::new(target, "str".to_owned(), true));
        assert!(!forged.evidence_is_consistent(&checked.structs, code),
            "型別錯置的移交即使存在 Checker AST 也不能產生有效計畫");

        let mut forged = VerifiedDeclarationFacts::from_checker(
            facts.by_source.clone(), facts.direct_moves.clone(),
            facts.named_returns.clone(), facts.field_writes.clone(),
        );
        forged.field_writes.insert((owner, write, "id".to_owned()));
        assert!(!forged.evidence_is_consistent(&checked.structs, code),
            "const 欄位不得在有效安全計畫中被當成可寫");

        let mut forged = VerifiedDeclarationFacts::from_checker(
            facts.by_source.clone(), facts.direct_moves.clone(),
            facts.named_returns.clone(), facts.field_writes.clone(),
        );
        forged.by_source.remove(&owner);
        assert!(!forged.evidence_is_consistent(&checked.structs, code),
            "任何資源證據都不可引用不存在的來源身分");

        let mut forged = VerifiedDeclarationFacts::from_checker(
            facts.by_source.clone(), facts.direct_moves.clone(),
            facts.named_returns.clone(), facts.field_writes.clone(),
        );
        forged.by_source.insert(target,
            crate::common_ir::TypedDeclaration::new(owner, "Holder".to_owned(), true));
        assert!(!forged.evidence_is_consistent(&checked.structs, code),
            "typed HIR 宣告表鍵與實際宣告位置不同必須拒絕");
        assert!(forged.typed_declaration(target).is_none(),
            "不能把來源身分竄改的宣告交給資源橋接");
        assert!(forged.verified_direct_move(owner, target, "Holder").is_none());

        let actual = facts.typed_declaration(owner).unwrap();
        assert_eq!(actual.declaration, owner);
        assert_eq!(actual.ty, "Holder");
        assert!(actual.mutable);
        assert_eq!(actual,
            crate::common_ir::TypedDeclaration::new(owner, "Holder", true));
    }

    #[test]
    fn f11_已驗證返回位置必須對應真實具名持有者且不得倒序() {
        let code = "fn provide()->str{\nvar value:str = \"資料\"\nreturn value.move()\n}\nfn main()->int{\nreturn 0\n}\n";
        let checked = verified_private_source(code).expect("具名返回通過 Checker");
        let facts = &checked.plan.declarations;
        assert!(facts.evidence_is_consistent(&checked.structs, code));
        let provider = checked.functions.iter().find(|fun| fun.name == "provide").unwrap();
        let owner = DeclarationId(provider.body[0].start);
        let StmtKind::Return(value) = &provider.body[1].kind
            else { panic!("應為已驗證具名返回") };
        assert!(facts.verified_named_return(owner, value.start, "str").is_some());

        let fake_owner = DeclarationId(owner.0 + 1);
        let mut duplicate_return = VerifiedDeclarationFacts::from_checker(
            facts.by_source.clone(), facts.direct_moves.clone(),
            facts.named_returns.clone(), facts.field_writes.clone(),
        );
        duplicate_return.by_source.insert(fake_owner,
            crate::common_ir::TypedDeclaration::new(
                fake_owner, "str".to_owned(), true));
        duplicate_return.named_returns.insert((fake_owner, value.start));
        assert!(!duplicate_return.evidence_is_consistent(&checked.structs, code),
            "同一個 return 來源位置不能同時交接兩名資源持有者");
        assert!(duplicate_return.verified_named_return(
            owner, value.start, "str").is_none(),
            "單筆 return 橋接必須拒絕整體證據表的多重來源");

        let mut forged = VerifiedDeclarationFacts::from_checker(
            facts.by_source.clone(), facts.direct_moves.clone(),
            facts.named_returns.clone(), facts.field_writes.clone(),
        );
        forged.named_returns.insert((owner, owner.0.saturating_sub(1)));
        assert!(!forged.evidence_is_consistent(&checked.structs, code));
        forged.named_returns.remove(&(owner, owner.0.saturating_sub(1)));
        forged.named_returns.insert((DeclarationId(code.len() + 1), value.start));
        assert!(!forged.evidence_is_consistent(&checked.structs, code));
        forged.named_returns.clear();
        forged.named_returns.insert((owner, code.len() + 1));
        assert!(!forged.evidence_is_consistent(&checked.structs, code),
            "合法持有者也不能在原始碼 EOF 之外宣稱已完成返回");

        forged.named_returns.clear();
        let chinese = code.find('資').expect("必須有 UTF-8 字串常值");
        assert!(!code.is_char_boundary(chinese + 1));
        forged.named_returns.insert((owner, chinese + 1));
        assert!(!forged.evidence_is_consistent(&checked.structs, code),
            "返回證據不能指向 UTF-8 字元中間的位元組");
    }

    #[test]
    fn f11_沒有_checker_成功轉移證據即使_ast_寫有_move_也不得授權資源橋接() {
        let code = "fn main()->int{\nvar owner:str = \"資源\"\nvar target:str = owner.move()\nreturn 0\n}\n";
        let source = Source::new(code.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        let (_, parsed) = Parser { source: &source, tokens: &tokens.tokens, cursor: 0 }
            .file().expect("先單獨解析合法的私有語法");
        let main = &parsed[0];
        let origin = DeclarationId(main.body[0].start);
        let target = DeclarationId(main.body[1].start);
        let no_checker_proof = VerifiedDeclarationFacts::from_checker(BTreeMap::new(), BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
        assert!(!no_checker_proof.has_direct_move(origin, target),
            "光有具名 move AST 不能自動獲得資源移交證據");
        assert_eq!(no_checker_proof.verified_direct_move(origin, target, "str"), None);

        let checked = verified_private_source(code).expect("必須通過真正私有 Checker");
        assert_eq!(checked.plan.declarations.verified_direct_move(origin, target, "str"),
            Some(VerifiedMoveEvidence { source: origin, target }));
        assert_eq!(checked.plan.declarations.verified_direct_move(target, origin, "str"), None);
    }

    #[test]
    fn f12_已核准多種_utf8_真實執行期事件必須通過_checker_和雙帳簿責任稽核() {
        use crate::owned_str_runtime::ResourceEvent;
        for literal in ["", "繁體中文", "🐋🪼", "A中B"] {
            let code = format!(
                "fn main()->int{{\nvar owner:str = \"{}\"\nvar target:str = owner.move()\nreturn 0\n}}\n",
                literal,
            );
            let actual = trace_named_string_move_source(&code)
                .expect("實際 OwnedStrRuntime 的合法 UTF-8 單次移交必須驗完安全與責任");
            let [ResourceEvent::Created(created), ResourceEvent::Moved(moved),
                ResourceEvent::Released(released)] = actual.as_slice()
                else { panic!("只允許真正單次建立、整筆移交、一次釋放") };
            assert_eq!(created, moved);
            assert_eq!(moved, released);
        }
    }

    #[test]
    fn 來源起點與真實緩衝區事件都對應唯一資源持有者() {
        let code = "fn main()->int{\nvar owner:str = \"繁體中文 🌊\"\nvar target:str = owner.move()\nreturn 0\n}\n";
        let checked = verified_private_source(code).expect("先經真正來源安全驗證");
        let main = checked.functions.iter().find(|fun| fun.name == "main").unwrap();
        let [first, second, _return] = main.body.as_slice() else {
            panic!("只接受已驗證三個語句的直線結構");
        };
        let (source_name, target_name) = match (&first.kind, &second.kind) {
            (
                StmtKind::Declare { name: source_name, .. },
                StmtKind::Declare { name: target_name, init: Some(Term {
                    kind: TermKind::Move(moved), ..
                }), .. },
            ) if source_name == moved => (source_name, target_name),
            _ => panic!("未取得真正來源中的具名整筆資源移交"),
        };
        assert_eq!(source_name, "owner");
        assert_eq!(target_name, "target");
        assert_ne!(first.start, second.start);
        let source_id = DeclarationId(first.start);
        let target_id = DeclarationId(second.start);
        assert_eq!(checked.plan.declarations.verified_direct_move(source_id, target_id, "str"),
            Some(VerifiedMoveEvidence { source: source_id, target: target_id }),
            "正式資源橋接應取得已驗證的原始轉移身分");
        assert_eq!(checked.plan.declarations.verified_direct_move(source_id, target_id, "int"),
            None, "不得把 str 資源轉移假冒 int 值轉移");
        assert_eq!(checked.plan.declarations.verified_direct_move(target_id, source_id, "str"),
            None, "轉移證據不能顛倒來源與目標");
        assert!(checked.plan.declarations.has_direct_move(source_id, target_id),
            "已驗證來源直接產生真正具名整筆移動證據");
        assert!(!checked.plan.declarations.has_direct_move(target_id, source_id),
            "不得顛倒真實資源移交方向");
        assert!(!checked.plan.declarations.has_direct_move(source_id, DeclarationId(code.len()+1)),
            "不存在的接收宣告不可冒充已驗證移動目標");
        let events = trace_named_string_move_source(code)
            .expect("UTF-8 字串實際生命週期證據");
        let [ResourceEvent::Created(resource), ResourceEvent::Moved(moved),
             ResourceEvent::Released(released)] = events.as_slice() else {
            panic!("只容許一次建立、整筆移動、最終一次釋放");
        };
        assert_eq!(resource, moved, "移動不能重新建立另一份資源");
        assert_eq!(resource, released, "必須釋放唯一已移交持有者");

        let mut pipeline = SafetyPipeline::new();
        pipeline.declare(source_id, CopyPermission::Forbidden).unwrap();
        pipeline.register_resource(source_id, *resource).unwrap();
        for (from, to, ty) in [
            (source_id, target_id, "int"),
            (target_id, source_id, "str"),
            (source_id, DeclarationId(code.len() + 1), "str"),
        ] {
            assert_eq!(checked.plan.declarations.apply_verified_direct_move(
                &mut pipeline, from, to, ty,
            ), Err(VerifiedMoveError::MissingEvidence));
            assert!(pipeline.read(source_id).is_ok(), "沒有 Checker 證據不能搬走所有者");
            assert_eq!(pipeline.outstanding(), 1);
        }
        checked.plan.declarations.apply_verified_direct_move(
            &mut pipeline, source_id, target_id, "str",
        ).expect("只有真正經 Checker 核准的來源及型別可進行抽象移交");
        assert!(pipeline.read(source_id).is_err(), "移動後原來源已失效");
        assert!(pipeline.read(target_id).is_ok());
        assert_eq!(pipeline.outstanding(), 1);
        assert_eq!(pipeline.record_cleanup(target_id).unwrap(), 1);
        assert_eq!(pipeline.outstanding(), 0);
        pipeline.audit_finished().expect("雙模型均無資源漏放");
        assert!(pipeline.record_cleanup(target_id).is_err(),
            "同一持有者不能清理兩次");
    }

    #[test]
    fn f12_驗證證據存在但目標已占用時不容許部分移交() {
        let source_text = "fn main()->int{\nvar from:str = \"數據\"\nvar to:str = from.move()\nreturn 0\n}\n";
        let checked = verified_private_source(source_text).unwrap();
        let main = checked.functions.iter().find(|fun| fun.name == "main").unwrap();
        let source = DeclarationId(main.body[0].start);
        let target = DeclarationId(main.body[1].start);
        let mut pipeline = SafetyPipeline::new();
        pipeline.declare(source, CopyPermission::Forbidden).unwrap();
        pipeline.register_resource(source, 7001).unwrap();
        // Checker 具備真實的移交證據，但目標在安全帳簿已被占用，
        // 雙側交易仍須預檢拒絕，不能先讓來源失效。
        pipeline.declare(target, CopyPermission::Allowed).unwrap();
        assert_eq!(checked.plan.declarations.apply_verified_direct_move(
            &mut pipeline, source, target, "bool",
        ), Err(VerifiedMoveError::MissingEvidence));
        assert_eq!(checked.plan.declarations.apply_verified_direct_move(
            &mut pipeline, source, target, "str",
        ), Err(VerifiedMoveError::Pipeline(
            crate::resource_borrow_pipeline::PipelineError::Ownership(
                crate::borrow_ownership_gate::GateError::DuplicateDestination,
            ),
        )));
        assert_eq!(pipeline.outstanding(), 1);
        assert!(pipeline.read(source).is_ok());
        assert!(pipeline.read(target).is_ok());
        assert_eq!(pipeline.record_cleanup_batch(&[source, target]), Ok(1));
        assert!(pipeline.audit_finished().is_ok());
    }

    #[test]
    fn 已驗證宣告依來源身分區分遮蔽與跨函式所有者() {
        let src = "fn create()->str{\nvar same:str = \"來源\"\nreturn same.move()\n}\nfn main()->int{\nvar same:str = create()\nif(true){\nvar same:int = 1\n}\nreturn 0\n}\n";
        let checked = verified_private_source(src).expect("保留已核准私有來源驗證");
        let create = checked.functions.iter().find(|f| f.name == "create").unwrap();
        let main = checked.functions.iter().find(|f| f.name == "main").unwrap();
        let from = DeclarationId(create.body[0].start);
        let to = DeclarationId(main.body[0].start);
        let StmtKind::If { branch, .. } = &main.body[1].kind
            else { panic!("必須有分支遮蔽") };
        let shadow = DeclarationId(branch.body[0].start);
        assert!(checked.plan.declarations.matches(from, "str", true));
        assert!(checked.plan.declarations.matches(to, "str", true));
        assert!(checked.plan.declarations.matches(shadow, "int", true));
        assert!(!checked.plan.declarations.matches(shadow, "str", true));
        assert!(!checked.plan.declarations.matches(DeclarationId(src.len()+10), "str", true));
        assert_ne!(from, to);
        assert_ne!(to, shadow);
    }

    #[test]
    fn 不合法隱含複製不能在資源模擬器中繞過已驗證門檻() {
        let illegal = "fn main()->int{\nvar owner:str = \"中文\"\nvar target:str = owner\nreturn 0\n}\n";
        let original = inspect(illegal).unwrap_err();
        let private = verified_private_source(illegal).err().unwrap();
        let runtime = trace_named_string_move_source(illegal).unwrap_err();
        assert_eq!(original, private);
        assert_eq!(private, runtime);
        assert_eq!(runtime.kind, DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p16_6_r03_3_函式返回跨來源宣告身分稽核 {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent;
    use crate::verified_resource_ir::NamedResourceReturn;

    #[test]
    fn f11_分支中可達但未確定執行的_return_move_不得簽發直線資源證據() {
        let code = "fn provide(flag:bool)->str{\nvar payload:str = \"繁體中文\"\nif(flag){\nreturn payload.move()\n}\nreturn \"fallback\"\n}\nfn main()->int{\nreturn 0\n}\n";
        let checked = verified_private_source(code)
            .expect("靜態可達分支具名 return move 是合法來源");
        let provider = checked.functions.iter().find(|f| f.name == "provide").unwrap();
        let original = DeclarationId(provider.body[0].start);
        let StmtKind::If { branch, .. } = &provider.body[1].kind
            else { panic!("必須是已驗證 if 分支") };
        let StmtKind::Return(value) = &branch.body[0].kind
            else { panic!("必須是分支內具名 return move") };
        assert_eq!(checked.plan.declarations.verified_named_return(
            original, value.start, "str",
        ), None, "靜態可能執行不能冒充已執行的直線 P16 資源返回");
        assert!(checked.plan.declarations.matches(original, "str", true),
            "不得因禁止資源橋接證據而錯誤否決合法靜態來源");
    }

    #[test]
    fn 真實具名函式返回依來源及接收宣告身分核對同一資源() {
        let src = "fn create()->str{\nvar payload:str = \"繁體中文🪼\"\nreturn payload.move()\n}\nfn main()->int{\nvar received:str = create()\nreturn 0\n}\n";
        let verified = verified_private_source(src).unwrap();
        let provider = verified.functions.iter().find(|f| f.name == "create").unwrap();
        let main = verified.functions.iter().find(|f| f.name == "main").unwrap();
        let source_id = DeclarationId(provider.body[0].start);
        let target_id = DeclarationId(main.body[0].start);
        let StmtKind::Return(return_term) = &provider.body[1].kind
            else { panic!("此橋接只接受真正的具名 return move") };
        assert_eq!(verified.declarations.verified_named_return(
            source_id, return_term.start, "str",
        ), Some(VerifiedReturnEvidence { source: source_id, return_start: return_term.start }));
        assert_eq!(verified.declarations.verified_named_return(
            source_id, return_term.start, "int",
        ), None, "型別不同不得冒充返回移交");
        assert_eq!(verified.declarations.verified_named_return(
            target_id, return_term.start, "str",
        ), None, "不能用呼叫端接收者冒充被呼叫端返回來源");
        let no_proof = VerifiedDeclarationFacts::from_checker(
            verified.declarations.by_source.clone(), BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
        assert!(no_proof.verified_named_return(source_id, return_term.start, "str").is_none(),
            "只有 AST 與宣告型別、卻沒有 Checker 回傳證據不可建立資源橋接");
        assert_ne!(source_id, target_id);
        assert_eq!(&src[source_id.0..source_id.0 + 3], "var");
        assert_eq!(&src[target_id.0..target_id.0 + 3], "var");

        let (value, events) = trace_string_return_source(src, "create", "received").unwrap();
        assert_eq!(value, "繁體中文🪼");
        assert_eq!(events, vec![
            ResourceEvent::Created(1),
            ResourceEvent::Returned(1),
            ResourceEvent::Released(1),
        ]);
        let proof = NamedResourceReturn::new(source_id, target_id).unwrap();
        assert_eq!(proof.verify_actual_events(&events), Ok(1));
    }

    #[test]
    fn 不合法隱含複製在函式返回資源橋接之前就被靜態拒絕() {
        let illegal = "fn create()->str{\nvar payload:str = \"來源\"\nreturn payload\n}\nfn main()->int{\nvar received:str = create()\nreturn 0\n}\n";
        let static_error = inspect(illegal).unwrap_err();
        let trace_error = trace_string_return_source(illegal, "create", "received").unwrap_err();
        assert_eq!(static_error, trace_error, "IR 不可取代或跳過真正靜態安全預檢");
    }
}

#[cfg(test)]
mod p16_6_r03_4_結構整筆移交與失敗回滾資源責任 {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent;
    use crate::verified_resource_ir::WholeStructResourcePlan;

    const SOURCE: &str =
        "struct Message{\nvar title:str = \"預設\"\nconst id:str = \"固定🪼\"\n}\nfn main()->int{\nvar original:Message = Message(title=\"繁體中文\")\nvar transferred:Message = original.move()\nreturn 0\n}\n";

    #[test]
    fn 真實來源多個欄位轉交的宣告身分與雙資源帳簿相符() {
        let verified = verified_private_source(SOURCE).expect("來源先完成安全檢查");
        let main = verified.functions.iter().find(|fun| fun.name == "main").unwrap();
        let [source, target, _] = main.body.as_slice() else { panic!("只接受三語句"); };
        let source_id = DeclarationId(source.start);
        let target_id = DeclarationId(target.start);
        assert!(verified.declarations.verified_direct_move(source_id, target_id, "Message").is_some(),
            "整筆結構移交必須有 Checker 真正驗證並交付的宣告身分與型別");
        assert!(verified.declarations.verified_direct_move(source_id, target_id, "str").is_none(),
            "不得把結構整筆移交偽裝成單一 str 欄位移動");
        assert!(verified.declarations.verified_direct_move(target_id, source_id, "Message").is_none(),
            "不得顛倒所有權移交方向");
        assert!(VerifiedDeclarationFacts::from_checker(
            BTreeMap::new(), BTreeSet::new(), BTreeSet::new(), BTreeSet::new(),
        ).verified_direct_move(source_id, target_id, "Message").is_none(),
            "沒有 Checker 完整證據時 AST 看似合法也不能驅動資源計畫");
        let plan = WholeStructResourcePlan::new(source_id, target_id).unwrap();
        let (retained, events) = trace_persistent_struct_move_source(SOURCE, None).unwrap();
        assert_eq!(retained.as_deref(), Some("固定🪼"));
        assert_eq!(events, vec![
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Moved(1), ResourceEvent::Moved(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
        assert_eq!(plan.verify_moved_events(&events), Ok(2));
    }

    #[test]
    fn 兩種已核准失敗切點只能逆序釋放已建立資源() {
        let verified = verified_private_source(SOURCE).unwrap();
        let main = verified.functions.iter().find(|fun| fun.name == "main").unwrap();
        let [source, target, _] = main.body.as_slice() else { panic!("只接受三語句"); };
        let plan = WholeStructResourcePlan::new(
            DeclarationId(source.start), DeclarationId(target.start)).unwrap();
        for (field, count) in [("title", 1), ("id", 2)] {
            let (value, events) = trace_persistent_struct_move_source(SOURCE, Some(field)).unwrap();
            assert_eq!(value, None);
            assert_eq!(plan.verify_failed_build_events(&events), Ok(count));
            assert!(events.iter().all(|e| !matches!(
                e, ResourceEvent::Moved(_) | ResourceEvent::Returned(_)
            )));
        }
    }

    #[test]
    fn 含有非_copyable_欄位的結構不能偷換成隱含複製後交給資源_ir() {
        let illegal = SOURCE.replace("original.move()", "original");
        let static_error = inspect(&illegal).unwrap_err();
        let trace_error = trace_persistent_struct_move_source(&illegal, None).unwrap_err();
        assert_eq!(static_error, trace_error);
    }
}

#[cfg(test)]
mod p16_6_r03_5_跨函式結構返回多資源責任 {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent;
    use crate::verified_resource_ir::WholeStructResourcePlan;

    const SOURCE: &str =
        "struct Bundle{\nvar title:str = \"預設\"\nconst id:str = \"固定🪼\"\n}\nfn provide()->Bundle{\nvar local:Bundle = Bundle(title=\"繁體中文\")\nreturn local.move()\n}\nfn main()->int{\nvar received:Bundle = provide()\nreturn 0\n}\n";

    fn plan() -> WholeStructResourcePlan {
        let verified = verified_private_source(SOURCE).expect("先驗證真正 voil 靜態安全");
        let provider = verified.functions.iter().find(|f| f.name == "provide").unwrap();
        let caller = verified.functions.iter().find(|f| f.name == "main").unwrap();
        assert_ne!(provider.body[0].start, caller.body[0].start);
        let source = DeclarationId(provider.body[0].start);
        let StmtKind::Return(return_term) = &provider.body[1].kind
            else { panic!("須有完整 Checker 已驗證整筆 return") };
        assert_eq!(verified.declarations.verified_named_return(
            source, return_term.start, "Bundle"),
            Some(VerifiedReturnEvidence { source, return_start: return_term.start }));
        assert!(verified.declarations.verified_named_return(
            source, return_term.start, "str").is_none(),
            "不能把整筆 Bundle 轉移冒充單一 str 資源");
        WholeStructResourcePlan::new(
            DeclarationId(provider.body[0].start),
            DeclarationId(caller.body[0].start)).unwrap()
    }

    #[test]
    fn 真正多資源函式返回只由呼叫端逆序清理() {
        let proof = plan();
        let (value, events) = trace_persistent_struct_return_source(
            SOURCE, "provide", "received", None).unwrap();
        assert_eq!(value.as_deref(), Some("固定🪼"));
        assert_eq!(events, vec![
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Returned(1), ResourceEvent::Returned(2),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
        assert_eq!(proof.verify_returned_events(&events), Ok(2));
        assert!(proof.verify_moved_events(&events).is_err());
    }

    #[test]
    fn 真正返回前欄位求值失敗不得產生任何返回事件() {
        let proof = plan();
        for (field, count) in [("title", 1), ("id", 2)] {
            let (value, events) = trace_persistent_struct_return_source(
                SOURCE, "provide", "received", Some(field)).unwrap();
            assert!(value.is_none());
            assert_eq!(proof.verify_failed_build_events(&events), Ok(count));
            assert!(events.iter().all(|event| !matches!(
                event, ResourceEvent::Moved(_) | ResourceEvent::Returned(_)
            )));
        }
    }

    #[test]
    fn 禁止在跨函式返回資源橋接前隱含複製結構() {
        let illegal = SOURCE.replace("return local.move()", "return local");
        let static_error = inspect(&illegal).unwrap_err();
        let trace_error = trace_persistent_struct_return_source(
            &illegal, "provide", "received", None).unwrap_err();
        assert_eq!(static_error, trace_error);
    }
}

#[cfg(test)]
mod p16_6_r03_8_真正欄位覆寫與失敗回滾對帳 {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent;
    use crate::verified_resource_ir::FieldOverwritePlan;

    const SOURCE: &str =
        "struct Holder{\nvar title:str = \"舊值\"\nconst id:str = \"固定🪼\"\n}\nfn main()->int{\nvar owner:Holder = Holder()\nowner.title = \"新值🪼\"\nreturn 0\n}\n";

    #[test]
    fn 真正_voil_覆寫後原值清理及失敗暫存回滾均核對同一宣告() {
        let verified = verified_private_source(SOURCE).unwrap();
        let main = verified.functions.iter().find(|f|f.name == "main").unwrap();
        let [declare, write, _] = main.body.as_slice() else { panic!("只接受受限三語句"); };
        assert!(matches!(write.kind, StmtKind::FieldAssign { .. }));
        let owner = DeclarationId(declare.start);
        let field_name = match &write.kind {
            StmtKind::FieldAssign { field, .. } => field.as_str(),
            _ => unreachable!("測試上方已核對欄位賦值"),
        };
        assert_eq!(verified.declarations.verified_field_write(
            owner, write.start, "Holder", field_name,
        ), Some(VerifiedFieldWriteEvidence {
            owner, statement_start: write.start, field: field_name.to_owned(),
        }));
        for (source, at, ty, member) in [
            (owner, write.start, "str", field_name),
            (owner, write.start, "Holder", "id"),
            (owner, write.start + 1, "Holder", field_name),
            (DeclarationId(write.start), write.start, "Holder", field_name),
        ] {
            assert!(verified.declarations.verified_field_write(
                source, at, ty, member,
            ).is_none(), "錯誤身分、型別、欄位或來源位置不得取得資源寫入證據");
        }
        let no_proof = VerifiedDeclarationFacts::from_checker(
            verified.declarations.by_source.clone(), BTreeSet::new(),
            BTreeSet::new(), BTreeSet::new(),
        );
        assert!(no_proof.verified_field_write(owner, write.start, "Holder", field_name)
            .is_none(), "AST 形狀及型別表不能冒充 Checker 真正提交證據");
        let plan = FieldOverwritePlan::new(owner, 2, Some(0)).unwrap();
        let (updated, success) = trace_persistent_struct_field_write_source(SOURCE, false).unwrap();
        assert_eq!(updated, "新值🪼");
        assert_eq!(success, vec![
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Created(3), ResourceEvent::Released(1),
            ResourceEvent::Released(3), ResourceEvent::Released(2),
        ]);
        assert_eq!(plan.verify_actual_events(&success,false), Ok(3));
        let (preserved, failed) = trace_persistent_struct_field_write_source(SOURCE, true).unwrap();
        assert_eq!(preserved, "舊值");
        assert_eq!(failed, vec![
            ResourceEvent::Created(1), ResourceEvent::Created(2),
            ResourceEvent::Created(3), ResourceEvent::Released(3),
            ResourceEvent::Released(2), ResourceEvent::Released(1),
        ]);
        assert_eq!(plan.verify_actual_events(&failed,true), Ok(3));
        assert!(plan.verify_actual_events(&success,true).is_err());
        assert!(plan.verify_actual_events(&failed,false).is_err());
    }

    #[test]
    fn 未初始化的_var_str_第一次寫入不可清理不存在的舊欄位() {
        let source = SOURCE.replace("var title:str = \"舊值\"", "var title:str");
        let checked = verified_private_source(&source).unwrap();
        let main = checked.functions.iter().find(|f|f.name == "main").unwrap();
        let plan=FieldOverwritePlan::new(DeclarationId(main.body[0].start),1,None).unwrap();
        let (value,events)=trace_persistent_struct_field_write_source(&source,false).unwrap();
        assert_eq!(value,"新值🪼");
        assert_eq!(plan.verify_actual_events(&events,false),Ok(2));
    }

    #[test]
    fn 靜態禁止_const_欄位覆寫不能繞過安全_ir() {
        let invalid=SOURCE.replace("owner.title = \"新值🪼\"",
            "owner.id = \"不合法\"");
        let original=inspect(&invalid).unwrap_err();
        let runtime=trace_persistent_struct_field_write_source(&invalid,false).unwrap_err();
        assert_eq!(original,runtime);
    }
}

#[cfg(test)]
mod p16_6_r03_9_混合純值與資源責任帳簿實際來源 {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent;
    use crate::verified_resource_ir::MixedResourcePlan;

    const MIXED: &str =
        "struct Mixed{\nvar num:int = 3\nvar text:str = \"舊值\"\nconst flag:bool = true\n}\nfn main()->int{\nvar original:Mixed = Mixed(text=\"繁體中文🪼\",num=7)\nvar target:Mixed = original.move()\nreturn 0\n}\n";
    const PURE: &str =
        "struct Plain{\nvar num:int = 3\nconst flag:bool = true\n}\nfn main()->int{\nvar original:Plain = Plain(num=9)\nvar target:Plain = original\nreturn 0\n}\n";

    fn proven(src: &str, str_count: usize) -> MixedResourcePlan {
        let checked=verified_private_source(src).expect("真實來源須先經私有安全檢查");
        let main=checked.functions.iter().find(|f|f.name=="main").unwrap();
        let [declare,assign,_]=main.body.as_slice() else { panic!("限制三語句"); };
        let origin = DeclarationId(declare.start);
        let destination = DeclarationId(assign.start);
        let kind = if str_count == 0 { "Plain" } else { "Mixed" };
        assert!(checked.plan.declarations.matches(origin, kind, true));
        assert!(checked.plan.declarations.matches(destination, kind, true));
        if str_count > 0 {
            assert_eq!(checked.plan.declarations.verified_direct_move(
                origin, destination, kind,
            ), Some(VerifiedMoveEvidence { source: origin, target: destination }));
            assert!(checked.plan.declarations.verified_direct_move(
                origin, destination, "str").is_none(),
                "資源 str 欄位不得冒充 Mixed 的整筆移動型別");
            assert!(checked.plan.declarations.verified_direct_move(
                destination, origin, kind).is_none());
        } else {
            assert!(checked.plan.declarations.verified_direct_move(origin,destination,kind).is_none(),
                "Copyable 直接讀取不允許製造非 Copyable 整筆移動證據");
        }
        MixedResourcePlan::new(origin,destination,str_count).unwrap()
    }

    #[test]
    fn 真正混合資源整筆移動只登記_str_並精確清理() {
        let proof=proven(MIXED,1);
        let (value,events)=trace_mixed_struct_source(MIXED,None).unwrap();
        assert_eq!(value,Some(7));
        assert_eq!(events,vec![
            ResourceEvent::Created(1),ResourceEvent::Moved(1),ResourceEvent::Released(1),
        ]);
        assert_eq!(proof.verify_completed(&events,true),Ok(1));
        assert!(proof.verify_completed(&events,false).is_err());
    }

    #[test]
    fn 真正全_copyable_結構複製沒有_任何_資源事件() {
        let proof=proven(PURE,0);
        let (value,events)=trace_mixed_struct_source(PURE,None).unwrap();
        assert_eq!(value,Some(9));
        assert!(events.is_empty());
        assert_eq!(proof.verify_completed(&events,false),Ok(0));
    }

    #[test]
    fn 求值至_int_或_bool_才失敗只回滾先前真的建立之_str() {
        let proof=proven(MIXED,1);
        for failure in ["num","flag"] {
            let (value,events)=trace_mixed_struct_source(MIXED,Some(failure)).unwrap();
            assert!(value.is_none());
            assert_eq!(events,vec![
                ResourceEvent::Created(1),ResourceEvent::Released(1),
            ]);
            assert_eq!(proof.verify_failed_build(&events,1),Ok(1));
        }
    }

    #[test]
    fn 混合結構不能在進入真實事件模擬前隱含複製_str() {
        let illegal=MIXED.replace("original.move()","original");
        let static_error=inspect(&illegal).unwrap_err();
        let runtime_error=trace_mixed_struct_source(&illegal,None).unwrap_err();
        assert_eq!(static_error,runtime_error);
    }
}

#[cfg(test)]
mod p16_6_r03_10_單層布林分支與作用域資源清理 {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent as E;
    use crate::verified_resource_ir::ScopedResourcePlan;

    const SRC: &str =
        "struct Holder{\nvar text:str = \"預設\"\n}\nfn main()->int{\nvar outer:Holder = Holder(text=\"外層\")\nif(true){\nvar inner:Holder = Holder(text=\"內層\")\n}\nreturn 0\n}\n";

    fn proof(src:&str, with_inner:bool)->ScopedResourcePlan {
        let verified=verified_private_source(src).unwrap();
        let main=verified.functions.iter().find(|f|f.name=="main").unwrap();
        let [outside,branch,_]=main.body.as_slice() else {panic!("須為三語句");};
        let outer = DeclarationId(outside.start);
        assert!(verified.declarations.matches(outer,"Holder",true),
            "外層實際持有者必須由 Checker 驗證型別和權限");
        assert!(!verified.declarations.matches(outer,"str",true));
        let selected=if with_inner {
            let StmtKind::If {branch:arm,..}=&branch.kind else {panic!("須為 if");};
            let inner = DeclarationId(arm.body[0].start);
            assert_ne!(outer,inner,"內外作用域不可沿用同一宣告身分");
            assert!(verified.declarations.matches(inner,"Holder",true));
            assert!(!verified.declarations.matches(inner,"Holder",false));
            Some((inner,1))
        } else {None};
        ScopedResourcePlan::new(DeclarationId(outside.start),1,selected).unwrap()
    }

    #[test]
    fn f04_f12_完整_checker_成功後才將_已驗證_bool_hir_交予真正作用域資源橋接() {
        let verified = verified_private_source(SRC).unwrap();
        let main = verified.functions.iter().find(|fun| fun.name == "main").unwrap();
        let StmtKind::If { branch, .. } = &main.body[1].kind
            else { panic!("原始來源必須是單一已驗證 if") };
        let actual = verified.scalar_hir.expression(branch.condition.node_span())
            .expect("真實 Checker 全部成功後必須交付 if 的 Typed HIR");
        assert_eq!(actual.ty, crate::common_ir::ScalarType::Bool);
        assert_eq!(actual.span, branch.condition.node_span());
        assert!(verified.scalar_hir.expression(
            crate::common_ir::NodeSpan::new(actual.span.start,
                actual.span.end + 1)).is_none(),
            "不允許相同起點、不同終點的偽造條件證據");

        let StmtKind::Return(return_term) = &main.body[2].kind
            else { panic!("必須有最後 return 0") };
        assert_eq!(verified.scalar_hir.expression(return_term.node_span())
            .expect("合法 return int 應有成功求值證據").ty,
            crate::common_ir::ScalarType::Int);
        for source in [SRC.to_owned(), SRC.replace("if(true)", "if(false)")] {
            assert!(trace_scope_struct_source(&source, None).is_ok(),
                "真實作用域橋接應直接消費 Checker 成功交付的 bool HIR");
        }
        let invalid = SRC.replace("if(true)", "if(1)");
        assert!(verified_private_source(&invalid).is_err(),
            "Checker 沒有全部成功時不得產生可交付 P16 橋接的 HIR");
        assert!(trace_scope_struct_source(&invalid, None).is_err());
    }

    #[test]
    fn 真正_if_true_內層釋放先於外層_資源事件和宣告身分一致() {
        let events=trace_scope_struct_source(SRC,None).unwrap();
        assert_eq!(events,vec![
            E::Created(1),E::Created(2),E::Released(2),E::Released(1),
        ]);
        assert_eq!(proof(SRC,true).verify_actual_events(&events),Ok(2));
    }

    #[test]
    fn 真正_if_false_沒有_else_只由外層負責清理() {
        let src=SRC.replace("if(true)","if(false)");
        let events=trace_scope_struct_source(&src,None).unwrap();
        assert_eq!(events,vec![E::Created(1),E::Released(1)]);
        assert_eq!(proof(&src,false).verify_actual_events(&events),Ok(1));
        assert!(proof(&src,true).verify_actual_events(&events).is_err());
    }

    #[test]
    fn 真正分支內提前_return_及欄位建立失敗依然先清理內層() {
        let early=SRC.replace(
            "var inner:Holder = Holder(text=\"內層\")\n}",
            "var inner:Holder = Holder(text=\"內層\")\nreturn 0\n}");
        for (source,failure) in [(SRC,None),(SRC,Some("text")),(early.as_str(),None)] {
            let events=trace_scope_struct_source(source,failure).unwrap();
            assert_eq!(proof(source,true).verify_actual_events(&events),Ok(2));
        }
    }

    #[test]
    fn 未知條件不能假裝已執行內層資源路徑() {
        let illegal=SRC.replace("if(true)","if(1==1)");
        assert!(trace_scope_struct_source(&illegal,None).is_err());
    }
}
