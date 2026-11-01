//! P06：僅內部使用的運算式語法樹；不替未核准的所有權／副作用補語意。

use crate::{Diagnostic, DiagnosticKind};
use crate::lexer::{Token, TokenKind};
use crate::source::Source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExprKind {
    IntLiteral(String),
    BoolLiteral(bool),
    DecimalLiteral(String),
    StringLiteral(String),
    Name(String),
    ZeroArgCall(String),
    Add(crate::common_ir::ArithmeticNode<Expr>),
    Subtract(crate::common_ir::ArithmeticNode<Expr>),
    Group(Box<Expr>),
    /// 僅為完整語言字串索引語法的內部準備；MVP-A 不接受。
    IndexAccess(Box<Expr>, Box<Expr>),
    /// 對已確認完整語言方法／move／borrow 的語法辨識；不核准所有權語意。
    ZeroArgMethod(Box<Expr>, String),
    /// 專用的字串第一層索引寫入運算式，不能泛化為其他賦值表達式。
    // 完整語言保留的私有辨識結點，MVP-A parser 尚不建構。
    #[allow(dead_code)]
    FirstIndexWrite(Box<Expr>, Box<Expr>),
}

/// 公開 MVP-A 運算式實際共用的 AST 外殼；節點種類維持公開核准子集。
pub(crate) type Expr = crate::common_ir::SpannedNode<ExprKind>;

pub(crate) struct ExprParser<'a> {
    source: &'a Source,
    tokens: &'a [Token],
    pub(crate) cursor: usize,
}

impl<'a> ExprParser<'a> {
    pub(crate) fn new(source: &'a Source, tokens: &'a [Token], cursor: usize) -> Self {
        Self { source, tokens, cursor }
    }

