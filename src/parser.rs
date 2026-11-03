//! P07：只接受 P01 的單檔 MVP-A 文法，形成可檢查的內部語法樹。
//! 完整語言 struct／Optional／share／字串等規則不因首版拒絕而永久非法。

use crate::{Diagnostic, DiagnosticKind};
use crate::expr::{Expr, ExprKind, ExprParser};
use crate::lexer::{Token, TokenKind};
use crate::source::Source;

// R02.6：公開 MVP-A Parser、P09～P11 與共同 AST 直接使用同一個已封閉
// 的 int／bool 專用型別，不再維護一份同形但分離的列舉型別。
pub(crate) use crate::common_ir::ScalarType as Type;

#[derive(Debug, Clone)]
pub(crate) struct File {
    pub(crate) items: Vec<Item>,
    // P08 AST 位置檢視仍需保留起迄跨度；非公開檢查流程不直接讀取。
    #[allow(dead_code)]
    pub(crate) start: usize,
    #[allow(dead_code)]
    pub(crate) end: usize,
}

#[derive(Debug, Clone)]
pub(crate) enum Item {
    Function(Function),
    Binding(Binding),
}

/// F03：公開 P01 的真正函式 AST 直接是共通 Parser 產物。
/// Params=() 僅表示已核准 P01 零參數，不放寬私有 P13 參數。
pub(crate) type Function = crate::syntax_shared::ParsedFunction<(), Type, Block>;

/// 公開 P01 的真正區塊 AST 直接由共通 parse_block 建構。
pub(crate) type Block = crate::syntax_shared::ParsedBlock<Stmt>;

/// F03：真正公開語句與頂層宣告 AST 直接持有 Parser 共通
/// `ParsedDeclaration`，不再經第二套同欄位的獨立 Binding 模型。
/// 私有 Parser 亦直接由相同 parse_declaration 產生此節點後，
/// 才由原安全 AST 型別隔離其擴充的語意。
pub(crate) type Binding = crate::syntax_shared::ParsedDeclaration<Type, Expr>;

/// 公開 MVP-A 語句 AST 的位元組範圍結構與私有 P13 共用。
pub(crate) type Stmt = crate::common_ir::SpannedNode<StmtKind>;

#[derive(Debug, Clone)]
pub(crate) enum StmtKind {
    Binding(Binding),
    Assignment(crate::syntax_shared::ParsedNamedAssignment<Expr>),
    /// F03：公開正式 AST 直接持有共用 if/else-if Parser 的完整結果。
    IfChain(crate::syntax_shared::ParsedIfChain<Expr, Block>),
    Return(crate::syntax_shared::ParsedReturn<Expr>),
}

struct Parser<'a> {
    source: &'a Source,
    tokens: &'a [Token],
    cursor: usize,
    /// 僅供 P13 內部來源試驗使用，不變更 P01 正式解析模式。
    allow_probe_move: bool,
}

