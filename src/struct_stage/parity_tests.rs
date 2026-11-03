// P16.6-R02：公開 MVP-A 與私有 P13～P16 在已核准交集上的正反行為對照。
// 不把私有結構語法偷開到公開入口，且兩路不同的錯誤文字不可假設相同。

#[cfg(test)]
mod p16_6_r02_公開私有前端交集契約 {
    use super::*;
    use crate::{parser, names, types, flow, functions};

    fn public_check(code: &str) -> Result<(), Diagnostic> {
        let source = Source::new(code.as_bytes().to_vec());
        if let Some(first) = source.lexical_diagnostics().into_iter().next() {
            return Err(first);
        }
        let tokens = lexer::scan(&source);
        if let Some(first) = tokens.diagnostics.into_iter().next() {
            return Err(first);
        }
        let ast = parser::parse(&source, &tokens.tokens)?;
        let facts = names::resolve_with_facts(&ast, &source)?;
        // 交集回歸必須走正式 check_file 相同的 P09→P10 型別 HIR→P11，
        // 不能透過 P10／P11 的獨立相容入口略過 HIR 交付證據。
        let typed = types::check_with_names_and_hir(&ast, &source, &facts)?;
        flow::check_with_typed_hir(&ast, &source, &facts, &typed)?;
        functions::check(&ast, &source)?;
        Ok(())
    }

    #[test]
    fn a1_f01_真正公開_parser_函式與所有宣告名稱型別_token_正反驗證() {
        let code = "# 註記🪼\r\nconst global:int = 4\r\nfn main()->int{\r\nvar item:int = global\r\nif(true){\r\nvar nested:int = 2\r\nreturn nested\r\n}else{\r\nreturn item\r\n}\r\nreturn item\r\n}\r\n";
        let source = Source::new(code.as_bytes().to_vec());
        let scan = lexer::scan(&source);
        assert!(scan.diagnostics.is_empty());
        let parsed = parser::parse(&source, &scan.tokens).unwrap();
        assert!(parser::verified_public_source_tokens(&parsed, code, &scan.tokens));
        let mut forged = parsed.clone();
        let parser::Item::Function(fun) = &mut forged.items[1]
            else { panic!("必須從真 Parser 取得完整函式") };
        fun.name = "other".into();
        assert!(!parser::verified_public_source_tokens(&forged, code, &scan.tokens),
            "函式 AST 名稱不得冒充原始 token");
        let parser::Item::Function(fun) = &mut forged.items[1]
            else { panic!("必須從真 Parser 取得完整函式") };
        fun.name = "main".into();
        fun.result_ty = crate::common_ir::ScalarType::Bool;
        assert!(!parser::verified_public_source_tokens(&forged, code, &scan.tokens),
            "回傳型別原始 token 不得在 Parser 成功後換成另一種型別");
        let parser::Item::Function(fun) = &mut forged.items[1]
            else { panic!("必須從真 Parser 取得完整函式") };
        fun.result_ty = crate::common_ir::ScalarType::Int;
        let parser::StmtKind::IfChain(chain) = &mut fun.body.statements[1].kind
            else { panic!("必須有巢狀 if") };
        let parser::StmtKind::Binding(decl) = &mut chain.arms[0].branch.body.statements[0].kind
            else { panic!("巢狀原始 var 未建立") };
        decl.ty = crate::common_ir::ScalarType::Bool;
        assert!(!parser::verified_public_source_tokens(&forged, code, &scan.tokens),
            "內層宣告的型別不能只修改 AST 即更改許可");
        let mut bad = scan.tokens.clone();
        let idx = bad.iter().position(|token|
            matches!(&token.kind, crate::lexer::TokenKind::Arrow)
        ).unwrap();
        bad[idx].kind = crate::lexer::TokenKind::Symbol(':');
        assert!(!parser::verified_public_source_tokens(&parsed, code, &bad),
            "沒有真正箭號 token 不得假造函式簽章");
    }

    #[test]
    fn a1_f01_真正私有_parser_方法與巢狀宣告_token_來源不可竄改() {
        let code = "# 私有來源🪼\r\nstruct Meter{\r\nconst field:int = 4\r\nfn get(value:int)->int{\r\nif(true){\r\nvar inner:int = value\r\nreturn inner\r\n}else{\r\nreturn 0\r\n}\r\nreturn value\r\n}\r\n}\r\nfn main()->int{\r\nreturn 1\r\n}\r\n";
        let source = Source::new(code.as_bytes().to_vec());
        let scan = lexer::scan(&source);
        assert!(scan.diagnostics.is_empty());
        let (mut structs, mut funcs) = Parser {
            source: &source, tokens: &scan.tokens, cursor: 0,
        }.file().unwrap();
        assert!(verified_private_syntax_tree(code, &structs, &funcs));
        assert!(verified_private_source_tokens(code, &scan.tokens, &structs, &funcs));
        let original = structs["Meter"].methods[0].result_ty.clone();
        structs.get_mut("Meter").unwrap().methods[0].result_ty = "bool".into();
        assert!(!verified_private_source_tokens(code, &scan.tokens, &structs, &funcs),
            "私有方法不得把原始 int 返回 token 改成 bool");
        structs.get_mut("Meter").unwrap().methods[0].result_ty = original;
        {
            let method = &mut structs.get_mut("Meter").unwrap().methods[0];
            let StmtKind::If { branch, .. } = &mut method.body.statements[0].kind
                else { panic!("必須有巢狀私有 if") };
            let StmtKind::Declare(decl) = &mut branch.branch.body.statements[0].kind
                else { panic!("巢狀宣告缺失") };
            decl.name = "changed".into();
        }
        assert!(!verified_private_source_tokens(code, &scan.tokens, &structs, &funcs),
            "巢狀私有原始名稱被修改應拒絕");
        {
            let method = &mut structs.get_mut("Meter").unwrap().methods[0];
            let StmtKind::If { branch, .. } = &mut method.body.statements[0].kind
                else { panic!("必須有巢狀私有 if") };
            let StmtKind::Declare(decl) = &mut branch.branch.body.statements[0].kind
                else { panic!("巢狀宣告缺失") };
            decl.name = "inner".into();
        }
        assert!(verified_private_source_tokens(code, &scan.tokens, &structs, &funcs));
        funcs[0].name = "forged".into();
        assert!(!verified_private_source_tokens(code, &scan.tokens, &structs, &funcs));
        let mut bad = scan.tokens.clone();
        let pos = bad.iter().position(|token| matches!(&token.kind,
            crate::lexer::TokenKind::Keyword(word) if word == "struct"
        )).unwrap();
        bad[pos].kind = crate::lexer::TokenKind::Keyword("fn".into());
        assert!(!verified_private_source_tokens(code, &bad, &structs, &funcs));
    }

