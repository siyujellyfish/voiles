#[cfg(test)]
mod tests {
    use super::*;
    use crate::ownership_model::CopyPermission::{Allowed, Forbidden, Undecided};

    #[test]
    fn f10_同種與混合的迴圈跳轉必須保留_break_continue_各自可達類別() {
        fn branch_transfer(left: StmtKind, right: StmtKind) -> FlowState {
            let source = Source::new(b"true".to_vec());
            let structs = HashMap::new();
            let functions = Vec::new();
            let checker = Checker {
                source: &source, structs: &structs, functions: &functions,
                function_index: HashMap::new(),
                ability: crate::ownership_policy::approved_leaf_copy_permission,
            recorded_function_exits: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_edges: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_method_safety: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_method_safety_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_direct_moves: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declarations: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_types: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declaration_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_named_returns: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_field_writes: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_call_borrows: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_call_borrow_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_scalar_hir: std::cell::RefCell::new(
                crate::common_ir::VerifiedScalarHir::default())
            };
            let cond = Term { kind: TermKind::Literal("bool"), start: 0, end: 4 };
            let mk = |kind| Stmt { kind, start: 0, end: 4 };
            let conditional = mk(StmtKind::If {
                branch: crate::syntax_shared::ParsedIfArm { start: 0, branch: crate::common_ir::ConditionalBranch::new(
                    cond, crate::syntax_shared::ParsedBlock {
                        statements: vec![mk(left)], start: 0, end: 4,
                    },
                ) },
                no: Some(vec![mk(right)].into()),
            });
            let mut vars = vec![Rc::new(HashMap::new())];
            let entry = vars.clone();
            checker.statement_body(
                &[conditional], &mut vars, "int", &[], Some(&entry),
            ).expect("兩條跳轉在迴圈內不應自行改變外層來源")
        }

        assert_eq!(branch_transfer(StmtKind::Break, StmtKind::Break),
            FlowState::Break);
        assert_eq!(branch_transfer(StmtKind::Continue, StmtKind::Continue),
            FlowState::Continue);
        assert_eq!(branch_transfer(StmtKind::Break, StmtKind::Continue),
            FlowState::LoopTransfer,
            "來源分支有不同跳轉類型時仍須保守，不能假定全是反向邊");
    }

    #[test]
    fn f11_同一來源路徑宣告重訪只能完全相同_衝突不得被覆寫() {
        let source = Source::new(b"fn main()->int{return 0}".to_vec());
        let structs = HashMap::new();
        let functions = Vec::new();
        let checker = Checker {
            source: &source, structs: &structs, functions: &functions,
            function_index: HashMap::new(),
            ability: crate::ownership_policy::approved_leaf_copy_permission,
            recorded_function_exits: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_edges: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_method_safety: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_method_safety_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_direct_moves: std::cell::RefCell::new(BTreeSet::new()),
            recorded_declarations: std::cell::RefCell::new(BTreeMap::new()),
            recorded_member_types: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declaration_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_named_returns: std::cell::RefCell::new(BTreeSet::new()),
            recorded_field_writes: std::cell::RefCell::new(BTreeSet::new()),
            recorded_call_borrows: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_call_borrow_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_scalar_hir: std::cell::RefCell::new(
                crate::common_ir::VerifiedScalarHir::default()),
        };
        let owner = DeclarationId(3);
        checker.record_declaration(owner, "int", true);
        checker.record_declaration(owner, "int", true);
        assert!(checker.recorded_declaration_conflicts.borrow().is_empty(),
            "合法路徑重訪相同已解析宣告必須冪等");
        checker.record_declaration(owner, "bool", true);
        checker.record_declaration(owner, "int", false);
        assert!(checker.recorded_declaration_conflicts.borrow().contains(&owner),
            "型別／var-const 權限不同不得以最後寫入覆蓋已驗證的來源");
        let original = checker.recorded_declarations.borrow()[&owner].clone();
        assert_eq!(original.ty, "int");
        assert!(original.mutable, "首次 Checker 宣告證據必須保留而不可被覆蓋");
        checker.record_declaration(owner, "int", true);
        assert!(checker.recorded_declaration_conflicts.borrow().contains(&owner),
            "後續再遇相同證據不能消除曾經存在的路徑矛盾");
    }

    #[test]
    fn f08_借用別名的讀取與欄位接收者須由同一_declid_與借用權限授權() {
        let source = Source::new(b"source alias".to_vec());
        let structs = HashMap::new();
        let functions = Vec::new();
        let checker = Checker {
            source: &source, structs: &structs, functions: &functions,
            function_index: HashMap::new(),
            ability: crate::ownership_policy::approved_leaf_copy_permission,
            recorded_function_exits: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_edges: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_method_safety: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_method_safety_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_direct_moves: std::cell::RefCell::new(BTreeSet::new()),
            recorded_declarations: std::cell::RefCell::new(BTreeMap::new()),
            recorded_member_types: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declaration_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_named_returns: std::cell::RefCell::new(BTreeSet::new()),
            recorded_field_writes: std::cell::RefCell::new(BTreeSet::new()),
            recorded_call_borrows: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_call_borrow_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_scalar_hir: std::cell::RefCell::new(
                crate::common_ir::VerifiedScalarHir::default()),
        };
        let original = DeclarationId(0);
        let borrowed = DeclarationId(7);
        let fact = BorrowFact::new(original, BorrowKind::SharedRead);
        let owner = Binding::new(
        original,
        "int".into(),
        false,
        ValueSlot::initialized(Allowed),
        None,
        None,
    );
        let alias = Binding::new(
        borrowed,
        "int".into(),
        false,
        ValueSlot::initialized(Allowed),
        None,
        Some(LoanBinding { fact, active: true }),
    );
        let mut vars = vec![Rc::new(HashMap::from([
            ("source".into(), owner), ("alias".into(), alias),
        ]))];
        let term = Term {
            kind: TermKind::Name("alias".into()), start: 7, end: 12,
        };
        checker.record_declaration(original, "int", false);
        checker.record_declaration(borrowed, "int", false);
        assert!(checker.check_direct(&vars, "alias", &term).is_ok(),
            "合法的共享借用別名仍可在其有效期內使用");
        assert_eq!(checker.evaluate(&term, &mut vars), Ok(("int".into(), true)));
        assert!(checker.check_direct(&vars, "source", &term).is_err(),
            "有效別名仍應禁止直接操作借用中的原來源");

        checker.recorded_declarations.borrow_mut().insert(
            borrowed, crate::common_ir::TypedDeclaration::new(
                borrowed, "bool".into(), false,
            ));
        assert!(checker.check_direct(&vars, "alias", &term).is_err(),
            "借用別名不得繞過 Checker 型別權威");
        checker.recorded_declarations.borrow_mut().insert(
            borrowed, crate::common_ir::TypedDeclaration::new(
                borrowed, "int".into(), false,
            ));
        for bad in [
            BorrowFact::new(DeclarationId(200), BorrowKind::SharedRead),
            BorrowFact::new(original, BorrowKind::ExclusiveWrite),
        ] {
            Rc::make_mut(&mut vars[0]).get_mut("alias").unwrap()
                .loan.as_mut().unwrap().fact = bad;
            assert!(checker.check_direct(&vars, "alias", &term).is_err(),
                "虛構來源與冒充獨占票據不能跳過具型別借用來源核對");
        }
        Rc::make_mut(&mut vars[0]).get_mut("alias").unwrap()
            .loan.as_mut().unwrap().fact = fact;
        Rc::make_mut(&mut vars[0]).get_mut("source").unwrap().ty = "bool".into();
        assert!(checker.check_direct(&vars, "alias", &term).is_err(),
            "即使 alias 本身正確，借用來源被置換型別也不得放行");
        Rc::make_mut(&mut vars[0]).get_mut("source").unwrap().ty = "int".into();

        checker.recorded_declarations.borrow_mut().insert(
            original, crate::common_ir::TypedDeclaration::new(
                original, "bool".into(), false,
            ));
        assert!(checker.check_direct(&vars, "alias", &term).is_err(),
            "來源 Binding 表面型別正確也不得繞過真正 Checker 的來源型別 HIR");
        checker.recorded_declarations.borrow_mut().insert(
            original, crate::common_ir::TypedDeclaration::new(
                original, "int".into(), false,
            ));
        assert!(checker.check_direct(&vars, "alias", &term).is_ok());

        // 對 alias 的局部證據完全合法，但另一條活躍別名偽造了
        // 與共享借用衝突的獨占票據，不能跳過整份快照一致性閘門。
        Rc::make_mut(&mut vars[0]).insert("forged".into(), Binding::new(
        DeclarationId(100),
        "int".into(),
        true,
        ValueSlot::initialized(Allowed),
        None,
        Some(LoanBinding {
                fact: BorrowFact::new(original, BorrowKind::ExclusiveWrite),
                active: true,
            }),
    ));
        assert!(!Checker::<fn(&str) -> CopyPermission>::active_loan_state_is_consistent(&vars));
        assert!(checker.check_direct(&vars, "alias", &term).is_err(),
            "局部來源有效時也不能忽略其它活躍借用的全域衝突");
        Rc::make_mut(&mut vars[0]).remove("forged");
        assert!(checker.check_direct(&vars, "alias", &term).is_ok(),
            "移除矛盾快照後原有合法共享借用仍應繼續有效");
    }

    #[test]
    fn f04_f08_私有實際讀取借用及移動必須消費同一_checker_具型別宣告權威() {
        let source = Source::new(b"var value:int = 1\nvalue\n".to_vec());
        let structs = HashMap::new();
        let functions = Vec::new();
        let checker = Checker {
            source: &source, structs: &structs, functions: &functions,
            function_index: HashMap::new(),
            ability: crate::ownership_policy::approved_leaf_copy_permission,
            recorded_function_exits: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_edges: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_method_safety: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_method_safety_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_direct_moves: std::cell::RefCell::new(BTreeSet::new()),
            recorded_declarations: std::cell::RefCell::new(BTreeMap::new()),
            recorded_member_types: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declaration_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_named_returns: std::cell::RefCell::new(BTreeSet::new()),
            recorded_field_writes: std::cell::RefCell::new(BTreeSet::new()),
            recorded_call_borrows: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_call_borrow_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_scalar_hir: std::cell::RefCell::new(
                crate::common_ir::VerifiedScalarHir::default()),
        };
        let owner = DeclarationId(0);
        let term = Term { kind: TermKind::Name("value".into()), start: 18, end: 23 };
        let mut vars = vec![Rc::new(HashMap::from([("value".into(), Binding::new(
        owner,
        "int".into(),
        true,
        ValueSlot::initialized(Allowed),
        None,
        None,
    ))]))];
        assert!(checker.check_direct(&vars, "value", &term).is_err(),
            "不可以只有一份自行組裝的 Binding 就授權直接存取");
        checker.record_declaration(owner, "int", true);
        assert_eq!(checker.evaluate(&term, &mut vars), Ok(("int".into(), true)));
        assert_eq!(checker.authorize_borrow(&vars, "value", true, &term)
            .map(|(ty, _)| ty), Ok("int".into()));

        for changed in [
            crate::common_ir::TypedDeclaration::new(owner, "bool".into(), true),
            crate::common_ir::TypedDeclaration::new(owner, "int".into(), false),
            crate::common_ir::TypedDeclaration::new(DeclarationId(6), "int".into(), true),
        ] {
            checker.recorded_declarations.borrow_mut().insert(owner, changed);
            assert!(checker.check_direct(&vars, "value", &term).is_err(),
                "型別、const/var 權限或身分竄改時不可讀取或直接移動");
            assert!(checker.authorize_borrow(&vars, "value", true, &term).is_err(),
                "相同宣告權威不合法時不可取得任何借用票據");
        }
        checker.recorded_declarations.borrow_mut().insert(owner,
            crate::common_ir::TypedDeclaration::new(owner, "int".into(), true));
        let moved = Term { kind: TermKind::Move("value".into()), start: 18, end: 23 };
        assert_eq!(checker.evaluate(&moved, &mut vars), Ok(("int".into(), true)),
            "只有已驗證的同一個來源身分與型別才能整筆 move");
    }

    #[test]
    fn f04_私有_checker_真正評估純值運算式時使用共通_typed_hir() {
        let source = Source::new(b"1+2".to_vec());
        let structs = HashMap::new();
        let functions = Vec::new();
        let checker = Checker {
            source: &source, structs: &structs, functions: &functions,
            function_index: HashMap::new(),
            ability: crate::ownership_policy::approved_leaf_copy_permission,
            recorded_function_exits: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_edges: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_method_safety: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_method_safety_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_direct_moves: std::cell::RefCell::new(BTreeSet::new()),
            recorded_declarations: std::cell::RefCell::new(BTreeMap::new()),
            recorded_member_types: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declaration_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_named_returns: std::cell::RefCell::new(BTreeSet::new()),
            recorded_field_writes: std::cell::RefCell::new(BTreeSet::new()),
            recorded_call_borrows: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_call_borrow_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_scalar_hir: std::cell::RefCell::new(
                crate::common_ir::VerifiedScalarHir::default()),
        };
        let left = Term { kind: TermKind::Literal("int"), start: 0, end: 1 };
        let right = Term { kind: TermKind::Literal("int"), start: 2, end: 3 };
        let expression = Term {
            kind: TermKind::BinaryInt(crate::common_ir::ArithmeticNode::new(
                left, '+', right)),
            start: 0, end: 3,
        };
        assert_eq!(checker.evaluate(&expression, &mut vec![Rc::new(HashMap::new())]),
            Ok(("int".into(), true)));
        let hir = checker.recorded_scalar_hir.borrow();
        for (start, end) in [(0, 1), (2, 3), (0, 3)] {
            let fact = hir.expression(crate::common_ir::NodeSpan::new(start, end))
                .expect("私有真正求值後應產生各層已驗證 int 運算式");
            assert_eq!(fact.ty, crate::common_ir::ScalarType::Int);
        }
        assert!(hir.expression(crate::common_ir::NodeSpan::new(0, 2)).is_none(),
            "不能憑相同起點冒充已驗證來源半開跨度");
        assert!(hir.expression(crate::common_ir::NodeSpan::new(1, 3)).is_none());
    }

    #[test]
    fn 公私正式_checker_具型別引用契約保留實際來源且不放寬私有型別() {
        let private = Binding::new(
        DeclarationId(420),
        "str".into(),
        true,
        ValueSlot::initialized(Forbidden),
        None,
        None,
    );
        let span = crate::common_ir::NodeSpan::new(420, 429);
        let declared = private.semantic_declaration();
        assert_eq!(declared.declaration, DeclarationId(420));
        assert_eq!(declared.ty, "str");
        assert!(declared.mutable);
        assert!(declared.matches_use(private.semantic_use_at(span).resolved));
        assert!(!declared.matches_use(crate::common_ir::ResolvedUse::new(
            DeclarationId(421), "str")),
            "相同型別不同原始宣告不等同真實持有者");
        let typed = private.semantic_use_at(span);
        assert_eq!(typed.span, span);
        assert_eq!(typed.resolved.declaration, DeclarationId(420));
        assert_eq!(typed.resolved.ty, "str");
        let scalar = crate::common_ir::ResolvedUse::new(
            DeclarationId(421), crate::common_ir::ScalarType::Bool);
        assert_eq!(scalar.declaration, DeclarationId(421));
        assert_eq!(scalar.ty, crate::common_ir::ScalarType::Bool);
        assert_eq!(crate::common_ir::ScalarType::from_name("str"), None,
            "共同 HIR 型別外殼不可把 str 解鎖成公開 scalar");
    }

