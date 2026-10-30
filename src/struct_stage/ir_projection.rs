// P16.6-R02.2：既有私有 AST 對已核准 scalar 交集的投影。
// 不建立公開解析入口、不改私有 Checker、不用 AST 投影反向放寬接受條件。

fn private_scalar_expression(term: &Term, text: &str)
    -> Option<crate::common_ir::ScalarExpr> {
    use crate::common_ir::ScalarExpr;
    match &term.kind {
        TermKind::Literal("int") => ScalarExpr::Int(
            crate::syntax_shared::integer_i64(text.get(term.start..term.end)?).ok()?
        ),
        TermKind::Literal("bool") => ScalarExpr::Bool(
            match text.get(term.start..term.end)? {
                "true" => true, "false" => false, _ => return None,
            }
        ),
        TermKind::Name(name) => ScalarExpr::Name(name.clone()),
        TermKind::BinaryInt { left, operator, right }
            if matches!(operator, '+' | '-') => ScalarExpr::Binary {
            left: Box::new(private_scalar_expression(left, text)?),
            op: *operator,
            right: Box::new(private_scalar_expression(right, text)?),
        },
        // 既有私有 Parser 以 Construct(name, []) 表示 name() 的
        // 無位置引數形式，須先確定沒有同名結構才可投影一般函式呼叫。
        TermKind::Call(name, args) if args.is_empty() =>
            ScalarExpr::CallZero(name.clone()),
        TermKind::Construct(name, args) if args.is_empty() =>
            ScalarExpr::CallZero(name.clone()),
        _ => return None,
    }.into()
}

fn private_scalar_statements(stmts: &[Stmt], text: &str)
    -> Option<Vec<crate::common_ir::ScalarStmt>> {
    use crate::common_ir::{ScalarStmt, ScalarType};
    stmts.iter().map(|statement| Some(match &statement.kind {
        StmtKind::Declare { name, ty, mutable, init } => ScalarStmt::Declare {
            declaration: DeclarationId(statement.start),
            name: name.clone(),
            ty: ScalarType::from_name(ty)?,
            mutable: *mutable,
            init: match init {
                Some(value) => Some(private_scalar_expression(value, text)?),
                None => None,
            },
        },
        StmtKind::Assign { name, value } => ScalarStmt::Assign {
            name: name.clone(), value: private_scalar_expression(value, text)?,
        },
        StmtKind::If { condition, yes, no } => ScalarStmt::If {
            condition: private_scalar_expression(condition, text)?,
            yes: private_scalar_statements(yes, text)?,
            no: match no {
                Some(branch) => Some(private_scalar_statements(branch, text)?),
                None => None,
            },
        },
        StmtKind::Return(expr) =>
            ScalarStmt::Return(private_scalar_expression(expr, text)?),
        _ => return None,
    })).collect()
}

/// 只包含未涉及所有權／借用／結構的標量交集；不作 P13 全語法投影。
fn private_to_common_ast(
    structs: &HashMap<String, StructDecl>, functions: &[Function], text: &str,
) -> Option<crate::common_ir::SharedAst> {
    use crate::common_ir::{ScalarFunction, ScalarType, SharedAst};
    if !structs.is_empty() { return None; }
    let functions = functions.iter().map(|function| {
        if !function.params.is_empty() { return None; }
        let result_ty = ScalarType::from_name(&function.result_ty)?;
        Some(ScalarFunction {
            name: function.name.clone(),
            result_ty,
            body: private_scalar_statements(&function.body, text)?,
        })
    }).collect::<Option<Vec<_>>>()?;
    Some(SharedAst { functions })
}

#[cfg(test)]
mod p16_6_r02_2_共同語意樹驗證 {
    use super::*;
    use crate::{common_ir, parser};