    #[test]
    fn f01_f04_真正公私_checker_簽出不可跨模式冒用的完整_typed_hir() {
        use crate::common_ir::FrontendMode;
        let code = "fn main()->int{\nvar n:int = 3\nreturn n + 1\n}\n";
        let source = Source::new(code.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        let parsed = parser::parse(&source, &tokens.tokens).unwrap();
        let resolved = names::resolve_with_facts(&parsed, &source).unwrap();
        let public = types::check_with_names_and_hir(&parsed, &source, &resolved)
            .expect("真正 P09→P10 應交付公開帶模式 HIR");
        let private = verified_private_source(code)
            .expect("真正 P13 Checker 應交付私有帶模式 HIR");
        assert!(public.source_spans_are_valid_for_mode(FrontendMode::PublicMvpA, code));
        assert!(private.plan.scalar_hir.source_spans_are_valid_for_mode(
            FrontendMode::PrivateVerified, code));
        assert!(!public.source_spans_are_valid_for_mode(FrontendMode::PrivateVerified, code),
            "公開純值即使同來源同型別，不能偽冒私有資源安全計畫");
        assert!(!private.plan.scalar_hir.source_spans_are_valid_for_mode(
            FrontendMode::PublicMvpA, code),
            "私有 Checker 的型別事實不得冒用公開 P10 授權");
        let pub_expr: Vec<_> = public.verified_expressions().collect();
        let priv_expr: Vec<_> = private.plan.scalar_hir.verified_expressions().collect();
        assert_eq!(pub_expr, priv_expr,
            "核准共同純值的型別／半開位置仍須完全一致");
        assert!(private.plan.is_consistent(&private.structs, &private.functions, code));
    }

    #[test]
    fn f01_公私正式共同函式與宣告維持相同跨度但模式身分互不混淆() {
        use crate::common_ir::{FrontendMode, ModeNodeIdentity, NodeIdentity, NodeRole, NodeSpan};
        let code = "fn main()->int{\nvar value:int = 1\nreturn value\n}\n";
        let source = Source::new(code.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        assert!(tokens.diagnostics.is_empty());
        let public = parser::parse(&source, &tokens.tokens).unwrap();
        let parser::Item::Function(public_function) = &public.items[0]
            else { panic!("必須由公開正式 Parser 產生函式") };
        let private = verified_private_source(code).unwrap();
        let private_function = &private.functions[0];

        let public_id = public_function.checked_identity(FrontendMode::PublicMvpA, code)
            .expect("公開函式應交付共用的合法來源身分");
        let private_id = private_function.checked_identity(FrontendMode::PrivateVerified, code)
            .expect("私有函式應交付共用的合法來源身分");
        assert_eq!(public_id.node, private_id.node);
        assert_eq!(public_id.node.role, NodeRole::Function);
        assert_ne!(public_id, private_id, "不同入口模式不能用相同身分授權");

        let public_body = public_function.body.checked_identity(FrontendMode::PublicMvpA, code)
            .expect("公開區塊由正式共同解析器產生");
        let private_body = private_function.body.checked_identity(
            FrontendMode::PrivateVerified, code).expect("私有區塊由正式共同解析器產生");
        assert_eq!(public_body.node, private_body.node);
        assert_eq!(public_body.node.role, NodeRole::Block);
        assert_ne!(public_body, private_body);

        let parser::StmtKind::Binding(public_decl) =
            &public_function.body.statements[0].kind
            else { panic!("公開共同宣告節點缺失") };
        let StmtKind::Declare(private_decl) =
            &private_function.body.statements[0].kind
            else { panic!("私有共同宣告節點缺失") };
        let public_decl_id = public_decl.checked_identity(FrontendMode::PublicMvpA, code)
            .expect("公開宣告跨度有效");
        let private_decl_id = private_decl.checked_identity(FrontendMode::PrivateVerified, code)
            .expect("私有宣告跨度有效");
        assert_eq!(public_decl_id.node, private_decl_id.node);
        assert_eq!(public_decl_id.node.role, NodeRole::Declaration);
        assert_ne!(public_decl_id, private_decl_id);

        let public_stmt = public_function.body.statements[0]
            .checked_mode_identity(FrontendMode::PublicMvpA, NodeRole::Statement, code)
            .unwrap();
        let private_stmt = private_function.body.statements[0]
            .checked_mode_identity(FrontendMode::PrivateVerified, NodeRole::Statement, code)
            .unwrap();
        assert_eq!(public_stmt.node, private_stmt.node);
        assert_ne!(public_stmt, private_stmt);

        let unicode = "A中🪼B";
        let real = NodeSpan::new(1, 8);
        assert_eq!(
            ModeNodeIdentity::checked(FrontendMode::PrivateVerified, real,
                NodeRole::Expression, unicode).unwrap().node,
            NodeIdentity::checked(real, NodeRole::Expression, unicode).unwrap(),
        );
        assert!(ModeNodeIdentity::checked(
            FrontendMode::PublicMvpA, NodeSpan::new(1, 2),
            NodeRole::Expression, unicode).is_none(),
            "UTF-8 中文字元中間的位元組不能冒充已驗證節點",
        );
        assert!(ModeNodeIdentity::checked(
            FrontendMode::PrivateVerified, NodeSpan::new(0, unicode.len() + 1),
            NodeRole::Block, unicode).is_none(),
            "不能使用 EOF 以後的來源節點",
        );
    }

    #[test]
    fn f01_正式區塊所有直接語句與私有方法來源皆保留共用模式身分() {
        use crate::common_ir::FrontendMode;

        let common = "fn main()->int{\nvar n:int = 1\nreturn n\n}\n";
        assert_eq!(public_check(common), Ok(()));
        assert!(inspect(common).is_ok());
        let input = Source::new(common.as_bytes().to_vec());
        let scanned = lexer::scan(&input);
        let file = parser::parse(&input, &scanned.tokens).unwrap();
        let parser::Item::Function(public_function) = &file.items[0]
            else { panic!("公開正式函式節點缺失") };
        assert!(public_function.body.checked_direct_statements(
            FrontendMode::PublicMvpA, common));
        let checked = verified_private_source(common).unwrap();
        assert!(checked.functions[0].body.checked_direct_statements(
            FrontendMode::PrivateVerified, common));
        let mut forged = public_function.body.clone();
        forged.statements[0].end = common.len() + 1;
        assert!(!forged.checked_direct_statements(
            FrontendMode::PublicMvpA, common),
            "超出真實來源的語句不能因合法函式外殼而通過");
        let mut crossed = public_function.body.clone();
        crossed.statements[0].start = public_function.start;
        assert!(!crossed.checked_direct_statements(
            FrontendMode::PublicMvpA, common),
            "子語句不得跨出真正解析的區塊範圍");

        let private_source = "struct OnlyMethod{\nfn get()->int{\nreturn 1\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(private_source).is_ok());
        let input = Source::new(private_source.as_bytes().to_vec());
        let scanned = lexer::scan(&input);
        let (structs, _) = Parser {
            source: &input, tokens: &scanned.tokens, cursor: 0,
        }.file().unwrap();
        let method = &structs["OnlyMethod"].methods[0];
        assert!(method.checked_identity(
            FrontendMode::PrivateVerified, private_source).is_some(),
            "不能遺漏結構方法的完整來源節點身分");
        assert!(method.body.checked_direct_statements(
            FrontendMode::PrivateVerified, private_source),
            "私有方法內的 return 必須具有真正來源跨度");
        let mut forged_method = method.body.clone();
        forged_method.statements[0].end = private_source.len() + 1;
        assert!(!forged_method.checked_direct_statements(
            FrontendMode::PrivateVerified, private_source),
            "私有方法的合成越界語句不可冒充已經驗證的節點");
    }

    #[test]
    fn f01_公私巢狀_ast_欄位_參數_所有節點皆由相同來源身分稽核() {
        use crate::common_ir::{NodeSpan, SourceIdentityAudit, FrontendMode, NodeRole};
        let common = "fn main()->int{\nvar item:int = 1\nif(true){\nvar nested:int = item\nreturn nested\n}else{\nreturn 0\n}\nreturn item\n}\n";
        assert_eq!(public_check(common), Ok(()));
        let source = Source::new(common.as_bytes().to_vec());
        let scanned = lexer::scan(&source);
        let ast = parser::parse(&source, &scanned.tokens).unwrap();
        assert!(parser::verified_public_syntax_tree(&ast, common));
        let checked = verified_private_source(common).unwrap();
        assert!(verified_private_syntax_tree(
            common, &checked.structs, &checked.functions
        ));
        let mut forged = ast.clone();
        let parser::Item::Function(main) = &mut forged.items[0]
            else { panic!("測試須取得正式函式") };
        main.body.statements[0].end = common.len() + 1;
        assert!(!parser::verified_public_syntax_tree(&forged, common),
            "不能用表面函式、區塊正確的來源遮掩深層非法子語句");

        let private_src = "struct Holder{\nvar amount:int = 1\nfn value(input:int)->int{\nif(true){\nreturn input\n}\nreturn self.amount\n}\n}\nfn main()->int{\nvar obj:Holder = Holder()\nreturn obj.value(1)\n}\n";
        let parsed_source = Source::new(private_src.as_bytes().to_vec());
        let tokens = lexer::scan(&parsed_source);
        let (mut structs, functions) = Parser {
            source: &parsed_source, tokens: &tokens.tokens, cursor: 0,
        }.file().unwrap();
        assert!(verified_private_syntax_tree(private_src, &structs, &functions));
        let stored = structs["Holder"].fields[0].declaration_span;
        structs.get_mut("Holder").unwrap().fields[0].declaration_span =
            NodeSpan::new(stored.start, private_src.len() + 1);
        assert!(!verified_private_syntax_tree(private_src, &structs, &functions),
            "私有 struct 欄位不可借用合法函式跨度偽造權限");
        structs.get_mut("Holder").unwrap().fields[0].declaration_span = stored;
        structs.get_mut("Holder").unwrap().methods[0].params[0].declaration_start =
            private_src.len() + 1;
        assert!(!verified_private_syntax_tree(private_src, &structs, &functions),
            "參數名字的 DeclarationId 必須是現存原始來源位置");

        let mut audit = SourceIdentityAudit::new("A中B", FrontendMode::PrivateVerified);
        assert!(audit.record(NodeRole::Expression, NodeSpan::new(0, 5), None));
        assert!(audit.record(NodeRole::Expression, NodeSpan::new(0, 4), None),
            "相同起點不同長度的真實嵌套運算式不可被誤殺");
        assert_eq!(audit.count(), 2);
        assert!(!audit.record(NodeRole::Expression, NodeSpan::new(1, 2), None),
            "中英混合 UTF-8 不可把多位元組拆半");
    }

    #[test]
    fn f01_欄位與方法參數型別必須逐_token_證明原始來源不可竄換() {
        use crate::common_ir::NodeSpan;
        let text = "struct Holder{\nvar amount:int = 1\nfn value(input:int)->int{\nreturn input\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let source = Source::new(text.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        let (mut structs, functions) = Parser {
            source: &source, tokens: &tokens.tokens, cursor: 0,
        }.file().unwrap();
        assert!(verified_private_syntax_tree(text, &structs, &functions));
        let field = &structs["Holder"].fields[0];
        assert_eq!(&text[field.name_span.start..field.name_span.end], "amount");
        assert_eq!(&text[field.type_span.start..field.type_span.end], "int");
        assert!(field.declaration_span.start < field.name_span.start);
        assert!(field.name_span.end < field.type_span.start);
        let param = &structs["Holder"].methods[0].params[0];
        assert_eq!(&text[param.name_span.start..param.name_span.end], "input");
        assert_eq!(&text[param.type_span.start..param.type_span.end], "int");
        assert_eq!(param.declaration_start, param.declaration_span.start);
        assert_eq!(param.declaration_span.end, param.type_span.end);

        let original_name = field.name_span;
        let original_type = structs["Holder"].fields[0].type_span;
        structs.get_mut("Holder").unwrap().fields[0].name_span = original_type;
        assert!(!verified_private_syntax_tree(text, &structs, &functions),
            "欄位名稱不得假借有效型別 token 的位置");
        structs.get_mut("Holder").unwrap().fields[0].name_span = original_name;
        structs.get_mut("Holder").unwrap().fields[0].type_span = original_name;
        assert!(!verified_private_syntax_tree(text, &structs, &functions),
            "欄位型別不得用相同檔案中的名稱 token 偽造");
        structs.get_mut("Holder").unwrap().fields[0].type_span = original_type;

        let original_param_type = structs["Holder"].methods[0].params[0].type_span;
        structs.get_mut("Holder").unwrap().methods[0].params[0].type_span =
            NodeSpan::new(original_param_type.start, text.len() + 1);
        assert!(!verified_private_syntax_tree(text, &structs, &functions),
            "參數型別不許越過 UTF-8 原始檔 EOF");
        structs.get_mut("Holder").unwrap().methods[0].params[0].type_span =
            original_param_type;
        let original_param_name = structs["Holder"].methods[0].params[0].name_span;
        structs.get_mut("Holder").unwrap().methods[0].params[0].name_span =
            original_param_type;
        assert!(!verified_private_syntax_tree(text, &structs, &functions),
            "名稱與型別即使同樣是合法 token 也不能交換角色來源");
        structs.get_mut("Holder").unwrap().methods[0].params[0].name_span =
            original_param_name;
        assert!(verified_private_syntax_tree(text, &structs, &functions),
            "恢復真正 Parser token 後不得留下多餘的拒絕狀態");
    }

    #[test]
    fn f01_f04_欄位及方法與函式參數的型別證據來自唯一_checker_且不能授權資源() {
        use crate::common_ir::{FrontendMode, NodeRole};
        let code = "struct Holder{\nconst text:str = \"中文🪼\"\nfn echo(value:int)->int{\nreturn value\n}\n}\nfn plus(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn plus(1)\n}\n";
        let mut checked = verified_private_source(code)
            .expect("合法含非 Copyable 欄位與 Copyable 方法參數的真正 Checker 應通過");
        assert!(checked.plan.is_consistent(&checked.structs, &checked.functions, code));
        let members = &checked.plan.members;
        assert_eq!(members.len(), 3, "一個真實欄位、方法參數及一般函式參數皆須有證據");
        assert!(members.keys().all(|key| key.mode == FrontendMode::PrivateVerified));
        assert!(members.values().all(|fact| fact.source_is_consistent(code)));
        let field = members.values().find(|fact|
            fact.identity.node.role == NodeRole::Field
        ).expect("缺少真正結構欄位 token 的型別註記");
        assert_eq!(field.declaration.ty, "str");
        assert!(!field.declaration.mutable, "const 欄位不可假造 var 權限");
        assert!(checked.plan.declarations.typed_declaration(
            field.declaration.declaration,
        ).is_none(), "欄位型別來源絕不能被當成獨立可借用 Binding 持有者");

        let keys: Vec<_> = checked.plan.members.keys().copied().collect();
        let field_id = field.identity;
        let original = checked.plan.members[&field_id].clone();
        checked.plan.members.get_mut(&field_id).unwrap().declaration.ty = "int".into();
        assert!(!checked.plan.is_consistent(&checked.structs, &checked.functions, code),
            "原始來源仍是 str 的欄位不得憑修改另一份型別表就變成 Copyable int");
        checked.plan.members.insert(field_id, original);
        assert!(checked.plan.is_consistent(&checked.structs, &checked.functions, code));

        let method_param = keys.into_iter().find(|key|
            key.node.role == NodeRole::Parameter
                && checked.plan.members[key].name == "value"
                && key.node.span.start < code.find("fn plus").unwrap()
        ).expect("私有方法參數的原始型別 token 應存在");
        let removed = checked.plan.members.remove(&method_param).unwrap();
        assert!(!checked.plan.is_consistent(&checked.structs, &checked.functions, code),
            "任何一筆真正通過 Checker 的方法參數證據都不可刪除");
        checked.plan.members.insert(method_param, removed);
        assert!(checked.plan.is_consistent(&checked.structs, &checked.functions, code));

        let invalid = code.replace("fn echo(value:int)", "fn echo(value:UnknownType)");
        let err = verified_private_source(&invalid).err()
            .expect("私有方法參數未知型別必須拒絕");
        assert_eq!(&*err.message, "P13：方法參數使用不存在的型別");
    }

    #[test]
    fn f05_公私正式_checker_相同已核准純值來源產生相同型別與半開位置證據() {
        let code = "fn main()->int{\nvar value:int = 3\nif(true){\nreturn value\n}\nreturn 0\n}\n";
        assert_eq!(public_check(code), Ok(()));
        let source = Source::new(code.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        let ast = parser::parse(&source, &tokens.tokens).unwrap();
        let names = names::resolve_with_facts(&ast, &source).unwrap();
        let public = types::check_with_names_and_hir(&ast, &source, &names).unwrap();
        assert_eq!(flow::check_with_typed_hir(&ast, &source, &names, &public), Ok(()));
        let private = verified_private_source(code).unwrap();
        let span = |start: usize, length: usize|
            crate::common_ir::NodeSpan::new(start, start + length);
        let condition = code.find("true").unwrap();
        let read = code.find("return value").unwrap() + "return ".len();
        let final_return = code.rfind("return 0").unwrap() + "return ".len();
        let init = code.find("= 3").unwrap() + 2;
        for (start, length, expected) in [
            (condition, 4, crate::common_ir::ScalarType::Bool),
            (read, "value".len(), crate::common_ir::ScalarType::Int),
            (final_return, 1, crate::common_ir::ScalarType::Int),
            (init, 1, crate::common_ir::ScalarType::Int),
        ] {
            let at = span(start, length);
            assert_eq!(public.expression(at).map(|fact| fact.ty), Some(expected),
                "公開 P10 缺少完整型別 HIR：{at:?}");
            assert_eq!(private.plan.scalar_hir.expression(at).map(|fact| fact.ty),
                Some(expected), "私有 Checker 與公開 P10 型別／來源位置不一致：{at:?}");
            assert!(private.plan.scalar_hir.expression(
                span(start, length + 1)).is_none(),
                "較長的半開位置不得冒充 Checker 真正核准的純值節點");
        }
        let declaration = crate::ownership_ledger::DeclarationId(
            code.find("var value").unwrap());
        let pub_decl = names.declaration(declaration).unwrap();
        let priv_decl = private.plan.declarations.typed_declaration(declaration).unwrap();
        assert_eq!(pub_decl.declaration, priv_decl.declaration);
        assert_eq!(match pub_decl.ty {
            crate::common_ir::ScalarType::Int => "int",
            crate::common_ir::ScalarType::Bool => "bool",
        }, priv_decl.ty);
        assert_eq!(pub_decl.mutable, priv_decl.mutable);
        assert!(public_check(&code.replace("if(true)", "if(1)")).is_err());
        assert!(verified_private_source(&code.replace("if(true)", "if(1)")).is_err());
    }

    #[test]
    fn f04_公私正式_checker_具名讀取及賦值目標的_typed_use_證據逐筆一致() {
        let code = "fn main()->int{\nvar n:int = 1\nvar alias:int = n\nn = alias\nreturn n\n}\n";
        assert_eq!(public_check(code), Ok(()));
        let source = Source::new(code.as_bytes().to_vec());
        let toks = lexer::scan(&source);
        let ast = parser::parse(&source, &toks.tokens).unwrap();
        let facts = names::resolve_with_facts(&ast, &source).unwrap();
        let public = types::check_with_names_and_hir(&ast, &source, &facts).unwrap();
        assert_eq!(flow::check_with_typed_hir(&ast, &source, &facts, &public), Ok(()));
        let private = verified_private_source(code).unwrap();
        let owner = DeclarationId(code.find("var n:int").unwrap());
        let alias = DeclarationId(code.find("var alias:int").unwrap());
        for (fragment, name, declaration) in [
            ("= n", "n", owner),
            ("= alias", "alias", alias),
            ("return n", "n", owner),
        ] {
            let at = code.find(fragment).unwrap() + fragment.len() - name.len();
            let span = crate::common_ir::NodeSpan::new(at, at + name.len());
            let public_read = public.reference(span)
                .expect("P10 應交付具名 int 讀取事實");
            let private_read = private.plan.scalar_hir.reference(span)
                .expect("私有 Checker 應交付相同來源 TypedUse");
            assert_eq!(private_read, public_read);
            assert_eq!(private_read.resolved.declaration, declaration);
            assert_eq!(private_read.resolved.ty, crate::common_ir::ScalarType::Int);
        }
        let target = code.find("n = alias").unwrap();
        let span = crate::common_ir::NodeSpan::new(target, target + "n = alias".len());
        let public_write = public.assignment(span)
            .expect("P10 寫入目標已驗證");
        let private_write = private.plan.scalar_hir.assignment(span)
            .expect("私有 Checker 完成真實賦值後才交付目標 HIR");
        assert_eq!(public_write, private_write);
        assert_eq!(private_write.resolved.declaration, owner);
        assert!(private.plan.scalar_hir.assignment(crate::common_ir::NodeSpan::new(
            target, target + "n = alias".len() + 1)).is_none());

        let moved = "fn main()->int{\nvar original:str = \"資源\"\nvar target:str = original.move()\nreturn 0\n}\n";
        let checked = verified_private_source(moved).unwrap();
        let start = moved.find("original.move()").unwrap();
        let attempted = crate::common_ir::NodeSpan::new(
            start, start + "original.move()".len());
        assert!(checked.plan.scalar_hir.reference(attempted).is_none(),
            "私有非 Copyable 的明確 move 不可冒充公開純值名稱引用");
        assert!(checked.plan.scalar_hir.expression(attempted).is_none(),
            "私有 str 資源移交不是 ScalarType 的 int/bool 表達式");
    }

    #[test]
    fn f05_全部已核准純值交集_公私_checker_完整_hir_節點逐一相等() {
        // 不只抽查個別來源位置；以正式 P10 / 私有 Checker 各自已
        // 完整檢查的 HIR 逐筆比對，缺少任何一筆亦不得通過 F05。
        for code in [
            "fn main()->int{\nreturn 0\n}\n",
            "fn main()->int{\nvar value:int = 1\nreturn value\n}\n",
            "fn main()->int{\nconst value:int = 0x2a\nreturn value\n}\n",
            "fn main()->int{\nconst value:int = 0b101\nreturn value\n}\n",
            "fn main()->int{\nvar n:int = 0\nif(true){\nn = 2\n}else{\nn = 3\n}\nreturn n\n}\n",
            "fn main()->int{\nvar n:int = 1\nvar alias:int = n\nn = alias\nreturn n\n}\n",
            // MVP-A 僅允許加減與無參數函式呼叫置於 return 表達式；
            // 宣告初始化與賦值仍只准相符型別常值或具名變數。
            // 不可為了擴大交集測試而擅自放寬正式公開語言範圍。
            "fn main()->int{\nvar n:int = 1\nreturn n + 4 - 5\n}\n",
            "fn helper()->int{\nreturn 2\n}\nfn main()->int{\nvar value:int = 1\nreturn helper() + value - 1\n}\n",
            "fn main()->int{\nvar n:int = 1\nif(true){\nn = 2\n}else if(false){\nn = 3\n}else{\nn = 4\n}\nreturn n + 1 - 2\n}\n",
            "fn main()->int{\nvar n:int = 1\nif(false){\nvar n:int = 2\nn = 3\n}else{\nn = 4\n}\nreturn n + 1\n}\n",
        ] {
            assert_eq!(public_check(code), Ok(()), "正式公開來源不合法：{code}");
            let source = Source::new(code.as_bytes().to_vec());
            let tokens = lexer::scan(&source);
            let file = parser::parse(&source, &tokens.tokens).unwrap();
            let facts = names::resolve_with_facts(&file, &source).unwrap();
            let public = types::check_with_names_and_hir(&file, &source, &facts).unwrap();
            assert_eq!(flow::check_with_typed_hir(&file, &source, &facts, &public),
                Ok(()));
            let private = verified_private_source(code).expect(
                "合法純值交集不可因私有安全模型而意外拒絕");

            let pub_refs: Vec<_> = public.verified_references().collect();
            let priv_refs: Vec<_> = private.plan.scalar_hir.verified_references().collect();
            assert_eq!(pub_refs, priv_refs, "公私所有具名讀取 HIR 不一致：{code}");
            let pub_assigns: Vec<_> = public.verified_assignments().collect();
            let priv_assigns: Vec<_> = private.plan.scalar_hir.verified_assignments().collect();
            assert_eq!(pub_assigns, priv_assigns,
                "公私全部賦值目標 HIR 不一致：{code}");
            let pub_expressions: Vec<_> = public.verified_expressions().collect();
            let priv_expressions: Vec<_> =
                private.plan.scalar_hir.verified_expressions().collect();
            assert_eq!(pub_expressions, priv_expressions,
                "公私全部運算式跨度與型別 HIR 不一致：{code}");
        }
    }

    #[test]
    fn f05_交集矩陣不可擅自擴大公開_mvp_a_初始化及賦值運算式() {
        let invalid_public = [
            "fn main()->int{\nvar n:int = 1 + 2\nreturn n\n}\n",
            "fn helper()->int{\nreturn 2\n}\nfn main()->int{\nvar n:int = helper()\nreturn n\n}\n",
            "fn main()->int{\nvar n:int = 1\nn = n + 2\nreturn n\n}\n",
        ];
        for code in invalid_public {
            assert!(public_check(code).is_err(),
                "R02 不得以內部運算式能力打開公開 MVP-A 未核准用法：{code}");
        }
    }

    #[test]
    fn 共同合法_int64_作用域_分支_必須兩套前端都接受() {
        for source in [
            "fn main()->int{\nreturn 0\n}\n",
            "fn main()->int{\nvar value:int = 1\nreturn value\n}\n",
            "fn main()->int{\nconst value:int = 0x2a\nreturn value\n}\n",
            "fn main()->int{\nconst value:int = 0b101\nreturn value\n}\n",
            "fn main()->int{\nvar n:int = 0\nif(true){\nn = 2\n}else{\nn = 3\n}\nreturn n\n}\n",
        ] {
            assert!(public_check(source).is_ok(), "公開交集來源拒絕：{source}");
            assert!(inspect(source).is_ok(), "私有交集來源拒絕：{source}");
        }
    }

    #[test]
    fn 共同不合法整數_未宣告名稱_語法缺失兩套都拒絕() {
        for source in [
            "fn main()->int{\nconst n:int = 9223372036854775808\nreturn n\n}\n",
            "fn main()->int{\nreturn unknown\n}\n",
            "fn main()->int{\nvar value:int = 1\nreturn value\n",
        ] {
            let public = public_check(source)
                .expect_err("公開前端不能假接受共同負例");
            let private = inspect(source)
                .expect_err("私有前端不能假接受共同負例");
            // 部分語意未合併為同一 AST，不比較錯誤種類與文字，
            // 但兩者都必須提供來源位置而非假造成功。
            assert!(public.span.is_some());
            assert!(private.span.is_some());
        }
    }

    #[test]
    fn 私有已核准結構語法絕不可意外開放公開入口() {
        let source = "struct Sample{\nvar n:int = 1\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(source).is_ok());
        assert!(public_check(source).is_err());
    }
}

#[cfg(test)]
mod p16_6_r02_f03_結構欄位共用宣告核心與錯誤隔離 {
    use super::*;
    use crate::DiagnosticKind;

    #[test]
    fn 正式_private_struct_的_var_const_欄位保留原始預設與可變權限() {
        let source = "struct Holder{\nvar value:int\nconst label:str = \"保留🪼\"\nvar count:int = 2\n}\nfn main()->int{\nreturn 0\n}\n";
        assert_eq!(inspect(source), Ok(()));
        let checked = verified_private_source(source).unwrap();
        let holder = checked.structs.get("Holder").unwrap();
        assert_eq!(holder.fields.len(), 3);
        assert_eq!(holder.fields[0].name, "value");
        assert!(!holder.fields[0].immutable);
        assert!(holder.fields[0].default.is_none());
        assert!(holder.fields[1].immutable);
        assert!(holder.fields[1].default.is_some());
        assert_eq!(holder.fields[2].name, "count");
        assert!(!holder.fields[2].immutable);
    }

    #[test]
    fn 共同欄位宣告核心保留歷史診斷種類與來源位置() {
        for (field, expected) in [
            ("const label:str", "const 結構欄位必須具有宣告時預設值"),
            ("var a", "結構欄位必須標註型別"),
            ("var a:int\nvar a:int", "同一結構不可重複欄位名稱"),
            ("share value:int", "結構內不可宣告巢狀 struct 或 share 欄位"),
            ("float value:int", "結構僅接受 var、const 欄位及 fn 方法"),
        ] {
            let code = format!("struct Holder{{\n{field}\n}}\nfn main()->int{{\nreturn 0\n}}\n");
            let error = inspect(&code).expect_err("原本不合法欄位不可由共用解析放寬");
            assert_eq!(error.kind, DiagnosticKind::Syntax);
            assert_eq!(error.message.as_ref(), expected);
            assert!(error.span.is_some(), "仍須保留真正來源位置");
        }
    }

    #[test]
    fn 公開入口不會因私有欄位共用宣告核心就接受_struct() {
        let source = "struct Holder{\nvar a:int = 2\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(source).is_ok());
        let src = Source::new(source.as_bytes().to_vec());
        let lexed = lexer::scan(&src);
        assert!(crate::parser::parse(&src, &lexed.tokens).is_err());
    }
}

#[cfg(test)]
mod p16_6_r02_f02_方法位置引數共用正式呼叫解析 {
    use super::*;

    fn parse_private(code: &str) -> Result<(HashMap<String, StructDecl>, Vec<Function>), Diagnostic> {
        let source = Source::new(code.as_bytes().to_vec());
        let scan = lexer::scan(&source);
        assert!(scan.diagnostics.is_empty());
        Parser { source: &source, tokens: &scan.tokens, cursor: 0 }.file()
    }

    #[test]
    fn 私有方法呼叫零個及多個位置引數保留順序與完整來源跨度() {
        for (args_text, count) in [("", 0), ("1", 1), ("1, 2, 3", 3)] {
            let code = format!("fn main()->int{{\nvar receiver:int = 1\nreturn receiver.work({args_text})\n}}\n");
            let (_, functions) = parse_private(&code).expect("具名方法呼叫語法原本即可解析");
            let StmtKind::Return(term) = &functions[0].body[1].kind
                else { panic!("最後一個語句應為 return") };
            let TermKind::MethodCall(receiver, name, args) = &term.kind
                else { panic!("應保留原私有 MethodCall AST") };
            assert_eq!(receiver, "receiver");
            assert_eq!(name, "work");
            assert_eq!(args.len(), count);
            assert_eq!(term.start, code.find("receiver.work").unwrap());
            assert_eq!(term.end, code.find(")\n}").unwrap() + 1);
        }
    }

    #[test]
    fn 缺逗號或未封閉括號仍使用原來私有方法診斷() {
        for text in ["1 2", "1, 2 3"] {
            let code = format!("fn main()->int{{\nvar receiver:int = 1\nreturn receiver.work({text})\n}}\n");
            let error = parse_private(&code).unwrap_err();
            assert_eq!(&*error.message, "方法位置引數之間需要逗號");
            assert_eq!(error.kind, crate::DiagnosticKind::Syntax);
            assert!(error.span.is_some());
        }
    }
}

#[cfg(test)]
mod r02_f02_私有_parser_完整共通純值_ast {
    use super::*;
    use crate::syntax_shared::ScalarLiteral;
    #[test]
    fn 真正私有_ast_保存十六進位原文及_bool_語意() {
        let code = "fn main()->int{\nvar n:int = 0x2a\nif(true){\nn = 1\n}else{\nn = 2\n}\nreturn n\n}\n";
        let source = Source::new(code.as_bytes().to_vec());
        let scanned = lexer::scan(&source);
        assert!(scanned.diagnostics.is_empty());
        let (_, funcs) = Parser { source:&source, tokens:&scanned.tokens, cursor:0 }.file().unwrap();
        let init = match &funcs[0].body[0].kind {
            StmtKind::Declare(declaration) => declaration.initializer.as_ref()
                .expect("必須有真正的整數初始值"),
            _ => panic!("應有已核准整數宣告"),
        };
        assert!(matches!(&init.kind,
            TermKind::Scalar(ScalarLiteral::Integer(raw)) if raw=="0x2a"));
        let cond = match &funcs[0].body[1].kind {
            StmtKind::If{branch,..} => &branch.condition,
            _ => panic!("應有 if"),
        };
        assert!(matches!(&cond.kind,
            TermKind::Scalar(ScalarLiteral::Boolean(true))));
        let start = code.find("0x2a").unwrap();
        assert_eq!(init.node_span(), crate::common_ir::NodeSpan::new(start,start+4));
        assert!(verified_private_source(code).is_ok());
    }
}

#[cfg(test)]
mod r02_f03_公私正式函式_ast_同一泛型權威 {
    use super::*;

    #[test]
    fn 私有解析器直接交付共同函式_AST_及準確來源_span() {
        let code = "fn main()->int{\nreturn 1\n}\n";
        let source = Source::new(code.as_bytes().to_vec());
        let lexed = lexer::scan(&source);
        assert!(lexed.diagnostics.is_empty());
        let (structs, functions) = Parser {
            source:&source, tokens:&lexed.tokens, cursor:0,
        }.file().unwrap();
        assert!(structs.is_empty());
        let [main] = functions.as_slice() else {panic!("必須恰有一個函式");};
        let _: &crate::syntax_shared::ParsedFunction<
            Vec<Parameter>,String,crate::syntax_shared::ParsedBlock<Stmt>> = main;
        assert_eq!(main.name, "main");
        assert!(main.params.is_empty());
        assert_eq!(main.result_ty, "int");
        assert_eq!(main.start, 0);
        assert_eq!(main.end, code.find('}').unwrap() + 1);
        assert_eq!(inspect(code), Ok(()));

        let public = crate::parser::parse(&source, &lexed.tokens).unwrap();
        let crate::parser::Item::Function(fun) = &public.items[0]
            else {panic!("缺少公開函式");};
        let _: &crate::syntax_shared::ParsedFunction<
            (),crate::common_ir::ScalarType,crate::parser::Block> = fun;
        assert_eq!(fun.start,main.start);
        assert_eq!(fun.end,main.end);
    }
}

#[cfg(test)]
mod r02_f02_私有真正位置引數呼叫_ast_同一資料模型 {
    use super::*;

    #[test]
    fn 私有函式呼叫直接保存共同_ast_名稱與引數_公開仍不能接受相同參數() {
        let code = "fn helper(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn helper(7)\n}\n";
        let source = Source::new(code.as_bytes().to_vec());
        let lexed = lexer::scan(&source);
        assert!(lexed.diagnostics.is_empty());
        let (_, functions) = Parser {source:&source, tokens:&lexed.tokens, cursor:0}
            .file().unwrap();
        let main = functions.iter().find(|f|f.name=="main").unwrap();
        let [Stmt{kind:StmtKind::Return(crate::syntax_shared::ParsedReturn { expression: Term{kind:TermKind::Call(call),..} }),..}] =
            main.body.as_slice() else {panic!("私有正式 Parser 須建立具名呼叫");};
        let _: &crate::syntax_shared::ParsedNamedCall<Vec<Term>> = call;
        assert_eq!(call.name,"helper");
        assert_eq!(call.arguments.len(),1);
        assert!(matches!(call.arguments[0].kind,
            TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(_))));
        assert!(inspect(code).is_ok(),"私有已核准 int 參數呼叫不可因重構被拒絕");
        assert!(crate::parser::parse(&source,&lexed.tokens).is_err(),
            "公開 MVP-A 參數與位置引數仍不可開放");
    }
}

#[cfg(test)]
mod r02_f02_正式具名呼叫單一變體與公開模式隔離 {
    use super::*;
    use crate::{expr, parser};