    const BASE: &str = "struct Sample{\nconst flag:bool = true\nvar number:int = 1\n}\nfn main()->int{\nvar a:Sample = Sample()\nvar b:Sample = a.move()\nreturn 0\n}\n";

    #[test]
    fn 正式_checker_活躍借用快照以宣告身分保守拒絕跨分支衝突() {
        fn binding(source: usize, declaration: usize, kind: BorrowKind) -> Binding {
            Binding::new(
        DeclarationId(declaration),
        "int".into(),
        kind == BorrowKind::ExclusiveWrite,
        ValueSlot::initialized(Allowed),
        None,
        Some(LoanBinding {
                    fact: BorrowFact::new(DeclarationId(source), kind), active: true,
                }),
    )
        }
        let source = 101;
        let shared = binding(source, 201, BorrowKind::SharedRead);
        let exclusive = binding(source, 202, BorrowKind::ExclusiveWrite);
        let different_source = binding(102, 203, BorrowKind::ExclusiveWrite);
        let same_source = vec![Rc::new(HashMap::from([
            ("alias1".into(), shared.clone()),
            ("alias2".into(), exclusive.clone()),
        ]))];
        assert!(!Checker::<fn(&str) -> CopyPermission>::active_loan_state_is_consistent(&same_source),
            "同一來源不得同時持有共享與獨占別名");
        let different = vec![Rc::new(HashMap::from([
            ("alias1".into(), shared.clone()),
            ("alias2".into(), different_source),
        ]))];
        assert!(Checker::<fn(&str) -> CopyPermission>::active_loan_state_is_consistent(&different),
            "不同宣告來源不互相衝突");
        let shared_only = vec![Rc::new(HashMap::from([
            ("alias1".into(), shared),
            ("alias2".into(), binding(source, 204, BorrowKind::SharedRead)),
        ]))];
        assert!(Checker::<fn(&str) -> CopyPermission>::active_loan_state_is_consistent(&shared_only),
            "同來源多共享借用維持 HC14-A 核准政策");
    }

    #[test]
    fn 真正_struct_ast_具名整筆移動_來源失效() {
        assert!(inspect_with_test_ability(BASE, |_| Allowed).is_ok());
        let invalid = BASE.replace("return 0", "var c:Sample = a\nreturn 0");
        let err = inspect_with_test_ability(&invalid, |_| Allowed).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("移動"));
    }

    #[test]
    fn 真正_struct_ast_整體複製資格由欄位逐一核對() {
        let source = BASE.replace("a.move()", "a");
        assert!(inspect_with_test_ability(&source, |_| Allowed).is_ok());
        for cap in [Forbidden, Undecided] {
            let err = inspect_with_test_ability(&source, |name| if name == "bool" { cap } else { Allowed })
                .unwrap_err();
            assert_eq!(err.kind, DiagnosticKind::Static);
            assert!(err.message.contains("複製") || err.message.contains("Copyable"));
        }
        // 即使複製資格未定，明確整筆移動亦不等於隱含複製。
        assert!(inspect_with_test_ability(BASE, |_| Undecided).is_ok());
    }

    #[test]
    fn 真正_struct_ast_移動來源重新初始化後可再用() {
        let source = BASE.replace("return 0", "a = Sample()\nvar c:Sample = a.move()\nreturn 0");
        assert!(inspect_with_test_ability(&source, |_| Undecided).is_ok());
    }

    #[test]
    fn 真正_struct_ast_拒絕欄位移動與方法消耗_self() {
        let bad_field = BASE.replace("a.move()", "a.number.move()");
        assert!(inspect_with_test_ability(&bad_field, |_| Allowed).unwrap_err().message.contains("S21"));
        let bad_method = "struct S{\nvar n:int = 1\nfn consume()->int{\nreturn self.move()\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect_with_test_ability(bad_method, |_| Allowed).unwrap_err().message.contains("S15"));
    }

    #[test]
    fn 真正_struct_ast_缺失欄位初始化不能假裝資源完整() {
        let source = "struct S{\nvar missing:int\nconst flag:bool = true\n}\nfn main()->int{\nvar a:S = S()\nvar b:S = a.move()\nreturn 0\n}\n";
        assert!(inspect_with_test_ability(source, |_| Allowed).is_err());
    }

    #[test]
    fn 真正_struct_ast_建立式具名欄位及型別() {
        let source = BASE.replace("Sample()", "Sample(number=3,flag=false,)");
        assert!(inspect_with_test_ability(&source, |_| Allowed).is_ok());
        for bad in ["Sample(extra=1)", "Sample(number=false)", "Sample(number=1,number=2)"] {
            assert!(inspect_with_test_ability(&BASE.replace("Sample()", bad), |_| Allowed).is_err());
        }
    }

    #[test]
    fn 不得把限制外的控制流程直接當成已驗證() {
        let source = BASE.replace("return 0", "try{\nreturn 0\n}\nreturn 0");
        assert!(inspect_with_test_ability(&source, |_| Allowed).is_err());
    }
    #[test]
    fn 實體_voil_素材須由完整受限_ast_及型別所有權檢查處理() {
        const GOOD: &[&str] = &[
            include_str!("../../tests/fixtures/p13_stage/整筆移動.voil"),
            include_str!("../../tests/fixtures/p13_stage/結構重新初始化.voil"),
        ];
        const BAD: &[&str] = &[
            include_str!("../../tests/fixtures/p13_stage/移動後重複使用.voil"),
            include_str!("../../tests/fixtures/p13_stage/禁止欄位移動.voil"),
        ];
        for source in GOOD {
            assert!(inspect_with_test_ability(source, |_| Undecided).is_ok(), "合法整筆移動案例");
        }
        for source in BAD {
            let error = inspect_with_test_ability(source, |_| Allowed).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::Static);
        }
        const UNKNOWN_COPY: &str =
            include_str!("../../tests/fixtures/p13_stage/未知複製資格.voil");
        assert!(inspect_with_test_ability(UNKNOWN_COPY, |_| Undecided).is_err());
        assert!(inspect_with_test_ability(UNKNOWN_COPY, |_| Allowed).is_ok());
    }

    #[test]
    fn 不存在的結構欄位型別不得由假設能力誤認() {
        let src = "struct Sample{\nvar mystery:Unknown\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect_with_test_ability(src, |_| Allowed).is_err());
    }

    #[test]
    fn 真正結構來源_單分支移動合流後必須拒絕() {
        let text = include_str!("../../tests/fixtures/p13_stage/單分支移動後讀取.voil");
        let error = inspect_with_test_ability(text, |_| Allowed).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("可能") || error.message.contains("無效"));
    }

    #[test]
    fn 真正結構來源_雙分支重新初始化合流後可合法移動() {
        let text = include_str!("../../tests/fixtures/p13_stage/雙分支重新初始化.voil");
        assert!(inspect_with_test_ability(text, |_| Undecided).is_ok());
    }

    #[test]
    fn 真正結構來源_已提前_return_分支不得污染可達路徑() {
        let text = include_str!("../../tests/fixtures/p13_stage/提前回傳分支.voil");
        assert!(inspect_with_test_ability(text, |_| Undecided).is_ok());
    }

    #[test]
    fn 分支區域內宣告不得外洩到外層() {
        let text = include_str!("../../tests/fixtures/p13_stage/雙分支重新初始化.voil")
            .replace("var d:Sample = a.move()", "var d:Sample = b");
        assert!(inspect_with_test_ability(&text, |_| Allowed).is_err());
    }

    #[test]
    fn 整數來源_非法進位與溢位均不得進入結構初始化() {
        for value in [
            "9223372036854775808",
            "18446744073709551616",
            "0x8000000000000000",
            "0b",
            "0o",
        ] {
            let source = BASE.replace("var number:int = 1",
                &format!("var number:int = {value}"));
            assert!(inspect_with_test_ability(&source, |_| Allowed).is_err(),
                "應拒絕不合法或超出 int64 上限的字面值：{value}");
        }
    }

    #[test]
    fn 整數來源_上限及合法多進位字面值仍可使用() {
        for value in ["9223372036854775807", "0x7fff_ffff_ffff_ffff", "0b101010", "0o777"] {
            let source = BASE.replace("var number:int = 1",
                &format!("var number:int = {value}"));
            assert!(inspect_with_test_ability(&source, |_| Undecided).is_ok());
        }
    }

    #[test]
    fn 尚未支援的浮點字面值不能繞過型別安全檢查() {
        let source = BASE.replace("var number:int = 1", "var number:float = 0.00000000000000001");
        assert!(inspect_with_test_ability(&source, |_| Allowed).is_err());
    }

}





#[cfg(test)]
mod readonly_self_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 已核准_copyable_欄位可藉共享唯讀_self_方法複製() {
        const CODE: &str =
            include_str!("../../tests/fixtures/p13_stage/唯讀方法複製與呼叫後移動.voil");
        assert!(inspect(CODE).is_ok());
    }

    #[test]
    fn self_唯讀借用於方法結束後歸還_仍可明確整筆移動() {
        const CODE: &str =
            include_str!("../../tests/fixtures/p13_stage/唯讀方法複製與呼叫後移動.voil");
        let invalid = CODE.replace("return copied", "var double:Counter = source.move()\nreturn copied");
        assert!(inspect(&invalid).unwrap_err().message.contains("已移動"));
    }

    #[test]
    fn 尚未核准_copyable_的_self_欄位不得從借用偷複製() {
        const CODE: &str =
            include_str!("../../tests/fixtures/p13_stage/未知欄位複製能力的唯讀方法.voil");
        let error = inspect(CODE).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
        assert!(error.span.is_some());
    }

    #[test]
    fn 方法呼叫者在移動後不可再被借用() {
        const CODE: &str =
            include_str!("../../tests/fixtures/p13_stage/移動後不可呼叫唯讀方法.voil");
        assert!(inspect(CODE).unwrap_err().message.contains("已移動"));
    }

    #[test]
    fn 尚未初始化的結構不可從方法讀取欄位() {
        const CODE: &str =
            include_str!("../../tests/fixtures/p13_stage/未初始化欄位不可呼叫方法.voil");
        assert!(inspect(CODE).unwrap_err().message.contains("初始化"));
    }

    #[test]
    fn 方法只能讀取合法_self_欄位_回傳型別不符必須拒絕() {
        let wrong = "struct A{\nvar value:int = 1\nfn read()->bool{\nreturn self.value\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(wrong).unwrap_err().message.contains("回傳型別"));
        let missing = wrong.replace("->bool", "->int").replace("self.value", "self.missing");
        assert!(inspect(&missing).unwrap_err().message.contains("不存在"));
        let owner = wrong.replace("->bool", "->A").replace("self.value", "self");
        assert!(inspect(&owner).unwrap_err().message.contains("其他借用"));
    }

    #[test]
    fn 未驗證方法寫入及跨作用域語意維持保守拒絕() {
        let write = "struct A{\nvar value:int = 1\nfn change()->int{\nself.value = other.value\nreturn 0\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(write).is_err());
        let calls_self = "struct A{\nvar value:int = 1\nfn read()->int{\nreturn self.other()\n}\nfn other()->int{\nreturn self.value\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(calls_self).is_err());
    }
}

#[cfg(test)]
mod default_constructor_cycle_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 相同結構的預設建構不得無限遞迴() {
        let code = include_str!("../../tests/fixtures/p13_stage/自身預設建構循環.voil");
        let error = inspect(code).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("循環"));
        assert!(error.span.is_some());
    }

    #[test]
    fn 多結構互相建立的預設值亦需保守拒絕() {
        let code = include_str!("../../tests/fixtures/p13_stage/跨結構預設建構循環.voil");
        let error = inspect(code).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("循環"));
        assert!(error.span.is_some());
    }

    #[test]
    fn 合法無循環巢狀預設值保留受限子集可用性() {
        let code = include_str!("../../tests/fixtures/p13_stage/無循環巢狀預設建構.voil");
        assert!(inspect(code).is_ok());
    }

    #[test]
    fn 具名建立引數內的遞迴預設值仍須拒絕() {
        let code = "struct Node{\nvar child:Node = Node(child=Node())\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(code).unwrap_err().message.contains("循環"));
    }
}

#[cfg(test)]
mod method_safety_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn s15_方法_if_內消耗_self_精準拒絕() {
        let code = include_str!("../../tests/fixtures/p13_stage/方法分支內消耗_self.voil");
        let error = inspect(code).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("S15"));
        assert!(error.span.is_some());
    }

    #[test]
    fn s15_方法結構建立引數內消耗_self_亦應拒絕() {
        let code = include_str!("../../tests/fixtures/p13_stage/方法建構引數內消耗_self.voil");
        let error = inspect(code).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("S15"));
        assert!(error.span.is_some());
    }

    #[test]
    fn s21_方法分支內局部欄位移動應保有來源位置() {
        let code = include_str!("../../tests/fixtures/p13_stage/方法分支內局部欄位移動.voil");
        let error = inspect(code).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("S21"));
        assert!(error.span.is_some());
    }

    #[test]
    fn 無違規的合法借用方法仍不可在未驗證安全前誤收() {
        let code = "struct Sample{\nvar x:int = 1\nfn value()->int{\nvar temp:int = 1\nreturn temp\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error = inspect(code).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("未驗證"));
    }
}

#[cfg(test)]
mod lexical_ownership_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn h06_內層同名結構移動不得毀損外層所有權() {
        let code = include_str!("../../tests/fixtures/p13_stage/內層遮蔽移動不傷外層.voil");
        assert!(inspect(code).is_ok());
    }

    #[test]
    fn 內層同名移動後不能透過同名重新複製() {
        let code = include_str!("../../tests/fixtures/p13_stage/內層移動後誤讀.voil");
        let err = inspect(code).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("移動"));
    }

    #[test]
    fn 外層移動後不可藉內層重新宣告偷恢復來源() {
        let code = include_str!("../../tests/fixtures/p13_stage/外層已移動不被內層遮蔽復原.voil");
        let err = inspect(code).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("移動"));
    }

    #[test]
    fn 同作用域重複宣告繼續拒絕() {
        let code = include_str!("../../tests/fixtures/p13_stage/同層重複宣告拒絕.voil");
        let err = inspect(code).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("重複宣告"));
    }

    #[test]
    fn 雙分支同名內層資源都不該污染同一外層來源() {
        let code = "struct S{\nvar v:int = 1\n}\nfn main()->int{\nvar a:S = S()\nif (true){\nvar a:S = S()\nvar x:S = a.move()\n}else{\nvar a:S = S()\nvar y:S = a.move()\n}\nvar last:S = a.move()\nreturn 0\n}\n";
        assert!(inspect(code).is_ok());
    }

    #[test]
    fn 真正內層移動外層來源仍必須讓外層失效() {
        let code = "struct S{\nvar v:int = 1\n}\nfn main()->int{\nvar a:S = S()\nif (true){\nvar moved:S = a.move()\n}\nvar last:S = a\nreturn 0\n}\n";
        assert!(inspect(code).unwrap_err().message.contains("可能"));
    }
}