    fn compare(text: &str) -> common_ir::SharedAst {
        assert!(inspect(text).is_ok(), "私有 P13 安全來源仍須先通過 Checker");
        let source = Source::new(text.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        assert!(tokens.diagnostics.is_empty());
        let public = parser::parse(&source, &tokens.tokens)
            .expect("公開 MVP-A 文法應解析共同交集");
        let (structs, functions) =
            Parser { source: &source, tokens: &tokens.tokens, cursor: 0 }
                .file().expect("私有受限文法應解析共同交集");
        let actual = private_to_common_ast(&structs, &functions, text)
            .expect("私有必須可以表達已核准交集");
        let expected = common_ir::from_public(&public)
            .expect("公開必須可以表達已核准交集");
        assert_eq!(actual, expected, "不得因不同 parser 產生不同語意樹");
        actual
    }

    #[test]
    fn scalar_int64_與分支合流的共同宣告身分及語法樹完全一致() {
        let code = "fn main()->int{\nvar n:int = 0x2a\nif(true){\nn = 1\n}else{\nn = 2\n}\nreturn n\n}\n";
        let tree = compare(code);
        assert_eq!(tree.functions.len(), 1);
        let common_ir::ScalarStmt::Declare { declaration, .. } =
            &tree.functions[0].body[0] else { panic!("第一個結點必須是宣告"); };
        assert_eq!(code.as_bytes()[declaration.0], b'v');
    }

    #[test]
    fn 多個無參數函式的共同結點順序一致() {
        compare("fn helper()->int{\nreturn 2\n}\nfn main()->int{\nreturn helper()\n}\n");
    }

    #[test]
    fn else_if_多分支與單一_if_巢狀表示應完全一致() {
        let source = "fn main()->int{\nvar n:int = 0\nif(false){\nn = 1\n}else if(true){\nn = 2\n}else if(false){\nn = 3\n}else{\nn = 4\n}\nreturn n\n}\n";
        let ast = compare(source);
        let crate::common_ir::ScalarStmt::If { no: Some(else_chain), .. } =
            &ast.functions[0].body[1] else { panic!("第一分支必須存在下一個 else-if"); };
        let [crate::common_ir::ScalarStmt::If { no: Some(third), .. }] =
            else_chain.as_slice() else { panic!("else-if 第二分支需延續"); };
        let [crate::common_ir::ScalarStmt::If { no: Some(last), .. }] =
            third.as_slice() else { panic!("else-if 第三分支需延續"); };
        assert!(matches!(last.as_slice(), [crate::common_ir::ScalarStmt::Assign { .. }]));
    }

    #[test]
    fn 沒有_else_之_else_if_需保留所有可達後繼分支() {
        let source = "fn main()->int{\nvar n:int = 0\nif(false){\nn = 1\n}else if(true){\nn = 2\n}\nreturn n\n}\n";
        let tree = compare(source);
        let crate::common_ir::ScalarStmt::If { no: Some(else_branch), .. } =
            &tree.functions[0].body[1] else { panic!("第二 if 應在 else 內"); };
        assert!(matches!(else_branch.as_slice(), [
            crate::common_ir::ScalarStmt::If { no: None, .. }
        ]));
    }

    #[test]
    fn 未初始化_var_須投影為_init_none_而非讓整個_ast_失敗() {
        let source = "fn main()->int{\nvar value:int\nvalue = 1\nreturn value\n}\n";
        let tree = compare(source);
        assert!(matches!(&tree.functions[0].body[0],
            crate::common_ir::ScalarStmt::Declare { init: None, .. }));
    }

    #[test]
    fn 巢狀遮蔽宣告即使同名也必須保留不同_declaration_id() {
        let source = "fn main()->int{\nvar value:int = 1\nif(true){\nvar value:int = 2\n}\nreturn value\n}\n";
        let tree = compare(source);
        let crate::common_ir::ScalarStmt::Declare { declaration: outer, .. } =
            &tree.functions[0].body[0] else { panic!("外層來源不應消失"); };
        let crate::common_ir::ScalarStmt::If { yes, .. } =
            &tree.functions[0].body[1] else { panic!("應保留分支"); };
        let [crate::common_ir::ScalarStmt::Declare { declaration: inner, .. }] =
            yes.as_slice() else { panic!("內層宣告應完整保留"); };
        assert_ne!(outer, inner, "詞彙名稱相同也不可視為同一所有權宣告");
        assert_eq!(&source[outer.0..outer.0 + 3], "var");
        assert_eq!(&source[inner.0..inner.0 + 3], "var");
    }

    #[test]
    fn 非純值宣告即使沒有右值也不得投影成共同標量_ast() {
        for src in [
            "fn main()->int{\nvar text:str\nreturn 0\n}\n",
            "fn main()->int{\nvar data:list\nreturn 0\n}\n",
        ] {
            let source = Source::new(src.as_bytes().to_vec());
            let tokens = lexer::scan(&source);
            let (structs, funcs) = Parser {
                source: &source, tokens: &tokens.tokens, cursor: 0
            }.file().expect("此處只測解析後 AST 投影是否拒絕非純值型別");
            assert!(private_to_common_ast(&structs, &funcs, src).is_none(),
                "不得把 str 或容器錯認為可安全共用的 int/bool AST");
        }
    }

    #[test]
    fn 非純值函式回傳型別不能由通用字串假冒共同_ast() {
        let src = "fn create()->str{\nreturn \"字串\"\n}\n";
        let source = Source::new(src.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        let (structs, funcs) = Parser {
            source: &source, tokens: &tokens.tokens, cursor: 0
        }.file().expect("私有受限語法可解析 str 返回");
        assert!(private_to_common_ast(&structs, &funcs, src).is_none());
    }

    #[test]
    fn 公開與私有解析器真正共用的加減運算維持左結合及括號次序() {
        let src = "fn main()->int{\nvar n:int = 10\nreturn n - 3 + (2 - 1)\n}\n";
        let ast = compare(src);
        let crate::common_ir::ScalarStmt::Return(expr) =
            &ast.functions[0].body[1] else {panic!("應保留 return 表達式");};
        let crate::common_ir::ScalarExpr::Binary {left,op:'+',right} = expr
            else {panic!("最外層應是最後一個加號運算");};
        assert!(matches!(&**left,crate::common_ir::ScalarExpr::Binary {op:'-',..}));
        assert!(matches!(&**right,crate::common_ir::ScalarExpr::Binary {op:'-',..}));
    }

    #[test]
    fn 非共有的資源語法不得冒充共同標量_ir() {
        let text = "fn main()->int{\nvar text:str = \"中文\"\nreturn 0\n}\n";
        let source = Source::new(text.as_bytes().to_vec());
        let lexed = lexer::scan(&source);
        let (structs, funs) = Parser {
            source: &source, tokens: &lexed.tokens, cursor: 0
        }.file().unwrap();
        assert!(private_to_common_ast(&structs, &funs, text).is_none());
    }
}