    pub(crate) fn peek(&self) -> Option<&'a Token> {
        self.tokens.get(self.cursor)
    }

    fn next(&mut self) -> Option<&'a Token> {
        let token = self.peek();
        if token.is_some() { self.cursor += 1; }
        token
    }

    fn symbol(&self, expected: char) -> bool {
        matches!(self.peek().map(|x| &x.kind), Some(TokenKind::Symbol(c)) if *c == expected)
    }

    fn error(&self, text: &str) -> Diagnostic {
        let (start, end) = match self.peek() {
            Some(Token { kind: TokenKind::End, .. }) | None => self.source.eof_range(),
            Some(t) => (t.start, t.end),
        };
        Diagnostic {
            kind: DiagnosticKind::Syntax,
            message: text.into(),
            span: Some(self.source.span(start, end)),
        }
    }

    fn expect_symbol(&mut self, expected: char, message: &str) -> Result<usize, Diagnostic> {
        crate::syntax_shared::take_symbol(self.tokens, &mut self.cursor, expected)
            .ok_or_else(|| self.error(message))
    }

    /// MVP-A 算術二元 +、- 同層左結合；括號及合法函式運算元。
    pub(crate) fn expression(&mut self) -> Result<Expr, Diagnostic> {
        let left = self.postfix()?;
        crate::syntax_shared::fold_additive_spanned(
            left,
            || {
                let Some(op) = crate::syntax_shared::additive_operator(
                    self.tokens, self.cursor) else { return Ok(None) };
                self.next();
                Ok(Some((op, self.postfix()?)))
            },
            |expr| expr.node_span(),
            |left, op, right, span| {
                let start = span.start;
                let end = span.end;
                let binary = crate::common_ir::ArithmeticNode::new(left, op, right);
                let kind = if op == '+' {
                    ExprKind::Add(binary)
                } else {
                    ExprKind::Subtract(binary)
                };
                Expr { kind, start, end }
            },
        )
    }

    fn postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut value = self.primary()?;
        loop {
            if self.symbol('[') {
                self.next();
                let index = self.expression()?;
                let end = self.expect_symbol(']', "索引缺少右中括號")?;
                let start = value.start;
                value = Expr {
                    start, end,
                    kind: ExprKind::IndexAccess(Box::new(value), Box::new(index)),
                };
                continue;
            }
            if self.symbol('.') {
                self.next();
                let method = match self.next() {
                    Some(Token { kind: TokenKind::Name(name), .. }) => name.clone(),
                    _ => return Err(self.error("方法呼叫缺少方法名稱")),
                };
                self.expect_symbol('(', "方法呼叫缺少左圓括號")?;
                let end = self.expect_symbol(')', "目前內部語法辨識只處理無引數方法形式")?;
                let start = value.start;
                value = Expr {
                    start, end,
                    kind: ExprKind::ZeroArgMethod(Box::new(value), method),
                };
                continue;
            }
            break;
        }
        Ok(value)
    }

    fn primary(&mut self) -> Result<Expr, Diagnostic> {
        // 直接消耗共通 Parser 核心建立的具跨度純值字面值 AST。
        if let Some(literal) =
            crate::syntax_shared::take_scalar_literal(self.tokens, &mut self.cursor) {
            let kind = match literal.kind {
                crate::syntax_shared::ScalarLiteral::Integer(text) =>
                    ExprKind::IntLiteral(text),
                crate::syntax_shared::ScalarLiteral::Boolean(value) =>
                    ExprKind::BoolLiteral(value),
            };
            return Ok(Expr { kind, start: literal.start, end: literal.end });
        }
        // 一般具名原子亦由同一共通 AST 位置產生器建立。
        if let Some(name) =
            crate::syntax_shared::take_spanned_name(self.tokens, &mut self.cursor) {
            let kind = if self.symbol('(') {
                self.next();
                let (_, end) = crate::syntax_shared::parse_call(self)?;
                return Ok(Expr {
                    kind: ExprKind::ZeroArgCall(name.kind),
                    start: name.start, end,
                });
            } else {
                ExprKind::Name(name.kind)
            };
            return Ok(Expr { kind, start: name.start, end: name.end });
        }
        let token = self.next().ok_or_else(|| self.error("缺少運算式"))?;
        let start = token.start;
        let end = token.end;
        match &token.kind {
            TokenKind::Decimal(text) => Ok(Expr { kind: ExprKind::DecimalLiteral(text.clone()), start, end }),
            TokenKind::StringLiteral(text) => Ok(Expr { kind: ExprKind::StringLiteral(text.clone()), start, end }),
            TokenKind::Symbol('(') => crate::syntax_shared::parse_group(self, start),
            _ => {
                self.cursor -= 1;
                Err(self.error("缺少合法運算元或首版未支援此運算式"))
            }
        }
    }

    /// 只辨識已確認的第一層字串索引寫入運算式形狀。
    /// 不推導任何寫入權限、Unicode 字素數、拷貝或求值順序。
    #[allow(dead_code)] // 只供已規劃的完整語言第一層索引寫入，不公開 MVP-A。
    pub(crate) fn first_index_write(&mut self) -> Result<Expr, Diagnostic> {
        let target = self.postfix()?;
        let first_level = matches!(
            &target.kind,
            ExprKind::IndexAccess(parent, _) if matches!(parent.kind, ExprKind::Name(_))
        );
        if !first_level {
            return Err(self.error("字串索引寫入僅辨識第一層索引目標"));
        }
        self.expect_symbol('=', "索引寫入缺少等號")?;
        let value = self.expression()?;
        Ok(Expr {
            start: target.start,
            end: value.end,
            kind: ExprKind::FirstIndexWrite(Box::new(target), Box::new(value)),
        })
    }
}

impl crate::syntax_shared::CallSyntax for ExprParser<'_> {
    type Arguments = ();
    fn call_arguments(&mut self) -> Result<(), Diagnostic> { Ok(()) }
    fn call_right_paren(&mut self) -> Result<usize, Diagnostic> {
        self.expect_symbol(')', "首版零引數函式呼叫必須為空引數列表")
    }
}

impl crate::syntax_shared::GroupSyntax for ExprParser<'_> {
    type Expression = Expr;
    fn group_expression(&mut self) -> Result<Expr, Diagnostic> { self.expression() }
    fn group_right_paren(&mut self) -> Result<usize, Diagnostic> {
        self.expect_symbol(')', "算術分組缺少右圓括號")
    }
    fn group_result(&mut self, expression: Expr, start: usize, end: usize) -> Expr {
        Expr { kind: ExprKind::Group(Box::new(expression)), start, end }
    }
}

#[cfg(test)]
mod tests {
    use super::{ExprKind, ExprParser};
    use crate::lexer::scan;
    use crate::source::Source;

    fn parse(input: &str) -> super::Expr {
        let source = Source::new(input.as_bytes().to_vec());
        let lexer = scan(&source);
        assert!(lexer.diagnostics.is_empty());
        let mut parser = ExprParser::new(&source, &lexer.tokens, 0);
        parser.expression().unwrap()
    }