#[cfg(test)]
mod approved_policy_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn ha13_核准_int_bool_且所有欄位可複製的_struct_可獨立複製() {
        const SOURCE: &str =
            include_str!("../../tests/fixtures/p13_approved/整體自動複製.voil");
        assert!(inspect(SOURCE).is_ok());
    }

    #[test]
    fn str_已禁止_copyable_及含_str_欄位的_struct_都不可隱含複製() {
        const SOURCE: &str =
            include_str!("../../tests/fixtures/p13_approved/未知字串資格拒絕複製.voil");
        let error = inspect(SOURCE).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
        let simple = "fn main()->int{\nvar s:str = \"ok\"\nvar t:str = s\nreturn 0\n}\n";
        assert!(inspect(simple).unwrap_err().message.contains("Copyable"));
    }

    #[test]
    fn hb13_未知複製資格不應擋住明確的整筆具名移動() {
        const SOURCE: &str =
            include_str!("../../tests/fixtures/p13_approved/未知字串允許具名整筆移動.voil");
        assert!(inspect(SOURCE).is_ok());
        let double_move = SOURCE.replace("return 0", "var c:Note = a.move()\nreturn 0");
        assert!(inspect(&double_move).unwrap_err().message.contains("已移動"));
    }

    #[test]
    fn hb13_不允許單欄位_未綁定來源或方法消耗_self() {
        const FIELD: &str =
            include_str!("../../tests/fixtures/p13_approved/禁止欄位單獨移動.voil");
        assert!(inspect(FIELD).unwrap_err().message.contains("S21"));
        let missing = "fn main()->int{\nvar b:int = not_declared.move()\nreturn 0\n}\n";
        assert!(inspect(missing).unwrap_err().message.contains("找不到"));
        let method = "struct Pair{\nvar value:int = 1\nfn take()->int{\nreturn self.move()\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(method).unwrap_err().message.contains("S15"));
    }

    #[test]
    fn hb13_移動後必須重新初始化才允許再次使用() {
        let source = "fn main()->int{\nvar a:int = 1\nvar b:int = a.move()\na = 2\nreturn a\n}\n";
        assert!(inspect(source).is_ok());
        let invalid = source.replace("a = 2\n", "");
        assert!(inspect(&invalid).unwrap_err().message.contains("已移動"));
    }

    #[test]
    fn mvp_a_公開解析器仍禁止_move_語法() {
        let source = crate::source::Source::new(b"fn main()->int{\nvar a:int = 1\nreturn a.move()\n}\n".to_vec());
        let lexed = crate::lexer::scan(&source);
        assert!(crate::parser::parse(&source, &lexed.tokens).is_err());
    }
}

#[cfg(test)]
mod automatic_self_borrow_tests {
    use super::{inspect, Parser, Checker};
    use crate::DiagnosticKind;
    use crate::borrow_safety::BorrowKind;

    #[test]
    fn 方案_a_依欄位寫入推導方法共享或獨占借用() {
        let text = include_str!("../../tests/fixtures/p13_stage/自動獨占與共享方法.voil");
        assert!(inspect(text).is_ok());
        let source = crate::source::Source::new(text.as_bytes().to_vec());
        let scan = crate::lexer::scan(&source);
        let (structs, _) = Parser {source: &source, tokens: &scan.tokens, cursor: 0}.file().unwrap();
        let methods = &structs["Counter"].methods;
        let checker = Checker {source: &source, structs: &structs, functions: &[],
            function_index: std::collections::HashMap::new(), ability: crate::ownership_policy::approved_leaf_copy_permission,
            recorded_function_exits: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_edges: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_method_safety: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_method_safety_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_direct_moves: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declarations: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_types: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declaration_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_named_returns: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_field_writes: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_call_borrows: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_call_borrow_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_scalar_hir: std::cell::RefCell::new(
                crate::common_ir::VerifiedScalarHir::default())};
        assert_eq!(checker.check_method("Counter", methods.iter().find(|m| m.name == "get").unwrap()).unwrap(),
            BorrowKind::SharedRead);
        assert_eq!(checker.check_method("Counter", methods.iter().find(|m| m.name == "set").unwrap()).unwrap(),
            BorrowKind::ExclusiveWrite);
    }

    #[test]
    fn s18_const_實例不得呼叫自動推導的獨占方法() {
        let code = include_str!("../../tests/fixtures/p13_stage/const實例禁止可變方法.voil");
        let err = inspect(code).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("const 結構實例"));
    }

    #[test]
    fn s18_const_欄位不得被方法修改() {
        let code = include_str!("../../tests/fixtures/p13_stage/const欄位禁止方法寫入.voil");
        let err = inspect(code).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("const 欄位"));
    }

    #[test]
    fn 方法對_self_欄位的錯誤型別必須拒絕() {
        let code = include_str!("../../tests/fixtures/p13_stage/可變方法錯誤型別.voil");
        assert!(inspect(code).unwrap_err().message.contains("型別"));
    }

    #[test]
    fn 已移動來源不得藉可變方法存取() {
        let code = include_str!("../../tests/fixtures/p13_stage/移動後禁止可變方法.voil");
        assert!(inspect(code).unwrap_err().message.contains("已移動"));
    }

    #[test]
    fn 不推定不同方法重入與運算式寫入的借用生命週期() {
        let src = "struct X{\nvar n:int = 1\nfn set()->int{\nself.n = other.n\nreturn 0\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).is_err());
        let nested = src.replace("self.n = other.n", "if (self.n){\nself.n = 2\n}");
        assert!(inspect(&nested).is_err());
    }

    #[test]
    fn 一般函式在完整持有來源上可合法寫入已核准_copyable_var_欄位() {
        let src = "struct X{\nvar n:int = 1\n}\nfn main()->int{\nvar x:X = X()\nx.n = 2\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod conditional_self_inference_tests {
    use super::{inspect, Parser, Checker};
    use crate::borrow_safety::BorrowKind;

    #[test]
    fn 任一分支修改_self_即推導獨占借用() {
        let text = include_str!("../../tests/fixtures/p13_stage/分支寫入推導獨占方法.voil");
        assert!(inspect(text).is_ok());
        let source = crate::source::Source::new(text.as_bytes().to_vec());
        let scanned = crate::lexer::scan(&source);
        let (types, _) = Parser {source: &source, tokens: &scanned.tokens, cursor: 0}.file().unwrap();
        let checker = Checker {source: &source, structs: &types, functions: &[],
            function_index: std::collections::HashMap::new(), ability: crate::ownership_policy::approved_leaf_copy_permission,
            recorded_function_exits: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_edges: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_method_safety: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_method_safety_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_direct_moves: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declarations: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_types: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declaration_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_named_returns: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_field_writes: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_call_borrows: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_call_borrow_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_scalar_hir: std::cell::RefCell::new(
                crate::common_ir::VerifiedScalarHir::default())};
        let method = &types["Counter"].methods[0];
        assert_eq!(checker.check_method("Counter", method).unwrap(), BorrowKind::ExclusiveWrite);
    }

    #[test]
    fn const_實例不得呼叫只在某條分支寫入的方法() {
        let text = include_str!("../../tests/fixtures/p13_stage/const實例禁止分支寫入.voil");
        assert!(inspect(text).unwrap_err().message.contains("const 結構實例"));
    }

    #[test]
    fn 自動借用推導也必須檢查兩條分支的欄位寫入型別() {
        let text = include_str!("../../tests/fixtures/p13_stage/分支寫入推導獨占方法.voil");
        let bad = text.replace("self.value = 3", "self.value = false");
        assert!(inspect(&bad).unwrap_err().message.contains("型別"));
    }

    #[test]
    fn self_非布林欄位不可用作條件() {
        let text = include_str!("../../tests/fixtures/p13_stage/分支寫入推導獨占方法.voil");
        let bad = text.replace("if (self.active)", "if (self.value)");
        assert!(inspect(&bad).unwrap_err().message.contains("bool"));
    }

    #[test]
    fn 無副作用常值_巢狀條件可保守推導獨占而非放行別名() {
        let text = include_str!("../../tests/fixtures/p13_stage/分支寫入推導獨占方法.voil");
        let nested = text.replace("self.value = 2", "if (true){\nself.value = 2\n}");
        assert!(inspect(&nested).is_ok());
        let unknown = text.replace("if (self.active)", "if (other())");
        assert!(inspect(&unknown).is_err());
    }

    #[test]
    fn 方法分支提前返回已核准_未知接收者寫入仍拒絕() {
        let text = include_str!("../../tests/fixtures/p13_stage/分支寫入推導獨占方法.voil");
        let early = text.replace("self.value = 2", "return 1");
        // 依已核准的分支提前回傳語意，此來源現在是合法正例。
        assert!(inspect(&early).is_ok());
        let unknown = text.replace("self.value = 2", "other.value = 2");
        assert!(inspect(&unknown).is_err());
    }
}

#[cfg(test)]
mod copyable_self_field_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 已核准可複製的_self_欄位可作方法獨占寫入來源() {
        let code = include_str!("../../tests/fixtures/p13_stage/方法複製自身可複製欄位.voil");
        assert!(inspect(code).is_ok());
    }

    #[test]
    fn 已禁止_str_複製能力不可在方法裡隱含複製() {
        let code = include_str!("../../tests/fixtures/p13_stage/方法不可複製未知欄位.voil");
        let err = inspect(code).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("Copyable"));
        assert!(err.span.is_some());
    }

    #[test]
    fn 已核准可複製來源若型別不一致亦不可寫入() {
        let code = include_str!("../../tests/fixtures/p13_stage/方法來源欄位錯誤型別.voil");
        assert!(inspect(code).unwrap_err().message.contains("型別"));
    }

    #[test]
    fn 不存在或非法_self_欄位不能假借複製模式跳過解析() {
        let code = include_str!("../../tests/fixtures/p13_stage/方法複製自身可複製欄位.voil");
        let invalid = code.replace("self.left = self.right", "self.left = self.missing");
        assert!(inspect(&invalid).unwrap_err().message.contains("不存在"));
    }

    #[test]
    fn s19_結構欄位若全可複製仍能整體複製作為_欄位_來源() {
        let src = "struct Leaf{\nvar n:int = 1\n}\nstruct Box{\nvar a:Leaf = Leaf()\nvar b:Leaf = Leaf()\nfn copy()->int{\nself.a = self.b\nreturn 0\n}\n}\nfn main()->int{\nvar box:Box = Box()\nreturn box.copy()\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod nonempty_struct_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn k06_空結構與只有註解的結構均必須拒絕() {
        for src in [
            "struct Empty{\n}\nfn main()->int{\nreturn 0\n}\n",
            "struct Empty{\n# 沒有實際欄位或方法\n}\nfn main()->int{\nreturn 0\n}\n",
            "struct Empty{\n\n}\nfn main()->int{\nreturn 0\n}\n",
        ] {
            let error = inspect(src).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::Syntax);
            assert!(error.message.contains("K06"));
            assert!(error.span.is_some());
        }
    }

    #[test]
    fn k06_只有欄位或只有方法都滿足結構非空條件() {
        let fields = "struct OnlyField{\nvar n:int = 1\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(fields).is_ok());
        let methods = "struct OnlyMethod{\nfn get()->int{\nreturn 1\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(methods).is_ok());
    }
}

#[cfg(test)]
mod p13_deferred_borrowed_return_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_a_非_copyable_self_字串欄位借用回傳必須拒絕() {
        let src = include_str!("../../tests/fixtures/p13_stage/禁止回傳未決字串欄位.voil");
        let error = inspect(src).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
        assert!(error.message.contains("P15"));
        assert!(error.span.is_some());
    }

    #[test]
    fn p13_a_巢狀結構含未決能力欄位不得從_self_逸出() {
        let src = include_str!("../../tests/fixtures/p13_stage/禁止回傳包含未決欄位的結構.voil");
        let error = inspect(src).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("P15"));
    }

    #[test]
    fn p13_a_全部欄位已核准_copyable_的巢狀結構可返回獨立複本() {
        let src = include_str!("../../tests/fixtures/p13_stage/允許回傳完全Copyable結構副本.voil");
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_a_與_self_無關的字串常值是新值_不是借用回傳() {
        let src = include_str!("../../tests/fixtures/p13_stage/合法獨立字串常值回傳.voil");
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn s21_即使回傳型別標註正確_仍不可移動單一_self_欄位() {
        let src = include_str!("../../tests/fixtures/p13_stage/拒絕移動self單一欄位回傳.voil");
        let error = inspect(src).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("S21"));
    }

    #[test]
    fn p13_a_未知_copyable_欄位不會因方法有寫入而可借用回傳() {
        let src = "struct X{\nvar label:str = \"A\"\nvar n:int = 1\nfn get()->str{\nself.n = 2\nreturn self.label\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).unwrap_err().message.contains("P15"));
    }
}

#[cfg(test)]
mod nested_default_initialization_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn s07_有預設運算式不代表子結構已完整初始化() {
        let src = include_str!("../../tests/fixtures/p13_stage/拒絕巢狀預設不完整值.voil");
        let err = inspect(src).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("未初始化"));
        assert!(err.span.is_some());
    }

    #[test]
    fn f01_const_欄位預設值也不可包含不完整子結構() {
        let src = include_str!("../../tests/fixtures/p13_stage/拒絕const巢狀預設不完整值.voil");
        let err = inspect(src).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("未初始化"));
    }

    #[test]
    fn s05_具名提供所有必要欄位的巢狀預設值可通過() {
        let src = include_str!("../../tests/fixtures/p13_stage/已初始化巢狀具名預設值.voil");
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn s07_var_欄位沒有預設依舊合法_只要求在使用前完成初始化() {
        let src = include_str!("../../tests/fixtures/p13_stage/沒有預設值的var欄位仍合法宣告.voil");
        assert!(inspect(src).is_ok());
        let incomplete = src.replace("Parent(child=Child(count=5))", "Parent()");
        // 可建立未完成初始化的具名 var，但不能將其誤當作合法回傳值。
        let returned = incomplete.replace("return 0", "var q:Parent = p.move()\nreturn 0");
        assert!(inspect(&returned).is_err());
    }

    #[test]
    fn 多層巢狀預設建立亦不得繞過最內層未初始化狀態() {
        let src = "struct Leaf{\nvar n:int\n}\nstruct Mid{\nvar leaf:Leaf = Leaf()\n}\nstruct Root{\nvar mid:Mid = Mid()\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).unwrap_err().message.contains("未初始化"));
    }
}

#[cfg(test)]
mod str_noncopyable_policy_tests {
    use super::inspect;
    use crate::{DiagnosticKind, ownership_model::CopyPermission};
    use crate::ownership_policy::approved_leaf_copy_permission;

