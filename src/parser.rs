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

#[derive(Debug, Clone)]
pub(crate) struct Function {
    pub(crate) name: String,
    pub(crate) returns: Type,
    pub(crate) body: Block,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct Block {
    pub(crate) statements: Vec<Stmt>,
    #[allow(dead_code)] // P08 AST 除錯保留來源起點。
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct Binding {
    pub(crate) name: String,
    pub(crate) ty: Type,
    pub(crate) mutable: bool,
    pub(crate) initializer: Option<Expr>,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct Stmt {
    pub(crate) kind: StmtKind,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Debug, Clone)]
pub(crate) enum StmtKind {
    Binding(Binding),
    Assignment { name: String, value: Expr },
    IfChain {
        branches: Vec<(Expr, Block)>,
        otherwise: Option<Block>,
    },
    Return(Expr),
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
        while self.at_newline() { self.cursor += 1; }
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
        if self.symbol(target) { Ok(self.next().unwrap().end) }
        else { Err(self.err(msg)) }
    }
    fn expect_keyword(&mut self, target: &str, msg: &str) -> Result<usize, Diagnostic> {
        if self.keyword(target) { Ok(self.next().unwrap().start) }
        else { Err(self.err(msg)) }
    }
    fn name(&mut self) -> Result<String, Diagnostic> {
        match self.current().map(|t| &t.kind) {
            Some(TokenKind::Name(s)) => {
                let result = s.clone();
                self.next();
                Ok(result)
            }
            _ => Err(self.err("缺少合法名稱")),
        }
    }
    fn ty(&mut self) -> Result<Type, Diagnostic> {
        match self.current().map(|t| &t.kind) {
            Some(TokenKind::Name(n)) if n == "int" => {
                self.next(); Ok(Type::Int)
            }
            Some(TokenKind::Name(n)) if n == "bool" => {
                self.next(); Ok(Type::Bool)
            }
            _ => Err(self.err("首版僅接受明確的 int 或 bool 型別")),
        }
    }
    fn expression(&mut self) -> Result<Expr, Diagnostic> {
        let mut parser = ExprParser::new(self.source, self.tokens, self.cursor);
        let value = parser.expression()?;
        self.cursor = parser.cursor;
        Ok(value)
    }
    fn is_initial_value(&self, expr: &Expr) -> bool {
        matches!(expr.kind,
            ExprKind::IntLiteral(_) | ExprKind::BoolLiteral(_) | ExprKind::Name(_)
        ) || (self.allow_probe_move && matches!(&expr.kind,
            ExprKind::ZeroArgMethod(inner, method)
                if method == "move" && matches!(inner.kind, ExprKind::Name(_))))
    }
    fn first_version_return(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::IntLiteral(_) | ExprKind::BoolLiteral(_) |
            ExprKind::Name(_) | ExprKind::ZeroArgCall(_) => true,
            ExprKind::Group(x) => self.first_version_return(x),
            ExprKind::Add(l,r) | ExprKind::Subtract(l,r) =>
                self.first_version_return(l) && self.first_version_return(r),
            ExprKind::ZeroArgMethod(inner, method) => self.allow_probe_move
                && method == "move" && matches!(inner.kind, ExprKind::Name(_)),
            _ => false,
        }
    }
    fn binding(&mut self) -> Result<Binding, Diagnostic> {
        let start = self.current().ok_or_else(|| self.err("缺少宣告"))?.start;
        let mutable = self.keyword("var");
        if !mutable && !self.keyword("const") {
            return Err(self.err("首版只接受 var 或 const 宣告"));
        }
        self.next();
        let name = self.name()?;
        self.expect_symbol(':', "一般變數宣告必須標註型別")?;
        let ty = self.ty()?;
        let mut initializer = None;
        if self.symbol('=') {
            self.next();
            let expr = self.expression()?;
            if !self.is_initial_value(&expr) {
                return Err(self.err("首版宣告初始化僅接受相符型別常值或合法具名變數"));
            }
            initializer = Some(expr);
        } else if !mutable {
            return Err(self.err("const 宣告時必須立即初始化"));
        }
        let end = initializer.as_ref().map(|x| x.end)
            .unwrap_or_else(|| self.tokens[self.cursor - 1].end);
        Ok(Binding { name, ty, mutable, initializer, start, end })
    }

