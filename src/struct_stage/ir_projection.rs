// F05：舊 ScalarExpr／ScalarStmt／SharedAst 兩次「投影」已刪除。
// 下列回歸直接消費真正公開／私有 Parser AST 與 Checker HIR，
// 不建構第三棵語意樹，也不推定任何私有資源事件。
#[cfg(test)]
mod p16_6_r02_2_共同語意樹驗證 {
    use super::*;
    use crate::{common_ir, flow, functions, names, parser, types};
    use crate::common_ir::{FrontendMode, ScalarType};

    fn checked(code: &str) -> (parser::File, VerifiedPrivateSource) {
        let input = Source::new(code.as_bytes().to_vec());
        let lexed = lexer::scan(&input);
        assert!(lexed.diagnostics.is_empty(), "測試須使用有效 UTF-8 詞彙來源");
        let public = parser::parse(&input, &lexed.tokens)
            .expect("公開 MVP-A 真正 Parser 應接受核准共同來源");
        let names = names::resolve_with_facts(&public, &input)
            .expect("公開 P09 名稱解析必須成功");
        let public_hir = types::check_with_names_and_hir(&public, &input, &names)
            .expect("真正公開 P10 必須交付來源型別 HIR");
        let reachable = flow::check_with_typed_hir_and_reachability(
            &public, &input, &names, &public_hir,
        ).expect("公開 P11 必須從真正 CFG 交付可達 Typed HIR 視圖");
        functions::check(&public, &input)
            .expect("公開原始函式契約須成功");
        let private = verified_private_source(code)
            .expect("私有真正 Checker 須接受同一核准交集");
        assert!(public_hir.source_spans_are_valid_for_mode(
            FrontendMode::PublicMvpA, code));
        assert!(private.plan.scalar_hir.source_spans_are_valid_for_mode(
            FrontendMode::PrivateVerified, code));
        // 公開 P10 仍對原始 AST 所有語句保留既有型別診斷；
        // 僅 P11 的真正可達 CFG 為同源 Checker 比對範圍。
        // 不建第二份型別表、不自動推論任何未執行資源事件。
        assert_eq!(
            reachable.verified_expressions().collect::<Vec<_>>(),
            private.plan.scalar_hir.verified_expressions().collect::<Vec<_>>(),
            "同源可達純值的真實 Checker HIR 型別與 UTF-8 跨度必須逐筆一致",
        );
        assert_eq!(
            reachable.verified_references().collect::<Vec<_>>(),
            private.plan.scalar_hir.verified_references().collect::<Vec<_>>(),
            "同源可達名稱必須保留相同詞彙宣告來源與型別",
        );
        assert_eq!(
            reachable.verified_assignments().collect::<Vec<_>>(),
            private.plan.scalar_hir.verified_assignments().collect::<Vec<_>>(),
            "同源可達賦值必須從相同 P09/Checker 來源位置與型別交付",
        );
        (public, private)
    }

    /// 以真實 P09/P10 與私有 Checker 列出目前只由公開記錄的節點；
    /// 不把不可達尾句、Group 內層證據錯認為可替代的正式 HIR。
    fn public_only_hir(text: &str) -> Vec<common_ir::VerifiedScalarExpression> {
        let source = Source::new(text.as_bytes().to_vec());
        let lexed = lexer::scan(&source);
        let ast = parser::parse(&source, &lexed.tokens).unwrap();
        let names = names::resolve_with_facts(&ast, &source).unwrap();
        let public = types::check_with_names_and_hir(&ast, &source, &names).unwrap();
        let private = verified_private_source(text).unwrap();
        let private_facts: Vec<_> =
            private.plan.scalar_hir.verified_expressions().collect();
        public.verified_expressions()
            .filter(|fact| !private_facts.contains(fact))
            .collect()
    }

    #[test]
    fn f04_公私_checker_待對齊差異有具體來源與可重現的精確跨度() {
        use crate::common_ir::{NodeSpan, VerifiedScalarExpression};
        let grouping = "fn main()->int{\nreturn (8 - 3) + 2\n}\n";
        let (_, private_grouped) = checked(grouping);
        let at = grouping.find("8 - 3").unwrap();
        let inner = NodeSpan::new(at, at + "8 - 3".len());
        assert_eq!(private_grouped.plan.scalar_hir.expression(inner),
            Some(VerifiedScalarExpression {span: inner, ty: ScalarType::Int}),
            "私有 Checker 必須真的交付保留的 Group 內層算術證據");
        assert!(public_only_hir(grouping).is_empty(),
            "公私正式 Checker 的合法分組應已核對完整內外來源");

        let unreachable = "fn main()->int{\nif(true){\nreturn 1\n}else{\nreturn 2\n}\nreturn 0\n}\n";
        let at = unreachable.find("return 0").unwrap() + "return ".len();
        assert_eq!(
            public_only_hir(unreachable),
            vec![VerifiedScalarExpression {
                span: NodeSpan::new(at, at + 1),
                ty: ScalarType::Int,
            }],
            "公開 P10 會型別檢查不可達 return，私有 Checker 的 CFG HIR 不會",
        );
    }