    #[test]
    fn str_明確禁止名稱隱含複製並保留來源位置() {
        let source =
            include_str!("../../tests/fixtures/p13_stage/禁止字串名稱隱含複製.voil");
        let error = inspect(source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable 已明確禁止"));
        assert!(error.span.is_some());
    }

    #[test]
    fn str_具名整筆移動及重新初始化不需_copyable() {
        let source =
            include_str!("../../tests/fixtures/p13_stage/字串具名整筆移動與重新初始化.voil");
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn str_移動後的原來源讀取必須失敗() {
        let source =
            include_str!("../../tests/fixtures/p13_stage/移動字串後不能再次讀取.voil");
        let error = inspect(source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("已移動"));
    }

    #[test]
    fn s19_包含非copyable字串欄位的結構確定禁止隱含複製() {
        let source =
            include_str!("../../tests/fixtures/p13_stage/包含字串欄位的結構禁止複製.voil");
        let error = inspect(source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable 已明確禁止"));
    }

    #[test]
    fn s19_包含字串欄位的結構仍可合法整筆移動() {
        let source =
            include_str!("../../tests/fixtures/p13_stage/包含字串欄位的結構允許整筆移動.voil");
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn p13_禁止借用回傳已確定非copyable的字串欄位_不提前p15() {
        let source =
            include_str!("../../tests/fixtures/p13_stage/禁止方法複製確定非Copyable字串欄位.voil");
        let error = inspect(source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable 已明確禁止"));
        assert!(error.message.contains("P15"));
    }

    #[test]
    fn str_明確非_copyable_float_明確_copyable_容器仍未決() {
        assert_eq!(approved_leaf_copy_permission("str"), CopyPermission::Forbidden);
        assert_eq!(approved_leaf_copy_permission("float"), CopyPermission::Allowed);
        assert_eq!(approved_leaf_copy_permission("list[float]"), CopyPermission::Undecided);
        assert_eq!(approved_leaf_copy_permission("int"), CopyPermission::Allowed);
        assert_eq!(approved_leaf_copy_permission("bool"), CopyPermission::Allowed);
    }
}

#[cfg(test)]
mod str_ownership_branch_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 非copyable字串雙分支皆重新初始化可再次整筆移動() {
        let source =
            include_str!("../../tests/fixtures/p13_stage/字串雙分支移動後重新初始化.voil");
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn 非copyable字串只有一條路徑移動合流後必須拒絕() {
        let source =
            include_str!("../../tests/fixtures/p13_stage/字串單分支移動合流後不得使用.voil");
        let err = inspect(source).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("可能"));
        assert!(err.span.is_some());
    }

    #[test]
    fn 已提前回傳的字串移動路徑不污染後續可達路徑() {
        let source =
            include_str!("../../tests/fixtures/p13_stage/字串提前回傳後合流可合法移動.voil");
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn 分支內遮蔽的字串所有權不得混入外層來源() {
        let source =
            include_str!("../../tests/fixtures/p13_stage/字串遮蔽來源移動不影響外層.voil");
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn str_巢狀提前_return_不可達移動不污染可達合流() {
        let source = include_str!(
            "../../tests/fixtures/p13_stage/字串巢狀分支提前回傳不污染合流.voil"
        );
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn str_巢狀分支有可達移動時合流後須拒絕再次使用() {
        let source = include_str!(
            "../../tests/fixtures/p13_stage/字串巢狀分支有可達移動不得再次使用.voil"
        );
        // VA16-03-A 已正式核准可證明安全的 str 常值覆寫。
        // 分支另一側仍可能已移動，不能因覆寫路徑合法就擅自宣稱
        // 合流之後來源一定有效，仍應回報「可能已移動」。
        let err = inspect(source).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("可能"));
        // 未覆寫的另一條路徑同樣必須維持此失效證明。
        let merged = source.replace("original = \"alternate\"", "var untouched2:int = 1");
        let err = inspect(&merged).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("可能"));
        assert!(err.span.is_some());
    }

    #[test]
    fn 字串在重新初始化前仍不得隱含複製且禁止讀取已移動來源() {
        let source =
            include_str!("../../tests/fixtures/p13_stage/字串雙分支移動後重新初始化.voil");
        let illegal = source.replace(
            "original = \"true\"", "var illegal:str = original");
        assert!(inspect(&illegal).unwrap_err().message.contains("已移動"));
        let illegal_copy = source.replace(
            "var last:str = original.move()", "var last:str = original");
        assert!(inspect(&illegal_copy).unwrap_err()
            .message.contains("Copyable 已明確禁止"));
    }
}

#[cfg(test)]
mod float_copyable_approved_tests {
    use super::{inspect, Checker, Parser};
    use crate::ownership_model::CopyPermission;
    use crate::ownership_policy::approved_leaf_copy_permission;
    use crate::source::Source;
    use crate::lexer;

    /// 結構體的 float 欄位可在無具體 decimal64 常值求值的情況下驗證 Copyable。
    #[test]
    fn float_與遞迴全部_copyable_結構已核准_但包含_str_的結構不准複製() {
        let source_text = include_str!("../../tests/fixtures/p13_stage/浮點與混合結構複製能力.voil");
        assert!(inspect(source_text).is_ok());
        let source = Source::new(source_text.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        let (structs, _) = Parser {
            source: &source, tokens: &tokens.tokens, cursor: 0
        }.file().unwrap();
        let checker = Checker {
            source: &source, structs: &structs, functions: &[],
            function_index: std::collections::HashMap::new(), ability: approved_leaf_copy_permission,
            recorded_function_exits: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_edges: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_method_safety: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_method_safety_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_direct_moves: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declarations: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_types: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declaration_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_named_returns: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_field_writes: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_call_borrows: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_call_borrow_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_scalar_hir: std::cell::RefCell::new(
                crate::common_ir::VerifiedScalarHir::default())
        };
        assert_eq!(checker.permission("FloatBox"), CopyPermission::Allowed);
        assert_eq!(checker.permission("NestedFloat"), CopyPermission::Allowed);
        assert_eq!(checker.permission("MixedString"), CopyPermission::Forbidden);
    }

    #[test]
    fn float_具名來源尚未初始化不能因_copyable_就略過安全檢查() {
        let source_text = "fn main()->int{\nvar number:float\nvar copy:float = number\nreturn 0\n}\n";
        let error = inspect(source_text).unwrap_err();
        assert!(error.message.contains("初始化"));
        assert!(error.span.is_some());
    }

    #[test]
    fn 受限前端沒有完整_decimal64_範圍證明時仍須保守拒絕() {
        let source_text = "fn main()->int{\nvar number:float = 0.00000000000000001\nreturn 0\n}\n";
        let error = inspect(source_text).unwrap_err();
        assert!(error.message.contains("decimal64"));
    }
}

#[cfg(test)]
mod float_copyable_method_tests {
    use super::{inspect, Checker, Parser};
    use crate::borrow_safety::BorrowKind;
    use crate::lexer;
    use crate::ownership_policy::approved_leaf_copy_permission;
    use crate::source::Source;

    #[test]
    fn float_已准_copyable_故唯讀_self_欄位方法可以靜態驗證() {
        let text = include_str!("../../tests/fixtures/p13_stage/浮點欄位唯讀方法型別能力.voil");
        assert!(inspect(text).is_ok());
        let source = Source::new(text.as_bytes().to_vec());
        let scan = lexer::scan(&source);
        let (types, _) = Parser {
            source: &source, tokens: &scan.tokens, cursor: 0
        }.file().unwrap();
        let checker = Checker {
            source: &source, structs: &types, functions: &[],
            function_index: std::collections::HashMap::new(), ability: approved_leaf_copy_permission,
            recorded_function_exits: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_edges: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_method_safety: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_method_safety_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_direct_moves: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declarations: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_types: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declaration_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_named_returns: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_field_writes: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_call_borrows: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_call_borrow_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_scalar_hir: std::cell::RefCell::new(
                crate::common_ir::VerifiedScalarHir::default())
        };
        assert_eq!(
            checker.check_method("FloatHolder", &types["FloatHolder"].methods[0]).unwrap(),
            BorrowKind::SharedRead);
    }

    #[test]
    fn float_欄位之間複製已准但仍必須推導獨占_self_借用() {
        let text = include_str!("../../tests/fixtures/p13_stage/浮點欄位複製至同型別欄位.voil");
        assert!(inspect(text).is_ok());
        let source = Source::new(text.as_bytes().to_vec());
        let scan = lexer::scan(&source);
        let (types, _) = Parser {
            source: &source, tokens: &scan.tokens, cursor: 0
        }.file().unwrap();
        let checker = Checker {
            source: &source, structs: &types, functions: &[],
            function_index: std::collections::HashMap::new(), ability: approved_leaf_copy_permission,
            recorded_function_exits: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_edges: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_method_safety: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_method_safety_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_direct_moves: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declarations: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_types: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declaration_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_named_returns: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_field_writes: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_call_borrows: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_call_borrow_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_scalar_hir: std::cell::RefCell::new(
                crate::common_ir::VerifiedScalarHir::default())
        };
        assert_eq!(
            checker.check_method("FloatPair", &types["FloatPair"].methods[0]).unwrap(),
            BorrowKind::ExclusiveWrite);
    }

    #[test]
    fn float_可複製並不解除方法回傳型別一致的要求() {
        let text = include_str!("../../tests/fixtures/p13_stage/浮點欄位回傳型別不一致.voil");
        assert!(inspect(text).unwrap_err().message.contains("回傳型別"));
    }

    #[test]
    fn float_未初始化的接收者仍不能直接呼叫方法() {
        let text = include_str!("../../tests/fixtures/p13_stage/浮點欄位唯讀方法型別能力.voil");
        let invalid = text.replace("return 0\n}", "var value:FloatHolder = FloatHolder()\nvar copy:float = value.read()\nreturn 0\n}");
        let err = inspect(&invalid).unwrap_err();
        assert!(err.message.contains("初始化"));
    }
}

#[cfg(test)]
mod conservative_decimal64_tests {
    use super::{inspect, Parser};
    use crate::DiagnosticKind;

    #[test]
    fn decimal64_有限子集與尾端小數點_前導零_底線可解析() {
        let source = include_str!("../../tests/fixtures/p13_stage/decimal64有限合法常值.voil");
        assert!(inspect(source).is_ok());
        for text in ["0.", "00.5", "1._2", "1.25", "12.345_678_901_234_5"] {
            assert!(Parser::decimal64_checked_subset(text).is_some(), "{text}");
        }
    }

    #[test]
    fn decimal64_平手時選尾數偶數_超過一半需進位() {
        assert_eq!(Parser::decimal64_checked_subset("1.2345678901234565"),
            Some((1_234_567_890_123_456, -15)));
        assert_eq!(Parser::decimal64_checked_subset("1.2345678901234555"),
            Some((1_234_567_890_123_456, -15)));
        assert_eq!(Parser::decimal64_checked_subset("1.2345678901234565001"),
            Some((1_234_567_890_123_457, -15)));
        assert_eq!(Parser::decimal64_checked_subset("1.2345678901234566"),
            Some((1_234_567_890_123_457, -15)));
        assert_eq!(Parser::decimal64_checked_subset("0.5"),
            Some((5_000_000_000_000_000, -16)));
    }

    #[test]
    fn decimal64_極端下溢與超出驗證範圍均保守拒絕() {
        let source = include_str!("../../tests/fixtures/p13_stage/decimal64超出安全子集.voil");
        let error = inspect(source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Syntax);
        assert!(error.message.contains("decimal64"));
        assert!(error.span.is_some());
        for text in ["0.00000000000000001", "99999999999999999.", "10000000000000000."] {
            assert!(Parser::decimal64_checked_subset(text).is_none(), "{text}");
        }
    }

    #[test]
    fn decimal64_copyable_只允許型別相符而非隱含轉成_int() {
        assert!(inspect("fn main()->int{\nvar x:float = 1.25\nvar y:float = x\nreturn 0\n}\n").is_ok());
        let bad = "fn main()->int{\nvar x:int = 1.25\nreturn 0\n}\n";
        assert!(inspect(bad).unwrap_err().message.contains("型別"));
    }
}

#[cfg(test)]
mod vb14_vb15_vb16_approved_source_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn vb16_具名引數可任意排列且未覆蓋預設值仍合法() {
        let source="struct Profile{\nvar first:str = \"甲\"\nvar second:str = \"乙\"\nconst id:int = 1\n}\nfn main()->int{\nvar source:str = \"新\"\nvar x:Profile = Profile(second=source.move(),first=\"覆蓋\")\nreturn 0\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn vb16_同一欄位重複具名引數先靜態拒絕() {
        let source="struct Item{\nvar text:str = \"預設\"\n}\nfn main()->int{\nvar x:Item=Item(text=\"甲\",text=\"乙\")\nreturn 0\n}\n";
        assert_eq!(inspect(source).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn vb14_既有借用別名再借用必須保守拒絕() {
        let source="fn main()->int{\nconst value:int = 1\nconst alias:int = value.borrow()\nconst second:int = alias.borrow()\nreturn second\n}\n";
        let err=inspect(source).unwrap_err();
        assert_eq!(err.kind,DiagnosticKind::Static);
        assert!(err.message.contains("再次借用"));
    }

    #[test]
    fn vb15_尚未支援的跨_await_借用與例外不可被私有檢查器放行() {
        for source in [
            "fn main()->int{\nvar x:int=1\nvar alias:int=x.borrow()\nawait alias\nreturn x\n}\n",
            "fn main()->int{\nvar x:int=1\nvar alias:int=x.borrow()\nthrow alias\nreturn x\n}\n",
        ] {
            let err=inspect(source).unwrap_err();
            assert!(matches!(err.kind,DiagnosticKind::Syntax|DiagnosticKind::Static));
        }
    }
}

#[cfg(test)]
mod va15_path_sensitive_last_use_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn va15_分支中真正最後使用後同分支恢復原來源直接讀取() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nif (true){\nvar copy:int = alias\nvar direct:int = owner\n}else{\nvar direct:int = owner\n}\nreturn owner\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va15_外層仍會使用借用時分支內不可提前解除() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nif (true){\nvar direct:int = owner\n}else{\nvar other:int = 2\n}\nreturn alias\n}\n";
        let err = inspect(source).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("HC14"));
    }

    #[test]
    fn va15_無_else_的隱含分支也須和已分析分支安全合流() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nif (true){\nvar copy:int = alias\nvar direct:int = owner\n}\nreturn owner\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va15_巢狀分支各自完成最後使用後來源可用() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nif (true){\nif (false){\nvar a:int = alias\n}else{\nvar b:int = alias\n}\nvar direct:int = owner\n}else{\nvar direct:int = owner\n}\nreturn owner\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va15_返回路徑不能把借用活性污染可達的其他路徑() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nif (true){\nreturn alias\n}else{\nvar copy:int = owner\n}\nreturn owner\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va15_跨分支仍有效的獨占借用不得允許重新借用() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nif (true){\nvar first:int = alias\n}else{\nvar second:int = 2\n}\nvar other:int = owner.borrow()\nreturn alias\n}\n";
        let err = inspect(source).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("HC14"));
    }
}