    fn block(&mut self) -> Result<Block, Diagnostic> {
        let start = self.current().ok_or_else(|| self.err("缺少語句區塊"))?.start;
        self.expect_symbol('{', "語句區塊必須使用左大括號")?;
        self.skip_newlines();
        let mut statements = Vec::new();
        loop {
            if self.at_end() {
                return Err(self.err("語句區塊缺少右大括號"));
            }
            if self.symbol('}') {
                if statements.is_empty() {
                    return Err(self.err("語句區塊至少必須有一條語句"));
                }
                let end = self.next().unwrap().end;
                return Ok(Block { statements, start, end });
            }
            statements.push(self.statement()?);
            if self.at_newline() {
                self.skip_newlines();
            } else if !self.symbol('}') {
                return Err(self.err("語句之間必須以換行分隔"));
            }
        }
    }

    fn condition(&mut self) -> Result<Expr, Diagnostic> {
        self.expect_symbol('(', "if 與 else if 條件須有完整圓括號")?;
        let value = self.expression()?;
        if !matches!(&value.kind, ExprKind::BoolLiteral(_) | ExprKind::Name(_)) {
            return Err(self.err("首版一般條件限 bool 常值或已綁定 bool 名稱"));
        }
        self.expect_symbol(')', "if 與 else if 條件缺少右圓括號")?;
        Ok(value)
    }

    fn if_chain(&mut self) -> Result<(StmtKind, usize), Diagnostic> {
        self.expect_keyword("if", "缺少 if")?;
        let condition = self.condition()?;
        let block = self.block()?;
        let mut end = block.end;
        let mut branches = vec![(condition, block)];
        let mut otherwise = None;
        loop {
            // 可容許 else 銜接前方 } 或在換行後出現；
            // 非 else 時還原位置，讓外層強制換行分句。
            let old = self.cursor;
            self.skip_newlines();
            if !self.keyword("else") {
                self.cursor = old;
                break;
            }
            self.next();
            if self.keyword("if") {
                self.next();
                let cond = self.condition()?;
                let body = self.block()?;
                end = body.end;
                branches.push((cond, body));
            } else {
                let body = self.block()?;
                end = body.end;
                otherwise = Some(body);
                break;
            }
        }
        Ok((StmtKind::IfChain { branches, otherwise }, end))
    }

    fn statement(&mut self) -> Result<Stmt, Diagnostic> {
        let start = self.current().ok_or_else(|| self.err("缺少語句"))?.start;
        let (kind, end) = if self.keyword("var") || self.keyword("const") {
            let binding = self.binding()?;
            let end = binding.end;
            (StmtKind::Binding(binding), end)
        } else if self.keyword("if") {
            self.if_chain()?
        } else if self.keyword("return") {
            self.next();
            let value = self.expression()?;
            if !self.first_version_return(&value) {
                return Err(self.err("首版 return 不接受此運算式來源"));
            }
            let end = value.end;
            (StmtKind::Return(value), end)
        } else if matches!(self.current().map(|t| &t.kind), Some(TokenKind::Name(_))) {
            let name = self.name()?;
            self.expect_symbol('=', "首版具名語句必須為 var 賦值")?;
            let value = self.expression()?;
            if !self.is_initial_value(&value) {
                return Err(self.err("首版 var 賦值只接受同型別常值或合法具名變數"));
            }
            let end = value.end;
            (StmtKind::Assignment {name, value}, end)
        } else {
            return Err(self.err("首版不接受的語句或未核准語法"));
        };
        Ok(Stmt { kind, start, end })
    }

    fn function(&mut self) -> Result<Function, Diagnostic> {
        let start = self.expect_keyword("fn", "缺少 fn")?;
        let name = self.name()?;
        self.expect_symbol('(', "首版函式宣告必須包含空參數圓括號")?;
        self.expect_symbol(')', "首版函式不得有參數")?;
        if !matches!(self.current().map(|t| &t.kind), Some(TokenKind::Arrow)) {
            return Err(self.err("首版函式必須標註 -> int 或 -> bool"));
        }
        self.next();
        let returns = self.ty()?;
        let body = self.block()?;
        if !matches!(body.statements.last().map(|s| &s.kind), Some(StmtKind::Return(_))) {
            return Err(self.err("函式最外層最後一條語句必須為明確 return"));
        }
        let end = body.end;
        Ok(Function {name, returns, body, start, end})
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

pub(crate) fn parse(source: &Source, tokens: &[Token]) -> Result<File, Diagnostic> {
    Parser { source, tokens, cursor: 0, allow_probe_move: false }.file()
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
        let StmtKind::IfChain { branches, otherwise } = &f.body.statements[0].kind else {panic!("缺少 if");};
        assert_eq!(branches.len(), 2);
        assert!(otherwise.is_some());
        assert!(matches!(&f.body.statements[1].kind, StmtKind::Return(_)));
    }

    #[test]
    fn 沒有分句換行不能默默當成兩個合法語句() {
        assert!(parse_text("fn main()->int{\nvar x:int = 1 return 0\n}\n").is_err());
    }
}