    #[test]
    fn f04_公開_p11_可達_hir_不將不可達型別事實冒充已執行且拒絕私有簽章() {
        use crate::common_ir::{NodeSpan, FrontendMode};
        let code = "fn main()->int{\nif(true){\nreturn 1\n}else{\nreturn 2\n}\nreturn 0\n}\n";
        let source = Source::new(code.as_bytes().to_vec());
        let lexed = lexer::scan(&source);
        let ast = parser::parse(&source, &lexed.tokens).unwrap();
        let resolved = names::resolve_with_facts(&ast, &source).unwrap();
        let public = types::check_with_names_and_hir(&ast, &source, &resolved).unwrap();
        let view = flow::check_with_typed_hir_and_reachability(
            &ast, &source, &resolved, &public,
        ).expect("P11 真正 CFG 檢查須能產生唯讀可達 HIR");
        let at = code.find("return 0").unwrap() + "return ".len();
        let unreachable = NodeSpan::new(at, at + 1);
        assert!(public.expression(unreachable).is_some(),
            "P10 原本應對不可達尾句執行型別檢查，不能變更既有診斷規格");
        assert!(!view.verified_expressions().any(|fact| fact.span == unreachable),
            "P11 唯讀可達證據不能把確定未執行的 return 當合法 CFG 後繼");
        let private = verified_private_source(code).unwrap();
        assert!(private.plan.scalar_hir.mode_is(FrontendMode::PrivateVerified));
        let error = match flow::check_with_typed_hir_and_reachability(
            &ast, &source, &resolved, &private.plan.scalar_hir,
        ) {
            Ok(_) => panic!("私有 Checker 的同型別 HIR 不得冒充公開 P11 授權"),
            Err(error) => error,
        };
        assert_eq!(&*error.message,
            "P11：可達來源證據必須由正式公開 P10 Checker 簽發");
    }

    #[test]
    fn 公開與私有正式運算式真正共用節點外殼與來源位置() {
        let code = "fn main()->int{\nreturn 2 + 3\n}\n";
        let (file, checked) = checked(code);
        let parser::Item::Function(fun) = &file.items[0] else { panic!("公開函式不存在") };
        let public = &fun.body.statements[0];
        let private = &checked.functions[0].body[0];
        assert_eq!(public.node_span(), private.node_span());
        assert_eq!(public.start, code.find("return").unwrap());
    }

    #[test]
    fn 共同宣告身分與真正已驗證純值型別一致() {
        let code = "fn main()->int{\nvar n:int = 0x2a\nif(true){\nn = 1\n}else{\nn = 2\n}\nreturn n\n}\n";
        let (file, private) = checked(code);
        let parser::Item::Function(fun) = &file.items[0] else { panic!("無公開函式") };
        let parser::StmtKind::Binding(decl) = &fun.body.statements[0].kind
            else { panic!("首項須為正式宣告") };
        let StmtKind::Declare(other) = &private.functions[0].body[0].kind
            else { panic!("私有缺少正式宣告") };
        assert_eq!((decl.name.as_str(), decl.start), (other.name.as_str(), other.start));
        assert_eq!(private.plan.declarations.typed_declaration(DeclarationId(other.start))
            .unwrap().ty, "int");
        assert_eq!(crate::common_ir::ScalarType::from_name(&other.ty), Some(ScalarType::Int));
    }

    #[test]
    fn 多個無參數函式仍保留原始順序及結果() {
        let code = "fn helper()->int{\nreturn 2\n}\nfn main()->int{\nreturn helper()\n}\n";
        let (file, private) = checked(code);
        assert_eq!(file.items.len(), private.functions.len());
        let parser::Item::Function(helper) = &file.items[0] else { panic!("必須有 helper") };
        assert_eq!(helper.name, private.functions[0].name);
        assert_eq!(&private.functions[1].name, "main");
    }