    #[test]
    fn 共同_ParsedNamedCall_由唯一_enum_宣告生成_保留不同模式引數權限() {
        let code = "fn helper()->int{\nreturn 2\n}\nfn main()->int{\nreturn helper()\n}\n";
        let source = Source::new(code.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        let public = parser::parse(&source, &tokens.tokens).unwrap();
        let parser::Item::Function(main) = &public.items[1]
            else { panic!("缺少公開 main") };
        let parser::StmtKind::Return(expression) = &main.body.statements[0].kind
            else { panic!("公開 main 必須回傳真正具名呼叫") };
        let expr::ExprKind::Call(public_call) = &expression.expression.kind
            else { panic!("公開正式 ExprKind 必須使用共同 Call 變體") };
        let _: &crate::syntax_shared::ParsedNamedCall<()> = public_call;
        assert_eq!(public_call.name, "helper");
        assert_eq!(
            &code[expression.expression.start..expression.expression.end], "helper()",
        );

        // 零引數 helper() 在私有受限 Parser 中仍要先保持
        // Construct/Call 的既有曖昧判定；不能把它改當所有權資源建構
        // 或擅自打破 P13 之後依已知 struct 名稱決定的核准規則。
        let (_, private) = Parser {
            source: &source, tokens: &tokens.tokens, cursor: 0,
        }.file().unwrap();
        let StmtKind::Return(returned) = &private[1].body[0].kind
            else { panic!("缺少私有 main return") };
        let TermKind::Construct(name, fields) = &returned.expression.kind
            else { panic!("無參數呼叫仍須保留私有名稱消歧義的原 AST") };
        assert_eq!(name, "helper");
        assert!(fields.is_empty());
        assert_eq!(inspect(code), Ok(()));
    }
}

#[cfg(test)]
mod r02_f03_私有真正函式區塊使用共同_parser_ast {
    use super::*;