#[cfg(test)]
mod va14_va15_borrow_source_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn va15_合法最後使用後來源恢復可用_不必等區塊出口() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nvar copy:int = alias\nreturn owner\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va14_來源借用尚有效時不得直接隱含複製() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nvar illegal:int = owner\nreturn alias\n}\n";
        let error = inspect(source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("HC14"));
    }

    #[test]
    fn va14_const_共享借用可同時存在() {
        let source = "fn main()->int{\nconst owner:int = 1\nconst first:int = owner.borrow()\nconst second:int = owner.borrow()\nvar read:int = first\nreturn second\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va14_var_獨占借用不可重疊() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar first:int = owner.borrow()\nvar second:int = owner.borrow()\nreturn first\n}\n";
        let error = inspect(source).unwrap_err();
        assert!(error.message.contains("HC14"));
    }

    #[test]
    fn va14_來源與借用宣告可變性不符必須拒絕() {
        for source in [
            "fn main()->int{\nconst owner:int = 1\nvar alias:int = owner.borrow()\nreturn 0\n}\n",
            "fn main()->int{\nvar owner:int = 1\nconst alias:int = owner.borrow()\nreturn 0\n}\n",
        ] {
            assert!(inspect(source).unwrap_err().message.contains("VA14-01"));
        }
    }

    #[test]
    fn va14_非擁有借用不得被明確_move_轉換成所有者() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nvar moved:int = alias.move()\nreturn owner\n}\n";
        let error = inspect(source).unwrap_err();
        assert!(error.message.contains("VA14-01"));
    }

    #[test]
    fn va15_借用回傳沒有獨立存活證明時拒絕_但_copyable_值可獨立複製() {
        let bad = "fn get_str()->str{\nvar owner:str = \"str\"\nvar alias:str = owner.borrow()\nreturn alias\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(bad).unwrap_err().message.contains("Copyable"));
        let direct = "fn main()->int{\nvar owner:int = 1\nreturn owner.borrow()\n}\n";
        assert!(inspect(direct).unwrap_err().message.contains("逃逸"));
        let copy = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nreturn alias\n}\n";
        assert!(inspect(copy).is_ok());
    }

    #[test]
    fn va14_借用原非_copyable_資源期間不能偷偷移動來源() {
        let source = "fn main()->int{\nvar owner:str = \"str\"\nvar alias:str = owner.borrow()\nvar stolen:str = owner.move()\nreturn 0\n}\n";
        // 沒有實際用到 alias 會即時歸還，因此另加最後使用以確保此處仍借用。
        let source = source.replace("return 0", "var copy:str = alias\nreturn 0");
        assert!(inspect(&source).unwrap_err().message.contains("HC14"));
    }

    #[test]
    fn va15_分支內借用於分支最後使用後歸還外層來源() {
        let source = "fn main()->int{\nvar owner:int = 1\nif (true){\nvar alias:int = owner.borrow()\nvar copy:int = alias\n}else{\nvar other:int = 2\n}\nreturn owner\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va14_借用接收者合法唯讀方法且最後使用後解除封鎖() {
        let source = "struct S{\nvar n:int = 1\nfn get()->int{\nreturn self.n\n}\n}\nfn main()->int{\nconst owner:S = S()\nconst alias:S = owner.borrow()\nvar independent:int = alias.get()\nreturn owner.get()\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va14_var_獨占借用可呼叫合法可寫方法且原來源於最後使用後恢復() {
        let source = "struct S{\nvar n:int = 1\nfn update()->int{\nself.n = 2\nreturn self.n\n}\nfn get()->int{\nreturn self.n\n}\n}\nfn main()->int{\nvar owner:S = S()\nvar alias:S = owner.borrow()\nvar independent:int = alias.update()\nreturn owner.get()\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va14_再借用_在缺乏別名關係證明前保守拒絕() {
        let source = "fn main()->int{\nconst owner:int = 1\nconst alias:int = owner.borrow()\nconst twice:int = alias.borrow()\nreturn twice\n}\n";
        assert!(inspect(source).unwrap_err().message.contains("再次借用"));
    }
}

#[cfg(test)]
mod p13_resource_overwrite_barrier_tests {
    use super::inspect;
    #[test]
    fn va16_已核准有效_str_新常值原子覆寫為合法來源() {
        let src = "fn main()->int{\nvar s:str = \"old\"\ns = \"new\"\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn va16_另一合法具名_str_整筆_move_可覆寫有效目標() {
        let src = "fn main()->int{\nvar s:str = \"old\"\nvar incoming:str = \"new\"\ns = incoming.move()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn va16_新資源覆寫前先檢查右側型別與原來源借用限制() {
        let mismatch = "fn main()->int{\nvar s:str = \"old\"\ns = false\nreturn 0\n}\n";
        assert!(inspect(mismatch).unwrap_err().message.contains("型別"));
        let borrowed = "fn main()->int{\nvar s:str = \"old\"\nvar alias:str = s.borrow()\ns = \"new\"\nvar invalid:str = alias\nreturn 0\n}\n";
        assert!(inspect(borrowed).unwrap_err().message.contains("HC14"));
    }

    #[test]
    fn 已存在非_copyable_struct_不得整筆資源覆寫() {
        let src = "struct S{\nvar text:str = \"old\"\n}\nfn main()->int{\nvar a:S = S()\na = S()\nreturn 0\n}\n";
        assert!(inspect(src).unwrap_err().message.contains("P16"));
    }

    #[test]
    fn 已存在_str_方法欄位寫入亦不可繞過_p16() {
        let src = "struct S{\nvar text:str = \"old\"\nfn change()->int{\nself.text = \"new\"\nreturn 0\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).unwrap_err().message.contains("P16"));
    }

    #[test]
    fn copyable_一般賦值與方法覆寫仍合法() {
        let src = "struct S{\nvar value:int = 1\nfn update()->int{\nself.value = 2\nreturn self.value\n}\n}\nfn main()->int{\nvar s:S = S()\nvar n:int = 1\nn = 2\nvar m:int = s.update()\nreturn m\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn str_移動後重新初始化保留已核准語意() {
        let src = "fn main()->int{\nvar s:str = \"original\"\nvar t:str = s.move()\ns = \"new\"\nvar result:str = s.move()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 同名自身_move_回寫暫緩至求值次序安全核准() {
        let src = "fn main()->int{\nvar n:int = 1\nn = n.move()\nreturn n\n}\n";
        assert!(inspect(src).unwrap_err().message.contains("同一名稱"));
    }

    #[test]
    fn 合流仍可能保有資源時禁止整筆覆寫() {
        let src = "fn main()->int{\nvar s:str = \"old\"\nif (true){\nvar taken:str = s.move()\n}\ns = \"new\"\nreturn 0\n}\n";
        assert!(inspect(src).unwrap_err().message.contains("可能"));
    }
}

#[cfg(test)]
mod p13_formal_partial_field_completion_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    const HEADER: &str = "struct S{\nvar x:int\nvar text:str\nconst flag:bool = true\n}\nfn main()->int{\nvar a:S = S()\n";

    #[test]
    fn p13_s07_var_逐欄首次初始化後方可整筆明確移動() {
        let source = format!("{HEADER}a.x = 2\na.text = \"text\"\nvar b:S = a.move()\nreturn b.x\n}}\n");
        assert!(inspect(&source).is_ok());
    }

    #[test]
    fn p13_s07_仍缺少一欄時不得讀取_複製_移動或呼叫方法() {
        let moved = format!("{HEADER}a.x = 2\nvar b:S = a.move()\nreturn 0\n}}\n");
        assert!(inspect(&moved).unwrap_err().message.contains("初始化"));
        let read = format!("{HEADER}a.x = 2\nreturn a.x\n}}\n");
        assert!(inspect(&read).unwrap_err().message.contains("初始化"));
        let copy = format!("{HEADER}a.x = 2\nvar b:S = a\nreturn 0\n}}\n");
        assert!(inspect(&copy).unwrap_err().message.contains("初始化"));
    }

    #[test]
    fn p13_s07_分支兩側同樣補齊欄位才能在合流後使用() {
        let both = format!(
            "{HEADER}if (true){{\na.x = 3\n}}else{{\na.x = 4\n}}\na.text = \"yes\"\nvar b:S = a.move()\nreturn b.x\n}}\n"
        );
        assert!(inspect(&both).is_ok());
        let divergent = format!(
            "{HEADER}if (true){{\na.x = 3\n}}else{{\na.text = \"str\"\n}}\na.x = 2\na.text = \"text\"\nreturn a.x\n}}\n"
        );
        let err = inspect(&divergent).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("初始化"));
    }

    #[test]
    fn p13_d02_const_部分實例不可透過欄位寫入補完() {
        let src = "struct S{\nvar n:int\n}\nfn main()->int{\nconst a:S = S()\na.n = 5\nreturn 0\n}\n";
        assert!(inspect(src).unwrap_err().message.contains("const"));
    }

    #[test]
    fn p13_s07_未建構的_var_宣告不可偽裝成可寫的結構實例() {
        let src = "struct S{\nvar n:int\n}\nfn main()->int{\nvar a:S\na.n = 5\nreturn 0\n}\n";
        assert!(inspect(src).unwrap_err().message.contains("初始化"));
    }

    #[test]
    fn p13_s07_已核准非_copyable_獨立字串可以連續安全覆寫() {
        let src = format!("{HEADER}a.x = 2\na.text = \"hi\"\na.text = \"bye\"\nreturn 0\n}}\n");
        assert!(inspect(&src).is_ok(),
            "字串右值先取得資源再覆寫，原所有者由 VA16-03-A 清理");
    }
}

#[cfg(test)]
mod p13_formal_field_write_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    const BODY: &str =
        "struct S{\nvar n:int = 1\nconst fixed:bool = true\nvar text:str = \"ok\"\n}\nfn main()->int{\nvar a:S = S()\n";

    #[test]
    fn p13_s13_s18_完整_var_接收者可覆寫_copyable_var_欄位() {
        let src = format!("{BODY}a.n = 2\nvar copy:int = a.n\nreturn copy\n}}\n");
        assert!(inspect(&src).is_ok());
        let in_branch = format!(
            "{BODY}if (true){{\na.n = 3\n}}else{{\na.n = 4\n}}\nreturn a.n\n}}\n"
        );
        assert!(inspect(&in_branch).is_ok());
    }

    #[test]
    fn p13_s18_const_實例及_const_欄位都不能寫入() {
        let const_owner = "struct S{\nvar n:int = 1\n}\nfn main()->int{\nconst a:S = S()\na.n = 2\nreturn 0\n}\n";
        let err = inspect(const_owner).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("S18"));
        let const_field = format!("{BODY}a.fixed = false\nreturn 0\n}}\n");
        assert!(inspect(&const_field).unwrap_err().message.contains("S18"));
    }

    #[test]
    fn p13_資源欄位覆寫依已核准_va16_03_獨立字串原子提交() {
        let src = format!("{BODY}a.text = \"new\"\nreturn 0\n}}\n");
        assert!(inspect(&src).is_ok(),
            "已核准 VA16-03-A 的新建 str 常值不再是未定清理責任");
        let unsafe_rhs = format!("{BODY}var source:str = \"new\"\na.text = source\nreturn 0\n}}\n");
        assert_eq!(inspect(&unsafe_rhs).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_移動後與分支不確定接收者仍拒絕_有建構基礎可首次補欄位() {
        let moved = format!(
            "{BODY}var b:S = a.move()\na.n = 2\nreturn 0\n}}\n"
        );
        assert!(inspect(&moved).unwrap_err().message.contains("已移動"));
        let uncertain = format!(
            "{BODY}if (true){{\nvar b:S = a.move()\n}}\na.n = 2\nreturn 0\n}}\n"
        );
        assert!(inspect(&uncertain).unwrap_err().message.contains("可能"));
        let partial = "struct S{\nvar n:int\n}\nfn main()->int{\nvar a:S = S()\na.n = 2\nreturn 0\n}\n";
        assert!(inspect(partial).is_ok());
    }

    #[test]
    fn p13_欄位型別不符與原來源仍受檢查() {
        let type_mismatch = format!("{BODY}a.n = false\nreturn 0\n}}\n");
        assert!(inspect(&type_mismatch).unwrap_err().message.contains("型別"));
        let missing = format!("{BODY}a.other = 5\nreturn 0\n}}\n");
        assert!(inspect(&missing).unwrap_err().message.contains("不存在"));
        let source_move = format!("{BODY}var n:int = 5\na.n = n.move()\nreturn n\n}}\n");
        assert!(inspect(&source_move).unwrap_err().message.contains("已移動"));
    }
}

#[cfg(test)]
mod p13_formal_implicit_move_matrix_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    fn forbidden_copy(code: &str) {
        let error = inspect(code).expect_err("已繫結 str 的隱含複製不得被當成移動");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable 已明確禁止"),
            "應清楚區分明確禁止與未決：{}", error.message);
        assert!(error.span.is_some());
    }

    #[test]
    fn p13_2_宣告初始化不允許把_str_名稱當成隱式移動() {
        forbidden_copy("fn main()->int{\nvar a:str = \"x\"\nvar b:str = a\nreturn 0\n}\n");
    }

    #[test]
    fn p13_2_重新賦值不允許把_str_名稱當成隱式移動() {
        forbidden_copy("fn main()->int{\nvar a:str = \"x\"\nvar b:str\nb = a\nreturn 0\n}\n");
    }

    #[test]
    fn p13_2_函式回傳已繫結_str_必須明確_move() {
        forbidden_copy("fn give()->str{\nvar a:str = \"x\"\nreturn a\n}\nfn main()->int{\nreturn 0\n}\n");
    }

    #[test]
    fn p13_2_結構具名欄位建立不得隱式移動_str() {
        forbidden_copy("struct Holder{\nvar value:str\n}\nfn main()->int{\nvar a:str = \"x\"\nvar h:Holder = Holder(value=a)\nreturn 0\n}\n");
    }

    #[test]
    fn p13_3_明確整筆_move_在宣告賦值回傳及具名建構仍合法() {
        let sources = [
            "fn main()->int{\nvar a:str = \"x\"\nvar b:str = a.move()\nreturn 0\n}\n",
            "fn main()->int{\nvar a:str = \"x\"\nvar b:str\nb = a.move()\nreturn 0\n}\n",
            "fn give()->str{\nvar a:str = \"x\"\nreturn a.move()\n}\nfn main()->int{\nreturn 0\n}\n",
            "struct Holder{\nvar value:str\n}\nfn main()->int{\nvar a:str = \"x\"\nvar h:Holder = Holder(value=a.move())\nreturn 0\n}\n",
        ];
        for source in sources {
            assert!(inspect(source).is_ok(), "合法的具名整筆明確移動不應被誤拒：{source}");
        }
    }

    #[test]
    fn p13_4_建構引數明確移動後原來源不得二次移動() {
        let source = "struct Holder{\nvar value:str\n}\nfn main()->int{\nvar a:str = \"x\"\nvar h:Holder = Holder(value=a.move())\nvar again:str = a.move()\nreturn 0\n}\n";
        let error = inspect(source).expect_err("已移動來源不得二次消耗");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("已移動"));
        assert!(error.span.is_some());
    }
}