impl<'a> Parser<'a> {
    fn current(&self) -> Option<&'a Token> { self.tokens.get(self.cursor) }
    fn next(&mut self) -> Option<&'a Token> {
        let value = self.current();
        if value.is_some() { self.cursor += 1; }
        value
    }
    fn symbol(&self, ch: char) -> bool {
        crate::syntax_shared::symbol(self.tokens, self.cursor, ch)
    }
    fn keyword(&self, expected: &str) -> bool {
        crate::syntax_shared::keyword(self.tokens, self.cursor, expected)
    }
    fn at_end(&self) -> bool {
        crate::syntax_shared::end(self.tokens, self.cursor)
    }
    fn at_newline(&self) -> bool {
        crate::syntax_shared::newline(self.tokens, self.cursor)
    }
    fn skip_newlines(&mut self) {
        crate::syntax_shared::skip_newlines(self.tokens, &mut self.cursor);
    }
    fn err(&self, text: &str) -> Diagnostic {
        let (start, end) = match self.current() {
            Some(Token { kind: TokenKind::End, .. }) | None => self.source.eof_range(),
            Some(token) => (token.start, token.end),
        };
        Diagnostic {
            kind: DiagnosticKind::Syntax,
            message: text.into(),
            span: Some(self.source.span(start, end)),
        }
    }
    fn expect_symbol(&mut self, target: char, msg: &str) -> Result<usize, Diagnostic> {
        crate::syntax_shared::take_symbol(self.tokens, &mut self.cursor, target)
            .ok_or_else(|| self.err(msg))
    }
    fn expect_keyword(&mut self, target: &str, msg: &str) -> Result<usize, Diagnostic> {
        crate::syntax_shared::take_keyword(self.tokens, &mut self.cursor, target)
            .ok_or_else(|| self.err(msg))
    }
    fn name(&mut self) -> Result<String, Diagnostic> {
        crate::syntax_shared::take_name(self.tokens, &mut self.cursor)
            .ok_or_else(|| self.err("缺少合法名稱"))
    }
    fn ty(&mut self) -> Result<Type, Diagnostic> {
        crate::syntax_shared::take_scalar_type(self.tokens, &mut self.cursor)
            .ok_or_else(|| self.err("首版僅接受明確的 int 或 bool 型別"))
    }
    fn expression(&mut self) -> Result<Expr, Diagnostic> {
        let mut parser = ExprParser::new(self.source, self.tokens, self.cursor);
        let value = parser.expression()?;
        self.cursor = parser.cursor;
        Ok(value)
    }
    fn is_initial_value(&self, expr: &Expr) -> bool {
        matches!(expr.kind,
            ExprKind::Scalar(_) | ExprKind::Name(_)
        ) || (self.allow_probe_move && matches!(&expr.kind,
            ExprKind::ZeroArgMethod(inner, method)
                if method == "move" && matches!(inner.kind, ExprKind::Name(_))))
    }
    fn first_version_return(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Scalar(_) |
            ExprKind::Name(_) | ExprKind::Call(_) => true,
            ExprKind::Group(x) => self.first_version_return(x),
            ExprKind::BinaryInt(binary) =>
                self.first_version_return(&binary.left) && self.first_version_return(&binary.right),
            ExprKind::ZeroArgMethod(inner, method) => self.allow_probe_move
                && method == "move" && matches!(inner.kind, ExprKind::Name(_)),
            _ => false,
        }
    }
    fn binding(&mut self) -> Result<Binding, Diagnostic> {
        crate::syntax_shared::parse_declaration(self)
    }

    fn block(&mut self) -> Result<Block, Diagnostic> {
        crate::syntax_shared::parse_block(self)
    }

    fn condition(&mut self) -> Result<Expr, Diagnostic> {
        self.expect_symbol('(', "if 與 else if 條件須有完整圓括號")?;
        let value = self.expression()?;
        if !matches!(&value.kind, ExprKind::Scalar(crate::syntax_shared::ScalarLiteral::Boolean(_)) | ExprKind::Name(_)) {
            return Err(self.err("首版一般條件限 bool 常值或已綁定 bool 名稱"));
        }
        self.expect_symbol(')', "if 與 else if 條件缺少右圓括號")?;
        Ok(value)
    }

    fn if_chain(&mut self) -> Result<(StmtKind, usize), Diagnostic> {
        let parsed = crate::syntax_shared::parse_if_chain(self)?;
        let end = parsed.end;
        Ok((StmtKind::IfChain(parsed), end))
    }

    fn statement(&mut self) -> Result<Stmt, Diagnostic> {
        let start = self.current().ok_or_else(|| self.err("缺少語句"))?.start;
        let head = crate::syntax_shared::statement_head(
            self.tokens, self.cursor, crate::common_ir::FrontendMode::PublicMvpA,
        );
        let (kind, end) = if head == Some(crate::syntax_shared::StatementHead::Declaration) {
            let binding = self.binding()?;
            let end = binding.end;
            (StmtKind::Binding(binding), end)
        } else if head == Some(crate::syntax_shared::StatementHead::Conditional) {
            self.if_chain()?
        } else if head == Some(crate::syntax_shared::StatementHead::Return) {
            let value = crate::syntax_shared::parse_return(self)?;
            let end = value.end;
            (StmtKind::Return(value), end)
        } else if head == Some(crate::syntax_shared::StatementHead::Named) {
            let parsed = crate::syntax_shared::parse_named_assignment(self)?;
            let end = parsed.expression.end;
            (StmtKind::Assignment(parsed), end)
        } else {
            return Err(self.err("首版不接受的語句或未核准語法"));
        };
        Ok(Stmt { kind, start, end })
    }

    fn function(&mut self) -> Result<Function, Diagnostic> {
        let parsed = crate::syntax_shared::parse_function_decl(self)?;
        // P07 保留原獨立判定及原錯誤游標，不能由共用簽章放寬公開規則。
        if !matches!(parsed.body.statements.last().map(|s| &s.kind), Some(StmtKind::Return(_))) {
            return Err(self.err("函式最外層最後一條語句必須為明確 return"));
        }
        Ok(parsed)
    }

    fn file(mut self) -> Result<File, Diagnostic> {
        self.skip_newlines();
        let mut items = Vec::new();
        while !self.at_end() {
            let item = if self.keyword("fn") {
                Item::Function(self.function()?)
            } else if self.keyword("var") || self.keyword("const") {
                Item::Binding(self.binding()?)
            } else {
                return Err(self.err("首版頂層只接受 fn、const、var 宣告"));
            };
            items.push(item);
            if !self.at_end() && !self.at_newline() {
                return Err(self.err("頂層宣告必須以換行分隔"));
            }
            self.skip_newlines();
        }
        if items.is_empty() {
            return Err(self.err("來源檔案至少需要合法頂層宣告"));
        }
        Ok(File { items, start: 0, end: self.source.bytes.len() })
    }
}

