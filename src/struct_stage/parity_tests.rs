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
        let [Stmt{kind:StmtKind::Return(Term{kind:TermKind::Call(call),..}),..}] =
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
