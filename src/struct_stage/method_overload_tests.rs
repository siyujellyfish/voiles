#[cfg(test)]
mod g18_private_method_overload_tests {
    use super::*;

    fn checked(source: &str) -> VerifiedPrivateSource {
        verify_source_with_ability(
            source, crate::ownership_policy::approved_leaf_copy_permission,
        ).expect("S20 已核准唯一靜態方法簽章須能通過原私有 Checker")
    }

    fn source(methods: &str, receiver: &str, argument: &str) -> String {
        format!(
            "struct Counter{{\nvar n:int = 1\n{methods}}}\nfn main()->int{{\n{receiver}\nreturn owner.choose({argument})\n}}\n"
        )
    }

    fn methods() -> &'static str {
        "fn choose(value:int)->int{\nreturn self.n\n}\nfn choose(flag:bool)->int{\nself.n = 3\nreturn self.n\n}\n"
    }

    #[test]
    fn g18_f_原_parser_保留同名不同參數簽章全部方法候選順序() {
        let src = source(methods(), "var owner:Counter = Counter()", "5");
        let verified = checked(&src);
        let definition = &verified.structs["Counter"];
        assert_eq!(definition.methods.len(), 2);
        assert!(definition.method("choose").is_none());
        let cases: Vec<Vec<String>> = definition.methods_named("choose")
            .map(|f| f.params.iter().map(|p| p.ty.clone()).collect()).collect();
        assert_eq!(cases, [vec!["int".to_owned()], vec!["bool".to_owned()]]);
    }

    #[test]
    fn g18_f_同名_int_多載只用完整靜態參數型別取得真共享借用() {
        let src = source(methods(), "var owner:Counter = Counter()", "5");
        let verified = checked(&src);
        let main = verified.functions.iter().find(|f| f.name == "main").unwrap();
        let StmtKind::Return(ret) = &main.body.statements[1].kind else { panic!() };
        assert_eq!(verified.plan.declarations.call_borrows_at(
            ret.expression.node_span(),
        ), Some(&[BorrowFact::new(
            DeclarationId(main.body.statements[0].start), BorrowKind::SharedRead,
        )][..]));
    }

    #[test]
    fn g18_f_同名_bool_多載從原_checker_選獨占_self_不得誤選唯讀() {
        let src = source(methods(), "var owner:Counter = Counter()", "true");
        let verified = checked(&src);
        let main = verified.functions.iter().find(|f| f.name == "main").unwrap();
        let StmtKind::Return(ret) = &main.body.statements[1].kind else { panic!() };
        assert_eq!(verified.plan.declarations.call_borrows_at(
            ret.expression.node_span(),
        ), Some(&[BorrowFact::new(
            DeclarationId(main.body.statements[0].start), BorrowKind::ExclusiveWrite,
        )][..]));
    }

    #[test]
    fn g18_f_同名多載方法宣告顛倒不影響靜態唯一選擇或借用權限() {
        let reversed = "fn choose(flag:bool)->int{\nself.n = 3\nreturn self.n\n}\nfn choose(value:int)->int{\nreturn self.n\n}\n";
        let src = source(reversed, "var owner:Counter = Counter()", "5");
        let verified = checked(&src);
        let main = &verified.functions[0];
        let StmtKind::Return(ret) = &main.body.statements[1].kind else { panic!() };
        assert_eq!(verified.plan.declarations.call_borrows_at(ret.expression.node_span()),
            Some(&[BorrowFact::new(
                DeclarationId(main.body.statements[0].start), BorrowKind::SharedRead,
            )][..]));
    }

    #[test]
    fn g18_f_同名多載不可憑返回型別或參數名稱視為不同簽章() {
        for methods in [
            "fn choose(first:int)->int{\nreturn first\n}\nfn choose(second:int)->int{\nreturn second\n}\n",
            "fn choose(first:int)->int{\nreturn first\n}\nfn choose(second:int)->bool{\nreturn true\n}\n",
        ] {
            let src = source(methods, "var owner:Counter = Counter()", "5");
            let err = verify_source_with_ability(
                &src, crate::ownership_policy::approved_leaf_copy_permission,
            ).unwrap_err();
            assert_eq!(err.kind, DiagnosticKind::Static);
            assert!(err.message.contains("S20"), "{}", err.message);
        }
    }

    #[test]
    fn g18_f_const_結構僅允許與參數型別匹配的共享唯讀多載() {
        let valid = source(methods(), "const owner:Counter = Counter()", "5");
        checked(&valid);
        let invalid = source(methods(), "const owner:Counter = Counter()", "true");
        let err = verify_source_with_ability(
            &invalid, crate::ownership_policy::approved_leaf_copy_permission,
        ).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("const 結構實例"), "{}", err.message);
    }

    #[test]
    fn g18_f_同名多載參數數量或型別無匹配必靜態拒絕() {
        for argument in ["\"文字\"", "true, 3", "unknown"] {
            let src = source(methods(), "var owner:Counter = Counter()", argument);
            let err = verify_source_with_ability(
                &src, crate::ownership_policy::approved_leaf_copy_permission,
            ).unwrap_err();
            assert_eq!(err.kind, DiagnosticKind::Static);
        }
    }

    #[test]
    fn g18_f_欄位方法同名無論宣告順序仍由原_parser_拒絕() {
        for body in [
            "var choose:int = 1\nfn choose()->int{\nreturn 1\n}\n",
            "fn choose()->int{\nreturn 1\n}\nvar choose:int = 1\n",
        ] {
            let src = format!(
                "struct Counter{{\n{body}}}\nfn main()->int{{\nreturn 0\n}}\n",
            );
            let err = verify_source_with_ability(
                &src, crate::ownership_policy::approved_leaf_copy_permission,
            ).unwrap_err();
            assert_eq!(err.kind, DiagnosticKind::Syntax);
        }
    }

    #[test]
    fn g18_f_原同名唯一方法索引與正常非多載不因新候選表退化() {
        let src = "struct Counter{\nvar n:int = 1\nfn read()->int{\nreturn self.n\n}\n}\nfn main()->int{\nconst owner:Counter = Counter()\nreturn owner.read()\n}\n";
        let verified = checked(src);
        let definition = &verified.structs["Counter"];
        assert_eq!(definition.method("read").unwrap().name, "read");
        assert_eq!(definition.methods_named("read").count(), 1);
    }
}