impl crate::syntax_shared::DeclarationSyntax for Parser<'_> {
    type Type = Type;
    type Expression = Expr;
    fn declaration_start(&self) -> usize {
        self.current().map(|token| token.start).unwrap_or(self.source.bytes.len())
    }
    fn begin_declaration(&mut self) -> Result<bool, Diagnostic> {
        let mutable = self.keyword("var");
        if !mutable && !self.keyword("const") {
            return Err(self.err("首版只接受 var 或 const 宣告"));
        }
        self.next();
        Ok(mutable)
    }
    fn declaration_name(&mut self) -> Result<String, Diagnostic> { self.name() }
    fn declaration_colon(&mut self) -> Result<(), Diagnostic> {
        self.expect_symbol(':', "一般變數宣告必須標註型別").map(|_| ())
    }
    fn declaration_type(&mut self) -> Result<Type, Diagnostic> { self.ty() }
    fn declaration_has_initializer(&self) -> bool { self.symbol('=') }
    fn declaration_equals(&mut self) { self.next(); }
    fn declaration_expression(&mut self) -> Result<Expr, Diagnostic> {
        let expression = self.expression()?;
        if !self.is_initial_value(&expression) {
            return Err(self.err("首版宣告初始化僅接受相符型別常值或合法具名變數"));
        }
        Ok(expression)
    }
    fn validate_declaration_initializer(
        &self, mutable: bool, initializer: &Option<Expr>,
    ) -> Result<(), Diagnostic> {
        if !mutable && initializer.is_none() {
            Err(self.err("const 宣告時必須立即初始化"))
        } else { Ok(()) }
    }
    fn declaration_end(&self, initializer: &Option<Expr>) -> usize {
        initializer.as_ref().map(|value| value.end)
            .unwrap_or_else(|| self.tokens[self.cursor - 1].end)
    }
}

impl crate::syntax_shared::NamedAssignmentSyntax for Parser<'_> {
    fn named_assignment_name(&mut self) -> Result<String, Diagnostic> { self.name() }
}

impl crate::syntax_shared::AssignmentSyntax for Parser<'_> {
    type Expression = Expr;
    fn assignment_equals(&mut self) -> Result<(), Diagnostic> {
        self.expect_symbol('=', "首版具名語句必須為 var 賦值").map(|_| ())
    }
    fn assignment_expression(&mut self) -> Result<Expr, Diagnostic> { self.expression() }
    fn validate_assignment(&self, expression: &Expr) -> Result<(), Diagnostic> {
        if self.is_initial_value(expression) { Ok(()) }
        else { Err(self.err("首版 var 賦值只接受同型別常值或合法具名變數")) }
    }
}