#[cfg(test)]
mod p13_d02_const_partial_safety_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    fn partial() -> &'static str {
        include_str!("../../tests/fixtures/p13_stage/D02_const結構允許保留未初始化var欄位.voil")
    }

    #[test]
    fn d02_a_const_實例可以建立但不能誤判var欄位全部初始化() {
        assert!(inspect(partial()).is_ok());
    }

    #[test]
    fn d02_a_const_來源未完整初始化禁止欄位讀取() {
        let bad = partial().replace("return 0", "var copy:int = item.value\nreturn 0");
        let err = inspect(&bad).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("初始化"));
        assert!(err.span.is_some());
    }

    #[test]
    fn d02_a_const_來源未完整初始化即使全部欄位copyable也禁止隱含複製() {
        let bad = partial().replace("return 0", "var duplicate:Sample = item\nreturn 0");
        assert!(inspect(&bad).unwrap_err().message.contains("初始化"));
    }

    #[test]
    fn d02_a_const_即使欄位有預設值_未完整初始化結構仍不得讀取欄位() {
        let bad = partial().replace("return 0", "var flag:bool = item.ready\nreturn 0");
        let err = inspect(&bad).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("初始化"));
        assert!(err.span.is_some());
    }

    #[test]
    fn d02_a_const_不完整來源不可透過新的_const_綁定偷渡複製() {
        let bad = partial().replace("return 0", "const alias:Sample = item\nreturn 0");
        let err = inspect(&bad).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("初始化"));
        assert!(err.span.is_some());
    }

    #[test]
    fn d02_a_const_來源未完整初始化禁止整筆_move() {
        let bad = partial().replace("return 0", "var moved:Sample = item.move()\nreturn 0");
        assert!(inspect(&bad).unwrap_err().message.contains("初始化"));
    }

    #[test]
    fn d02_a_const_來源未完整初始化禁止方法借用讀取() {
        let bad = include_str!("../../tests/fixtures/p13_stage/D02_const結構未初始化欄位不得藉方法使用.voil");
        assert!(inspect(bad).unwrap_err().message.contains("初始化"));
    }

    #[test]
    fn d02_a_const_來源不得透過賦值或欄位寫入補足初始化() {
        let reassignment = partial().replace("return 0", "item = Sample(value=9)\nreturn 0");
        assert!(inspect(&reassignment).unwrap_err().message.contains("const"));
        let field_write = partial().replace("return 0", "item.value = 9\nreturn 0");
        assert!(inspect(&field_write).unwrap_err().message.contains("const"));
    }

    #[test]
    fn d02_a_已完整初始化的_const_結構仍可安全複製讀取及使用唯讀方法() {
        let positive =
            include_str!("../../tests/fixtures/p13_stage/D02_const結構完整初始化允許安全使用.voil");
        assert!(inspect(positive).is_ok());
    }

    #[test]
    fn d02_a_完整初始化_const_可讀取有預設值欄位並合法複製至另一_const() {
        let ready = include_str!(
            "../../tests/fixtures/p13_stage/D02_const結構完整初始化允許安全使用.voil"
        );
        let positive = ready.replace(
            "return result",
            "const alias:Sample = item\nvar flag:bool = alias.ready\nreturn result"
        );
        assert!(inspect(&positive).is_ok());
    }

    #[test]
    fn d02_a_const_須有建立式且const欄位仍須提供預設值() {
        let no_init = partial().replace("const item:Sample = Sample()", "const item:Sample");
        assert!(inspect(&no_init).unwrap_err().message.contains("const"));
        let no_default = partial().replace("const ready:bool = true", "const ready:bool");
        assert!(inspect(&no_default).unwrap_err().message.contains("const"));
    }
}

#[cfg(test)]
mod p13_完整來源閉包推進測試 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_2_已核准_copyable_一般賦值及函式回傳不需隱式移動() {
        let cases = [
            "fn main()->int{\nvar a:int = 1\nvar b:int = a\nreturn a\n}\n",
            "fn main()->int{\nvar a:bool = true\nvar b:bool = a\nreturn 0\n}\n",
            "fn give()->int{\nvar a:int = 7\nreturn a\n}\nfn main()->int{\nreturn 0\n}\n",
        ];
        for source in cases {
            assert!(inspect(source).is_ok(), "合法 Copyable 來源不應誤拒：{source}");
        }
    }

    #[test]
    fn p13_3_明確移動後_原來源不可再次作為隱含複製或移動() {
        let suffixes = [
            "var again:str = a",
            "var again:str = a.move()",
        ];
        for suffix in suffixes {
            let source=format!(
                "fn main()->int{{\nvar a:str = \"原始\"\nvar b:str = a.move()\n{suffix}\nreturn 0\n}}\n"
            );
            let error=inspect(&source).expect_err("移動後來源不可再次使用");
            assert_eq!(error.kind,DiagnosticKind::Static);
            assert!(error.span.is_some());
        }
    }

    #[test]
    fn p13_4_移動後重新初始化可以再次明確移動_但不得隱含複製() {
        let valid="fn main()->int{\nvar a:str = \"原始\"\nvar b:str = a.move()\na = \"重新初始化\"\nvar c:str = a.move()\nreturn 0\n}\n";
        assert!(inspect(valid).is_ok());
        let invalid=valid.replace("var c:str = a.move()", "var c:str = a");
        let error=inspect(&invalid).expect_err("str 重新初始化後仍禁止隱含複製");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"),"{}",error.message);
    }
}

#[cfg(test)]
mod p13_分支合流所有權來源矩陣 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_4_單側移動合流後不能讀取或再移動來源() {
        for operation in ["var copy:str = source", "var taken:str = source.move()"] {
            let source=format!(
                "fn main()->int{{\nvar source:str = \"text\"\nif (true){{\nvar inner:str = source.move()\n}}else{{\nvar keep:int = 1\n}}\n{operation}\nreturn 0\n}}\n"
            );
            let error=inspect(&source).expect_err("單側移動後合流不得視為仍持有");
            assert_eq!(error.kind,DiagnosticKind::Static);
            assert!(error.span.is_some());
        }
    }

    #[test]
    fn p13_4_雙側移動合流後仍不得再次消耗() {
        let source="fn main()->int{\nvar source:str = \"text\"\nif (true){\nvar left:str = source.move()\n}else{\nvar right:str = source.move()\n}\nvar after:str = source.move()\nreturn 0\n}\n";
        let error=inspect(source).expect_err("兩側皆移動後不得重用");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.span.is_some());
    }

    #[test]
    fn p13_4_分支內合法_copyable_讀取不應令合流來源失效() {
        let source="fn main()->int{\nvar original:int = 3\nif (true){\nvar left:int = original\n}else{\nvar right:int = original\n}\nreturn original\n}\n";
        assert!(inspect(source).is_ok());
    }
}

#[cfg(test)]
mod p13_s19_s21_結構所有權來源矩陣 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_6_結構整體複製僅在全部欄位可複製時合法() {
        let valid="struct S{\nvar n:int = 1\nconst flag:bool = true\n}\nfn main()->int{\nvar original:S = S()\nvar copy:S = original\nreturn original.n\n}\n";
        assert!(inspect(valid).is_ok());
        let invalid="struct S{\nvar n:int = 1\nvar text:str = \"resource\"\n}\nfn main()->int{\nvar original:S = S()\nvar copy:S = original\nreturn 0\n}\n";
        let error=inspect(invalid).expect_err("含非 Copyable 欄位的結構不可整筆隱含複製");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.span.is_some());
    }

    #[test]
    fn p13_6_禁止從結構個別欄位明確移動() {
        let source="struct S{\nvar text:str = \"resource\"\n}\nfn main()->int{\nvar owner:S = S()\nvar stolen:str = owner.text.move()\nreturn 0\n}\n";
        let error=inspect(source).expect_err("S21 不允許只移動結構單一欄位");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.span.is_some());
    }
}

#[cfg(test)]
mod p13_5_資源與可複製值來源對照矩陣 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_5_資源_str_允許明確整筆移動_但不允許隱含複製() {
        let allowed="fn main()->int{\nvar resource:str = \"resource\"\nvar transferred:str = resource.move()\nreturn 0\n}\n";
        assert!(inspect(allowed).is_ok());
        let forbidden="fn main()->int{\nvar resource:str = \"resource\"\nvar duplicated:str = resource\nreturn 0\n}\n";
        let error=inspect(forbidden).expect_err("資源不得透過一般 = 隱含複製");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }

    #[test]
    fn p13_5_可複製值_允許複製與明確移動_移動後來源失效() {
        let copied="fn main()->int{\nvar source:int = 3\nvar copied:int = source\nreturn source\n}\n";
        assert!(inspect(copied).is_ok());
        let moved="fn main()->int{\nvar source:int = 3\nvar transferred:int = source.move()\nreturn source\n}\n";
        let error=inspect(moved).expect_err("即使是 Copyable，明確移動仍須使來源失效");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.span.is_some());
    }

    #[test]
    fn p13_5_資源重新初始化後可再次明確移動() {
        let source="fn main()->int{\nvar resource:str = \"first\"\nvar first:str = resource.move()\nresource = \"second\"\nvar second:str = resource.move()\nreturn 0\n}\n";
        assert!(inspect(source).is_ok());
    }
}

#[cfg(test)]
mod p13_函式參數來源安全 {
    use super::inspect;

    #[test]
    fn p13_可複製型別參數允許函式內讀取與回傳() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_非可複製參數須由明確移交的呼叫端供應() {
        let src="fn identity(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar source:str = \"hello\"\nvar result:str = identity(source.move())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_參數名稱不可重複() {
        let src="fn identity(value:int,value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).is_err());
    }
}

#[cfg(test)]
mod p13_函式參數型別閉包 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_函式參數不得以不存在的型別建立虛構所有權() {
        let src="fn identity(value:Missing)->int{\nreturn 0\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("參數型別必須在現有型別環境中可解析");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("不存在的型別"));
    }

    #[test]
    fn p13_可複製的已宣告結構可作為受限函式參數() {
        let src="struct Item{\nvar n:int = 1\n}\nfn identity(value:Item)->int{\nreturn value.n\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_一般函式呼叫解析邊界 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_位置引數呼叫通過已驗證_copyable_路徑() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn identity(1)\n}\n";
        assert!(inspect(src).is_ok(), "已核准 Copyable 函式呼叫應通過完整引數檢查");
    }

    #[test]
    fn p13_位置引數巢狀移動不得繞過方法禁令() {
        let src="struct Item{\nvar n:int = 1\nfn value()->int{\nreturn identity(self.move())\n}\n}\nfn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("方法中巢狀 self.move 不得通過");
        assert_eq!(error.kind,DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p13_可複製函式呼叫 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_可複製參數與回傳可安全呼叫() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn identity(1)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_巢狀可複製呼叫可通過() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn identity(identity(1))\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_呼叫參數數量不符必須拒絕() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn identity()\n}\n";
        let err=inspect(src).expect_err("引數數量不符");
        assert_eq!(err.kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_呼叫參數型別不符必須拒絕() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn identity(true)\n}\n";
        let err=inspect(src).expect_err("引數型別不符");
        assert_eq!(err.kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_資源參數呼叫不得隱含複製() {
        let src="fn identity(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar source:str = \"hello\"\nvar result:str = identity(source)\nreturn 0\n}\n";
        assert!(inspect(src).is_err());
    }
}

#[cfg(test)]
mod p13_函式簽章唯一性 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_重名函式不可讓呼叫端選擇錯誤簽章() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn identity(value:bool)->bool{\nreturn value\n}\nfn main()->int{\nreturn identity(1)\n}\n";
        let error=inspect(src).expect_err("重名函式不得被任意挑選");
        assert_eq!(error.kind,DiagnosticKind::Syntax);
        assert!(error.message.contains("不得重複宣告函式名稱"));
    }

    #[test]
    fn p13_main_不可接受未經入口供應的參數() {
        let src="fn main(value:int)->int{\nreturn value\n}\n";
        let error=inspect(src).expect_err("main 參數不可被視為已初始化來源");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("main 入口不可宣告參數"));
    }

    #[test]
    fn p13_唯一函式簽章與合法入口仍可呼叫() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn identity(1)\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_跨函式資源移交 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_資源參數明確移動並由函式回傳() {
        let src="fn take(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar item:str = \"hello\"\nvar received:str = take(item.move())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_移交後原來源不得再用() {
        let src="fn take(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar item:str = \"hello\"\nvar received:str = take(item.move())\nvar invalid:str = item.move()\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_非_copyable_具名引數不能隱含複製() {
        let src="fn take(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar item:str = \"hello\"\nvar invalid:str = take(item)\nreturn 0\n}\n";
        let error=inspect(src).expect_err("被呼叫端須移動資源，不能只把來源借出");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("P15.6"));
    }

    #[test]
    fn p13_資源函式呼叫結果可再移交() {
        let src="fn take(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar item:str = \"hello\"\nvar received:str = take(take(item.move()))\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_跨函式資源分支合流 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_單側移交後合流不得再次使用來源() {
        let src="fn take(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar source:str = \"hello\"\nif (true){\nvar result:str = take(source.move())\n}\nvar invalid:str = source.move()\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_雙分支皆移交後合流不得再次使用來源() {
        let src="fn take(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar source:str = \"hello\"\nif (true){\nvar result:str = take(source.move())\n}else{\nvar result:str = take(source.move())\n}\nvar invalid:str = source.move()\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_分支內遮蔽資源移交不影響外層來源() {
        let src="fn take(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar source:str = \"outer\"\nif (true){\nvar source:str = \"inner\"\nvar result:str = take(source.move())\n}\nvar result:str = take(source.move())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_雙分支重新初始化後可再次移交() {
        let src="fn take(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar source:str = \"hello\"\nif (true){\nvar result:str = take(source.move())\nsource = \"again\"\n}else{\nvar result:str = take(source.move())\nsource = \"again\"\n}\nvar result:str = take(source.move())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_函式呼叫所有權預檢 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_非可複製參數唯讀預設借用不隱含複製() {
        let src="fn consume(value:str)->int{\nreturn 0\n}\nfn main()->int{\nvar source:str = \"hello\"\nreturn consume(source)\n}\n";
        assert!(inspect(src).is_ok(), "已核准函式參數預設借用，來源不需複製");
    }

    #[test]
    fn p13_呼叫引數數量錯誤不應消耗資源() {
        let src="fn consume(value:str)->int{\nreturn 0\n}\nfn main()->int{\nvar source:str = \"hello\"\nreturn consume(source.move(),source.move())\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_明確資源移交與可複製參數可混用() {
        let src="fn consume(value:str,count:int)->int{\nreturn count\n}\nfn main()->int{\nvar source:str = \"hello\"\nreturn consume(source.move(),1)\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_零引數一般函式 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_零參數函式可回傳可複製值() {
        let src="fn answer()->int{\nreturn 42\n}\nfn main()->int{\nreturn answer()\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_零參數函式可建立獨立資源() {
        let src="fn make()->str{\nreturn \"hello\"\n}\nfn main()->int{\nvar result:str = make()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_缺少函式引數必須拒絕() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn identity()\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p13_巢狀結構資源正反來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    const PREFIX: &str = "struct Inner{\nvar content:str = \"hello\"\n}\nstruct Outer{\nvar inner:Inner = Inner()\n}\n";

    #[test]
    fn p13_巢狀非可複製結構可明確整筆移交() {
        let source=format!("{PREFIX}fn main()->int{{\nvar source:Outer = Outer()\nvar target:Outer = source.move()\nreturn 0\n}}\n");
        assert!(inspect(&source).is_ok());
    }

    #[test]
    fn p13_巢狀資源不可隱含複製() {
        let source=format!("{PREFIX}fn main()->int{{\nvar source:Outer = Outer()\nvar target:Outer = source\nreturn 0\n}}\n");
        assert_eq!(inspect(&source).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_巢狀資源移交後不得再次使用() {
        let source=format!("{PREFIX}fn main()->int{{\nvar source:Outer = Outer()\nvar target:Outer = source.move()\nvar again:Outer = source.move()\nreturn 0\n}}\n");
        assert_eq!(inspect(&source).unwrap_err().kind,DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p13_h_巢狀函式資源來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_h_巢狀函式可逐層移交新建資源() {
        let src="fn make()->str{\nreturn \"hello\"\n}\nfn forward(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar owned:str = forward(make())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_h_巢狀函式不可隱含複製資源() {
        let src="fn forward(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar owned:str = \"hello\"\nvar next:str = forward(owned)\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_h_巢狀函式移交後不得再次使用來源() {
        let src="fn forward(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar owned:str = \"hello\"\nvar next:str = forward(owned.move())\nvar invalid:str = owned.move()\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_h_巢狀函式引數型別必須匹配() {
        let src="fn forward(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar owned:str = forward(1)\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p13_i_方法與_self_所有權來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_i_合法唯讀方法複製_copyable_欄位() {
        let src="struct Counter{\nvar count:int = 1\nfn get()->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.get()\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_i_禁止方法移動整個_self() {
        let src="struct Counter{\nvar count:int = 1\nfn take()->int{\nreturn self.move()\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).unwrap_err();
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("S15"));
    }

    #[test]
    fn p13_i_禁止方法回傳非_copyable_self_欄位() {
        let src="struct Resource{\nvar content:str = \"hello\"\nfn get()->str{\nreturn self.content\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_i_禁止方法單欄位移動() {
        let src="struct Counter{\nvar count:int = 1\nfn take()->int{\nreturn self.count.move()\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).unwrap_err();
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("S21"));
    }
}

#[cfg(test)]
mod p13_j_分支移交與重新初始化 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_j_兩條分支皆移交來源則合流不可再次使用() {
        let src="fn main()->int{\nvar source:str = \"hello\"\nif (true){\nvar left:str = source.move()\n}else{\nvar right:str = source.move()\n}\nvar invalid:str = source.move()\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_j_單側移交後合流不可假定來源有效() {
        let src="fn main()->int{\nvar source:str = \"hello\"\nif (true){\nvar left:str = source.move()\n}\nvar invalid:str = source.move()\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_j_分支合流後重新初始化恢復有效所有權() {
        let src="fn main()->int{\nvar source:str = \"hello\"\nif (true){\nvar left:str = source.move()\n}else{\nvar right:str = source.move()\n}\nsource = \"again\"\nvar valid:str = source.move()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_j_分支區域變數不得逃逸作用域() {
        let src="fn main()->int{\nif (true){\nvar local:int = 1\n}\nreturn local\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p13_函式參數綁定唯一性 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 重複_copyable_參數名稱必須靜態拒絕() {
        let src="fn same(value:int,value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).unwrap_err();
        assert_eq!(error.kind,DiagnosticKind::Syntax);
        assert!(error.message.contains("函式參數名稱不得重複"));
    }

    #[test]
    fn 重複資源參數名稱不可掩蓋持有者() {
        let src="fn same(value:str,value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).unwrap_err();
        assert_eq!(error.kind,DiagnosticKind::Syntax);
        assert!(error.message.contains("函式參數名稱不得重複"));
    }

    #[test]
    fn 不同名稱的資源參數可各自明確移交() {
        let src="fn choose(first:str,second:str)->str{\nreturn first.move()\n}\nfn main()->int{\nvar left:str = \"left\"\nvar right:str = \"right\"\nvar result:str = choose(left.move(),right.move())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_函式與方法回傳型別封閉 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 未呼叫的函式也不得宣告不存在的回傳型別() {
        let src="fn unused()->Unknown{\nreturn 1\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).unwrap_err();
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("函式回傳型別不存在"));
    }

    #[test]
    fn 未呼叫的方法也不得宣告不存在的回傳型別() {
        let src="struct Boxed{\nvar value:int = 1\nfn get()->Unknown{\nreturn self.value\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).unwrap_err();
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("方法回傳型別不存在"));
    }

    #[test]
    fn 私有正式函式純值返回不得繞過公私共同裁決() {
        let positive = "fn flag()->bool{\nreturn true\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(positive).is_ok());
        for src in [
            "fn main()->int{\nreturn true\n}\n",
            "fn flag()->bool{\nreturn 1\n}\nfn main()->int{\nreturn 0\n}\n",
            "fn main()->int{\nvar value:int\nreturn value\n}\n",
        ] {
            let error = inspect(src).expect_err("錯誤型別或未初始化的回傳必須拒絕");
            assert_eq!(error.kind, DiagnosticKind::Static);
            assert!(error.message.contains("回傳") || error.message.contains("初始化"));
        }
    }

    #[test]
    fn 私有方法純值回傳與資源返回各守原核准界線() {
        let valid="struct Status{\nvar ok:bool = true\nfn read()->bool{\nreturn self.ok\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(valid).is_ok());
        let wrong="struct Status{\nvar ok:bool = true\nfn read()->int{\nreturn self.ok\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let err=inspect(wrong).expect_err("bool 欄位不可當成 int 返回");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("回傳型別"));
    }

    #[test]
    fn 合法結構回傳型別保持允許整筆移交() {
        let src="struct Boxed{\nvar value:int = 1\n}\nfn make()->Boxed{\nreturn Boxed()\n}\nfn main()->int{\nvar item:Boxed = make()\nreturn item.value\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_方法回傳與借用權限交叉來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 合法唯讀方法回傳_copyable_欄位() {
        let src="struct Counter{\nvar count:int = 1\nfn read()->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar counter:Counter = Counter()\nreturn counter.read()\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 不可寫接收者不得呼叫寫入方法() {
        let src="struct Counter{\nvar count:int = 1\nfn reset()->int{\nself.count = 0\nreturn self.count\n}\n}\nfn main()->int{\nconst counter:Counter = Counter()\nreturn counter.reset()\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn 非_copyable_方法唯讀參數合法宣告且具名資源預設借用() {
        let src="struct Counter{\nvar count:int = 1\nfn add(amount:str)->int{\nreturn self.count\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
        let borrowed=src.replace("return 0\n}\n",
            "var value:str = \"資源\"\nconst counter:Counter = Counter()\nreturn counter.add(value)\n}\n");
        assert!(inspect(&borrowed).is_ok(), "預設借用不得被誤當成 str 隱含複製");
    }
}