    #[test]
    fn 真正_else_if_分支直接保存共有解析器的順序及位置() {
        let code = "fn main()->int{\nvar n:int = 0\nif(false){\nn = 1\n}else if(true){\nn = 2\n}else if(false){\nn = 3\n}else{\nn = 4\n}\nreturn n\n}\n";
        let (file, private) = checked(code);
        let parser::Item::Function(main) = &file.items[0] else { panic!("缺 main") };
        let parser::StmtKind::IfChain(chain) = &main.body.statements[1].kind
            else { panic!("公用必須是正式 ParsedIfChain") };
        assert_eq!(chain.arms.len(), 3);
        let mut current = &private.functions[0].body[1];
        for arm in &chain.arms {
            let StmtKind::If { branch, no } = &current.kind
                else { panic!("私有只可沿用真正巢狀 ParsedIfArm") };
            assert_eq!(branch.start, arm.start);
            assert_eq!(branch.body.start, arm.branch.body.start);
            assert_eq!(branch.body.end, arm.branch.body.end);
            match no {
                Some(crate::syntax_shared::ParsedElse::ElseIf(next)) => current = next,
                Some(crate::syntax_shared::ParsedElse::Block(otherwise)) => {
                    assert_eq!(otherwise.start, chain.otherwise.as_ref().unwrap().start);
                    assert_eq!(otherwise.end, chain.otherwise.as_ref().unwrap().end);
                    break;
                }
                _ => break,
            }
        }
        assert!(matches!(
            &current.kind, StmtKind::If {no:Some(crate::syntax_shared::ParsedElse::Block(_)),..}
        ), "最後一條巢狀 if 須保留原始 else 大括號");
    }

    #[test]
    fn 沒有_else_之_else_if_仍保留完整可達後繼() {
        let code = "fn main()->int{\nvar n:int = 0\nif(false){\nn = 1\n}else if(true){\nn = 2\n}\nreturn n\n}\n";
        let (file, private) = checked(code);
        let parser::Item::Function(fun) = &file.items[0] else { panic!("缺函式") };
        let parser::StmtKind::IfChain(chain) = &fun.body.statements[1].kind
            else { panic!("應有公開分支鏈") };
        assert_eq!(chain.arms.len(), 2);
        let StmtKind::If { no:Some(crate::syntax_shared::ParsedElse::ElseIf(next)),.. } =
            &private.functions[0].body[1].kind else {panic!("缺真正 else-if")};
        let StmtKind::If { no:None,.. } = &next.kind else {panic!("多造了 else")};
    }

    #[test]
    fn 未初始化的_var_仍由原本正式_ast_保持_none() {
        let code = "fn main()->int{\nvar value:int\nvalue = 1\nreturn value\n}\n";
        let (file, private) = checked(code);
        let parser::Item::Function(fun) = &file.items[0] else { panic!("缺函式") };
        let parser::StmtKind::Binding(binding) = &fun.body.statements[0].kind
            else { panic!("公開缺宣告") };
        let StmtKind::Declare(other) = &private.functions[0].body[0].kind
            else { panic!("私有缺宣告") };
        assert!(binding.initializer.is_none());
        assert!(other.initializer.is_none());
    }

    #[test]
    fn 同名遮蔽的兩個來源宣告必須是不同身份() {
        let code = "fn main()->int{\nvar value:int = 1\nif(true){\nvar value:int = 2\n}\nreturn value\n}\n";
        let (file, private) = checked(code);
        let parser::Item::Function(fun) = &file.items[0] else { panic!("缺函式") };
        let parser::StmtKind::Binding(outer) = &fun.body.statements[0].kind
            else { panic!("外層宣告遺失") };
        let parser::StmtKind::IfChain(chain) = &fun.body.statements[1].kind
            else { panic!("缺分支") };
        let parser::StmtKind::Binding(inner) = &chain.arms[0].branch.body[0].kind
            else { panic!("內層宣告遺失") };
        assert_ne!(DeclarationId(outer.start), DeclarationId(inner.start));
        assert!(private.plan.declarations.typed_declaration(DeclarationId(outer.start)).is_some());
        assert!(private.plan.declarations.typed_declaration(DeclarationId(inner.start)).is_some());
    }

    #[test]
    fn 真正括號來源保留相同半開位置及不同語法診斷() {
        checked("fn main()->int{\nreturn (8 - 3) + 2\n}\n");
        let code = "fn main()->int{\nreturn (8 - 3\n}\n";
        let source = Source::new(code.as_bytes().to_vec());
        let lexed = lexer::scan(&source);
        let public = parser::parse(&source, &lexed.tokens).unwrap_err();
        let private = Parser {
            source: &source, tokens: &lexed.tokens, cursor: 0,
        }.file().unwrap_err();
        assert_eq!(public.kind, DiagnosticKind::Syntax);
        assert_eq!(private.kind, DiagnosticKind::Syntax);
        assert_eq!(&*public.message, "算術分組缺少右圓括號");
        assert_eq!(&*private.message, "括號運算式缺少右括號");
        assert_eq!(public.span, private.span);
    }