impl crate::syntax_shared::ReturnSyntax for Parser<'_> {
    type Expression = Expr;
    fn begin_return(&mut self) -> Result<(), Diagnostic> {
        self.expect_keyword("return", "缺少 return").map(|_| ())
    }
    fn return_expression(&mut self) -> Result<Expr, Diagnostic> { self.expression() }
    fn validate_return(&self, expression: &Expr) -> Result<(), Diagnostic> {
        if self.first_version_return(expression) { Ok(()) }
        else { Err(self.err("首版 return 不接受此運算式來源")) }
    }
}

impl crate::syntax_shared::IfSyntax for Parser<'_> {
    type Condition = Expr;
    type Body = Block;

    fn current_if_start(&self) -> usize {
        self.current().expect("已確認 if token").start
    }
    fn begin_if(&mut self) -> Result<(), Diagnostic> {
        self.expect_keyword("if", "缺少 if").map(|_| ())
    }
    fn if_condition(&mut self) -> Result<Expr, Diagnostic> { self.condition() }
    fn if_body(&mut self) -> Result<Block, Diagnostic> { self.block() }
    fn maybe_else(&mut self) -> bool {
        let saved = self.cursor;
        self.skip_newlines();
        if self.keyword("else") {
            self.next();
            true
        } else {
            self.cursor = saved;
            false
        }
    }
    fn next_is_if(&self) -> bool { self.keyword("if") }
    fn last_end(&self) -> usize { self.tokens[self.cursor - 1].end }
}

impl crate::syntax_shared::FunctionSyntax for Parser<'_> {
    type Params = ();
    type ResultType = Type;
    type Body = Block;

    fn begin_function(&mut self) -> Result<usize, Diagnostic> {
        self.expect_keyword("fn", "缺少 fn")
    }
    fn function_name(&mut self) -> Result<String, Diagnostic> { self.name() }
    fn function_left_paren(&mut self) -> Result<(), Diagnostic> {
        self.expect_symbol('(', "首版函式宣告必須包含空參數圓括號").map(|_| ())
    }
    fn function_params(&mut self) -> Result<(), Diagnostic> { Ok(()) }
    fn function_right_paren(&mut self) -> Result<(), Diagnostic> {
        self.expect_symbol(')', "首版函式不得有參數").map(|_| ())
    }
    fn function_return_arrow(&mut self) -> Result<(), Diagnostic> {
        crate::syntax_shared::take_return_arrow(self.tokens, &mut self.cursor)
            .map(|_| ()).ok_or_else(|| self.err("首版函式必須標註 -> int 或 -> bool"))
    }
    fn function_result_type(&mut self) -> Result<Type, Diagnostic> { self.ty() }
    fn function_body(&mut self) -> Result<Block, Diagnostic> { self.block() }
    fn function_last_end(&self) -> usize { self.tokens[self.cursor - 1].end }
}

impl crate::syntax_shared::BlockSyntax for Parser<'_> {
    type Statement = Stmt;
    fn begin_block(&mut self) -> Result<usize, Diagnostic> {
        let start = self.current().ok_or_else(|| self.err("缺少語句區塊"))?.start;
        self.expect_symbol('{', "語句區塊必須使用左大括號")?;
        self.skip_newlines();
        Ok(start)
    }
    fn block_at_end(&self) -> bool { self.at_end() }
    fn block_at_close(&self) -> bool { self.symbol('}') }
    fn block_missing_close(&self) -> Diagnostic { self.err("語句區塊缺少右大括號") }
    fn parse_block_statement(&mut self) -> Result<Stmt, Diagnostic> {
        self.statement()
    }
    fn finish_block_statement(&mut self) -> Result<(), Diagnostic> {
        if crate::syntax_shared::finish_statement(self.tokens, &mut self.cursor) {
            Ok(())
        } else {
            Err(self.err("語句之間必須以換行分隔"))
        }
    }
    fn finish_block(&mut self, count: usize) -> Result<usize, Diagnostic> {
        if count == 0 {
            return Err(self.err("語句區塊至少必須有一條語句"));
        }
        Ok(self.next().expect("已確定有關閉大括號").end)
    }
}

// 正式 AST 全遞迴來源身分稽核，僅消費 Parser 已產生的唯一 AST。
include!("parser_source_identity.rs");