    #[test]
    fn 同層加減依左結合形成_ast() {
        let e = parse("8 - 3 - 1");
        match e.kind {
            ExprKind::Subtract(binary) => {
                assert_eq!(binary.operator, '-');
                assert!(matches!(binary.right.kind, ExprKind::IntLiteral(_)));
                assert!(matches!(binary.left.kind, ExprKind::Subtract(_)));
            }
            _ => panic!("二元減法必須左結合"),
        }
    }

    #[test]
    fn 算術括號保留且不變更結合方向() {
        let e = parse("8 - (3 - 1)");
        match e.kind {
            ExprKind::Subtract(binary) => assert!(matches!(binary.right.kind, ExprKind::Group(_))),
            _ => panic!("分組須保留為內部節點"),
        }
    }

    #[test]
    fn 已核准零引數函式結果可作加法運算元() {
        let e = parse("helper() + 2");
        match e.kind {
            ExprKind::Add(binary) => assert!(matches!(binary.left.kind, ExprKind::ZeroArgCall(ref n) if n=="helper")),
            _ => panic!("缺少合法函式運算元"),
        }
    }

    #[test]
    fn 共同呼叫核心仍只允許公開零參數且完整保留來源位元組跨度() {
        let call = parse("水母()");
        assert_eq!(call.start, 0);
        assert_eq!(call.end, "水母()".len());
        assert!(matches!(call.kind, ExprKind::ZeroArgCall(ref name) if name == "水母"));
        let nested = parse("helper() + 2");
        assert!(matches!(nested.kind, ExprKind::Add(_)));
    }

    #[test]
    fn 共同呼叫核心拒絕未核准參數且不吞掉原始診斷位置() {
        for source_text in ["helper(1)", "helper(", "helper(中)", "helper(1, 2)"] {
            let source = Source::new(source_text.as_bytes().to_vec());
            let tokens = scan(&source);
            assert!(tokens.diagnostics.is_empty(), "測試必須能完成詞彙解析");
            let mut parser = ExprParser::new(&source, &tokens.tokens, 0);
            let err = parser.expression().expect_err("公開 MVP-A 不接受具參數函式呼叫");
            assert_eq!(err.kind, crate::DiagnosticKind::Syntax);
            assert_eq!(&*err.message, "首版零引數函式呼叫必須為空引數列表");
            assert!(err.span.is_some());
        }
    }

    #[test]
    fn 多位元組來源跨度按原始位元組保留() {
        let e = parse("\"中\"[0]");
        assert_eq!(e.start, 0);
        assert_eq!(e.end, 8);
        assert!(matches!(e.kind, ExprKind::IndexAccess(_, _)));
    }

    #[test]
    fn 方法借用與移動形式僅作內部語法辨識() {
        for name in ["move","borrow"] {
            let e = parse(&format!("value.{name}()"));
            assert!(matches!(e.kind, ExprKind::ZeroArgMethod(_, ref n) if n == name));
        }
    }

    #[test]
    fn 字串第一層索引寫入表達式可解析但不許可第二層() {
        let source = Source::new("s[0] = \"中\"".as_bytes().to_vec());
        let lexer = scan(&source);
        let mut p = ExprParser::new(&source, &lexer.tokens, 0);
        let expr = p.first_index_write().unwrap();
        assert!(matches!(expr.kind, ExprKind::FirstIndexWrite(_, _)));

        let source = Source::new(b"s[0][1] = 7".to_vec());
        let lexer = scan(&source);
        let mut p = ExprParser::new(&source, &lexer.tokens, 0);
        assert!(p.first_index_write().is_err());
    }

    #[test]
    fn 缺少運算元與括號不配對產生語法診斷() {
        for source_text in ["1 +", "1 -", "8 - (3 - 1"] {
            let source = Source::new(source_text.as_bytes().to_vec());
            let lexer = scan(&source);
            let mut p = ExprParser::new(&source, &lexer.tokens, 0);
            let e = p.expression().unwrap_err();
            assert_eq!(e.kind, crate::DiagnosticKind::Syntax);
            assert!(e.span.is_some());
        }
    }

    #[test]
    fn 未核准一元負號不能默認成首版運算式() {
        let source = Source::new(b"-1".to_vec());
        let lexer = scan(&source);
        let mut p = ExprParser::new(&source, &lexer.tokens, 0);
        assert!(p.expression().is_err());
    }
}