    #[test]
    fn 公私函式區塊共用_parsedblock_且保留_utf8_半開跨度() {
        let code="fn main()->int{\nvar n:int = 2\nreturn n\n}\n";
        let source=Source::new(code.as_bytes().to_vec());
        let lexed=lexer::scan(&source);
        let (structs,functions)=Parser{source:&source,tokens:&lexed.tokens,cursor:0}
            .file().unwrap();
        assert!(structs.is_empty());
        let [main]=functions.as_slice() else {panic!("應有唯一 main");};
        let _: &crate::syntax_shared::ParsedBlock<Stmt> = &main.body;
        let _: &crate::syntax_shared::ParsedFunction<
            Vec<Parameter>,String,crate::syntax_shared::ParsedBlock<Stmt>> = main;
        assert_eq!(main.body.start,code.find('{').unwrap());
        assert_eq!(main.body.end,code.find('}').unwrap()+1);
        assert_eq!(main.body.statements.len(),2);
        assert!(matches!(main.body[0].kind,StmtKind::Declare(_)));
        assert!(verified_private_source(code).is_ok());
        let public=crate::parser::parse(&source,&lexed.tokens).unwrap();
        let crate::parser::Item::Function(fun)=&public.items[0]
            else {panic!("公開入口應有同形函式");};
        assert_eq!(fun.body.start,main.body.start);
        assert_eq!(fun.body.end,main.body.end);
    }
}

#[cfg(test)]
mod r02_f04_公私正式運算式_typed_hir_同一加減證據 {
    use super::*;
    use crate::{names,parser,types};