pub(crate) fn parse(source: &Source, tokens: &[Token]) -> Result<File, Diagnostic> {
    let parsed = Parser { source, tokens, cursor: 0, allow_probe_move: false }.file()?;
    if let Ok(text) = std::str::from_utf8(&source.bytes) {
        if !verified_public_syntax_tree(&parsed, text) {
            return Err(Diagnostic {
                kind: DiagnosticKind::Static,
                message: "公開解析器的 AST 原始來源身分或巢狀跨度不一致".into(),
                span: None,
            });
        }
    }
    Ok(parsed)
}

/// 僅為 P13 內部測試解析最窄的具名來源 .move()：
/// 允許宣告／賦值初值與 return 表達式中明確整體移動；
/// 不開放借用、結構欄位或非具名來源，也不更動正式 P01 語法。
pub(crate) fn parse_move_probe(source: &Source, tokens: &[Token]) -> Result<File, Diagnostic> {
    Parser { source, tokens, cursor: 0, allow_probe_move: true }.file()
}

#[cfg(test)]
mod tests {
    use super::{parse, Item, StmtKind};
    use crate::{lexer::scan, source::Source, DiagnosticKind};

    fn parse_text(text: &str) -> Result<super::File, crate::Diagnostic> {
        let source = Source::new(text.as_bytes().to_vec());
        let lexed = scan(&source);
        assert!(lexed.diagnostics.is_empty(), "測試案例須先能完成詞彙分析");
        parse(&source, &lexed.tokens)
    }

    #[test]
    fn 公開純值型別規則與私有結構型別界線保持原診斷及錯誤位置() {
        for ty in ["str","list","Custom"] {
            let source=format!("fn main()->int{{\nvar value:{ty}\nreturn 0\n}}\n");
            let error=parse_text(&source).unwrap_err();
            assert_eq!(error.kind,DiagnosticKind::Syntax);
            assert_eq!(&*error.message,"首版僅接受明確的 int 或 bool 型別");
            let location=error.span.expect("型別錯誤不能丟失來源位置");
            assert_eq!(location.start.line,2);
            assert_eq!(location.start.column,11);
        }
        for ty in ["int","bool"] {
            let src=format!("fn main()->int{{\nvar value:{ty}\nreturn 0\n}}\n");
            assert!(parse_text(&src).is_ok(),"{ty} 必須仍被 P01 Parser 接受");
        }
    }

    #[test]
    fn p01_所有十九個正例皆可完成首版語法解析() {
        for index in 1..=19 {
            let filename = format!("tests/fixtures/voiles/P01-正{index:02}.voil");
            let content = std::fs::read_to_string(&filename).expect("正向素材必須存在");
            let result = parse_text(&content);
            assert!(result.is_ok(), "{filename}: {result:?}");
        }
    }

    #[test]
    fn 純首版語法錯誤的負例被拒絕而不臆測型別錯誤() {
        for index in [3,4,5,6,9,10,16,18,19,21,22,23,25,26,27,28] {
            let filename = format!("tests/fixtures/voiles/P01-反{index:02}.voil");
            let content = std::fs::read_to_string(&filename).expect("反向素材必須存在");
            let result = parse_text(&content);
            assert!(result.is_err(), "{filename} 不應被解析器接受");
            assert_eq!(result.unwrap_err().kind, DiagnosticKind::Syntax);
        }
    }

    #[test]
    fn 公開正式_ast_宣告就是共通解析核心所產出的同一型別() {
        let code = "const item:int = 0x2a\nfn main()->int{\nvar alias:int = item\nreturn alias\n}\n";
        let parsed = parse_text(code).unwrap();
        let Item::Binding(top) = &parsed.items[0] else {panic!("缺少頂層 const");};
        let _: &crate::syntax_shared::ParsedDeclaration<super::Type, super::Expr> = top;
        let Item::Function(main) = &parsed.items[1] else {panic!("缺少函式");};
        let StmtKind::Binding(inner) = &main.body.statements[0].kind
            else {panic!("缺少 var");};
        let _: &crate::syntax_shared::ParsedDeclaration<super::Type, super::Expr> = inner;
        assert_eq!(top.name, "item");
        assert_eq!(top.ty, super::Type::Int);
        assert!(!top.mutable);
        assert_eq!(top.start, 0);
        assert_eq!(inner.name, "alias");
        assert!(inner.mutable);
        assert_eq!(inner.start, code.find("var alias:int").unwrap());
        assert_eq!(inner.end, code.find("= item").unwrap() + "= item".len());
    }