#[cfg(test)]
mod p13_函式呼叫簽章與資源移交交叉驗證 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 函式呼叫不得引用不存在的名稱() {
        let src="fn main()->int{\nreturn missing()\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn 資源引數普通名稱可預設借用但不隱含複製() {
        let src="fn use_it(item:str)->int{\nreturn 1\n}\nfn main()->int{\nvar item:str = \"x\"\nreturn use_it(item)\n}\n";
        assert!(inspect(src).is_ok(), "函式唯讀借用不是建立第二所有者");
    }

    #[test]
    fn 資源引數明確移交後不可再次使用() {
        let src="fn use_it(item:str)->int{\nreturn 1\n}\nfn main()->int{\nvar item:str = \"x\"\nvar result:int = use_it(item.move())\nvar again:str = item.move()\nreturn result\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn 資源引數明確移交與合法回傳保持可用() {
        let src="fn use_it(item:str)->int{\nreturn 1\n}\nfn main()->int{\nvar item:str = \"x\"\nreturn use_it(item.move())\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_預設建構巢狀函式引數循環 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 函式引數中的預設結構建構循環必須拒絕() {
        let src="struct Loop{\nvar next:Loop = Loop()\n}\nfn identity(item:Loop)->Loop{\nreturn item.move()\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).unwrap_err();
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("循環"));
    }

    #[test]
    fn 合法非循環的巢狀預設建構仍可使用() {
        let src="struct Leaf{\nvar count:int = 1\n}\nstruct Holder{\nvar leaf:Leaf = Leaf()\n}\nfn main()->int{\nvar holder:Holder = Holder()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_預設值來源跨型別循環 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 兩種結構的預設建構互相依賴必須拒絕() {
        let src="struct Alpha{\nvar other:Beta = Beta()\n}\nstruct Beta{\nvar other:Alpha = Alpha()\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).unwrap_err();
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("循環"));
    }

    #[test]
    fn 未使用的非循環預設結構可以宣告() {
        let src="struct Leaf{\nvar value:int = 1\n}\nstruct Outer{\nvar inner:Leaf = Leaf()\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_1_copyable_完整能力分類 {
    use super::{Checker, Parser};
    use crate::ownership_model::CopyPermission;
    use crate::ownership_policy::approved_leaf_copy_permission;
    use crate::source::Source;

    fn permission(src: &str, ty: &str) -> CopyPermission {
        let source = Source::new(src.as_bytes().to_vec());
        let tokens = crate::lexer::scan(&source);
        let (structs, functions) = Parser { source: &source, tokens: &tokens.tokens, cursor: 0 }.file().expect("來源語法合法");
        let checker = Checker {
            source: &source, structs: &structs, functions: &functions,
            function_index: functions.iter().enumerate().map(|(i, f)| (f.name.clone(), i)).collect(), ability: approved_leaf_copy_permission,
            recorded_function_exits: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_edges: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_method_safety: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_method_safety_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_direct_moves: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declarations: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_types: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declaration_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_named_returns: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_field_writes: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_call_borrows: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_call_borrow_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_scalar_hir: std::cell::RefCell::new(
                crate::common_ir::VerifiedScalarHir::default())
        };
        checker.permission(ty)
    }

    #[test]
    fn 已核准葉型別及容器各自維持三態() {
        let src="fn main()->int{\nreturn 0\n}\n";
        for ty in ["int", "bool", "float"] {
            assert_eq!(permission(src,ty),CopyPermission::Allowed);
        }
        assert_eq!(permission(src,"str"),CopyPermission::Forbidden);
        for ty in ["list[int]","dict{int,bool}","list[list[float]]","Unknown"] {
            assert_eq!(permission(src,ty),CopyPermission::Undecided);
        }
    }

    #[test]
    fn 巢狀結構全欄位允許時才允許複製() {
        let src="struct Leaf{\nvar value:float = 1.0\n}\nstruct Middle{\nvar leaf:Leaf = Leaf()\n}\nstruct Root{\nvar middle:Middle = Middle()\n}\nfn main()->int{\nreturn 0\n}\n";
        for ty in ["Leaf","Middle","Root"] {
            assert_eq!(permission(src,ty),CopyPermission::Allowed);
        }
    }

    #[test]
    fn 巢狀結構含資源欄位整體明確禁止複製() {
        let src="struct Resource{\nvar text:str = \"x\"\n}\nstruct Wrapper{\nvar resource:Resource = Resource()\n}\nfn main()->int{\nreturn 0\n}\n";
        for ty in ["Resource","Wrapper"] {
            assert_eq!(permission(src,ty),CopyPermission::Forbidden);
        }
    }
}

#[cfg(test)]
mod p13_1_正式來源閉包 {
    use super::{inspect, Checker, Parser};
    use crate::ownership_model::CopyPermission;
    use crate::ownership_policy::approved_leaf_copy_permission;
    use crate::{DiagnosticKind, lexer, source::Source};

    fn ability(src: &str, ty: &str) -> CopyPermission {
        let source = Source::new(src.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        assert!(tokens.diagnostics.is_empty());
        let (structs, functions) = Parser {
            source: &source, tokens: &tokens.tokens, cursor: 0,
        }.file().expect("來源語法應可解析");
        Checker {
            source: &source, structs: &structs, functions: &functions,
            function_index: functions.iter().enumerate().map(|(i, f)| (f.name.clone(), i)).collect(), ability: approved_leaf_copy_permission,
            recorded_function_exits: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_edges: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_control_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_method_safety: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_method_safety_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_direct_moves: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declarations: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_types: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_member_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_declaration_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_named_returns: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_field_writes: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_call_borrows: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            recorded_call_borrow_conflicts: std::cell::RefCell::new(std::collections::BTreeSet::new()),
            recorded_scalar_hir: std::cell::RefCell::new(
                crate::common_ir::VerifiedScalarHir::default())
        }.permission(ty)
    }

    #[test]
    fn 全部欄位可複製的三層結構可實際隱含複製() {
        let src = "struct Leaf{\nvar amount:float = 1.0\n}\nstruct Middle{\nvar leaf:Leaf = Leaf()\n}\nstruct Root{\nvar middle:Middle = Middle()\n}\nfn main()->int{\nvar first:Root = Root()\nvar second:Root = first\nvar third:Root = first\nreturn 0\n}\n";
        assert_eq!(ability(src, "Root"), CopyPermission::Allowed);
        assert!(inspect(src).is_ok(), "可複製來源的多次使用必須合法");
    }

    #[test]
    fn 三層結構內含字串時整體確定禁止隱含複製() {
        let src = "struct Leaf{\nvar title:str = \"文字\"\n}\nstruct Middle{\nvar leaf:Leaf = Leaf()\n}\nstruct Root{\nvar middle:Middle = Middle()\n}\nfn main()->int{\nvar first:Root = Root()\nvar second:Root = first\nreturn 0\n}\n";
        assert_eq!(ability(src, "Root"), CopyPermission::Forbidden);
        let error = inspect(src).expect_err("非 Copyable 結構不得隱含複製");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }

    #[test]
    fn 容器未決與未知型別不誤報為永久不可複製() {
        let src = "fn main()->int{\nreturn 0\n}\n";
        for ty in ["list[int]", "list[list[float]]", "dict{str,int}", "Optional[int]", "Unknown"] {
            assert_eq!(ability(src, ty), CopyPermission::Undecided, "{ty}");
        }
        assert_eq!(ability(src, "str"), CopyPermission::Forbidden);
        for ty in ["int", "bool", "float"] {
            assert_eq!(ability(src, ty), CopyPermission::Allowed);
        }
    }
}

#[cfg(test)]
mod p13_p14_函式參數真實宣告身分 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 借用第一參數不應封鎖另一個獨立參數() {
        let src = "fn calculate(first:int, second:int)->int{\nconst borrowed:int = first.borrow()\nvar other:int = second\nreturn borrowed\n}\nfn main()->int{\nreturn calculate(1, 2)\n}\n";
        assert!(inspect(src).is_ok(), "不同參數的來源身分必須各自獨立");
    }

    #[test]
    fn 借用第一參數時仍禁止直接使用同一參數() {
        let src = "fn calculate(first:int, second:int)->int{\nconst borrowed:int = first.borrow()\nvar other:int = first\nreturn borrowed\n}\nfn main()->int{\nreturn calculate(1, 2)\n}\n";
        let error=inspect(src).expect_err("來源仍被借用時應拒絕直接使用");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }
}

#[cfg(test)]
mod p13_2_一般加減及所有權來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 已核准括號加減可使用多個可複製來源() {
        let src = "fn sum(x:int, y:int)->int{\nvar first:int = (x + y) - 1\nreturn first + (y - 2)\n}\nfn main()->int{\nreturn sum(4, 5)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 明確移動值參與加減後來源必須失效() {
        let src = "fn main()->int{\nvar number:int = 3\nvar result:int = number.move() + 1\nreturn number\n}\n";
        let error=inspect(src).expect_err("明確移交的 int 來源已失效");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("移動"));
    }

    #[test]
    fn 加減不能藉括號隱含複製字串資源() {
        let src = "fn main()->int{\nvar word:str = \"資源\"\nvar number:int = (word + 1)\nreturn number\n}\n";
        let error=inspect(src).expect_err("非 Copyable 來源不得隱含複製");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }

    #[test]
    fn 加減不能默許布林型別混入() {
        let src = "fn main()->int{\nreturn true + 2\n}\n";
        let error=inspect(src).expect_err("型別不符合 int 加減");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("int"));
    }
}