    #[test]
    fn 公私真實_checker_已核准左結合運算證據逐筆相同() {
        let code="fn main()->int{\nvar n:int = 1\nreturn n + 2 - 3\n}\n";
        let src=Source::new(code.as_bytes().to_vec());
        let tokens=lexer::scan(&src);
        let parsed=parser::parse(&src,&tokens.tokens).unwrap();
        let named=names::resolve_with_facts(&parsed,&src).unwrap();
        let public=types::check_with_names_and_hir(&parsed,&src,&named).unwrap();
        let private=verified_private_source(code).unwrap();
        let public_arithmetic:Vec<_>=public.verified_arithmetic().collect();
        let private_arithmetic:Vec<_>=private.plan.scalar_hir.verified_arithmetic().collect();
        assert_eq!(public_arithmetic.len(),2,"兩筆左結合運算必須完整留存");
        assert_eq!(public_arithmetic,private_arithmetic);
        assert!(public.source_spans_are_valid(code));
        assert!(private.plan.scalar_hir.source_spans_are_valid(code));
        let first=crate::common_ir::NodeSpan::new(
            code.find("n + 2").unwrap(),
            code.find("n + 2").unwrap()+"n + 2".len());
        assert_eq!(public.arithmetic(first).map(|fact|fact.operator),Some('+'));
        assert_eq!(private.plan.scalar_hir.arithmetic(first),public.arithmetic(first));
        for illegal in ["true + 1","1 - false"] {
            let source=format!("fn main()->int{{\nreturn {illegal}\n}}\n");
            assert!(verified_private_source(&source).is_err());
            let src=Source::new(source.as_bytes().to_vec());
            let lexed=lexer::scan(&src);
            let ast=parser::parse(&src,&lexed.tokens).unwrap();
            let facts=names::resolve_with_facts(&ast,&src).unwrap();
            assert!(types::check_with_names_and_hir(&ast,&src,&facts).is_err());
        }
    }
}

#[cfg(test)]
mod r02_f03_私有真正_if_分支直接保存共同來源身分 {
    use super::*;
    #[test]
    fn 真正解析器_if_else_if_每一來源起點均來自唯一共同_ast() {
        let code="fn main()->int{\nif(true){\nreturn 1\n}else if(false){\nreturn 2\n}else{\nreturn 3\n}\nreturn 0\n}\n";
        let source=Source::new(code.as_bytes().to_vec());
        let lexed=lexer::scan(&source);
        let (_,functions)=Parser {source:&source,tokens:&lexed.tokens,cursor:0}
            .file().unwrap();
        let main=&functions[0];
        let StmtKind::If {branch, no}=&main.body[0].kind
            else {panic!("應有 if");};
        let _: &crate::syntax_shared::ParsedIfArm<
            Term, crate::syntax_shared::ParsedBlock<Stmt>
        >=branch;
        assert_eq!(&code[branch.body.start..branch.body.end], "{\nreturn 1\n}");
        assert_eq!(branch.start,code.find("if(true)").unwrap());
        let otherwise=no.as_ref().expect("應有 else-if");
        assert!(matches!(otherwise, crate::syntax_shared::ParsedElse::ElseIf(_)),
            "Parser 必須保留正式 else-if 來源身分而非 Vec 投影");
        let [next]=otherwise.as_slice() else {panic!("else-if 不可丟失");};
        let StmtKind::If {branch:second,no:tail}=&next.kind
            else {panic!("應由原安全 Checker 辨識巢狀 if");};
        assert_eq!(second.start,code.find("if(false)").unwrap());
        assert_eq!(&code[second.body.start..second.body.end], "{\nreturn 2\n}");
        assert_eq!(second.start,next.start);
        assert_eq!(tail.as_ref().expect("缺少 else").len(),1);
        assert!(verified_private_source(code).is_ok(),
            "保持原 P13 巢狀 if 安全合流及資源證明");
    }
}

#[cfg(test)]
mod r02_f03_私有_else_直接使用共同_parser_區塊 {
    use super::*;