    #[test]
    fn 真正公開函式與區塊_ast_不再轉換共通_parser_產物() {
        let code = "fn main()->int{\nvar value:int = 3\nreturn value\n}\n";
        let ast = parse_text(code).unwrap();
        let Item::Function(main) = &ast.items[0] else {panic!("缺少 main");};
        // Rust 編譯期驗證這不是第二份具相同欄位的公開 Function／Block。
        let _: &crate::syntax_shared::ParsedFunction<(), super::Type, super::Block> = main;
        let _: &crate::syntax_shared::ParsedBlock<super::Stmt> = &main.body;
        assert_eq!(main.params, ());
        assert_eq!(main.name, "main");
        assert_eq!(main.result_ty, super::Type::Int);
        assert_eq!(main.start, 0);
        assert_eq!(main.end, code.find('}').unwrap() + 1);
        assert_eq!(main.body.start, code.find('{').unwrap());
        assert_eq!(main.body.end, main.end);
    }

    #[test]
    fn 頂層_fn_和_const_可自由交錯且具有原始來源跨度() {
        let file = parse_text(
            "fn helper()->int{\nreturn 1\n}\nconst first:int = 2\nfn main()->int{\nreturn first\n}\n"
        ).unwrap();
        assert_eq!(file.items.len(), 3);
        assert!(matches!(&file.items[0], Item::Function(_)));
        assert!(matches!(&file.items[1], Item::Binding(_)));
        assert!(matches!(&file.items[2], Item::Function(_)));
        assert!(file.end > file.start);
    }

    #[test]
    fn 條件鏈內部_ast_保留分支及尾端_return() {
        let file = parse_text(
            "fn main()->int{\nif (true){\nreturn 1\n}else if (false){\nreturn 2\n}else{\nvar n:int = 3\n}\nreturn 0\n}\n"
        ).unwrap();
        let Item::Function(f) = &file.items[0] else { panic!("缺少函式"); };
        assert_eq!(f.body.statements.len(), 2);
        let StmtKind::IfChain(chain) = &f.body.statements[0].kind else {panic!("缺少 if");};
        assert_eq!(chain.arms.len(), 2);
        assert!(chain.otherwise.is_some());
        assert!(matches!(&f.body.statements[1].kind, StmtKind::Return(_)));
    }

    #[test]
    fn 真正共同_ifchain_ast_保留每個分支_utf8_起點與鏈尾端() {
        let code = "fn main()->int{\nif (true){\nreturn 1\n}else if (false){\nreturn 2\n}else{\nreturn 3\n}\nreturn 0\n}\n";
        let file = parse_text(code).unwrap();
        let Item::Function(main) = &file.items[0] else { panic!("缺少 main"); };
        let stmt = &main.body.statements[0];
        let StmtKind::IfChain(chain) = &stmt.kind else { panic!("未交付共同 if AST"); };
        let _: &crate::syntax_shared::ParsedIfChain<super::Expr, super::Block> = chain;
        assert_eq!(chain.arms.len(), 2);
        assert_eq!(chain.arms[0].start, code.find("if (true)").unwrap());
        assert_eq!(chain.arms[1].start, code.find("if (false)").unwrap());
        assert!(chain.arms[0].branch.condition.start < chain.arms[1].start);
        assert_eq!(chain.end, code.find("}\nreturn 0").unwrap() + 1);
        assert_eq!(stmt.end, chain.end);
        let position = crate::common_ir::NodeSpan::new(stmt.start, stmt.end);
        assert!(crate::common_ir::NodeIdentity::checked(
            position, crate::common_ir::NodeRole::Statement, code).is_some());
    }

    #[test]
    fn 沒有分句換行不能默默當成兩個合法語句() {
        assert!(parse_text("fn main()->int{\nvar x:int = 1 return 0\n}\n").is_err());
    }
}