    #[test]
    fn 公私真正名稱解析診斷不吞掉錯誤_token() {
        checked("fn helper()->int{\nreturn 2\n}\nfn main()->int{\nvar n:int = 3\nreturn helper() + n\n}\n");
        let code = "fn ()->int{\nreturn 0\n}\n";
        let source = Source::new(code.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        let public = parser::parse(&source,&tokens.tokens).unwrap_err();
        let private = Parser {
            source: &source, tokens: &tokens.tokens, cursor: 0,
        }.file().unwrap_err();
        assert_eq!(&*public.message, "缺少合法名稱");
        assert_eq!(&*private.message, "需要合法的名稱或型別名稱");
        assert_eq!(public.span, private.span);
    }

    #[test]
    fn 真正_parser_語句分隔仍分別保留模式原診斷() {
        checked("fn main()->int{\nvar n:int = 3\nreturn n}\n");
        let invalid = "fn main()->int{\nvar n:int = 3 return n\n}\n";
        let source = Source::new(invalid.as_bytes().to_vec());
        let lexed = lexer::scan(&source);
        let public = parser::parse(&source, &lexed.tokens).unwrap_err();
        let private = Parser {
            source: &source, tokens: &lexed.tokens, cursor: 0
        }.file().unwrap_err();
        assert_eq!(&*public.message, "語句之間必須以換行分隔");
        assert_eq!(&*private.message, "語句之間必須換行");
        assert_eq!(public.span, private.span);
    }

    #[test]
    fn 多行布林_if_條件兩路真正_checker_交付同一_hir() {
        let code = "\n\nfn main()->int{\n\nvar flag:bool = true\n\nif(false){\nreturn 1\n}else{\nreturn 2\n}\n\nreturn 0\n}\n";
        let (file, private) = checked(code);
        let parser::Item::Function(fun) = &file.items[0] else {panic!("缺函式")};
        let parser::StmtKind::Binding(decl) = &fun.body.statements[0].kind
            else {panic!("缺旗標宣告")};
        assert_eq!(decl.ty, ScalarType::Bool);
        let parser::StmtKind::IfChain(chain) = &fun.body.statements[1].kind
            else {panic!("缺分支")};
        let expr = &chain.arms[0].branch.condition;
        assert_eq!(private.plan.scalar_hir.expression(expr.node_span()).unwrap().ty,
            ScalarType::Bool);
    }

    #[test]
    fn 三重加減巢狀括號直接比對真正_ast_運算子及_hir() {
        for code in [
            "fn main()->int{\nreturn 10 - 3 + 2 - (1 + 4)\n}\n",
            "fn main()->int{\nvar n:int = 10\nreturn n - 3 + (2 - 1)\n}\n",
        ] {
            let (file, private) = checked(code);
            let parser::Item::Function(fun) = &file.items[0] else {panic!("缺函式")};
            let ret = fun.body.statements.last().unwrap();
            let parser::StmtKind::Return(value) = &ret.kind else {panic!("缺返回")};
            let crate::expr::ExprKind::BinaryInt(binary) = &value.expression.kind
                else {panic!("公私必須使用唯一 BinaryInt AST")};
            assert!(matches!(binary.operator, '+'|'-'));
            assert!(private.plan.scalar_hir.arithmetic(value.expression.node_span()).is_some());
        }
    }

    #[test]
    fn 未核准私有資源類型不能成為公開純值解析授權() {
        for code in [
            "fn main()->int{\nvar text:str = \"中文字串\"\nreturn 0\n}\n",
            "fn main()->int{\nvar data:list\nreturn 0\n}\n",
        ] {
            let source = Source::new(code.as_bytes().to_vec());
            let tokens = lexer::scan(&source);
            assert!(parser::parse(&source, &tokens.tokens).is_err(),
                "MVP-A 不可因共用 AST 擴張 str/list 語法");
            assert!(crate::common_ir::ScalarType::from_name("str").is_none());
            assert!(crate::common_ir::ScalarType::from_name("list").is_none());
        }
        let checked = verified_private_source(
            "fn main()->int{\nvar value:str = \"中文\"\nreturn 0\n}\n"
        ).unwrap();
        assert!(checked.plan.scalar_hir.verified_expressions().all(
            |fact| fact.ty == ScalarType::Int || fact.ty == ScalarType::Bool
        ), "非 Copyable 私有資源不得冒充公開純值 HIR");
    }
}