    #[test]
    fn 正式_else_保留來源大括號且不再轉換成_vec_語句() {
        let src = "fn main()->int{\nvar result:int = 1\nif(false){\nresult = 2\n}else{\nresult = 3\n}\nreturn result\n}\n";
        let source = Source::new(src.as_bytes().to_vec());
        let lexed = lexer::scan(&source);
        let (_, funcs) = Parser {
            source: &source, tokens: &lexed.tokens, cursor: 0,
        }.file().unwrap();
        let main = &funcs[0];
        let StmtKind::If { branch, no } = &main.body[1].kind
            else { panic!("真正 if 未建立") };
        assert_eq!(&src[branch.body.start..branch.body.end],
            "{\nresult = 2\n}");
        let tail = no.as_ref().expect("真正 else 分支應保留");
        let crate::syntax_shared::ParsedElse::Block(otherwise) = tail
            else { panic!("真正 Parser 的 else 必須直接保存 ParsedBlock") };
        assert_eq!(&src[otherwise.start..otherwise.end],
            "{\nresult = 3\n}");
        assert_eq!(otherwise.len(), 1);
        assert!(verified_private_source(src).is_ok());
    }
}

#[cfg(test)]
mod r02_f02_公私正式算術結點唯一變體與_operator_權威 {
    use super::*;
    use crate::{expr, parser};