#[cfg(test)]
mod p13_2_有參數合法方法來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn copyable_參數可寫入_var_self_欄位並返回獨立值() {
        let src = "struct Counter{\nvar count:int = 1\nfn replace(value:int)->int{\nself.count = value\nreturn self.count\n}\n}\nfn main()->int{\nvar counter:Counter = Counter()\nreturn counter.replace(3)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 唯讀方法可回傳_copyable_參數() {
        let src = "struct Sample{\nvar number:int = 0\nfn choose(value:int)->int{\nreturn value\n}\n}\nfn main()->int{\nconst sample:Sample = Sample()\nreturn sample.choose(7)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn copyable_參數錯誤型別必須在來源檢查階段拒絕() {
        let src = "struct Sample{\nvar number:int = 0\nfn choose(value:int)->int{\nreturn value\n}\n}\nfn main()->int{\nvar sample:Sample = Sample()\nreturn sample.choose(true)\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind, DiagnosticKind::Static);
    }

    #[test]
    fn const_方法接收者不得透過參數修改_self() {
        let src = "struct Counter{\nvar count:int = 1\nfn replace(value:int)->int{\nself.count = value\nreturn self.count\n}\n}\nfn main()->int{\nconst counter:Counter = Counter()\nreturn counter.replace(3)\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind, DiagnosticKind::Static);
    }

    #[test]
    fn 資源方法參數使用已核准擁有權移交時可合法宣告() {
        let src = "struct Holder{\nvar count:int = 0\nfn use_text(text:str)->int{\nreturn 1\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}


// P16.6-R05：索引僅加速名稱查找，不可重排欄位宣告及允許重複名稱。
#[cfg(test)]
mod p16_6_r05_定義索引安全回歸 {
    use super::*;

    #[test]
    fn 大量欄位與方法可按名稱找到且保留宣告順序() {
        let mut text = String::from("struct Many{\n");
        for i in 0..96 {
            text.push_str(&format!("var field_{i}:int = {i}\n"));
        }
        for i in 0..24 {
            text.push_str(&format!("fn method_{i}()->int{{\nreturn self.field_{i}\n}}\n"));
        }
        text.push_str("}\nfn main()->int{\nreturn 0\n}\n");
        let source = Source::new(text.into_bytes());
        let lexed = lexer::scan(&source);
        assert!(lexed.diagnostics.is_empty());
        let (decls, _) = Parser { source: &source, tokens: &lexed.tokens, cursor: 0 }
            .file().expect("欄位及方法應能正確解析");
        let definition = decls.get("Many").expect("結構應存在");
        assert_eq!(definition.fields.len(), 96);
        assert_eq!(definition.methods.len(), 24);
        assert_eq!(definition.field("field_95").unwrap().name, "field_95");
        assert_eq!(definition.field("field_0").unwrap().name, "field_0");
        assert_eq!(definition.method("method_23").unwrap().name, "method_23");
        assert!(definition.field("field_96").is_none());
        assert!(definition.method("method_24").is_none());
        assert_eq!(definition.fields[0].name, "field_0");
        assert_eq!(definition.fields[95].name, "field_95");
    }

    #[test]
    fn 具相同欄位名稱仍按原有解析錯誤拒絕() {
        let input = "struct S{\nvar n:int = 1\nvar n:int = 2\n}\nfn main()->int{\nreturn 0\n}\n";
        let source = Source::new(input.as_bytes().to_vec());
        let lexed = lexer::scan(&source);
        assert!(lexed.diagnostics.is_empty());
        let error = Parser { source: &source, tokens: &lexed.tokens, cursor: 0 }
            .file().unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Syntax);
        assert!(error.span.is_some());
    }
}

#[cfg(test)]
mod f07_全後繼詞彙使用身分計畫 {
    use super::*;

    fn scan(code: &str) -> (BTreeSet<DeclarationId>, DeclarationId, DeclarationId) {
        let source = Source::new(code.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        assert!(tokens.diagnostics.is_empty());
        let (_, functions) =
            Parser { source: &source, tokens: &tokens.tokens, cursor: 0 }
                .file().expect("必須可解析私有受限語法");
        let main = functions.iter().find(|f| f.name == "main").unwrap();
        let owner = DeclarationId(main.body[0].start);
        let alias = DeclarationId(main.body[1].start);
        let mut visible = HashMap::from([
            ("owner".to_owned(), owner), ("alias".to_owned(), alias),
        ]);
        let mut future = BTreeSet::new();
        Checker::<fn(&str) -> CopyPermission>::scan_future_block_uses(
            &main.body[2..], &mut visible, &mut future);
        (future, owner, alias)
    }

    #[test]
    fn 分支內的遮蔽借用別名與外層同名來源必須有不同_use_def() {
        let code = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nif(true){\nvar alias:int = 2\nvar copied:int = alias\n}\nreturn owner\n}\n";
        let (used, owner, alias) = scan(code);
        assert!(used.contains(&owner));
        assert!(!used.contains(&alias),
            "只讀取分支內同名新宣告不能延長外層借用票據");
        let inner = DeclarationId(code.rfind("var alias:int").unwrap());
        assert_ne!(inner, alias);
        assert!(!used.contains(&inner),
            "後繼掃描只追蹤進入時仍存活的詞彙借用；新宣告不屬原票據");
        assert!(inspect(code).is_ok(),
            "內層遮蔽的最後使用點之後，原來源可直接使用");
    }

    #[test]
    fn 分支內宣告初始化式必須先引用外層而後再遮蔽() {
        let code = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nif(true){\nvar alias:int = alias\nreturn alias\n}\nreturn 0\n}\n";
        let (used, _, alias) = scan(code);
        let inner = DeclarationId(code.rfind("var alias:int").unwrap());
        assert!(used.contains(&alias),
            "內層宣告右值先使用外層已驗證借用身分");
        assert!(!used.contains(&inner),
            "新宣告後的使用不應延長掃描入口的舊借用票據");
    }

    #[test]
    fn 迴圈迭代器遮蔽與分支外層使用分開記錄() {
        let outer = DeclarationId(12);
        let iterator = DeclarationId(90);
        let t = Term { kind: TermKind::Name("alias".to_owned()),
            start: 115, end: 120 };
        let body = vec![Stmt {
            start: 100, end: 130,
            kind: StmtKind::ForRange {
                iterator: "alias".to_owned(), iterator_start: iterator.0,
                lower: "0".to_owned(), upper: "2".to_owned(),
                body: crate::syntax_shared::ParsedBlock {
                    statements: vec![Stmt {
                        start: 113, end: 124,
                        kind: StmtKind::Return(crate::syntax_shared::ParsedReturn::new(t)),
                    }],
                    start: 98, end: 130,
                },
            },
        }];
        let mut visible = HashMap::from([("alias".to_owned(), outer)]);
        let mut used = BTreeSet::new();
        Checker::<fn(&str) -> CopyPermission>::scan_future_block_uses(
            &body, &mut visible, &mut used);
        assert!(!used.contains(&iterator),
            "for 新迭代實例不是入口既有的借用身分");
        assert!(!used.contains(&outer),
            "迴圈迭代器不得依相同名稱誤延長外層借用票據");
        assert_eq!(visible["alias"], outer,
            "迴圈出口不可以污染外層的詞彙可見來源");
    }

    #[test]
    fn 真正跨分支後繼仍使用外層別名時不可提前解除借用() {
        let code = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nif(true){\nvar alias:int = 2\n}else{\nvar copy:int = alias\n}\nreturn alias\n}\n";
        let (used, _, alias) = scan(code);
        assert!(used.contains(&alias),
            "至少一條可達後繼使用外層別名時必須保守保留票據");
    }
}

#[cfg(test)]
mod p16_6_r03_f08_f10_單一作用域安全狀態契約 {
    use super::*;

    fn sample() -> Binding {
        Binding::new(
        DeclarationId(30),
        "int".to_owned(),
        true,
        ValueSlot::initialized(CopyPermission::Allowed),
        None,
        Some(LoanBinding {
                fact: BorrowFact::new(DeclarationId(15), BorrowKind::SharedRead),
                active: true,
            }),
    )
    }

    #[test]
    fn 同一安全狀態可跨分支與迴圈比較_型別和權限也不得漂移() {
        let original = sample();
        assert!(original.same_safety_state(&original.clone()));
        let mut moved = original.clone();
        moved.value.move_whole().unwrap();
        assert!(!original.same_safety_state(&moved));
        let mut wrong_type = original.clone();
        wrong_type.ty = "bool".to_owned();
        assert!(!original.same_safety_state(&wrong_type));
        let mut wrong_permission = original.clone();
        wrong_permission.mutable = false;
        assert!(!original.same_safety_state(&wrong_permission));
        let mut wrong_declaration = original.clone();
        wrong_declaration.declaration_start = DeclarationId(31);
        assert!(!original.same_safety_state(&wrong_declaration));
    }

    #[test]
    fn 反向邊的部分初始化與靜態借用票據來源及有效期均不可漂移() {
        let original = sample();
        let mut pending = original.clone();
        pending.pending_fields = Some(HashSet::from(["text".to_owned()]));
        assert!(!original.same_safety_state(&pending));
        let mut released = original.clone();
        released.loan.as_mut().unwrap().active = false;
        assert!(!original.same_safety_state(&released));
        let mut wrong_source = original.clone();
        wrong_source.loan.as_mut().unwrap().fact.source = DeclarationId(16);
        assert!(!original.same_safety_state(&wrong_source));
        let mut exclusive = original.clone();
        exclusive.loan.as_mut().unwrap().fact.kind = BorrowKind::ExclusiveWrite;
        assert!(!original.same_safety_state(&exclusive));
        let mut no_loan = original.clone();
        no_loan.loan = None;
        assert!(!original.same_safety_state(&no_loan));
    }
}

#[cfg(test)]
mod p16_6_r03_f08_f09_綁定轉移原子契約 {
    use super::*;

    fn incomplete() -> Binding {
        Binding::new(
        DeclarationId(650),
        "Box".to_owned(),
        true,
        ValueSlot::uninitialized(CopyPermission::Forbidden),
        Some(HashSet::from([
                "first".to_owned(), "second".to_owned(),
            ])),
        None,
    )
    }

    #[test]
    fn 部分初始化逐欄提交_最後一欄才變成可讀取() {
        let mut value = incomplete();
        let original = value.clone();
        assert!(!value.commit_first_field("unknown"));
        assert!(value.same_safety_state(&original),
            "未宣告欄位不得部分提交");
        assert!(value.commit_first_field("first"));
        assert_eq!(value.value.state(), ValueState::Uninitialized);
        assert_eq!(value.pending_fields,
            Some(HashSet::from(["second".to_owned()])));
        assert!(!value.commit_first_field("first"));
        assert!(value.commit_first_field("second"));
        assert_eq!(value.value.state(), ValueState::Available);
        assert!(value.pending_fields.is_none());
        assert!(!value.commit_first_field("first"));
    }

    #[test]
    fn 移動拒絕不修改未初始化欄位_合法移動與重新綁定不可殘留局部責任() {
        let mut partial = incomplete();
        let before = partial.clone();
        assert_eq!(partial.move_whole(), Err(ModelError::UninitializedRead));
        assert!(partial.same_safety_state(&before));
        partial.commit_rebind();
        assert!(partial.pending_fields.is_none());
        assert!(partial.move_whole().is_ok());
        assert_eq!(partial.value.state(), ValueState::Moved);
        assert!(partial.pending_fields.is_none());
        assert_eq!(partial.move_whole(), Err(ModelError::MovedRead));
        partial.commit_rebind();
        assert_eq!(partial.value.state(), ValueState::Available);
        assert!(partial.pending_fields.is_none());
    }
}


#[cfg(test)]
mod r03_g3_函式參數來源詞彙身份 {
    use super::*;
    use crate::ownership_model::CopyPermission;

    fn consumed(code: &str, function_name: &str) -> bool {
        let checked = verified_private_source(code)
            .expect("測試的原 Lexer/Parser/Checker 應接受此受限私有來源");
        let function = checked.functions.iter()
            .find(|function| function.name == function_name).unwrap();
        Checker::<fn(&str) -> CopyPermission>::parameter_is_consumed(function, "value")
    }

    #[test]
    fn g3_同名內層字串移動不得冒充外層參數已真正消耗() {
        let code = "fn check(value:str)->int{\nif(true){\nvar value:str = \"內層🪼\"\nvar moved:str = value.move()\n}\nreturn 0\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(!consumed(code, "check"));
    }

    #[test]
    fn g3_真正參數來源的整筆移動仍必須保持已消耗() {
        let code = "fn check(value:str)->int{\nvar moved:str = value.move()\nreturn 0\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(consumed(code, "check"));
    }

    #[test]
    fn g3_確定零次迴圈內的來源移動不能當成已執行參數移交() {
        let code = "fn check(value:str)->int{\nfor i in 0..0{\nvar moved:str = value.move()\n}\nreturn 0\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(!consumed(code, "check"));
    }

    #[test]
    fn g3_同一函式中先遮蔽再解除遮蔽後的外層移動來源必須正確() {
        let code = "fn check(value:str)->int{\nif(true){\nvar value:str = \"內層\"\nvar moved:str = value.move()\n}\nvar actual:str = value.move()\nreturn 0\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(consumed(code, "check"));
    }
}


#[cfg(test)]
mod r03_g3_唯一詞彙來源_use_def_掃描 {
    use super::*;
    use crate::ownership_model::CopyPermission;
    use std::collections::{BTreeSet, HashMap};

    fn scan(source: &str, fun_name: &str) ->
        (BTreeSet<DeclarationId>, BTreeSet<DeclarationId>, DeclarationId) {
        let checked = verified_private_source(source)
            .expect("真正 Parser/Checker 應成功接受此受限私有來源");
        let fun = checked.functions.iter().find(|fun| fun.name == fun_name).unwrap();
        let parameter = &fun.params[0];
        let id = DeclarationId(parameter.declaration_start);
        let visible: HashMap<String, DeclarationId> = fun.params.iter().map(|p|
            (p.name.clone(), DeclarationId(p.declaration_start))).collect();
        let mut all = BTreeSet::new();
        let mut moved = BTreeSet::new();
        Checker::<fn(&str) -> CopyPermission>::scan_resolved_block_uses(
            &fun.body, &mut visible.clone(), &mut all, ResolvedUseKind::All);
        Checker::<fn(&str) -> CopyPermission>::scan_resolved_block_uses(
            &fun.body, &mut visible.clone(), &mut moved, ResolvedUseKind::WholeMove);
        (all, moved, id)
    }

    #[test]
    fn g3_正常參數純讀僅算後繼使用不算整筆移動() {
        let source = "fn inspect(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn 0\n}\n";
        let (all, moved, id) = scan(source, "inspect");
        assert!(all.contains(&id));
        assert!(!moved.contains(&id));
    }

    #[test]
    fn g3_真正參數移動同時出現在唯一全引用與移動來源集合() {
        let source = "fn inspect(value:str)->int{\nvar next:str = value.move()\nreturn 0\n}\nfn main()->int{\nreturn 0\n}\n";
        let (all, moved, id) = scan(source, "inspect");
        assert!(all.contains(&id));
        assert!(moved.contains(&id));
    }

    #[test]
    fn g3_內層同名遮蔽移動不可出現在外層參數的任何來源集合() {
        let source = "fn inspect(value:str)->int{\nif(true){\nvar value:str = \"覆蓋🪼\"\nvar next:str = value.move()\n}\nreturn 0\n}\nfn main()->int{\nreturn 0\n}\n";
        let (all, moved, id) = scan(source, "inspect");
        assert!(!all.contains(&id));
        assert!(!moved.contains(&id));
    }

    #[test]
    fn g3_確定零次for之不可達整筆移動不算任何後繼參數來源() {
        let source = "fn inspect(value:str)->int{\nfor i in 0..0{\nvar next:str = value.move()\n}\nreturn 0\n}\nfn main()->int{\nreturn 0\n}\n";
        let (all, moved, id) = scan(source, "inspect");
        assert!(!all.contains(&id));
        assert!(!moved.contains(&id));
    }
}
