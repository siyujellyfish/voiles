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
        types::check_with_names(&ast, &source, &facts)?;
        flow::check_with_names(&ast, &source, &facts)?;
        functions::check(&ast, &source)?;
        Ok(())
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