    #[test]
    fn 雙正式_parser_皆以_binaryint_與同一原始跨度記錄左結合加減() {
        let source_text = "fn main()->int{\nvar n:int = 2\nreturn n + 4 - 1\n}\n";
        let source = Source::new(source_text.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        let public = parser::parse(&source, &tokens.tokens).unwrap();
        let parser::Item::Function(fun) = &public.items[0]
            else { panic!("缺少公開函式") };
        let parser::StmtKind::Return(returned) = &fun.body.statements[1].kind
            else { panic!("缺少公開算術返回") };
        let expr::ExprKind::BinaryInt(public_outer) = &returned.expression.kind
            else { panic!("公開算術不能重複分成 Add/Sub AST 權威") };
        let expr::ExprKind::BinaryInt(public_inner) = &public_outer.left.kind
            else { panic!("左結合應直接保存單一內層二元 AST") };
        assert_eq!((public_outer.operator, public_inner.operator), ('-', '+'));

        let private = verified_private_source(source_text).unwrap();
        let StmtKind::Return(private_return) = &private.functions[0].body[1].kind
            else { panic!("缺少私有算術返回") };
        let TermKind::BinaryInt(private_outer) = &private_return.expression.kind
            else { panic!("私有仍須使用共通 ArithmeticNode") };
        let TermKind::BinaryInt(private_inner) = &private_outer.left.kind
            else { panic!("私有內層同樣遵守左結合") };
        assert_eq!(public_outer.operator, private_outer.operator);
        assert_eq!(public_inner.operator, private_inner.operator);
        for (left, right) in [
            (public_outer.left.node_span(), private_outer.left.node_span()),
            (public_outer.right.node_span(), private_outer.right.node_span()),
            (public_inner.left.node_span(), private_inner.left.node_span()),
            (public_inner.right.node_span(), private_inner.right.node_span()),
        ] {
            assert_eq!(left, right, "不容許獨立 Parser 重造另一份跨度");
        }
        let bindings = crate::names::resolve_with_facts(&public, &source).unwrap();
        let typed = crate::types::check_with_names_and_hir(&public, &source, &bindings).unwrap();
        assert!(crate::flow::check_with_typed_hir(
            &public, &source, &bindings, &typed).is_ok(),
            "此項必須通過 P09→P10 Typed HIR→P11 正式語意路徑");
    }
}

#[cfg(test)]
mod r02_f02_真正公私_scalar_literal_ast_同一節點種類 {
    use super::*;
    use crate::{expr,parser};
    #[test]
    fn 公私真正_int_bool_ast_直接持有同一_scalar_literal_無第二次組樹() {
        let code="fn main()->int{\nvar n:int = 0x2a\nif(true){\nreturn n\n}\nreturn 0\n}\n";
        let source=Source::new(code.as_bytes().to_vec());
        let tokens=lexer::scan(&source);
        let public=parser::parse(&source,&tokens.tokens).unwrap();
        let private=verified_private_source(code).unwrap();
        let parser::Item::Function(pubfun)=&public.items[0]
            else {panic!("公開函式應存在");};
        let parser::StmtKind::Binding(pubdecl)=&pubfun.body[0].kind
            else {panic!("公開 int 宣告必須由共同 AST 構成");};
        let pubinit=pubdecl.initializer.as_ref().unwrap();
        let StmtKind::Declare(privdecl)=&private.functions[0].body[0].kind
            else {panic!("私有 int 宣告必須由共同 AST 構成");};
        let privinit=privdecl.initializer.as_ref().unwrap();
        let expr::ExprKind::Scalar(pubatom)=&pubinit.kind
            else {panic!("公開 Parser 不能再建第二套 int 節點");};
        let TermKind::Scalar(privatom)=&privinit.kind
            else {panic!("私有 Parser 應直接產生共同 ScalarLiteral");};
        assert_eq!(pubatom,privatom);
        assert_eq!(pubinit.node_span(),privinit.node_span());
        let parser::StmtKind::IfChain(chain)=&pubfun.body[1].kind
            else {panic!("公開 if 必須由共同解析器構成");};
        let TermKind::Scalar(private_bool)=&
            (match &private.functions[0].body[1].kind {
                StmtKind::If {branch,..} => &branch.condition,
                _=>panic!("私有 if 必須由共同解析器構成")
            }).kind else {panic!("私有 bool 應為 Scalar");};
        let expr::ExprKind::Scalar(public_bool)=&chain.arms[0].branch.condition.kind
            else {panic!("公開 bool 應為 Scalar");};
        assert_eq!(public_bool,private_bool);
        assert!(matches!(public_bool,crate::syntax_shared::ScalarLiteral::Boolean(true)));
        assert!(crate::common_ir::NodeIdentity::checked(
            pubinit.node_span(),crate::common_ir::NodeRole::Expression,code).is_some());
    }
}

#[cfg(test)]
mod r02_f04_真正公私算術型別錯誤仍保持各自核准診斷 {
    use super::*;
    use crate::{names,parser,types};
    #[test]
    fn bool_及字串運算不得因共用型別檢查器偷渡合法化() {
        let invalid=[
            "fn main()->int{\nreturn true + 1\n}\n",
            "fn main()->int{\nreturn 1 - false\n}\n",
        ];
        for code in invalid {
            let source=Source::new(code.as_bytes().to_vec());
            let lexed=lexer::scan(&source);
            let public=parser::parse(&source,&lexed.tokens).unwrap();
            let facts=names::resolve_with_facts(&public,&source).unwrap();
            let pe=types::check_with_names_and_hir(&public,&source,&facts).unwrap_err();
            assert_eq!(pe.kind, DiagnosticKind::Static);
            assert!(pe.message.contains("整數加減"),
                "公開 P10 診斷不可被私有所有權診斷取代");
            let re=verified_private_source(code).err().expect("已核准私有型別錯誤必須拒絕");
            assert_eq!(re.kind, DiagnosticKind::Static);
            assert!(re.message.contains("P13：加減"),
                "私有既有 P13 診斷不得被公開 MVP-A 診斷取代");
        }
    }
}

#[cfg(test)]
mod r02_f04_公私共同_bool_checker_真實條件回歸 {
    use super::*;
    use crate::{names,parser,types,flow};
    #[test]
    fn 真實_if_合法來源保持一致_整數條件保留原拒絕() {
        let valid="fn main()->int{\nvar flag:bool = true\nif(flag){\nreturn 1\n}\nreturn 0\n}\n";
        let source=Source::new(valid.as_bytes().to_vec());
        let scanned=lexer::scan(&source);
        let file=parser::parse(&source,&scanned.tokens).unwrap();
        let facts=names::resolve_with_facts(&file,&source).unwrap();
        let hir=types::check_with_names_and_hir(&file,&source,&facts).unwrap();
        assert!(flow::check_with_typed_hir(&file,&source,&facts,&hir).is_ok());
        assert!(verified_private_source(valid).is_ok());
        let invalid=valid.replace("if(flag)","if(1)");
        let source=Source::new(invalid.as_bytes().to_vec());
        let scanned=lexer::scan(&source);
        assert!(parser::parse(&source,&scanned.tokens).is_err());
        assert!(verified_private_source(&invalid).is_err());
    }
}

#[cfg(test)]
mod r02_f03_真正公私_return_直接保存共同_parsedreturn {
    use super::*;
    #[test]
    fn 公私_parser_唯一共同_return_ast_仍保留精確來源跨度與型別() {
        let code="fn main()->int{\nreturn 7\n}\n";
        let source=Source::new(code.as_bytes().to_vec());
        let lexed=lexer::scan(&source);
        let public=crate::parser::parse(&source,&lexed.tokens).unwrap();
        let crate::parser::Item::Function(pubfun)=&public.items[0]
            else {panic!("必須有合法公開函式");};
        let crate::parser::StmtKind::Return(pubreturn)=&pubfun.body[0].kind
            else {panic!("公開 return 沒有共同 AST");};
        let _: &crate::syntax_shared::ParsedReturn<crate::expr::Expr>=pubreturn;
        let (_,private)=Parser {source:&source,tokens:&lexed.tokens,cursor:0}
            .file().unwrap();
        let StmtKind::Return(privreturn)=&private[0].body[0].kind
            else {panic!("私有 return 沒有共同 AST");};
        let _: &crate::syntax_shared::ParsedReturn<Term>=privreturn;
        assert_eq!(pubreturn.node_span(),privreturn.node_span());
        assert_eq!(pubreturn.start,code.find('7').unwrap());
        assert_eq!(pubreturn.end,pubreturn.start+1);
        assert!(verified_private_source(code).is_ok());
        let named=crate::names::resolve_with_facts(&public,&source).unwrap();
        let typed=crate::types::check_with_names_and_hir(&public,&source,&named).unwrap();
        assert!(crate::flow::check_with_typed_hir(&public,&source,&named,&typed).is_ok());
        assert!(crate::functions::check(&public,&source).is_ok(),
            "公開正式 P09→P10→P11→P12 不得因 return AST 轉換回歸");
    }
}
