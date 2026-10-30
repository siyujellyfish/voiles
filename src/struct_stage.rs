//! P13 受限結構體所有權前端：真正 .voil → 私有 AST → 型別 → 所有權狀態。
//! 只接受可以完整驗證的子集，未支援的語法拒絕，不影響 MVP-A check_file。
//! 正式內部入口依 HA13-A／str 方案 A 使用核准政策；自訂能力僅限 Rust 測試。
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use crate::{Diagnostic, DiagnosticKind};
use crate::lexer::{self, Token, TokenKind};
use crate::ownership_model::{CopyPermission, ModelError, ValueSlot, ValueState};
use crate::ownership_ledger::DeclarationId;
use crate::borrow_safety::{BorrowKind, BorrowSafety};
use crate::source::Source;
use crate::struct_ownership::{self, MoveTarget, SelfAction};

#[derive(Debug, Clone)]
struct Field {
    name: String,
    ty: String,
    immutable: bool,
    default: Option<Term>,
}

#[derive(Debug, Clone)]
struct StructDecl {
    fields: Vec<Field>,
    methods: Vec<Function>,
}

#[derive(Debug, Clone)]
struct Function {
    name: String,
    params: Vec<Parameter>,
    result_ty: String,
    body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
struct Parameter {
    name: String,
    ty: String,
    declaration_start: usize,
}

#[derive(Debug, Clone)]
struct Stmt {
    kind: StmtKind,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone)]
enum StmtKind {
    Declare { name: String, ty: String, mutable: bool, init: Option<Term> },
    Assign { name: String, value: Term },
    FieldAssign { receiver: String, field: String, value: Term },
    If { condition: Term, yes: Vec<Stmt>, no: Option<Vec<Stmt>> },
    /// P15：只在能證明反向邊的所有權狀態不變時接受的 while 子集。
    While { condition: Term, body: Vec<Stmt> },
    /// D04 右端點不含的整數常值區間；P15 僅建構靜態借用證據。
    ForRange { iterator: String, iterator_start: usize,
        lower: String, upper: String, body: Vec<Stmt> },
    Return(Term),
    /// 既有 for／while 的最內層控制轉移；無迴圈時必須拒絕。
    Break,
    Continue,
}

#[derive(Debug, Clone)]
struct Term {
    kind: TermKind,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone)]
enum TermKind {
    Literal(&'static str),
    Name(String),
    /// P01 已核准的 int 加減；計算前必須依序驗證兩端所有權。
    BinaryInt { left: Box<Term>, operator: char, right: Box<Term> },
    Construct(String, Vec<(String, Term)>),
    /// 一般函式位置引數；呼叫端所有權資料流完成前不得接受。
    Call(String, Vec<Term>),
    Move(String),
    /// VA14-01-A：只於具型別 var／const 直接初始化時建立非擁有借用。
    Borrow(String),
    FieldMove(String, String),
    /// 只讀欄位值的複製；需另外通過已初始化與 HA13/S19 Copyable 檢查。
    FieldRead(String, String),
    /// S16 已核准的 instance.method() 零參數子集；此處不推定其他簽章。
    MethodCall(String, String, Vec<Term>),
}

struct Parser<'a> {
    source: &'a Source,
    tokens: &'a [Token],
    cursor: usize,
}

impl Parser<'_> {
    fn tok(&self) -> &Token {
        &self.tokens[self.cursor.min(self.tokens.len() - 1)]
    }
    fn syntax(&self, msg: &str) -> Diagnostic {
        let t = self.tok();
        Diagnostic {
            kind: DiagnosticKind::Syntax,
            message: msg.into(),
            span: Some(self.source.span(t.start, t.end)),
        }
    }
    fn keyword(&self, word: &str) -> bool {
        matches!(&self.tok().kind, TokenKind::Keyword(s) if s == word)
    }
    fn symbol(&self, c: char) -> bool {
        matches!(self.tok().kind, TokenKind::Symbol(x) if x == c)
    }
    fn eof(&self) -> bool { matches!(self.tok().kind, TokenKind::End) }
    fn lines(&mut self) {
        while matches!(self.tok().kind, TokenKind::Newline) { self.cursor += 1; }
    }
    fn take(&mut self, ch: char, msg: &str) -> Result<(), Diagnostic> {
        if !self.symbol(ch) { return Err(self.syntax(msg)); }
        self.cursor += 1;
        Ok(())
    }
    fn name(&mut self) -> Result<String, Diagnostic> {
        match &self.tok().kind {
            TokenKind::Name(s) => {
                let out = s.clone();
                self.cursor += 1;
                Ok(out)
            }
            _ => Err(self.syntax("需要合法的名稱或型別名稱")),
        }
    }
    fn end_statement(&mut self) -> Result<(), Diagnostic> {
        if self.symbol('}') { return Ok(()); }
        if !matches!(self.tok().kind, TokenKind::Newline) {
            return Err(self.syntax("語句之間必須換行"));
        }
        self.lines();
        Ok(())
    }
    /// 和 MVP-A i64 有效整數字面值規則一致；底線只作分隔，
    /// 範圍溢位在靜態前端拒絕，不移交未知執行時處理。
    fn valid_integer(literal: &str) -> bool {
        let (radix, digits) = if let Some(rest) = literal.strip_prefix("0x") {
            (16, rest)
        } else if let Some(rest) = literal.strip_prefix("0b") {
            (2, rest)
        } else if let Some(rest) = literal.strip_prefix("0o") {
            (8, rest)
        } else {
            (10, literal)
        };
        let normalized: String = digits.chars().filter(|c| *c != '_').collect();
        !normalized.is_empty()
            && normalized.chars().all(|c| c.to_digit(radix).is_some())
            && u128::from_str_radix(&normalized, radix)
                .map(|value| value <= i64::MAX as u128)
                .unwrap_or(false)
    }
    /// P13 已核准的 decimal64 有限常值子集：採十進位字元和最近偶數捨入，
    /// 不將宿主二進位 f64 當成 decimal64。僅驗證調整指數 -15..=15 的數值。
    /// 回傳 (16 位十進位係數, 十進位指數)，保留未決邊界之外的拒絕。
    fn decimal64_checked_subset(literal: &str) -> Option<(u64, i32)> {
        let cleaned: String = literal.chars().filter(|c| *c != '_').collect();
        let (integer, fraction) = cleaned.split_once('.')?;
        if integer.is_empty()
            || !integer.bytes().all(|b| b.is_ascii_digit())
            || !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let digits = format!("{integer}{fraction}");
        let Some(first_nonzero) = digits.bytes().position(|b| b != b'0') else {
            return Some((0, 0));
        };
        let mut adjusted = integer.len() as i64 - first_nonzero as i64 - 1;
        if !(-15..=15).contains(&adjusted) { return None; }
        let significant = &digits[first_nonzero..];
        let mut coefficient = 0u64;
        let first_sixteen = significant.bytes().take(16).collect::<Vec<_>>();
        for digit in &first_sixteen {
            coefficient = coefficient * 10 + (digit - b'0') as u64;
        }
        for _ in first_sixteen.len()..16 { coefficient *= 10; }
        let following = significant.as_bytes().get(16).copied().unwrap_or(b'0');
        let further_nonzero = significant.as_bytes().get(17..)
            .unwrap_or(&[]).iter().any(|b| *b != b'0');
        let round_up = following > b'5'
            || (following == b'5' && (further_nonzero || coefficient % 2 == 1));
        if round_up {
            coefficient += 1;
            if coefficient == 10_000_000_000_000_000 {
                coefficient = 1_000_000_000_000_000;
                adjusted += 1;
                if adjusted > 15 { return None; }
            }
        }
        Some((coefficient, (adjusted - 15) as i32))
    }

    fn term(&mut self) -> Result<Term, Diagnostic> {
        // P01 加減為相同優先序、左結合；括號在原子解析處理。
        let mut left = self.atom()?;
        while self.symbol('+') || self.symbol('-') {
            let operator = if self.symbol('+') { '+' } else { '-' };
            self.cursor += 1;
            let right = self.atom()?;
            left = Term {
                start: left.start, end: right.end,
                kind: TermKind::BinaryInt {
                    left: Box::new(left), operator, right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn atom(&mut self) -> Result<Term, Diagnostic> {
        let start = self.tok().start;
        let (kind, mut end) = match self.tok().kind.clone() {
            TokenKind::Integer(text) => {
                if !Self::valid_integer(&text) {
                    return Err(self.syntax("整數字面值無效或超出有號 64 位元範圍"));
                }
                let end = self.tok().end; self.cursor += 1;
                (TermKind::Literal("int"), end)
            }
            TokenKind::Decimal(ref text) => {
                if Self::decimal64_checked_subset(text).is_none() {
                    return Err(self.syntax(
                        "P13 decimal64 常值超出已驗證的有限安全子集；下溢、超界與特殊值仍待正式規格"
                    ));
                }
                let end = self.tok().end;
                self.cursor += 1;
                (TermKind::Literal("float"), end)
            }
            TokenKind::StringLiteral(_) => {
                let end = self.tok().end; self.cursor += 1;
                (TermKind::Literal("str"), end)
            }
            TokenKind::Keyword(s) if s == "true" || s == "false" => {
                let end = self.tok().end; self.cursor += 1;
                (TermKind::Literal("bool"), end)
            }
            TokenKind::Symbol('(') => {
                self.cursor += 1;
                let mut expression = self.term()?;
                self.take(')', "括號運算式缺少右括號")?;
                expression.start = start;
                expression.end = self.tokens[self.cursor - 1].end;
                return Ok(expression);
            }
            TokenKind::Name(s) => {
                let end = self.tok().end; self.cursor += 1;
                if self.symbol('(') {
                    self.cursor += 1;
                    // 只有 name = value 才是結構具名欄位建立式；
                    // 位置引數須另建 AST，避免錯當欄位或略過所有權檢查。
                    let named = matches!(self.tok().kind, TokenKind::Name(_))
                        && self.tokens.get(self.cursor + 1)
                            .is_some_and(|token| matches!(token.kind, TokenKind::Symbol('=')));
                    if named || self.symbol(')') {
                        let mut args = Vec::new();
                        let mut names = HashSet::new();
                        while !self.symbol(')') {
                            let field = self.name()?;
                            if !names.insert(field.clone()) {
                                return Err(Diagnostic {
                                    kind: DiagnosticKind::Static,
                                    message: "VB16：DuplicateField，建立結構時不得重複指定同一欄位".into(),
                                    span: Some(self.source.span(self.tok().start, self.tok().end)),
                                });
                            }
                            self.take('=', "具名欄位引數需要等號")?;
                            let value = self.term()?;
                            args.push((field, value));
                            if self.symbol(',') { self.cursor += 1; }
                            else if !self.symbol(')') {
                                return Err(self.syntax("具名欄位引數之間需要逗號"));
                            }
                        }
                        let end = self.tok().end; self.cursor += 1;
                        (TermKind::Construct(s, args), end)
                    } else {
                        let mut args = Vec::new();
                        while !self.symbol(')') {
                            args.push(self.term()?);
                            if self.symbol(',') { self.cursor += 1; }
                            else if !self.symbol(')') {
                                return Err(self.syntax("函式位置引數之間需要逗號"));
                            }
                        }
                        let end = self.tok().end; self.cursor += 1;
                        (TermKind::Call(s, args), end)
                    }
                } else if self.symbol('.') {
                    self.cursor += 1;
                    let field_or_move = self.name()?;
                    if field_or_move == "borrow" && self.symbol('(') {
                        self.cursor += 1;
                        self.take(')', "一般借用 .borrow() 不可帶引數")?;
                        (TermKind::Borrow(s), self.tokens[self.cursor - 1].end)
                    } else if field_or_move == "move" && self.symbol('(') {
                        self.cursor += 1;
                        self.take(')', "明確移動目前只接受零參數")?;
                        (TermKind::Move(s), self.tokens[self.cursor - 1].end)
                    } else if self.symbol('.') {
                        self.cursor += 1;
                        let action = self.name()?;
                        if action == "borrow" {
                            self.take('(', "欄位借用呼叫缺少左括號")?;
                            self.take(')', "欄位借用呼叫缺少右括號")?;
                            return Err(Diagnostic {
                                kind: DiagnosticKind::Static,
                                message: "P15.4：直接借用結構欄位尚無獨立來源身分與有效期證明，保守拒絕".into(),
                                span: Some(self.source.span(start, self.tokens[self.cursor - 1].end)),
                            });
                        }
                        if action != "move" {
                            return Err(self.syntax("此子集只接受明確整筆移動"));
                        }
                        self.take('(', "欄位移動呼叫缺少左括號")?;
                        self.take(')', "欄位移動呼叫缺少右括號")?;
                        (TermKind::FieldMove(s, field_or_move),
                            self.tokens[self.cursor - 1].end)
                    } else if self.symbol('(') {
                        self.cursor += 1;
                        let mut args = Vec::new();
                        while !self.symbol(')') {
                            args.push(self.term()?);
                            if self.symbol(',') { self.cursor += 1; }
                            else if !self.symbol(')') {
                                return Err(self.syntax("方法位置引數之間需要逗號"));
                            }
                        }
                        self.take(')', "方法呼叫缺少右括號")?;
                        (TermKind::MethodCall(s, field_or_move, args),
                            self.tokens[self.cursor - 1].end)
                    } else {
                        (TermKind::FieldRead(s, field_or_move),
                            self.tokens[self.cursor - 1].end)
                    }
                } else if self.symbol('[') {
                    // P15.4：仍無索引項目的來源身分／存活期證明，
                    // 不得把 str、list 或 dict 的元素當成可直接借用的持有者。
                    return Err(self.syntax(
                        "P15.4：索引元素的借用／別名有效期尚未證明，私有所有權前端保守拒絕"));
                } else {
                    (TermKind::Name(s), end)
                }
            }
            _ => return Err(self.syntax("此階段不接受的運算式或來源形式")),
        };
        if self.symbol('.') || self.symbol('(') || self.symbol('[') {
            return Err(self.syntax("此階段尚不支援複合接收者或額外後綴操作"));
        }
        if end < start { end = start; }
        Ok(Term { kind, start, end })
    }
    /// 讓已核准的 else if 與獨立 if 共用同一個可達控制流程節點；
    /// 不將 else if 視為不同所有權語法，三條分支均由後端合流追蹤。
    fn if_statement_kind(&mut self) -> Result<StmtKind, Diagnostic> {
        self.cursor += 1; // if
        self.take('(', "if 條件需要左括號")?;
        let condition = self.term()?;
        self.take(')', "if 條件需要右括號")?;
        let yes = self.body()?;
        let saved = self.cursor;
        self.lines();
        let no = if self.keyword("else") {
            self.cursor += 1;
            if self.keyword("if") {
                let start = self.tok().start;
                let kind = self.if_statement_kind()?;
                let end = self.tokens[self.cursor - 1].end;
                Some(vec![Stmt { kind, start, end }])
            } else {
                Some(self.body()?)
            }
        } else {
            self.cursor = saved;
            None
        };
        Ok(StmtKind::If { condition, yes, no })
    }

    fn statement(&mut self) -> Result<Stmt, Diagnostic> {
        let start = self.tok().start;
        let kind = if self.keyword("var") || self.keyword("const") {
            let mutable = self.keyword("var");
            self.cursor += 1;
            let name = self.name()?;
            self.take(':', "宣告必須有型別註記")?;
            let ty = self.name()?;
            let init = if self.symbol('=') {
                self.cursor += 1;
                Some(self.term()?)
            } else { None };
            if !mutable && init.is_none() {
                return Err(self.syntax("const 宣告必須立即初始化"));
            }
            StmtKind::Declare { name, ty, mutable, init }
        } else if self.keyword("if") {
            self.if_statement_kind()?
        } else if self.keyword("for") {
            self.cursor += 1;
            let iterator_start = self.tok().start;
            let iterator = self.name()?;
            if !matches!(&self.tok().kind, TokenKind::Name(word) if word == "in") {
                return Err(self.syntax("for 區間迭代變數後需要 in"));
            }
            self.cursor += 1;
            let lower = match self.tok().kind.clone() {
                TokenKind::Integer(number) if Self::valid_integer(&number) => number,
                _ => return Err(self.syntax("此子集 for 左端點必須是合法整數常值")),
            };
            self.cursor += 1;
            if !matches!(self.tok().kind, TokenKind::Range) {
                return Err(self.syntax("for 半開區間缺少 .. 範圍記號"));
            }
            self.cursor += 1;
            let upper = match self.tok().kind.clone() {
                TokenKind::Integer(number) if Self::valid_integer(&number) => number,
                _ => return Err(self.syntax("此子集 for 右端點必須是合法整數常值")),
            };
            self.cursor += 1;
            let body = self.body()?;
            StmtKind::ForRange { iterator, iterator_start, lower, upper, body }
        } else if self.keyword("while") {
            self.cursor += 1;
            self.take('(', "while 條件需要左括號")?;
            let condition = self.term()?;
            self.take(')', "while 條件需要右括號")?;
            let body = self.body()?;
            StmtKind::While { condition, body }
        } else if self.keyword("return") {
            self.cursor += 1;
            StmtKind::Return(self.term()?)
        } else if self.keyword("break") {
            self.cursor += 1;
            StmtKind::Break
        } else if self.keyword("continue") {
            self.cursor += 1;
            StmtKind::Continue
        } else {
            let name = self.name()?;
            if self.symbol('.') {
                self.cursor += 1;
                let field = self.name()?;
                self.take('=', "欄位賦值需要等號")?;
                let value = self.term()?;
                StmtKind::FieldAssign { receiver: name, field, value }
            } else {
                self.take('=', "此階段具名語句必須為賦值")?;
                let value = self.term()?;
                StmtKind::Assign { name, value }
            }
        };
        let end = self.tokens[self.cursor - 1].end;
        self.end_statement()?;
        Ok(Stmt { kind, start, end })
    }
    fn body(&mut self) -> Result<Vec<Stmt>, Diagnostic> {
        self.take('{', "函式需要左大括號")?;
        self.lines();
        let mut body = Vec::new();
        while !self.symbol('}') {
            if self.eof() { return Err(self.syntax("函式缺少右大括號")); }
            body.push(self.statement()?);
        }
        self.cursor += 1;
        if body.is_empty() {
            return Err(self.syntax("此階段函式必須具有明確 return"));
        }
        Ok(body)
    }
    fn function(&mut self) -> Result<Function, Diagnostic> {
        if !self.keyword("fn") { return Err(self.syntax("函式必須以 fn 宣告")); }
        self.cursor += 1;
        let name = self.name()?;
        self.take('(', "函式名稱後需左括號")?;
        let mut params = Vec::new();
        let mut seen = HashSet::new();
        while !self.symbol(')') {
            let declaration_start = self.tok().start;
            let parameter = self.name()?;
            if !seen.insert(parameter.clone()) {
                return Err(self.syntax("函式參數名稱不得重複"));
            }
            self.take(':', "函式參數需要明確型別")?;
            let ty = self.name()?;
            params.push(Parameter { name: parameter, ty, declaration_start });
            if self.symbol(',') { self.cursor += 1; }
            else if !self.symbol(')') { return Err(self.syntax("函式參數之間需要逗號")); }
        }
        self.take(')', "函式參數缺少右括號")?;
        if !matches!(self.tok().kind, TokenKind::Arrow) {
            return Err(self.syntax("函式需要回傳型別標註"));
        }
        self.cursor += 1;
        let result_ty = self.name()?;
        let body = self.body()?;
        Ok(Function { name, params, result_ty, body })
    }
    fn strukt(&mut self) -> Result<(String, StructDecl), Diagnostic> {
        self.cursor += 1; // struct
        let name = self.name()?;
        self.take('{', "結構名稱後需要左大括號；泛型尚未開放")?;
        self.lines();
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut names = HashSet::new();
        while !self.symbol('}') {
            if self.eof() { return Err(self.syntax("結構缺少右大括號")); }
            if self.keyword("struct") || self.keyword("share") {
                return Err(self.syntax("結構內不可宣告巢狀 struct 或 share 欄位"));
            }
            if self.keyword("fn") {
                let fun = self.function()?;
                if !names.insert(fun.name.clone()) {
                    return Err(self.syntax("欄位與方法不得共用名稱"));
                }
                methods.push(fun);
            } else {
                if !self.keyword("var") && !self.keyword("const") {
                    return Err(self.syntax("結構僅接受 var、const 欄位及 fn 方法"));
                }
                let immutable = self.keyword("const");
                self.cursor += 1;
                let field = self.name()?;
                if !names.insert(field.clone()) {
                    return Err(self.syntax("同一結構不可重複欄位名稱"));
                }
                self.take(':', "結構欄位必須標註型別")?;
                let ty = self.name()?;
                let default = if self.symbol('=') {
                    self.cursor += 1;
                    Some(self.term()?)
                } else { None };
                if immutable && default.is_none() {
                    return Err(self.syntax("const 結構欄位必須具有宣告時預設值"));
                }
                fields.push(Field { name: field, ty, immutable, default });
            }
            self.end_statement()?;
            self.lines();
        }
        // K06-B：真正 struct 至少有一個欄位或方法；
        // 不把空大括號或只有註解、換行的結構當成已完成宣告。
        if fields.is_empty() && methods.is_empty() {
            return Err(self.syntax("K06：struct 至少需要一個欄位或方法"));
        }
        self.cursor += 1;
        Ok((name, StructDecl { fields, methods }))
    }
    fn file(mut self) -> Result<(HashMap<String, StructDecl>, Vec<Function>), Diagnostic> {
        let mut structs = HashMap::new();
        let mut funcs = Vec::new();
        let mut function_names = HashSet::new();
        self.lines();
        while !self.eof() {
            if self.keyword("struct") {
                let (name, def) = self.strukt()?;
                if structs.insert(name, def).is_some() {
                    return Err(self.syntax("不得重複宣告相同結構型別"));
                }
            } else if self.keyword("fn") {
                let fun = self.function()?;
                if !function_names.insert(fun.name.clone()) {
                    return Err(self.syntax("不得重複宣告函式名稱"));
                }
                funcs.push(fun);
            } else {
                return Err(self.syntax("受限 P13 來源僅接受 struct 與 fn 頂層宣告"));
            }
            if !self.eof() && !matches!(self.tok().kind, TokenKind::Newline) {
                return Err(self.syntax("頂層宣告之間必須換行"));
            }
            self.lines();
        }
        Ok((structs, funcs))
    }
}

#[derive(Clone)]
struct LoanBinding {
    source: usize,
    kind: BorrowKind,
    active: bool,
}

#[derive(Clone)]
struct Binding {
    declaration_start: usize,
    ty: String,
    mutable: bool,
    value: ValueSlot,
    /// 僅以合法 struct 建立式已知的未初始化 var 欄位集合。
    /// None 表示不存在可追蹤的部分建立證明；不等於全部欄位可用。
    pending_fields: Option<HashSet<String>>,
    /// 非擁有的借用別名，不擁有來源值，不允許 .move()。
    loan: Option<LoanBinding>,
}

/// 各大括號區塊都有獨立詞彙作用域；內層同名宣告不得覆蓋外層所有權來源。
type Scopes = Vec<HashMap<String, Binding>>;

/// 資料流需區分正常後繼、函式返回與迴圈轉移；後兩者不能混用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowState { Fallthrough, Returned, LoopTransfer }

struct Checker<'a, C> {
    source: &'a Source,
    structs: &'a HashMap<String, StructDecl>,
    functions: &'a [Function],
    ability: C,
}

impl<C: Fn(&str) -> CopyPermission> Checker<'_, C> {
    fn error(&self, text: &str, term: &Term) -> Diagnostic {
        Diagnostic {
            kind: DiagnosticKind::Static,
            message: text.into(),
            span: Some(self.source.span(term.start, term.end)),
        }
    }
    fn copy_permission(&self, ty: &str, active: &mut HashSet<String>) -> CopyPermission {
        if let Some(def) = self.structs.get(ty) {
            if !active.insert(ty.to_string()) { return CopyPermission::Undecided; }
            let fields: Vec<_> = def.fields.iter()
                .map(|field| self.copy_permission(&field.ty, active)).collect();
            active.remove(ty);
            match struct_ownership::whole_struct_copy(&fields) {
                Ok(()) => CopyPermission::Allowed,
                Err(crate::struct_ownership::StructOwnershipError::FieldCopyForbidden) =>
                    CopyPermission::Forbidden,
                _ => CopyPermission::Undecided,
            }
        } else {
            (self.ability)(ty)
        }
    }
    fn permission(&self, ty: &str) -> CopyPermission {
        self.copy_permission(ty, &mut HashSet::new())
    }

    /// 已明確禁止複製與仍待規格核准，必須使用不同診斷，不可混為一談。
    fn reject_noncopyable(&self, permission: CopyPermission,
        context: &str, term: &Term) -> Diagnostic {
        let reason = match permission {
            CopyPermission::Forbidden => "Copyable 已明確禁止",
            CopyPermission::Undecided => "Copyable 資格未決",
            CopyPermission::Allowed => "Copyable 檢查出現不一致",
        };
        self.error(&format!("{context}：{reason}，不能隱含複製"), term)
    }
    fn lookup<'b>(vars: &'b Scopes, name: &str) -> Option<&'b Binding> {
        vars.iter().rev().find_map(|scope| scope.get(name))
    }
    fn lookup_mut<'b>(vars: &'b mut Scopes, name: &str) -> Option<&'b mut Binding> {
        vars.iter_mut().rev().find_map(|scope| scope.get_mut(name))
    }

    /// 在受限來源中，只有真正包含具名整筆 .move() 的形狀
    /// 才能消耗參數；否則非 Copyable 參數不可被虛構成獨立所有者。
    /// 此檢查不推定一般間接副作用、別名再借用或跨例外逃逸。
    fn term_moves_name(term: &Term, name: &str) -> bool {
        match &term.kind {
            TermKind::Move(source) => source == name,
            TermKind::Construct(_, args) =>
                args.iter().any(|(_, t)| Self::term_moves_name(t, name)),
            TermKind::Call(_, args) | TermKind::MethodCall(_, _, args) =>
                args.iter().any(|t| Self::term_moves_name(t, name)),
            TermKind::BinaryInt {left, right, ..} =>
                Self::term_moves_name(left, name) || Self::term_moves_name(right, name),
            _ => false,
        }
    }

    fn statements_move_name(body: &[Stmt], name: &str) -> bool {
        body.iter().any(|stmt| match &stmt.kind {
            StmtKind::Declare { init, .. } => init.as_ref()
                .is_some_and(|t| Self::term_moves_name(t, name)),
            StmtKind::Assign { value, .. } | StmtKind::FieldAssign { value, .. }
                | StmtKind::Return(value) => Self::term_moves_name(value, name),
            StmtKind::If {condition, yes, no} =>
                Self::term_moves_name(condition, name)
                    || Self::statements_move_name(yes, name)
                    || no.as_ref().is_some_and(|b| Self::statements_move_name(b, name)),
            StmtKind::While {condition, body} =>
                Self::term_moves_name(condition, name)
                    || Self::statements_move_name(body, name),
            StmtKind::ForRange {body, ..} => Self::statements_move_name(body, name),
            StmtKind::Break | StmtKind::Continue => false,
        })
    }

    fn parameter_is_consumed(function: &Function, name: &str) -> bool {
        Self::statements_move_name(&function.body, name)
    }

    /// HC14-A：以實際宣告來源身分查找全部仍有效的非擁有借用。
    /// 不能只以目前作用域或表面名稱判斷，否則遮蔽可繞過來源封鎖。
    fn active_loans(vars: &Scopes, source: usize) -> Vec<BorrowKind> {
        vars.iter().flat_map(|scope| scope.values()).filter_map(|binding| {
            binding.loan.as_ref().and_then(|loan| {
                (loan.active && loan.source == source).then_some(loan.kind)
            })
        }).collect()
    }

    fn check_direct(&self, vars: &Scopes, name: &str, term: &Term)
        -> Result<(), Diagnostic> {
        let owner = Self::lookup(vars, name)
            .ok_or_else(|| self.error("找不到原始來源宣告", term))?;
        if owner.loan.is_none() && !Self::active_loans(vars, owner.declaration_start).is_empty() {
            return Err(self.error("HC14：來源借用仍有效，禁止直接讀寫、複製或明確移動", term));
        }
        Ok(())
    }

    fn authorize_borrow(&self, vars: &Scopes, source: &str, mutable: bool, term: &Term)
        -> Result<(String, LoanBinding), Diagnostic> {
        let owner = Self::lookup(vars, source)
            .ok_or_else(|| self.error("P14 找不到可借用來源", term))?;
        if owner.loan.is_some() {
            return Err(self.error("P14：由借用再次借用尚未有完整別名證明", term));
        }
        owner.value.read().map_err(|e| self.model_error(e, term))?;
        if owner.mutable != mutable {
            return Err(self.error("VA14-01：借用變數的 var／const 權限必須跟隨來源", term));
        }
        let kind = if mutable { BorrowKind::ExclusiveWrite } else { BorrowKind::SharedRead };
        let conflicts = Self::active_loans(vars, owner.declaration_start);
        BorrowSafety::validate_new_borrow(&conflicts, kind)
            .map_err(|_| self.error("HC14：共享／獨占借用與現有別名衝突", term))?;
        Ok((owner.ty.clone(), LoanBinding {
            source: owner.declaration_start, kind, active: true,
        }))
    }

    /// 本批只對真正已解析的語句追蹤別名是否未來仍被使用。
    /// 此處寧可把有遮蔽的同名字串當成仍存活（假陽性），也不能
    /// 漏掉未來引用造成提前解鎖；完整詞彙來源精細度留待 P15 驗收。
    fn term_mentions(term: &Term, name: &str) -> bool {
        match &term.kind {
            TermKind::Name(id) | TermKind::Move(id) | TermKind::Borrow(id)
                => id == name,
            TermKind::FieldRead(id, _) | TermKind::FieldMove(id, _) => id == name,
            TermKind::MethodCall(id, _, args) => id == name
                || args.iter().any(|arg| Self::term_mentions(arg, name)),
            TermKind::Construct(_, arguments) =>
                arguments.iter().any(|(_, value)| Self::term_mentions(value, name)),
            TermKind::Call(_, arguments) =>
                arguments.iter().any(|value| Self::term_mentions(value, name)),
            TermKind::BinaryInt { left, right, .. } =>
                Self::term_mentions(left, name) || Self::term_mentions(right, name),
            TermKind::Literal(_) => false,
        }
    }

    fn statement_mentions(statement: &Stmt, name: &str) -> bool {
        match &statement.kind {
            StmtKind::Declare { init, .. } => init.as_ref()
                .is_some_and(|expr| Self::term_mentions(expr, name)),
            StmtKind::Assign { name: target, value } =>
                target == name || Self::term_mentions(value, name),
            StmtKind::FieldAssign { receiver, value, .. } =>
                receiver == name || Self::term_mentions(value, name),
            StmtKind::If { condition, yes, no } =>
                Self::term_mentions(condition, name)
                || yes.iter().any(|stmt| Self::statement_mentions(stmt, name))
                || no.as_ref().is_some_and(|body|
                    body.iter().any(|stmt| Self::statement_mentions(stmt, name))),
            StmtKind::While { condition, body } =>
                Self::term_mentions(condition, name)
                || body.iter().any(|stmt| Self::statement_mentions(stmt, name)),
            StmtKind::ForRange { body, .. } =>
                body.iter().any(|stmt| Self::statement_mentions(stmt, name)),
            StmtKind::Return(term) => Self::term_mentions(term, name),
            StmtKind::Break | StmtKind::Continue => false,
        }
    }

    /// 依語句來源順序分辨「同名新宣告」與「原借用別名」：
    /// 新宣告的初始化式先在外層環境求值，宣告完成後才遮蔽。
    /// if 分支各有新作用域，不得把分支內遮蔽誤帶到另一分支。
    fn scope_mentions_binding(body: &[Stmt], name: &str, initially_hidden: bool) -> bool {
        let mut hidden = initially_hidden;
        for statement in body {
            match &statement.kind {
                StmtKind::Declare { name: declared, init, .. } => {
                    if !hidden && init.as_ref().is_some_and(|value|
                        Self::term_mentions(value, name)) {
                        return true;
                    }
                    if declared == name { hidden = true; }
                }
                StmtKind::If { condition, yes, no } => {
                    if !hidden && (Self::term_mentions(condition, name)
                        || Self::scope_mentions_binding(yes, name, false)
                        || no.as_ref().is_some_and(|other|
                            Self::scope_mentions_binding(other, name, false))) {
                        return true;
                    }
                }
                StmtKind::While { condition, body } => {
                    if !hidden && (Self::term_mentions(condition, name)
                        || Self::scope_mentions_binding(body, name, false)) {
                        return true;
                    }
                }
                StmtKind::ForRange { iterator, body, .. } => {
                    if !hidden && Self::scope_mentions_binding(
                        body, name, iterator == name) {
                        return true;
                    }
                }
                _ => {
                    if !hidden && Self::statement_mentions(statement, name) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// VA15-01-A：在已知的當前作用域中，以來源順序及宣告遮蔽
    /// 計算最後合法使用點。外層 continuation 尚未有完整的
    /// 跨作用域已解析身分表，因此仍採名稱保守掃描，不能提早歸還。
    fn release_last_uses(vars: &mut Scopes, rest: &[Stmt],
        continuation: &[(usize, Vec<Stmt>)]) {
        let scope_names: Vec<HashSet<String>> = vars.iter()
            .map(|scope| scope.keys().cloned().collect()).collect();
        let mut younger_names = HashSet::new();
        for (depth, scope) in vars.iter_mut().enumerate().rev() {
            for (name, binding) in scope.iter_mut() {
                if let Some(loan) = binding.loan.as_mut() {
                    let needed_here = Self::scope_mentions_binding(
                        rest, name, younger_names.contains(name));
                    // 各後繼分開保留真正的詞彙深度：內層局部遮蔽
                    // 不會延長外層別名；退出內層後重新可見的外層
                    // 借用，仍須保留至外層自己的最後使用點。
                    let needed_later = continuation.iter().any(|(at_depth, following)| {
                        *at_depth >= depth
                            && (*at_depth == depth
                                || !scope_names[depth + 1..=*at_depth].iter()
                                    .any(|known| known.contains(name)))
                            && Self::scope_mentions_binding(following, name, false)
                    });
                    // 函式預設借用參數以其本身宣告身分保存非擁有
                    // 借用票據代理，直到函式作用域真正退出才歸還。
                    // 一般 owner.borrow() 來源不同，仍以最後使用點歸還。
                    let parameter_scope_loan = loan.source == binding.declaration_start;
                    if loan.active && !parameter_scope_loan
                        && !needed_here && !needed_later {
                        loan.active = false;
                    }
                }
            }
            younger_names.extend(scope.keys().cloned());
        }
    }

    /// 這裡僅核准完全唯讀且可靜態證明安全的 self 方法子集。
    /// 根據方法實際操作自動推導共享或獨占借用。
    /// 在此明確受限子集，方法只能修改自身 var 欄位、讀取已核准 Copyable
    /// 的 self 欄位，並於最後回傳可靜態確定型別的值；其他行為保守拒絕。
    /// 只接受無副作用且可複製的布林條件。沒有取得獨立引用值。
    /// 方法中的純值來源共用一套型別／Copyable 檢查，避免合法的
    /// 已核准加減運算式僅因出現在方法內就被錯誤拒絕。
    /// 不處理可移動的擁有者、借用建立、巢狀呼叫或副作用。
    fn method_pure_value_type(&self, owner: &str, method: &Function, term: &Term)
        -> Result<String, Diagnostic> {
        match &term.kind {
            TermKind::Literal(ty) => Ok((*ty).to_string()),
            TermKind::Name(name) if name == "self" =>
                Err(self.error("S15：方法不可回傳整個 self，涉及其他借用與消耗", term)),
            TermKind::Name(name) => {
                let param = method.params.iter().find(|param| param.name == *name)
                    .ok_or_else(|| self.error("方法值來源參數不存在", term))?;
                let permission = self.permission(&param.ty);
                if permission != CopyPermission::Allowed {
                    return Err(self.reject_noncopyable(permission, "方法參數值", term));
                }
                Ok(param.ty.clone())
            }
            TermKind::FieldRead(receiver, name) if receiver == "self" => {
                let field = self.structs.get(owner)
                    .and_then(|def| def.fields.iter().find(|field| field.name == *name))
                    .ok_or_else(|| self.error("方法 self 欄位不存在", term))?;
                let permission = self.permission(&field.ty);
                if permission != CopyPermission::Allowed {
                    let mut error = self.reject_noncopyable(permission, "P13 self 欄位", term);
                    error.message = format!(
                        "{}；不可借用回傳，留待 P15 有效期與逃逸驗證",
                        error.message).into_boxed_str();
                    return Err(error);
                }
                Ok(field.ty.clone())
            }
            TermKind::Call(name, args) => {
                // 一般函式可操作自己的局部來源，但方法目前僅准把
                // Copyable 的獨立引數交給它，且回傳也須已核准 Copyable。
                let function = self.functions.iter().find(|function| function.name == *name)
                    .ok_or_else(|| self.error("方法呼叫不存在的一般函式", term))?;
                if function.params.len() != args.len() {
                    return Err(self.error("方法內函式引數數量不符", term));
                }
                if self.permission(&function.result_ty) != CopyPermission::Allowed {
                    return Err(self.reject_noncopyable(
                        self.permission(&function.result_ty), "方法內函式回傳值", term));
                }
                for (value, parameter) in args.iter().zip(&function.params) {
                    if self.permission(&parameter.ty) != CopyPermission::Allowed {
                        return Err(self.reject_noncopyable(
                            self.permission(&parameter.ty), "方法內函式引數", value));
                    }
                    let actual = self.method_pure_value_type(owner, method, value)?;
                    if actual != parameter.ty {
                        return Err(self.error("方法內函式引數型別不一致", value));
                    }
                }
                Ok(function.result_ty.clone())
            }
            TermKind::Construct(name, args) if args.is_empty()
                && !self.structs.contains_key(name) => {
                // helper() 與 Struct() 共用解析分支，只有已存在的
                // 零參數一般函式可以作為純 Copyable 方法值來源。
                let function = self.functions.iter().find(|function| function.name == *name)
                    .ok_or_else(|| self.error("方法內零參數函式不存在", term))?;
                if !function.params.is_empty() {
                    return Err(self.error("方法內零參數函式實際需傳入引數", term));
                }
                let permission = self.permission(&function.result_ty);
                if permission != CopyPermission::Allowed {
                    return Err(self.reject_noncopyable(permission, "方法內函式回傳值", term));
                }
                Ok(function.result_ty.clone())
            }
            TermKind::BinaryInt { left, operator, right } => {
                let left_ty = self.method_pure_value_type(owner, method, left)?;
                if left_ty != "int" {
                    return Err(self.error("方法整數加減左運算元必須為 int", left));
                }
                let right_ty = self.method_pure_value_type(owner, method, right)?;
                if right_ty != "int" {
                    return Err(self.error("方法整數加減右運算元必須為 int", right));
                }
                debug_assert!(*operator == '+' || *operator == '-');
                Ok("int".to_string())
            }
            _ => Err(self.error("方法值來源尚未證明可安全獨立求值", term)),
        }
    }

    fn method_bool_condition(&self, owner: &str, method: &Function, term: &Term)
        -> Result<(), Diagnostic> {
        let ty = self.method_pure_value_type(owner, method, term)?;
        if ty != "bool" {
            return Err(self.error("方法 if 條件必須為可複製的 bool", term));
        }
        Ok(())
    }

    /// 方法可將合法具名參數的整筆持有權在 return 時明確移交。
    /// 不推定 self 欄位的借用回傳或隱含複製；本階段方法在
    /// 提前／最後 return 前只能使用已驗證的純 Copyable 語句。
    fn method_return_type(&self, owner: &str, method: &Function, term: &Term)
        -> Result<String, Diagnostic> {
        if let TermKind::Move(name) = &term.kind {
            if name == "self" {
                return Err(self.error("S15：方法不可消耗整個 self", term));
            }
            let parameter = method.params.iter().find(|parameter| parameter.name == *name)
                .ok_or_else(|| self.error(
                    "方法僅能明確移動已取得持有權的具名參數", term))?;
            return Ok(parameter.ty.clone());
        }
        // 非 Copyable 結果可以由「不借用 self、不移動既有資源」
        // 的一般函式呼叫直接新建並在 return 時整筆交接。
        // 僅放行已核准 str 擁有值；尚無安全證明的其他資源保留拒絕。
        let ordinary_call: Option<(&str, &[Term])> = match &term.kind {
            TermKind::Call(name, args) => Some((name.as_str(), args.as_slice())),
            TermKind::Construct(name, args) if args.is_empty()
                && !self.structs.contains_key(name) => Some((name.as_str(), &[])),
            _ => None,
        };
        if let Some((name, args)) = ordinary_call {
            if let Some(function) = self.functions.iter().find(|f| f.name == name) {
                if function.result_ty == "str" {
                    if function.params.len() != args.len() {
                        return Err(self.error("方法資源回傳的函式引數數量不符", term));
                    }
                    for (arg, parameter) in args.iter().zip(&function.params) {
                        let permission = self.permission(&parameter.ty);
                        if permission != CopyPermission::Allowed {
                            return Err(self.reject_noncopyable(
                                permission, "方法資源回傳呼叫引數", arg));
                        }
                        let actual = self.method_pure_value_type(owner, method, arg)?;
                        if actual != parameter.ty {
                            return Err(self.error("方法資源回傳呼叫引數型別不符", arg));
                        }
                    }
                    return Ok("str".to_string());
                }
            }
        }
        self.method_pure_value_type(owner, method, term)
    }

    /// 以所有可能執行路徑的操作推導方法整體借用模式；
    /// 任一路徑寫入都需要在方法入口取得獨占借用。
    fn method_writes(&self, owner: &str, method: &Function, body: &[Stmt])
        -> Result<bool, Diagnostic> {
        let definition = self.structs.get(owner).expect("方法所屬型別已確認");
        let mut writes = false;
        for statement in body {
            match &statement.kind {
                StmtKind::FieldAssign { receiver, field, value } if receiver == "self" => {
                    let target = definition.fields.iter().find(|f| f.name == *field)
                        .ok_or_else(|| self.error("self 欄位不存在", value))?;
                    if target.immutable {
                        return Err(self.error("S18：const 欄位禁止透過方法寫入", value));
                    }
                    let actual = self.method_pure_value_type(owner, method, value)?;
                    if actual != target.ty {
                        return Err(self.error("方法欄位賦值型別與宣告不一致", value));
                    }
                    // 方法接收者在此子集已完整初始化，欄位寫入必然覆寫舊值。
                    // 非 Copyable 資源的舊值清理順序尚待 P16，必須先保守拒絕。
                    let permission = self.permission(&target.ty);
                    if permission != CopyPermission::Allowed {
                        return Err(self.reject_noncopyable(permission,
                            "P16 尚未核准方法覆寫既有資源的清理責任", value));
                    }
                    writes = true;
                }
                StmtKind::Return(value) => {
                    // P01 已核准可達分支提前 return；方法的每個出口
                    // 均須遵守相同回傳型別與 Copyable／self 借用限制。
                    let actual = self.method_return_type(owner, method, value)?;
                    if actual != method.result_ty {
                        return Err(self.error("方法提前回傳型別與簽章不一致", value));
                    }
                    // 後續同一條路徑已不可達，不得把它當作額外借用操作。
                    return Ok(writes);
                }
                StmtKind::If { condition, yes, no } => {
                    self.method_bool_condition(owner, method, condition)?;
                    let yes_writes = self.method_writes(owner, method, yes)?;
                    let no_writes = if let Some(otherwise) = no {
                        self.method_writes(owner, method, otherwise)?
                    } else { false };
                    writes |= yes_writes || no_writes;
                }
                _ => return Err(Diagnostic {
                    kind: DiagnosticKind::Static,
                    message: "方法仍有未驗證的語句／別名或生命週期語意".into(),
                    span: Some(self.source.span(statement.start, statement.end)),
                }),
            }
        }
        Ok(writes)
    }

    fn check_method(&self, owner: &str, method: &Function)
        -> Result<BorrowKind, Diagnostic> {
        for parameter in &method.params {
            if parameter.name == "self" {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "S15：方法不得宣告遮蔽 self 的參數".into(), span: None });
            }
            if !["int", "bool", "float", "str"].contains(&parameter.ty.as_str())
                && !self.structs.contains_key(&parameter.ty) {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "P13：方法參數使用不存在的型別".into(), span: None });
            }
        }
        self.reject_method_statements(&method.body)?;
        let Some(Stmt { kind: StmtKind::Return(return_value), .. }) = method.body.last() else {
            return Err(Diagnostic { kind: DiagnosticKind::Static,
                message: "目前方法必須以單一明確 return 結束".into(), span: None });
        };
        let mode = if self.method_writes(owner, method,
            &method.body[..method.body.len() - 1])? {
            BorrowKind::ExclusiveWrite
        } else { BorrowKind::SharedRead };
        let actual = self.method_return_type(owner, method, return_value)?;
        if actual != method.result_ty {
            return Err(self.error("方法回傳型別與已驗證運算式不一致", return_value));
        }
        Ok(mode)
    }

    /// 方法引數依來源順序在取得 self 借用前求值。
    /// 已核准 Copyable 參數可接受具名整筆 .move()；求值後必須
    /// 再查核接收者仍有效。仍禁止未完成的再借用／巢狀呼叫資源副作用。
    fn method_argument_safe_before_borrow(term: &Term) -> bool {
        match &term.kind {
            TermKind::Literal(_) | TermKind::Name(_) | TermKind::Move(_)
                | TermKind::FieldRead(_, _) => true,
            TermKind::BinaryInt { left, right, .. } =>
                Self::method_argument_safe_before_borrow(left)
                    && Self::method_argument_safe_before_borrow(right),
            _ => false,
        }
    }

    /// 迴圈條件每次重新求值；不像方法位置引數，
    /// 整筆 .move() 在這裡不能視為可重複使用的安全來源。
    /// D04-A 對來源整數常值的半開遞增區間：僅在端點相差一且
    /// 可確定只迭代一次時消除不必要的反向邊。其他範圍仍保守驗證。
    fn known_range_iterations(lower: &str, upper: &str) -> Option<u8> {
        fn read_positive_int(text: &str) -> Option<u128> {
            let (radix, digits) = if let Some(rest) = text.strip_prefix("0x") {
                (16, rest)
            } else if let Some(rest) = text.strip_prefix("0b") {
                (2, rest)
            } else if let Some(rest) = text.strip_prefix("0o") {
                (8, rest)
            } else {
                (10, text)
            };
            let normalized: String = digits.chars().filter(|ch| *ch != '_').collect();
            u128::from_str_radix(&normalized, radix).ok()
        }
        match (read_positive_int(lower), read_positive_int(upper)) {
            (Some(start), Some(end)) if start == end => Some(0),
            (Some(start), Some(end)) if start.checked_add(1) == Some(end) => Some(1),
            _ => None,
        }
    }

    fn while_condition_nonconsuming(term: &Term) -> bool {
        match &term.kind {
            TermKind::Literal(_) | TermKind::Name(_) | TermKind::FieldRead(_, _) => true,
            TermKind::BinaryInt { left, right, .. } =>
                Self::while_condition_nonconsuming(left)
                    && Self::while_condition_nonconsuming(right),
            _ => false,
        }
    }

    fn evaluate(&self, term: &Term, vars: &mut Scopes)
        -> Result<(String, bool), Diagnostic> {
        match &term.kind {
            TermKind::Literal(ty) => Ok(((*ty).to_string(), true)),
            TermKind::BinaryInt { left, operator, right } => {
                // 順序由 P01 左結合決定，不得跳過內層 .move() 狀態更新。
                let (left_ty, left_ready) = self.evaluate(left, vars)?;
                if left_ty != "int" || !left_ready {
                    return Err(self.error("P13：加減左運算元必須為已初始化 int", left));
                }
                let (right_ty, right_ready) = self.evaluate(right, vars)?;
                if right_ty != "int" || !right_ready {
                    return Err(self.error("P13：加減右運算元必須為已初始化 int", right));
                }
                debug_assert!(*operator == '+' || *operator == '-');
                Ok(("int".to_string(), true))
            }
            TermKind::Name(name) => {
                self.check_direct(vars, name, term)?;
                let binding = Self::lookup(vars, name)
                    .ok_or_else(|| self.error("找不到合法的來源宣告", term))?;
                if binding.loan.as_ref().is_some_and(|loan| !loan.active) {
                    return Err(self.error("P15：借用已於最後使用點歸還", term));
                }
                // 非擁有借用僅能對已核准 Copyable 值產生獨立複本，
                // 不得以一般名稱使用把 str 借用轉成新資源擁有者。
                binding.value.duplicate().map_err(|err| self.model_error(err, term))?;
                Ok((binding.ty.clone(), true))
            }
            TermKind::Borrow(_) => Err(self.error(
                "VA14／VA15：.borrow() 僅可直接綁定到合法 var／const；回傳與逃逸保守拒絕",
                term)),
            TermKind::Move(name) => {
                if name == "self" {
                    return Err(self.error("S15：方法不可消耗或移動整個 self", term));
                }
                self.check_direct(vars, name, term)?;
                let binding = Self::lookup_mut(vars, name)
                    .ok_or_else(|| self.error("找不到可移動的具名來源", term))?;
                if binding.loan.is_some() {
                    return Err(self.error("VA14-01：借用別名不是所有者，不能 .move()", term));
                }
                binding.value.move_whole().map_err(|err| self.model_error(err, term))?;
                Ok((binding.ty.clone(), true))
            }
            TermKind::FieldRead(owner, name) => {
                if owner == "self" {
                    return Err(self.error("只有方法內才能讀取 self 欄位", term));
                }
                self.check_direct(vars, owner, term)?;
                let bound = Self::lookup(vars, owner)
                    .ok_or_else(|| self.error("找不到欄位來源宣告", term))?;
                if bound.loan.as_ref().is_some_and(|loan| !loan.active) {
                    return Err(self.error("P15：借用別名已不再有效", term));
                }
                bound.value.read().map_err(|err| self.model_error(err, term))?;
                let field = self.structs.get(&bound.ty)
                    .and_then(|def| def.fields.iter().find(|f| f.name == *name))
                    .ok_or_else(|| self.error("來源不是具有此欄位的結構", term))?;
                let permission = self.permission(&field.ty);
                if permission != CopyPermission::Allowed {
                    return Err(self.reject_noncopyable(permission,
                        "欄位讀取", term));
                }
                Ok((field.ty.clone(), true))
            }
            TermKind::MethodCall(receiver, name, args) => {
                if receiver == "self" {
                    return Err(self.error("重入 self 方法的借用安全尚未驗證", term));
                }
                self.check_direct(vars, receiver, term)?;
                let owner_type = {
                    let bound = Self::lookup(vars, receiver)
                        .ok_or_else(|| self.error("找不到方法接收者來源", term))?;
                    bound.value.read().map_err(|err| self.model_error(err, term))?;
                    bound.ty.clone()
                };
                let method = self.structs.get(&owner_type)
                    .and_then(|def| def.methods.iter().find(|m| m.name == *name))
                    .ok_or_else(|| self.error("找不到結構上的合法方法", term))?;
                let mode = self.check_method(&owner_type, method)?;
                if method.params.len() != args.len() {
                    return Err(self.error("P13：方法引數數量不符", term));
                }
                // P15.6：方法參數也屬函式的預設借用，但 self
                // 在本次呼叫期間有自己的借用票據。先評估所有
                // 參數並保持其借用到方法出口；具名 str 不等同複製。
                let receiver_source = Self::lookup(vars, receiver)
                    .ok_or_else(|| self.error("找不到方法接收者來源", term))?
                    .declaration_start;
                let mut argument_loans = BorrowSafety::new();
                let mut argument_tickets = Vec::new();
                let mut borrowed_names: Vec<String> = Vec::new();
                for (arg, parameter) in args.iter().zip(&method.params) {
                    if !Self::method_argument_safe_before_borrow(arg) {
                        return Err(self.error(
                            "P13：方法引數僅接受已驗證的純值或具名整筆移動", arg));
                    }
                    let is_default_borrow = self.permission(&parameter.ty)
                        != CopyPermission::Allowed
                        && matches!(&arg.kind, TermKind::Name(_))
                        && !Self::parameter_is_consumed(method, &parameter.name);
                    if !is_default_borrow && borrowed_names.iter()
                        .any(|name| Self::term_mentions(arg, name)) {
                        return Err(self.error(
                            "P15.6：方法其餘引數不能使用尚未歸還的借用來源", arg));
                    }
                    if is_default_borrow {
                        let TermKind::Name(name) = &arg.kind else { unreachable!() };
                        let original = Self::lookup(vars, name)
                            .ok_or_else(|| self.error("P15.6：方法引數借用來源不存在", arg))?;
                        if original.loan.is_some() {
                            return Err(self.error(
                                "VB14：方法參數不可在未證明安全時再借用別名", arg));
                        }
                        if original.declaration_start == receiver_source {
                            return Err(self.error(
                                "P15.6：self 與方法參數不能對同一來源建立未證明的重疊借用", arg));
                        }
                        original.value.read().map_err(|err| self.model_error(err, arg))?;
                        if original.ty != parameter.ty {
                            return Err(self.error("P15.6：方法借用參數型別不一致", arg));
                        }
                        let kind = if original.mutable {
                            BorrowKind::ExclusiveWrite
                        } else { BorrowKind::SharedRead };
                        BorrowSafety::validate_new_borrow(
                            &Self::active_loans(vars, original.declaration_start), kind)
                            .map_err(|_| self.error(
                                "HC14：方法參數與既有借用衝突", arg))?;
                        let ticket = argument_loans.acquire(
                            DeclarationId(original.declaration_start), kind)
                            .map_err(|_| self.error(
                                "HC14：同一方法呼叫的參數借用衝突", arg))?;
                        argument_tickets.push(ticket);
                        borrowed_names.push(name.clone());
                        continue;
                    }
                    // 非 Copyable 參數若確實被方法整筆 .move() 消耗，
                    // 普通名稱仍不得通過 evaluate() 偷成第二所有者。
                    let (actual, ready) = self.evaluate(arg, vars)?;
                    if !ready || actual != parameter.ty {
                        return Err(self.error("P13：方法引數型別錯誤或尚未初始化", arg));
                    }
                }
                // 任何參數求值後再次驗證接收者與來源借用狀態。
                self.check_direct(vars, receiver, term)?;
                let bound = Self::lookup(vars, receiver)
                    .ok_or_else(|| self.error("找不到方法接收者來源", term))?;
                if bound.loan.as_ref().is_some_and(|loan| !loan.active) {
                    return Err(self.error("P15：借用別名已不再有效", term));
                }
                bound.value.read().map_err(|err| self.model_error(err, term))?;
                if mode == BorrowKind::ExclusiveWrite && !bound.mutable {
                    return Err(self.error(
                        "S18：const 結構實例不能呼叫會修改 self 欄位的方法", term));
                }
                if let Some(loan) = &bound.loan {
                    if mode == BorrowKind::ExclusiveWrite
                        && loan.kind != BorrowKind::ExclusiveWrite {
                        return Err(self.error("HC14：唯讀借用不可呼叫可寫方法", term));
                    }
                } else {
                    let mut loans = BorrowSafety::new();
                    let loan = loans.acquire(DeclarationId(bound.declaration_start), mode)
                        .map_err(|_| self.error("方法 self 借用與現有借用衝突", term))?;
                    if loans.check_direct_access(DeclarationId(bound.declaration_start)).is_ok() {
                        return Err(self.error("HC14：方法期間不得直接使用被借用來源", term));
                    }
                    loans.finish_scope(&[loan])
                        .map_err(|_| self.error("方法 self 借用未安全歸還", term))?;
                    loans.check_direct_access(DeclarationId(bound.declaration_start))
                        .map_err(|_| self.error("方法後來源借用未解除", term))?;
                }
                // 方法實際完成後才歸還參數，不使用一般借用最後
                // 使用點規則，也不讓未通過的其他控制路徑釋放票據。
                argument_loans.finish_scope(&argument_tickets)
                    .map_err(|_| self.error("P15.6：方法參數作用域歸還失敗", term))?;
                Ok((method.result_ty.clone(), true))
            }
            TermKind::FieldMove(_owner, _field) => {
                let _ = struct_ownership::check_struct_move_target(MoveTarget::SingleField);
                Err(self.error("S21：結構欄位不能單獨移動", term))
            }
            TermKind::Call(name, args) => {
                let function = self.functions.iter().find(|function| function.name == *name)
                    .ok_or_else(|| self.error("P13：呼叫不存在的函式", term))?;
                if function.params.len() != args.len() {
                    return Err(self.error("P13：函式引數數量不符", term));
                }
                // P15.6：函式參數預設借用。若被呼叫端確實包含
                // 該參數的整筆 .move()，則呼叫端必須明確移交來源；
                // 不得把非擁有別名假裝成能被消耗的資源持有者。
                // 引數形式先全部預檢，避免驗到後面的錯誤時已
                // 將前面具名資源移走。
                for (arg, parameter) in args.iter().zip(&function.params) {
                    let expected = &parameter.ty;
                    if self.permission(expected) != CopyPermission::Allowed {
                        match &arg.kind {
                            TermKind::Name(_) if !Self::parameter_is_consumed(
                                function, &parameter.name) => {}
                            TermKind::Literal("str") | TermKind::Move(_)
                                | TermKind::Construct(_, _) | TermKind::Call(_, _) => {}
                            _ => return Err(self.error(
                                "P15.6：非 Copyable 參數只接受可證明的預設借用或明確擁有權轉交", arg)),
                        }
                    }
                }
                // 使用同一套 HC14 借用衝突與票據模型，票據只存活
                // 於函式呼叫期間。不能讓同一呼叫較後的引數提前
                // 移動、覆寫或再次獨占借用仍被傳入的來源。
                let mut call_loans = BorrowSafety::new();
                let mut tickets = Vec::new();
                let mut borrowed_names: Vec<String> = Vec::new();
                for (arg, parameter) in args.iter().zip(&function.params) {
                    let expected = &parameter.ty;
                    let new_borrow = self.permission(expected) != CopyPermission::Allowed
                        && matches!(&arg.kind, TermKind::Name(_))
                        && !Self::parameter_is_consumed(function, &parameter.name);
                    if !new_borrow && borrowed_names.iter()
                        .any(|name| Self::term_mentions(arg, name)) {
                        return Err(self.error(
                            "P15.6：同一呼叫的後續引數不可使用尚未歸還的借用來源", arg));
                    }
                    if self.permission(expected) != CopyPermission::Allowed {
                        if let TermKind::Name(name) = &arg.kind {
                            if !Self::parameter_is_consumed(function, &parameter.name) {
                                let original = Self::lookup(vars, name)
                                    .ok_or_else(|| self.error("P15.6：函式借用來源不存在", arg))?;
                                if original.loan.is_some() {
                                    return Err(self.error(
                                        "VB14：借用別名不能在缺少重借用證明時傳入函式", arg));
                                }
                                original.value.read().map_err(|err| self.model_error(err, arg))?;
                                if original.ty != *expected {
                                    return Err(self.error("P15.6：函式借用實參型別不符", arg));
                                }
                                let kind = if original.mutable {
                                    BorrowKind::ExclusiveWrite
                                } else { BorrowKind::SharedRead };
                                BorrowSafety::validate_new_borrow(
                                    &Self::active_loans(vars, original.declaration_start), kind)
                                    .map_err(|_| self.error(
                                        "HC14：函式參數借用與仍有效別名衝突", arg))?;
                                let ticket = call_loans.acquire(
                                    DeclarationId(original.declaration_start), kind)
                                    .map_err(|_| self.error(
                                        "HC14：同一呼叫內函式借用來源衝突", arg))?;
                                tickets.push(ticket);
                                borrowed_names.push(name.clone());
                                continue;
                            }
                        }
                    }
                    let (actual, initialized) = self.evaluate(arg, vars)?;
                    if actual != *expected || !initialized {
                        return Err(self.error("P13：函式引數型別不符或尚未初始化", arg));
                    }
                }
                call_loans.finish_scope(&tickets)
                    .map_err(|_| self.error("P15.6：函式退出時借用票據未完整歸還", term))?;
                Ok((function.result_ty.clone(), true))
            },
            TermKind::Construct(name, args) => {
                // 零引數語法與結構建立式共用括號。只有不存在同名結構
                // 且已宣告零參數函式時，才按函式呼叫處理。
                if args.is_empty() && !self.structs.contains_key(name) {
                    if let Some(function) = self.functions.iter().find(|function| function.name == *name) {
                        if !function.params.is_empty() {
                            return Err(self.error("P13：函式引數數量不符", term));
                        }
                        return Ok((function.result_ty.clone(), true));
                    }
                }
                let definition = self.structs.get(name)
                    .ok_or_else(|| self.error("找不到結構型別或不支援一般函式呼叫", term))?;
                // VB16-01-A：嚴格依書寫順序先求值所有具名引數，之後
                // 再依宣告順序求值未被覆蓋的預設值。先檢查整份
                // 計畫再求值，重複／不存在欄位不得先消耗資源來源。
                let fields: Vec<_> = definition.fields.iter().map(|field|
                    crate::struct_eval_plan::FieldInfo {
                        name: field.name.as_str(),
                        has_default: field.default.is_some(),
                        immutable: field.immutable,
                    }).collect();
                let names: Vec<_> = args.iter().map(|(field, _)| field.as_str()).collect();
                let ordered = crate::struct_eval_plan::plan(&fields, &names)
                    .map_err(|error| self.error(&format!(
                        "VB16：結構求值計畫不合法：{error:?}"), term))?;
                for step in &ordered.steps {
                    let expected = definition.fields.iter()
                        .find(|field| field.name == step.field)
                        .expect("計畫中的欄位已預檢存在");
                    let (expr, actual, ready) = match step.origin {
                        crate::struct_eval_plan::Origin::Named => {
                            let expr = args.iter()
                                .find(|(field, _)| *field == step.field)
                                .map(|(_, expression)| expression)
                                .expect("具名引數已預檢存在");
                            let (actual, ready) = self.evaluate(expr, vars)?;
                            (expr, actual, ready)
                        }
                        crate::struct_eval_plan::Origin::Default => {
                            let expr = expected.default.as_ref()
                                .expect("未被覆蓋的預設值已預檢存在");
                            // 欄位預設值在其宣告本身的獨立上下文求值，
                            // 不暗中捕獲呼叫端的區域名稱。
                            let (actual, ready) = self.evaluate(
                                expr, &mut vec![HashMap::new()])?;
                            (expr, actual, ready)
                        }
                    };
                    if actual != expected.ty || !ready {
                        return Err(self.error(
                            "VB16：結構欄位求值型別錯誤或資源尚未初始化",
                            expr));
                    }
                }
                Ok((name.clone(), ordered.missing_mutable_fields.is_empty()))
            }
        }
    }
    fn model_error(&self, err: ModelError, term: &Term) -> Diagnostic {
        let message = match err {
            ModelError::MovedRead => "來源已移動，禁止再次使用",
            ModelError::UninitializedRead => "來源尚未完成初始化，不可使用",
            ModelError::UncertainRead => "存在可達路徑使來源可能無效",
            ModelError::CopyForbidden => "型別 Copyable 已明確禁止，拒絕隱含複製",
            ModelError::CopyUndecided => "型別 Copyable 能力未核准，拒絕預設複製",
            _ => "所有權狀態無法獲得合法證明",
        };
        self.error(message, term)
    }
    /// 只尋找會跳出目前 for／while 的控制轉移；內層迴圈的
    /// break／continue 不屬於外層，不能因此封鎖合法單次移動。
    fn has_current_loop_transfer(body: &[Stmt]) -> bool {
        body.iter().any(|stmt| match &stmt.kind {
            StmtKind::Break | StmtKind::Continue => true,
            StmtKind::If { yes, no, .. } => Self::has_current_loop_transfer(yes)
                || no.as_ref().is_some_and(|other| Self::has_current_loop_transfer(other)),
            StmtKind::ForRange { .. } | StmtKind::While { .. } => false,
            _ => false,
        })
    }

    /// P15.3：對 break／continue 的每條實際轉移路徑確認所有
    /// 外層持有者、部分初始化與借用票據均與迴圈入口一致。
    /// 不把未達不動點的資源變化偽裝成安全跳出或下一輪。
    fn check_loop_exit_stability(&self, vars: &Scopes, entry: &Scopes,
        statement: &Stmt) -> Result<(), Diagnostic> {
        for (depth, scope) in entry.iter().enumerate() {
            for (name, original) in scope {
                let candidate = vars.get(depth).and_then(|scope| scope.get(name))
                    .ok_or_else(|| Diagnostic {
                        kind: DiagnosticKind::Static,
                        message: "P15：跳轉後外層來源宣告遺失".into(),
                        span: Some(self.source.span(statement.start, statement.end)),
                    })?;
                let original_loan = original.loan.as_ref().map(|loan|
                    (loan.source, loan.kind, loan.active));
                let candidate_loan = candidate.loan.as_ref().map(|loan|
                    (loan.source, loan.kind, loan.active));
                if candidate.declaration_start != original.declaration_start
                    || candidate.value != original.value
                    || candidate.pending_fields != original.pending_fields
                    || original_loan != candidate_loan {
                    return Err(Diagnostic {
                        kind: DiagnosticKind::Static,
                        message: if matches!(&statement.kind, StmtKind::ForRange { .. }) {
                            "P15：單次 for 混合迴圈跳轉與正常路徑時外層所有權或借用狀態不穩定".into()
                        } else {
                            "P15：break／continue 轉移時外層所有權或借用狀態不穩定".into()
                        },
                        span: Some(self.source.span(statement.start, statement.end)),
                    });
                }
            }
        }
        Ok(())
    }

    fn statements(&self, body: &[Stmt], vars: &mut Scopes,
        expected_return: &str, new_scope: bool) -> Result<FlowState, Diagnostic> {
        self.statements_with_follow(body, vars, expected_return, new_scope, &[], None)
    }

    fn statements_with_follow(&self, body: &[Stmt], vars: &mut Scopes,
        expected_return: &str, new_scope: bool,
        continuation: &[(usize, Vec<Stmt>)],
        loop_entry: Option<&Scopes>) -> Result<FlowState, Diagnostic> {
        if new_scope { vars.push(HashMap::new()); }
        let result = self.statement_body(body, vars, expected_return, continuation, loop_entry);
        if new_scope { vars.pop(); }
        result
    }

    fn statement_body(&self, body: &[Stmt], vars: &mut Scopes,
        expected_return: &str, continuation: &[(usize, Vec<Stmt>)],
        loop_entry: Option<&Scopes>) -> Result<FlowState, Diagnostic> {
        // 區塊入口若某外層借用在本條路徑及其外層後繼均不再使用，
        // 便已過最後使用點，不需要等到下一筆語句才歸還。
        Self::release_last_uses(vars, body, continuation);
        for (position, statement) in body.iter().enumerate() {
            match &statement.kind {
                StmtKind::Declare { name, ty, mutable, init } => {
                    if vars.last().expect("應有詞彙作用域").contains_key(name) {
                        return Err(Diagnostic { kind: DiagnosticKind::Static,
                            message: "此受限子集不接受同作用域重複宣告".into(),
                            span: Some(self.source.span(statement.start, statement.end)) });
                    }
                    if !["int", "bool", "float", "str"].contains(&ty.as_str())
                        && !self.structs.contains_key(ty) {
                        return Err(Diagnostic { kind: DiagnosticKind::Static,
                            message: "宣告使用不存在的型別".into(),
                            span: Some(self.source.span(statement.start, statement.end)) });
                    }
                    let mut loan = None;
                    let initialized = if let Some(expr) = init {
                        let (actual, ready) = if let TermKind::Borrow(source) = &expr.kind {
                            let (actual, alias) = self.authorize_borrow(vars, source, *mutable, expr)?;
                            loan = Some(alias);
                            (actual, true)
                        } else {
                            self.evaluate(expr, vars)?
                        };
                        if &actual != ty {
                            return Err(self.error("初始化運算式與宣告型別不一致", expr));
                        }
                        ready
                    } else { false };
                    // 只有具名 struct 建立式可給出精確的缺失欄位集合；
                    // 無初始化的普通 var 宣告不得假設已分配結構實例。
                    let pending_fields = if !initialized {
                        if let Some(Term { kind: TermKind::Construct(actual, args), .. }) = init {
                            self.structs.get(actual).and_then(|definition| {
                                let explicit: HashSet<&str> =
                                    args.iter().map(|(name, _)| name.as_str()).collect();
                                let missing: HashSet<String> = definition.fields.iter()
                                    .filter(|field| field.default.is_none()
                                        && !explicit.contains(field.name.as_str()))
                                    .map(|field| field.name.clone())
                                    .collect();
                                if missing.is_empty() { None } else { Some(missing) }
                            })
                        } else { None }
                    } else { None };
                    let permission = self.permission(ty);
                    let slot = if initialized {
                        ValueSlot::initialized(permission)
                    } else {
                        ValueSlot::uninitialized(permission)
                    };
                    vars.last_mut().expect("應有詞彙作用域").insert(name.clone(), Binding {
                        declaration_start: statement.start, ty: ty.clone(),
                        mutable: *mutable, value: slot, pending_fields, loan
                    });
                }
                StmtKind::FieldAssign { receiver, field, value } => {
                    // VA16-03-A：已核准的 str 獨立常值可以先取得唯一新資源，
                    // 成功後原子覆寫具名 var 結構的 var str 欄位；
                    // 其他非 Copyable 右值仍維持靜態保守拒絕。
                    self.check_direct(vars, receiver, value)?;
                    let owner = Self::lookup(vars, receiver)
                        .ok_or_else(|| self.error("欄位寫入找不到合法接收者", value))?;
                    if owner.loan.as_ref().is_some_and(|loan| !loan.active) {
                        return Err(self.error("P15：借用別名已不再有效", value));
                    }
                    let first_init = owner.value.state() == ValueState::Uninitialized
                        && owner.pending_fields.as_ref()
                            .is_some_and(|pending| pending.contains(field));
                    if !first_init {
                        owner.value.read().map_err(|err| self.model_error(err, value))?;
                    }
                    if !owner.mutable {
                        return Err(self.error("S18：const 結構實例不可修改欄位", value));
                    }
                    let member = self.structs.get(&owner.ty)
                        .and_then(|def| def.fields.iter().find(|f| f.name == *field))
                        .ok_or_else(|| self.error("欄位寫入的型別或欄位不存在", value))?;
                    if member.immutable {
                        return Err(self.error("S18：const 欄位不可重新賦值", value));
                    }
                    let expected_type = member.ty.clone();
                    if !first_init {
                        let permission = self.permission(&expected_type);
                        let proven_fresh_string = expected_type == "str"
                            && matches!(&value.kind, TermKind::Literal("str"));
                        if permission != CopyPermission::Allowed && !proven_fresh_string {
                            return Err(self.reject_noncopyable(permission,
                                "P16：非 Copyable 欄位只核准新建 str 常值原子覆寫，其他資源右值仍未證明", value));
                        }
                    }
                    let (actual, ready) = self.evaluate(value, vars)?;
                    if actual != expected_type || !ready {
                        return Err(self.error("欄位賦值型別錯誤或來源尚未完整初始化", value));
                    }
                    // RHS 有可能消耗或重新繫結接收者：重新查核後才更新證明。
                    let owner = Self::lookup_mut(vars, receiver)
                        .ok_or_else(|| self.error("欄位寫入接收者已失效", value))?;
                    if first_init {
                        if owner.value.state() != ValueState::Uninitialized
                            || !owner.pending_fields.as_ref()
                                .is_some_and(|pending| pending.contains(field)) {
                            return Err(self.error("欄位初始化來源身分已失效", value));
                        }
                        let pending = owner.pending_fields.as_mut()
                            .expect("首次初始化必須有待完成欄位證明");
                        pending.remove(field);
                        if pending.is_empty() {
                            owner.pending_fields = None;
                            owner.value.mark_reinitialized();
                        }
                    } else {
                        owner.value.read().map_err(|err| self.model_error(err, value))?;
                    }
                }
                StmtKind::Assign { name, value } => {
                    self.check_direct(vars, name, value)?;
                    let expected = Self::lookup(vars, name).ok_or_else(|| self.error(
                        "無法賦值給尚未宣告的名稱", value))?;
                    if expected.loan.is_some() {
                        return Err(self.error(
                            "VA14-01：不得把非擁有借用綁定重新賦值成持有者", value));
                    }
                    if !expected.mutable { return Err(self.error("const 不可重新賦值", value)); }
                    let ty = expected.ty.clone();
                    // VA16-03-A 已核准非 Copyable 資源原子覆寫。
                    // 僅對已實作真實 P16 單一持有者緩衝區的 str，
                    // 准許獨立新常值與另一具名整筆移動；含資源結構、
                    // 容器與未確認欄位清理仍不得假裝有相同執行期保證。
                    let state = expected.value.state();
                    if self.permission(&ty) != CopyPermission::Allowed
                        && state == ValueState::MayBeUnavailable {
                        return Err(self.error(
                            "VA16-03：合流後資源可能存在或已移動，條件性舊值清理尚未證明",
                            value));
                    }
                    if self.permission(&ty) != CopyPermission::Allowed
                        && !matches!(state, ValueState::Uninitialized | ValueState::Moved) {
                        let proven_str_rhs = ty == "str"
                            && matches!(&value.kind,
                                TermKind::Literal("str") | TermKind::Move(_));
                        if !proven_str_rhs {
                            return Err(self.error(
                                "P16：此型別／右值的既有資源覆寫尚未完成端到端清理證明",
                                value));
                        }
                    }
                    // 自身轉移與寫回的重疊求值次序尚未有獨立安全證據。
                    if matches!(&value.kind, TermKind::Move(source) if source == name) {
                        return Err(self.error(
                            "P13：同一名稱不可在同一次賦值中移動並重綁自身",
                            value));
                    }
                    let (actual, ready) = self.evaluate(value, vars)?;
                    if actual != ty || !ready {
                        return Err(self.error("賦值型別錯誤或來源尚未完成初始化", value));
                    }
                    let destination = Self::lookup_mut(vars, name).expect("已確認來源");
                    destination.value.mark_reinitialized();
                    destination.pending_fields = None;
                }
                StmtKind::Return(term) => {
                    let (actual, ready) = self.evaluate(term, vars)?;
                    if actual != expected_return || !ready {
                        return Err(self.error("函式回傳型別不一致或來源未初始化", term));
                    }
                    return Ok(FlowState::Returned);
                }
                StmtKind::Break | StmtKind::Continue => {
                    let Some(snapshot) = loop_entry else {
                        return Err(Diagnostic {
                            kind: DiagnosticKind::Static,
                            message: "P15：break／continue 只能位於已核准的 for／while 迴圈內".into(),
                            span: Some(self.source.span(statement.start, statement.end)),
                        });
                    };
                    // 迴圈的中斷或反向邊都不能把外層來源變成
                    // 另一個未證明的持有者／別名狀態。嵌套 if 各路
                    // 徑在轉移點個別查核，因此不會把 continue 偷當 return。
                    self.check_loop_exit_stability(vars, snapshot, statement)?;
                    return Ok(FlowState::LoopTransfer);
                }
                StmtKind::ForRange { iterator, iterator_start,
                    lower, upper, body: loop_body } => {
                    // D04-A 半開區間為已核准來源，但正式執行期的
                    // 整數計數器／步幅不屬 P15，本階段只驗證借用
                    // 與反向邊：一次以上迭代皆不能破壞外層狀態。
                    let known_iterations = Self::known_range_iterations(lower, upper);
                    let one_iteration = known_iterations == Some(1);
                    // 端點相同的半開區間沒有任何迭代：不得把未執行
                    // 的迴圈本體當成已消耗或仍持有外層借用的路徑。
                    if known_iterations != Some(0) {
                    let snapshot = vars.clone();
                    let mut simulated = snapshot.clone();
                    // for 迭代名稱每次重新建立，不能在出口沿用前
                    // 一次迭代的借用或將其誤認成外層同名宣告。
                    simulated.push(HashMap::new());
                    simulated.last_mut().expect("迭代作用域存在").insert(
                        iterator.clone(),
                        Binding {
                            declaration_start: *iterator_start, ty: "int".into(),
                            mutable: false,
                            value: ValueSlot::initialized(CopyPermission::Allowed),
                            pending_fields: None, loan: None,
                        });
                    let mut after_loop = Vec::with_capacity(continuation.len() + 2);
                    // 已知恰好一次迭代時不存在下一次本體；
                    // 不得將假想反向邊延長借用最後使用點。
                    if !one_iteration {
                        after_loop.push((vars.len(), loop_body.clone()));
                    }
                    after_loop.push((vars.len() - 1, body[position + 1..].to_vec()));
                    after_loop.extend_from_slice(continuation);
                    let can_repeat = self.statements_with_follow(
                        loop_body, &mut simulated, expected_return, false, &after_loop,
                        Some(&snapshot))?;
                    simulated.pop();
                    if one_iteration {
                        // break／continue 只退出本迴圈，不是函式 return。
                        // 若入口狀態經轉移驗證保持穩定，後繼仍須檢查。
                        match can_repeat {
                            FlowState::Returned => return Ok(FlowState::Returned),
                            FlowState::LoopTransfer => *vars = snapshot,
                            FlowState::Fallthrough => {
                                // 此三態回傳只記錄「可繼續」的分支狀態。
                                // 若同一輪也存在 break／continue，不能把
                                // 正常分支的重新初始化或移動當成所有出口
                                // 都發生過；跳轉路徑已個別驗證與入口一致，
                                // 正常路徑也必須一致才可安全合流。
                                if Self::has_current_loop_transfer(loop_body) {
                                    self.check_loop_exit_stability(&simulated, &snapshot, statement)?;
                                }
                                *vars = simulated;
                            }
                        }
                    } else if can_repeat == FlowState::Fallthrough {
                        for (depth, scope) in snapshot.iter().enumerate() {
                            for (name, original) in scope {
                                let candidate = simulated[depth].get(name)
                                    .expect("迴圈不得移除外層宣告");
                                let old_loan = original.loan.as_ref().map(|loan|
                                    (loan.source, loan.kind, loan.active));
                                let new_loan = candidate.loan.as_ref().map(|loan|
                                    (loan.source, loan.kind, loan.active));
                                if candidate.declaration_start != original.declaration_start
                                    || candidate.value != original.value
                                    || candidate.pending_fields != original.pending_fields
                                    || old_loan != new_loan {
                                    return Err(Diagnostic {
                                        kind: DiagnosticKind::Static,
                                        message: "P15：for 反向邊來源狀態不穩定，不可跨迭代沿用借用／所有權".into(),
                                        span: Some(self.source.span(statement.start, statement.end)),
                                    });
                                }
                            }
                        }
                    }
                    } // 有至少一種迭代可能時才檢查迴圈本體
                }
                StmtKind::While { condition, body: loop_body } => {
                    // 條件於每次迭代重新求值；不得讓條件本身移動
                    // 所有者、建立借用或觸發未受控函式副作用。
                    if !Self::while_condition_nonconsuming(condition) {
                        return Err(self.error(
                            "P15：while 條件只能使用無資源副作用的合法純值", condition));
                    }
                    let (actual, ready) = self.evaluate(condition, vars)?;
                    if actual != "bool" || !ready {
                        return Err(self.error("while 條件必須是已初始化 bool", condition));
                    }
                    let snapshot = vars.clone();
                    let mut simulated = snapshot.clone();
                    // 下一次條件檢查及迭代本體都屬未來可達路徑，
                    // 不能在本次迭代提早歸還還會被引用的外層別名。
                    let condition_read = Stmt {
                        kind: StmtKind::Return(condition.clone()),
                        start: condition.start,
                        end: condition.end,
                    };
                    let mut after_loop = Vec::with_capacity(continuation.len() + 3);
                    after_loop.push((vars.len() - 1, vec![condition_read]));
                    after_loop.push((vars.len() - 1, loop_body.clone()));
                    after_loop.push((vars.len() - 1, body[position + 1..].to_vec()));
                    after_loop.extend_from_slice(continuation);
                    let can_repeat = self.statements_with_follow(
                        loop_body, &mut simulated, expected_return, true, &after_loop,
                        Some(&snapshot))?;
                    if can_repeat == FlowState::Fallthrough {
                        // 未建立不動點證明前，所有會回到條件的外部來源
                        // 必須保持所有權、部分初始化及借用活性不變。
                        // 迴圈局部宣告會於每次區塊退出後消失。
                        for (depth, scope) in snapshot.iter().enumerate() {
                            for (name, original) in scope {
                                let candidate = simulated[depth].get(name)
                                    .expect("迴圈不得移除外層宣告");
                                let old_loan = original.loan.as_ref().map(|loan|
                                    (loan.source, loan.kind, loan.active));
                                let new_loan = candidate.loan.as_ref().map(|loan|
                                    (loan.source, loan.kind, loan.active));
                                if candidate.declaration_start != original.declaration_start
                                    || candidate.value != original.value
                                    || candidate.pending_fields != original.pending_fields
                                    || old_loan != new_loan {
                                    return Err(self.error(
                                        "P15：while 反向邊的來源所有權／借用狀態不穩定，需完整不動點證明",
                                        condition));
                                }
                            }
                        }
                    }
                    // while 可執行零次，因此後續只使用入口快照；
                    // 任何可回返狀態已確認與此快照一致。
                }
                StmtKind::If { condition, yes, no } => {
                    let (actual, ready) = self.evaluate(condition, vars)?;
                    if actual != "bool" || !ready {
                        return Err(self.error("if 條件必須是已初始化 bool", condition));
                    }
                    let snapshot = vars.clone();
                    // 每條分支只能參考自己之後真正可達的語句。
                    // 外層後繼若還會使用某個借用別名，任何分支都
                    // 不得在提前歸還後直接使用原來源。
                    let mut after_branch = Vec::with_capacity(continuation.len() + 1);
                    after_branch.push((vars.len() - 1, body[position + 1..].to_vec()));
                    after_branch.extend_from_slice(continuation);
                    let mut on_true = snapshot.clone();
                    let true_flow = self.statements_with_follow(
                        yes, &mut on_true, expected_return, true, &after_branch,
                        loop_entry)?;
                    let mut on_false = snapshot.clone();
                    let false_flow = if let Some(otherwise) = no {
                        self.statements_with_follow(
                            otherwise, &mut on_false, expected_return, true, &after_branch,
                            loop_entry)?
                    } else {
                        Self::release_last_uses(&mut on_false, &[], &after_branch);
                        FlowState::Fallthrough
                    };
                    let live_true = true_flow == FlowState::Fallthrough;
                    let live_false = false_flow == FlowState::Fallthrough;
                    if !live_true && !live_false {
                        return Ok(if true_flow == FlowState::Returned
                            && false_flow == FlowState::Returned {
                            FlowState::Returned
                        } else { FlowState::LoopTransfer });
                    }
                    // 僅合流分支進入前可見的真正名稱。分支私有宣告不得洩漏。
                    for (depth, scope) in vars.iter_mut().enumerate() {
                        for (name, original) in scope.iter_mut() {
                            // 真正來源以原始宣告位元組起點及詞彙深度識別。
                            let left = if live_true {
                                let same = on_true[depth].get(name)
                                    .expect("原來源必須存在");
                                if same.declaration_start != original.declaration_start {
                                    return Err(self.error("分支來源身分不一致", condition));
                                }
                                Some(same.value)
                            } else { None };
                            let right = if live_false {
                                let same = on_false[depth].get(name)
                                    .expect("原來源必須存在");
                                if same.declaration_start != original.declaration_start {
                                    return Err(self.error("分支來源身分不一致", condition));
                                }
                                Some(same.value)
                            } else { None };
                            original.value = ValueSlot::join_reachable(&[left, right])
                                .map_err(|err| self.model_error(err, condition))?;
                            // 只有各可達分支的缺失欄位集合完全相同，才能
                            // 保留首次寫入許可；否則失去部分初始化證明。
                            let pending_true = if live_true {
                                Some(&on_true[depth][name].pending_fields)
                            } else { None };
                            let pending_false = if live_false {
                                Some(&on_false[depth][name].pending_fields)
                            } else { None };
                            original.pending_fields = match (pending_true, pending_false) {
                                (Some(left), Some(right)) if left == right => left.clone(),
                                // 兩個可達分支皆持有尚未完整的相同結構，
                                // 但完成欄位的順序不同時，只有「可能已初始化」
                                // 的差異欄位全為 Copyable，才可安全合併缺失聯集：
                                // 後續寫入若在某路徑是覆寫，也不會遺失資源責任。
                                (Some(Some(left)), Some(Some(right)))
                                    if original.value.state() == ValueState::Uninitialized
                                        && left.symmetric_difference(right).all(|field_name| {
                                            self.structs.get(&original.ty)
                                                .and_then(|definition| definition.fields.iter()
                                                    .find(|field| field.name == *field_name))
                                                .is_some_and(|field|
                                                    self.permission(&field.ty) == CopyPermission::Allowed)
                                        }) =>
                                    Some(left.union(right).cloned().collect()),
                                (Some(one), None) | (None, Some(one)) => one.clone(),
                                _ => None,
                            };
                            if let Some(loan) = original.loan.as_mut() {
                                // 僅合流實際繼續執行的分支；若任一可達
                                // 路徑仍有借用，合流後必須維持來源封鎖。
                                let true_active = live_true && on_true[depth][name].loan
                                    .as_ref().is_some_and(|item| item.active);
                                let false_active = live_false && on_false[depth][name].loan
                                    .as_ref().is_some_and(|item| item.active);
                                loan.active = true_active || false_active;
                            }
                        }
                    }
                }
            }
            // 本區塊剩餘路徑已不再引用的非擁有借用可即時歸還；
            // 不得因來源尚存在就把借用自動延長到作用域結束。
            Self::release_last_uses(vars, &body[position + 1..], continuation);
        }
        Ok(FlowState::Fallthrough)
    }

    fn function(&self, fun: &Function) -> Result<(), Diagnostic> {
        if !matches!(fun.body.last().map(|s| &s.kind), Some(StmtKind::Return(_))) {
            return Err(Diagnostic { kind: DiagnosticKind::Syntax,
                message: "函式最外層最後必須為明確 return".into(),
                span: Some(self.source.span(
                    fun.body.last().map_or(0, |s| s.start),
                    fun.body.last().map_or(0, |s| s.end))) });
        }
        let mut vars = vec![HashMap::new()];
        for parameter in &fun.params {
            let name = &parameter.name;
            let ty = &parameter.ty;
            if !["int", "bool", "float", "str"].contains(&ty.as_str())
                && !self.structs.contains_key(ty) {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "P13：函式參數使用不存在的型別".into(), span: None });
            }
            vars[0].insert(name.clone(), Binding {
                declaration_start: parameter.declaration_start,
                ty: ty.clone(), mutable: false,
                value: ValueSlot::initialized(self.permission(ty)),
                pending_fields: None,
                loan: (self.permission(ty) != CopyPermission::Allowed
                    && !Self::parameter_is_consumed(fun, name)).then_some(LoanBinding {
                        // 只在被呼叫端本地使用代理宣告身份，真正來源
                        // 的借用票據由呼叫端持有至呼叫結束。
                        source: parameter.declaration_start,
                        kind: BorrowKind::SharedRead, active: true,
                    }),
            });
        }
        self.statements(&fun.body, &mut vars, &fun.result_ty, false)?;
        Ok(())
    }
    /// 對方法內所有深度的運算式查核明確禁令，不只檢查頂層 return。
    /// 方法借用完整合法性仍待 P14，不能藉此宣告方法可使用。
    fn reject_method_term(&self, term: &Term) -> Result<(), Diagnostic> {
        match &term.kind {
            TermKind::Move(name) if name == "self" => {
                let _ = struct_ownership::check_method_self(SelfAction::ConsumeWhole);
                Err(self.error("S15：方法禁止消耗整個 self", term))
            }
            TermKind::FieldMove(_, _) => {
                let _ = struct_ownership::check_struct_move_target(MoveTarget::SingleField);
                Err(self.error("S21：方法不可個別移動結構欄位", term))
            }
            TermKind::Construct(_, args) => {
                for (_, value) in args { self.reject_method_term(value)?; }
                Ok(())
            }
            TermKind::Call(_, args) => {
                // 函式呼叫的合法型別、Copyable 資格與完整參數來源
                // 由 method_pure_value_type 統一查核；仍先檢查 S15／S21。
                for value in args { self.reject_method_term(value)?; }
                Ok(())
            }
            TermKind::BinaryInt { left, right, .. } => {
                self.reject_method_term(left)?;
                self.reject_method_term(right)
            }
            _ => Ok(()),
        }
    }

    fn reject_method_statements(&self, body: &[Stmt]) -> Result<(), Diagnostic> {
        for stmt in body {
            match &stmt.kind {
                StmtKind::Declare { init, .. } => {
                    if let Some(value) = init { self.reject_method_term(value)?; }
                }
                StmtKind::Assign { value, .. } | StmtKind::Return(value)
                    | StmtKind::FieldAssign { value, .. } => {
                    self.reject_method_term(value)?;
                }
                StmtKind::If { condition, yes, no } => {
                    self.reject_method_term(condition)?;
                    self.reject_method_statements(yes)?;
                    if let Some(otherwise) = no {
                        self.reject_method_statements(otherwise)?;
                    }
                }
                StmtKind::While { condition, .. } => {
                    return Err(self.error("P15：方法中的 while 借用重入尚未完成安全證明", condition));
                }
                StmtKind::ForRange { .. } => {
                    return Err(Diagnostic { kind: DiagnosticKind::Static,
                        message: "P15：方法中的 for 借用重入尚未完成安全證明".into(),
                        span: Some(self.source.span(stmt.start, stmt.end)) });
                }
                StmtKind::Break | StmtKind::Continue => {
                    return Err(Diagnostic { kind: DiagnosticKind::Static,
                        message: "P15：方法內跳出／繼續迭代的 self 有效期尚未驗證".into(),
                        span: Some(self.source.span(stmt.start, stmt.end)) });
                }
            }
        }
        Ok(())
    }

    /// 檢查結構欄位預設建立式是否形成循環。
    /// 未有資源執行期之前，不能把互相建立的預設值誤視為完成初始化。
    fn check_default_constructor_cycles(&self) -> Result<(), Diagnostic> {
        let mut active = HashSet::new();
        let mut checked = HashSet::new();
        for name in self.structs.keys() {
            self.check_default_type(name, &mut active, &mut checked)?;
        }
        Ok(())
    }

    fn check_default_type(&self, name: &str, active: &mut HashSet<String>,
        checked: &mut HashSet<String>) -> Result<(), Diagnostic> {
        if checked.contains(name) { return Ok(()); }
        if !active.insert(name.to_string()) {
            return Err(Diagnostic {
                kind: DiagnosticKind::Static,
                message: "結構預設建構形成循環，無法證明安全初始化".into(),
                span: None,
            });
        }
        if let Some(definition) = self.structs.get(name) {
            for field in &definition.fields {
                if let Some(default) = &field.default {
                    self.check_default_term(default, active, checked)?;
                }
            }
        }
        active.remove(name);
        checked.insert(name.to_string());
        Ok(())
    }

    fn check_default_term(&self, term: &Term, active: &mut HashSet<String>,
        checked: &mut HashSet<String>) -> Result<(), Diagnostic> {
        match &term.kind {
            TermKind::Construct(name, args) => {
                if active.contains(name) {
                    return Err(self.error("結構欄位預設建構形成循環，無法安全完成初始化", term));
                }
                // 明確指定的欄位會覆蓋其預設式；不能把已覆蓋的
                // 預設建構誤判為實際執行的循環。保留現行保守規則：
                // 先檢查型別既有預設依賴，再遍歷具名引數。
                self.check_default_type(name, active, checked)?;
                // 具名欄位實際求值時，也不得在巢狀建立引數中形成循環。
                for (_, value) in args {
                    self.check_default_term(value, active, checked)?;
                }
            }
            TermKind::Call(_, args) => {
                // 呼叫引數內的結構建立式亦屬預設值的巢狀來源，
                // 不得繞過循環建構的靜態檢查。
                for value in args {
                    self.check_default_term(value, active, checked)?;
                }
            }
            TermKind::BinaryInt { left, right, .. } => {
                self.check_default_term(left, active, checked)?;
                self.check_default_term(right, active, checked)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn all(&self, funcs: &[Function]) -> Result<(), Diagnostic> {
        // 一般呼叫以名稱查找簽章；若允許重名，呼叫端可能
        // 檢查第一份簽章而執行其他定義，破壞型別／所有權安全。
        let mut seen = HashSet::new();
        for function in funcs {
            if !seen.insert(function.name.as_str()) {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "P13：函式名稱重複，呼叫簽章不唯一".into(), span: None });
            }
            if function.name == "main" && !function.params.is_empty() {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "P13：main 入口不可宣告參數".into(), span: None });
            }
        }
        // 所有函式與方法簽章須先封閉型別名稱，不能只檢查參數或
        // 等待 return 運算式碰巧揭露不存在的回傳型別。
        for function in funcs {
            if !["int", "bool", "float", "str"].contains(&function.result_ty.as_str())
                && !self.structs.contains_key(&function.result_ty) {
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "P13：函式回傳型別不存在".into(), span: None });
            }
        }
        for definition in self.structs.values() {
            for method in &definition.methods {
                if !["int", "bool", "float", "str"].contains(&method.result_ty.as_str())
                    && !self.structs.contains_key(&method.result_ty) {
                    return Err(Diagnostic { kind: DiagnosticKind::Static,
                        message: "P13：方法回傳型別不存在".into(), span: None });
                }
            }
        }
        self.check_default_constructor_cycles()?;
        for (struct_name, definition) in self.structs.iter() {
            for field in &definition.fields {
                if !["int", "bool", "float", "str"].contains(&field.ty.as_str())
                    && !self.structs.contains_key(&field.ty) {
                    return Err(Diagnostic {
                        kind: DiagnosticKind::Static,
                        message: "結構欄位使用不存在的型別".into(),
                        span: None,
                    });
                }
                if let Some(default) = &field.default {
                    let (ty, initialized) =
                        self.evaluate(default, &mut vec![HashMap::new()])?;
                    if ty != field.ty {
                        return Err(self.error("結構預設欄位型別不一致", default));
                    }
                    // S07 容許 var 欄位延後初始化，但不能把「存在預設運算式」
                    // 當成嵌入的結構值已全部初始化。受限模型尚無欄位級
                    // 初始化證明時，必須拒絕不完整的巢狀預設建立式。
                    if !initialized {
                        return Err(self.error(
                            "S07：結構預設值尚含未初始化欄位，不能證明完整初始化",
                            default));
                    }
                }
                // immutable 僅保存 AST，不在此決定後續 S18 可變借用。
                let _ = field.immutable;
            }
            for method in &definition.methods {
                self.check_method(struct_name, method)?;
            }
        }
        if !funcs.iter().any(|fun| fun.name == "main" && fun.result_ty == "int") {
            return Err(Diagnostic { kind: DiagnosticKind::Static,
                message: "受限驗證需要 fn main()->int".into(), span: None });
        }
        for fun in funcs { self.function(fun)?; }
        Ok(())
    }
}

/// 成功僅代表完整檢查了這個明確受限的 P13 子集，不代表 P13 Phase 已通過。
/// P16 私有測試橋接：先完成真正 .voil 的來源／型別／所有權檢查，
/// 再對可靜態確定的單一結構建立式建立真實 UTF-8 擁有者資源追蹤。
/// 不推定主程式控制流程、一般運算式、轉義解碼或 P17 直譯執行。
#[cfg(test)]
fn trace_string_struct_source(
    text: &str, variable: &str, fail_after: Option<&str>,
) -> Result<crate::struct_initialization::BuildTrace, Diagnostic> {
    use crate::struct_initialization::{build_string_struct, NamedString, StringField};
    let unsupported = |message: &str| Diagnostic {
        kind: DiagnosticKind::Static, message: message.into(), span: None,
    };
    fn plain_literal<'a>(text: &'a str, term: &Term) -> Option<&'a str> {
        if !matches!(term.kind, TermKind::Literal("str")) { return None; }
        let raw = text.get(term.start..term.end)?;
        let content = raw.strip_prefix('"')?.strip_suffix('"')?;
        // 此處沒有完整字串轉義求值器：不能把原始碼中的
        // 反斜線轉義當成實際 UTF-8 緩衝區內容。
        if content.contains('\\') || content.contains('\n') || content.contains('\r') {
            return None;
        }
        Some(content)
    }
    let source = Source::new(text.as_bytes().to_vec());
    if let Some(error) = source.lexical_diagnostics().into_iter().next() {
        return Err(error);
    }
    let tokens = lexer::scan(&source);
    if let Some(error) = tokens.diagnostics.into_iter().next() {
        return Err(error);
    }
    let (structs, funcs) = Parser {
        source: &source, tokens: &tokens.tokens, cursor: 0,
    }.file()?;
    Checker {
        source: &source, structs: &structs, functions: &funcs,
        ability: crate::ownership_policy::approved_leaf_copy_permission,
    }.all(&funcs)?;
    let main = funcs.iter().find(|fun| fun.name == "main")
        .ok_or_else(|| unsupported("P16：來源不存在 main"))?;
    // 只接通單一確定執行的建立式，避免把不可達分支或
    // 多次執行的程式偷當成已支援的執行期控制流程。
    let [declaration, trailing_return] = main.body.as_slice() else {
        return Err(unsupported("P16：來源追蹤只接受單次宣告接直接返回"));
    };
    if !matches!(&trailing_return.kind, StmtKind::Return(Term {
        kind: TermKind::Literal("int"), start, end,
    }) if text.get(*start..*end) == Some("0")) {
        return Err(unsupported("P16：返回及控制流程尚未接通資源追蹤"));
    }
    let (ty, args) = match &declaration.kind {
        StmtKind::Declare {
            name, ty, init: Some(Term {kind: TermKind::Construct(owner, args), ..}), ..
        } if name == variable && ty == owner => (ty.as_str(), args),
        _ => return Err(unsupported("P16：不存在單次具名字串結構建立式")),
    };
    let definition = structs.get(ty)
        .ok_or_else(|| unsupported("P16：結構型別不存在"))?;
    let mut fields = Vec::new();
    for field in &definition.fields {
        if field.ty != "str" {
            return Err(unsupported("P16：資源橋接只證明 str 欄位"));
        }
        let default = match &field.default {
            Some(term) => Some(plain_literal(text, term)
                .ok_or_else(|| unsupported("P16：僅接受未經轉義的字串預設常值"))?),
            None => None,
        };
        fields.push(StringField {
            name: &field.name, default, immutable: field.immutable,
        });
    }
    let mut named = Vec::new();
    for (field, term) in args {
        named.push(NamedString {
            field, value: plain_literal(text, term)
                .ok_or_else(|| unsupported("P16：僅接受未經轉義的具名字串常值"))?,
        });
    }
    // 模擬失敗只能放在此次真正會求值的具名或未覆蓋預設步驟。
    // 沒有預設、也未被具名提供的 var 欄位雖存在於結構宣告中，
    // 卻不會建立資源，不可把「指定失敗卻未發生」算成清理證據。
    if let Some(name) = fail_after {
        let in_named = named.iter().any(|arg| arg.field == name);
        let in_defaults = fields.iter().any(|field|
            field.name == name && field.default.is_some());
        if !in_named && !in_defaults {
            return Err(unsupported(
                "P16：失敗注入欄位不存在，或未被此結構求值計畫執行"));
        }
    }
    Ok(build_string_struct(&fields, &named, fail_after))
}

/// P16：真正 .voil 來源經完整靜態檢查後，才交給
/// 持續存活的 UTF-8 結構執行期模型。僅驗證「單次建構、
/// 整筆 .move()、直接 return 0」可證明的受限直線來源。
#[cfg(test)]
fn trace_persistent_struct_move_source(
    text: &str, fail_after: Option<&str>,
) -> Result<(Option<String>, Vec<crate::owned_str_runtime::ResourceEvent>), Diagnostic> {
    use crate::struct_initialization::{
        BuildStatus, NamedString, OwnedStructRuntime, StringField, StructRuntimeError,
    };
    let reject = |message: &str| Diagnostic {
        kind: DiagnosticKind::Static, message: message.into(), span: None,
    };
    fn plain_literal<'a>(source: &'a str, value: &Term) -> Option<&'a str> {
        if !matches!(value.kind, TermKind::Literal("str")) { return None; }
        let value=source.get(value.start..value.end)?
            .strip_prefix('"')?.strip_suffix('"')?;
        (!value.contains('\\') && !value.contains('\n')
            && !value.contains('\r')).then_some(value)
    }
    let source=Source::new(text.as_bytes().to_vec());
    if let Some(error)=source.lexical_diagnostics().into_iter().next() {
        return Err(error);
    }
    let tokens=lexer::scan(&source);
    if let Some(error)=tokens.diagnostics.into_iter().next() {
        return Err(error);
    }
    let (structs,functions)=Parser {
        source:&source, tokens:&tokens.tokens, cursor:0,
    }.file()?;
    Checker {
        source:&source, structs:&structs, functions:&functions,
        ability:crate::ownership_policy::approved_leaf_copy_permission,
    }.all(&functions)?;
    let main=functions.iter().find(|function|function.name=="main")
        .ok_or_else(||reject("P16：字串結構來源須有合法 main"))?;
    let [declare, transfer, last]=main.body.as_slice() else {
        return Err(reject("P16：整筆結構來源只接受建構、移動、返回三語句"));
    };
    if !matches!(&last.kind, StmtKind::Return(Term {
        kind: TermKind::Literal("int"), start, end,
    }) if text.get(*start..*end)==Some("0")) {
        return Err(reject("P16：非直接 return 0 的控制流程尚無資源執行證明"));
    }
    let (owner, struct_name, named_args)=match &declare.kind {
        StmtKind::Declare {
            name, ty, init:Some(Term {kind:TermKind::Construct(actual,args), ..}),
            ..
        } if ty==actual => (name.as_str(),ty.as_str(),args),
        _=>return Err(reject("P16：第一語句須建立具名 str 欄位結構")),
    };
    let target=match &transfer.kind {
        StmtKind::Declare {
            name, ty, init:Some(Term {kind:TermKind::Move(source),..}), ..
        } if ty==struct_name && source==owner => name.as_str(),
        _=>return Err(reject("P16：第二語句須將剛建立的整個結構 .move() 至新持有者")),
    };
    let definition=structs.get(struct_name)
        .ok_or_else(||reject("P16：結構宣告型別不存在"))?;
    let fields=definition.fields.iter().map(|field| {
        if field.ty!="str" {
            return Err(reject("P16：此資源橋接只支援 str 欄位"));
        }
        let default=field.default.as_ref().map(|value|
            plain_literal(text,value).ok_or_else(||
                reject("P16：此資源橋接不支援帶轉義的字串預設值")))
            .transpose()?;
        Ok(StringField {
            name:field.name.as_str(),default,immutable:field.immutable,
        })
    }).collect::<Result<Vec<_>,Diagnostic>>()?;
    let named=named_args.iter().map(|(name,value)|
        plain_literal(text,value).map(|literal|NamedString {
            field:name.as_str(),value:literal,
        }).ok_or_else(||reject("P16：此資源橋接只支援無轉義具名字串常值"))
    ).collect::<Result<Vec<_>,Diagnostic>>()?;
    let mut runtime=OwnedStructRuntime::new();
    let status=runtime.construct(owner,&fields,&named,fail_after);
    if matches!(&status, Err(StructRuntimeError::EvaluationFailed(_))) {
        // 真實失敗注入下不得建立第二持有者，已建立的部分
        // 資源必須在回滾時依序釋放並完成事件帳簿平衡。
        return Ok((None,runtime.finish()
            .map_err(|_|reject("P16：失敗回滾後資源身分未完整清理"))?));
    }
    match status {
        Ok(BuildStatus::Complete)=>{},
        Ok(BuildStatus::PartiallyInitialized(_)) =>
            return Err(reject("P16：尚未全欄位初始化的結構不能整筆移動")),
        Ok(_) | Err(_) =>
            return Err(reject("P16：整體資源建立失敗或注入切點不在求值計畫中")),
    }
    runtime.move_whole(owner,target)
        .map_err(|_|reject("P16：整個結構未能安全轉交單一持有者"))?;
    let observed=fields.last().map(|field|field.name).ok_or_else(||
        reject("P16：此來源橋接須有可觀察的 str 欄位"))?;
    let retained=runtime.read_field(target,observed)
        .map_err(|_|reject("P16：移動後新持有者欄位不再有效"))?.to_owned();
    let events=runtime.finish()
        .map_err(|_|reject("P16：整筆移動後未完整清理全部資源身分"))?;
    Ok((Some(retained),events))
}

/// P16 私有字串函式返回證據：只模擬已通過來源檢查且
/// 呼叫端／被呼叫端都屬唯一可證明直線路徑的資源交接。
/// 這不是一般函式直譯器，不能拿此入口運行任意控制流程。
#[cfg(test)]
fn trace_string_return_source(
    text: &str, provider: &str, receiver: &str,
) -> Result<(String, Vec<crate::owned_str_runtime::ResourceEvent>), Diagnostic> {
    use crate::owned_str_runtime::OwnedStrRuntime;
    let unsupported = |message: &str| Diagnostic {
        kind: DiagnosticKind::Static, message: message.into(), span: None,
    };
    fn plain_literal<'a>(text: &'a str, term: &Term) -> Option<&'a str> {
        if !matches!(term.kind, TermKind::Literal("str")) { return None; }
        let bytes = text.get(term.start..term.end)?;
        let raw = bytes.strip_prefix('"')?.strip_suffix('"')?;
        if raw.contains('\\') || raw.contains('\n') || raw.contains('\r') {
            return None;
        }
        Some(raw)
    }
    let source = Source::new(text.as_bytes().to_vec());
    if let Some(error) = source.lexical_diagnostics().into_iter().next() {
        return Err(error);
    }
    let tokens = lexer::scan(&source);
    if let Some(error) = tokens.diagnostics.into_iter().next() {
        return Err(error);
    }
    let (structs, funcs) = Parser {
        source: &source, tokens: &tokens.tokens, cursor: 0,
    }.file()?;
    Checker {
        source: &source, structs: &structs, functions: &funcs,
        ability: crate::ownership_policy::approved_leaf_copy_permission,
    }.all(&funcs)?;
    let main = funcs.iter().find(|fun| fun.name == "main")
        .ok_or_else(|| unsupported("P16：缺少 main"))?;
    let called = funcs.iter().find(|fun| fun.name == provider
        && fun.params.is_empty() && fun.result_ty == "str")
        .ok_or_else(|| unsupported("P16：須為無參數且回傳 str 的函式"))?;
    let [declaration, final_return] = main.body.as_slice() else {
        return Err(unsupported("P16：呼叫端僅接受單次資源接收及直接返回"));
    };
    let valid_receiver = matches!(&declaration.kind, StmtKind::Declare {
        name, ty, mutable: true,
        init: Some(Term {kind: TermKind::Construct(name_called, args), ..}),
    } if name == receiver && ty == "str" && name_called == provider && args.is_empty());
    if !valid_receiver {
        return Err(unsupported("P16：呼叫端須以 var str 接收單次函式返回"));
    }
    if !matches!(&final_return.kind, StmtKind::Return(Term {
        kind: TermKind::Literal("int"), start, end,
    }) if text.get(*start..*end) == Some("0")) {
        return Err(unsupported("P16：呼叫端其他返回路徑尚未接通執行期"));
    }
    let content = match called.body.as_slice() {
        [Stmt {kind: StmtKind::Return(term), ..}] =>
            plain_literal(text, term),
        [Stmt {kind: StmtKind::Declare {
            name, ty, init: Some(value), ..
        }, ..}, Stmt {kind: StmtKind::Return(Term {
            kind: TermKind::Move(returned_name), ..
        }), ..}] if ty == "str" && name == returned_name =>
            plain_literal(text, value),
        _ => None,
    }.ok_or_else(|| unsupported(
        "P16：被呼叫端僅接受獨立 str 常值或具名 str 的整筆移動返回"))?;
    // 以 Rust 真正 String 的單一持有者驗證作用域交接。
    // 呼叫端根作用域尚無同名來源，接收必須原子成功。
    let mut runtime = OwnedStrRuntime::new();
    runtime.enter_scope();
    runtime.declare("__p16_result", true, content)
        .expect("合法來源應可建立唯一 str 緩衝區");
    let returned = runtime.return_named("__p16_result")
        .expect("剛建立且仍有效的字串可返回");
    runtime.leave_scope()
        .expect("被呼叫端作用域必須能安全退出");
    runtime.accept_return(receiver, returned)
        .expect("呼叫端作用域預先為空且未耗盡資源身分");
    let retained = runtime.read(receiver)
        .expect("返回資源必須仍由呼叫端唯一持有").to_owned();
    let events = runtime.finish()
        .expect("呼叫端退出後每個資源必須恰好釋放一次");
    Ok((retained, events))
}

/// VA16-03-A：真正 .voil 受限「建構→var str 欄位賦值」
/// 由同一靜態檢查過的 AST 交由 UTF-8 持續資源模型執行。
/// 其他操作及未核准右值不能被資源橋接假裝成已執行。
#[cfg(test)]
fn trace_persistent_struct_field_write_source(
    text: &str, fail_after_rhs: bool,
) -> Result<(String, Vec<crate::owned_str_runtime::ResourceEvent>), Diagnostic> {
    use crate::struct_initialization::{BuildStatus, NamedString,
        OwnedStructRuntime, StringField, StructRuntimeError};
    let reject=|message:&str| Diagnostic {
        kind:DiagnosticKind::Static,message:message.into(),span:None,
    };
    fn unescaped_literal<'a>(source:&'a str, term:&Term)->Option<&'a str> {
        if !matches!(&term.kind,TermKind::Literal("str")) { return None; }
        let raw=source.get(term.start..term.end)?
            .strip_prefix('"')?.strip_suffix('"')?;
        (!raw.contains('\\')&&!raw.contains('\n')
            &&!raw.contains('\r')).then_some(raw)
    }
    let source=Source::new(text.as_bytes().to_vec());
    if let Some(error)=source.lexical_diagnostics().into_iter().next() {
        return Err(error);
    }
    let tokens=lexer::scan(&source);
    if let Some(error)=tokens.diagnostics.into_iter().next() {
        return Err(error);
    }
    let (structs,functions)=Parser {
        source:&source,tokens:&tokens.tokens,cursor:0,
    }.file()?;
    Checker {
        source:&source,structs:&structs,functions:&functions,
        ability:crate::ownership_policy::approved_leaf_copy_permission,
    }.all(&functions)?;
    let main=functions.iter().find(|function|function.name=="main")
        .ok_or_else(||reject("P16：結構欄位來源缺少 main"))?;
    let [declaration,assignment,last]=main.body.as_slice() else {
        return Err(reject("P16：僅支援一次結構建構、欄位賦值與直接返回"));
    };
    if !matches!(&last.kind,StmtKind::Return(Term {
        kind:TermKind::Literal("int"),start,end,
    }) if text.get(*start..*end)==Some("0")) {
        return Err(reject("P16：欄位資源清理僅證明直接 return 0"));
    }
    let (owner,ty,mutable,args)=match &declaration.kind {
        StmtKind::Declare {
            name,ty,mutable,init:Some(Term {
                kind:TermKind::Construct(ctor,args),..
            }),
        } if ty==ctor =>(name.as_str(),ty.as_str(),*mutable,args),
        _=>return Err(reject("P16：來源須先宣告具名字串結構")),
    };
    let (field,new_value)=match &assignment.kind {
        StmtKind::FieldAssign {receiver,field,value} if receiver==owner=>
            (field.as_str(),unescaped_literal(text,value)
                .ok_or_else(||reject("P16：欄位覆寫只支援未轉義的獨立 str 常值"))?),
        _=>return Err(reject("P16：來源須有針對同一結構的欄位賦值")),
    };
    let definition=structs.get(ty)
        .ok_or_else(||reject("P16：結構型別不存在"))?;
    let fields=definition.fields.iter().map(|member| {
        if member.ty!="str" {
            return Err(reject("P16：此私有執行期僅接通 str 欄位"));
        }
        let default=member.default.as_ref().map(|term|
            unescaped_literal(text,term).ok_or_else(||
                reject("P16：尚無被核准的字串轉義求值器")))
            .transpose()?;
        Ok(StringField {
            name:member.name.as_str(),default,immutable:member.immutable,
        })
    }).collect::<Result<Vec<_>,Diagnostic>>()?;
    let named=args.iter().map(|(field,value)|
        unescaped_literal(text,value).map(|text_value|NamedString {
            field:field.as_str(),value:text_value,
        }).ok_or_else(||reject("P16：具名引數只支援未轉義 str 常值"))
    ).collect::<Result<Vec<_>,Diagnostic>>()?;
    let mut rt=OwnedStructRuntime::new();
    let ready=rt.construct_with_access(owner,mutable,&fields,&named,None)
        .map_err(|_|reject("P16：真實來源結構無法安全完成建立"))?;
    if !matches!(ready,BuildStatus::Complete|BuildStatus::PartiallyInitialized(_)) {
        return Err(reject("P16：非可執行的結構狀態"));
    }
    let actual=rt.assign_field_checked(owner,field,new_value,fail_after_rhs);
    if fail_after_rhs {
        if !matches!(actual,Err(StructRuntimeError::EvaluationFailed(_))) {
            return Err(reject("P16：右值失敗注入未由資源模型確實發生"));
        }
    } else {
        actual.map_err(|_|reject("P16：合法右值未能安全原子提交"))?;
    }
    let observed=rt.read_field(owner,field)
        .map_err(|_|reject("P16：原持有者欄位沒有可讀資源"))?
        .to_owned();
    let resources=rt.finish()
        .map_err(|_|reject("P16：結構欄位覆寫未完整平衡資源身分"))?;
    Ok((observed,resources))
}

/// P16.6 混合 int／bool／str 受限來源橋接。
/// 先完整通過現行靜態 Checker，才執行真實純值複製或
/// 含非 Copyable 欄位的整筆資源移動／失敗回滾。
#[cfg(test)]
fn trace_mixed_struct_source(text:&str,fail_after:Option<&str>)
    -> Result<(Option<i64>,Vec<crate::owned_str_runtime::ResourceEvent>),Diagnostic> {
    use crate::mixed_struct_runtime::{
        MixedStructRuntime,MixedFieldSpec,NamedMixed,SeedValue,ScalarKind,Observed,MixedError,
    };
    let reject=|message:&str| Diagnostic {
        kind:DiagnosticKind::Static,message:message.into(),span:None,
    };
    fn literal<'a>(text:&'a str,term:&Term,kind:ScalarKind)->Option<SeedValue<'a>> {
        let raw=text.get(term.start..term.end)?;
        match (kind,&term.kind) {
            (ScalarKind::Str,TermKind::Literal("str"))=>{
                let text=raw.strip_prefix('"')?.strip_suffix('"')?;
                (!text.contains('\\')&&!text.contains('\n')
                    &&!text.contains('\r')).then_some(SeedValue::Str(text))
            },
            (ScalarKind::Bool,TermKind::Literal("bool"))=>match raw {
                "true"=>Some(SeedValue::Bool(true)),
                "false"=>Some(SeedValue::Bool(false)),_=>None
            },
            (ScalarKind::Int,TermKind::Literal("int"))=>{
                if !raw.chars().all(|ch|ch.is_ascii_digit()||ch=='_') {
                    return None;
                }
                raw.replace('_',"").parse().ok().map(SeedValue::Int)
            },
            _=>None,
        }
    }
    let source=Source::new(text.as_bytes().to_vec());
    if let Some(error)=source.lexical_diagnostics().into_iter().next() {
        return Err(error);
    }
    let tokens=lexer::scan(&source);
    if let Some(error)=tokens.diagnostics.into_iter().next() {
        return Err(error);
    }
    let (structs,functions)=Parser {
        source:&source,tokens:&tokens.tokens,cursor:0,
    }.file()?;
    Checker {
        source:&source,structs:&structs,functions:&functions,
        ability:crate::ownership_policy::approved_leaf_copy_permission,
    }.all(&functions)?;
    let main=functions.iter().find(|fun|fun.name=="main")
        .ok_or_else(||reject("P16：缺少 main"))?;
    let [declare,transfer,end]=main.body.as_slice() else {
        return Err(reject("P16：混合結構執行路徑僅支援建構、整筆複製／移動、返回"));
    };
    if !matches!(&end.kind,StmtKind::Return(Term {
        kind:TermKind::Literal("int"),start,end,
    }) if text.get(*start..*end)==Some("0")) {
        return Err(reject("P16：尚未證明非直接返回的清理順序"));
    }
    let (owner,ty,args)=match &declare.kind {
        StmtKind::Declare {
            name,ty,init:Some(Term {kind:TermKind::Construct(ctor,args),..}),..
        } if ty==ctor=>(name.as_str(),ty.as_str(),args),
        _=>return Err(reject("P16：必須先建構混合結構")),
    };
    let (target,moved)=match &transfer.kind {
        StmtKind::Declare {
            name,ty:target_ty,init:Some(term),..
        } if target_ty==ty=>match &term.kind {
            TermKind::Move(source) if source==owner=>(name.as_str(),true),
            TermKind::Name(source) if source==owner=>(name.as_str(),false),
            _=>return Err(reject("P16：只支援整體結構來源的已核准複製或 .move()")),
        },
        _=>return Err(reject("P16：第二語句必須建立同型別結構持有者")),
    };
    let definition=structs.get(ty)
        .ok_or_else(||reject("P16：混合型別結構不存在"))?;
    let mut fields=Vec::new();
    for field in &definition.fields {
        let kind=match field.ty.as_str() {
            "int"=>ScalarKind::Int,"bool"=>ScalarKind::Bool,"str"=>ScalarKind::Str,
            _=>return Err(reject("P16：本次混合執行期尚未支援其他欄位型別")),
        };
        let default=field.default.as_ref().map(|term|
            literal(text,term,kind).ok_or_else(||
                reject("P16：尚未證明此混合欄位預設運算式可直接執行")))
            .transpose()?;
        fields.push(MixedFieldSpec {
            name:field.name.as_str(),kind,default,immutable:field.immutable,
        });
    }
    let mut named=Vec::new();
    for (name,term) in args {
        let field=fields.iter().find(|field|field.name==name)
            .ok_or_else(||reject("P16：混合欄位名稱未核准"))?;
        let value=literal(text,term,field.kind)
            .ok_or_else(||reject("P16：只接受可完整解碼的混合欄位獨立常值"))?;
        named.push(NamedMixed {field:name.as_str(),value});
    }
    let mut rt=MixedStructRuntime::new();
    match rt.construct(owner,&fields,&named,fail_after) {
        Ok(())=>{},
        Err(MixedError::EvaluationFailed(_))=>{
            return Ok((None,rt.finish().map_err(|_|
                reject("P16：混合欄位失敗未能逆序清理資源"))?));
        },
        Err(_)=>return Err(reject("P16：混合結構求值計畫無法安全完成")),
    }
    if moved {
        rt.move_whole(owner,target).map_err(|_|
            reject("P16：混合結構整筆移動未完成"))?;
    } else {
        rt.copy_whole(owner,target).map_err(|_|
            reject("P16：不可從非 Copyable 混合結構隱含複製"))?;
    }
    let number=match rt.field(target,"num") {
        Ok(Observed::Int(value))=>value,
        _=>return Err(reject("P16：本次混合來源須有可觀察的 int num 欄位")),
    };
    let events=rt.finish().map_err(|_|
        reject("P16：混合結構退出後資源身分未平衡"))?;
    Ok((Some(number),events))
}

/// P16.3：真實 .voil 的已知 bool 分支與內外詞彙作用域。
/// 只追蹤完整靜態核准的單層「外層結構；if 的內層結構；
/// 直接或提前 return 0」。失敗點只會出現在實際走到的分支。
#[cfg(test)]
fn trace_scope_struct_source(
    text:&str,fail_inner:Option<&str>,
)->Result<Vec<crate::owned_str_runtime::ResourceEvent>,Diagnostic> {
    use crate::struct_initialization::{BuildStatus,NamedString,
        OwnedStructRuntime,StringField,StructRuntimeError};
    let reject=|message:&str|Diagnostic{
        kind:DiagnosticKind::Static,message:message.into(),span:None,
    };
    fn plain<'a>(source:&'a str,term:&Term)->Option<&'a str> {
        if !matches!(&term.kind,TermKind::Literal("str")) {return None;}
        let raw=source.get(term.start..term.end)?
            .strip_prefix('"')?.strip_suffix('"')?;
        (!raw.contains('\\')&&!raw.contains('\n')
            &&!raw.contains('\r')).then_some(raw)
    }
    let source=Source::new(text.as_bytes().to_vec());
    if let Some(err)=source.lexical_diagnostics().into_iter().next() {
        return Err(err);
    }
    let tokens=lexer::scan(&source);
    if let Some(err)=tokens.diagnostics.into_iter().next() {return Err(err);}
    let (structs,functions)=Parser{
        source:&source,tokens:&tokens.tokens,cursor:0,
    }.file()?;
    Checker {
        source:&source,structs:&structs,functions:&functions,
        ability:crate::ownership_policy::approved_leaf_copy_permission,
    }.all(&functions)?;
    let main=functions.iter().find(|fun|fun.name=="main")
        .ok_or_else(||reject("P16：區塊清理來源缺少 main"))?;
    let [outside,branch,final_return]=main.body.as_slice() else {
        return Err(reject("P16：區塊橋接僅接受外層宣告、if、返回"));
    };
    let returns_zero=|term:&Term|matches!(&term.kind,
        TermKind::Literal("int") if text.get(term.start..term.end)==Some("0"));
    if !matches!(&final_return.kind,StmtKind::Return(term) if returns_zero(term)) {
        return Err(reject("P16：外層必須以直接 return 0 結束"));
    }
    let (cond,yes,no)=match &branch.kind {
        StmtKind::If{condition,yes,no}=>(condition,yes,no),
        _=>return Err(reject("P16：須有一個可靜態確定的 if 分支")),
    };
    let condition=if matches!(&cond.kind,TermKind::Literal("bool")) {
        match text.get(cond.start..cond.end) {
            Some("true")=>true,Some("false")=>false,
            _=>return Err(reject("P16：if bool 來源常值不明")),
        }
    } else {
        return Err(reject("P16：未知條件不可以模擬單一資源路徑"));
    };
    let executed=if condition {Some(yes.as_slice())}
        else {no.as_ref().map(Vec::as_slice)};
    let construct=|runtime:&mut OwnedStructRuntime,
        stmt:&Stmt,fail_after:Option<&str>|
        ->Result<Result<(),StructRuntimeError>,Diagnostic> {
        let (name,ty,mutable,args)=match &stmt.kind {
            StmtKind::Declare{name,ty,mutable,
                init:Some(Term{kind:TermKind::Construct(ctor,args),..}),
            } if ty==ctor=>(name.as_str(),ty.as_str(),*mutable,args),
            _=>return Err(reject("P16：區塊橋接僅執行明確結構建立")),
        };
        let def=structs.get(ty)
            .ok_or_else(||reject("P16：區塊結構型別未宣告"))?;
        let mut fields=Vec::new();
        for member in &def.fields {
            if member.ty!="str" {
                return Err(reject("P16：此路徑僅支援真實 str 資源欄位"));
            }
            let default=member.default.as_ref().map(|term|
                plain(text,term).ok_or_else(||
                    reject("P16：區塊橋接沒有字串轉義求值器")))
                .transpose()?;
            fields.push(StringField{
                name:member.name.as_str(),default,immutable:member.immutable,
            });
        }
        let named=args.iter().map(|(field,term)|
            plain(text,term).map(|value|NamedString{
                field:field.as_str(),value,
            }).ok_or_else(||reject("P16：區塊具名來源限未轉義 str 常值"))
        ).collect::<Result<Vec<_>,Diagnostic>>()?;
        Ok(runtime.construct_with_access(name,mutable,&fields,&named,fail_after)
            .and_then(|status|match status {
                BuildStatus::Complete=>Ok(()),
                _=>Err(StructRuntimeError::IncompleteStruct),
            }))
    };
    let mut rt=OwnedStructRuntime::new();
    construct(&mut rt,outside,None)?
        .map_err(|_|reject("P16：外層結構初始化失敗"))?;
    if let Some(body)=executed {
        let [inner]=body else {
            // 可選末尾提前 return 0；此情況在分支清理後
            // 須直接退出 main，而不得再次執行已不可達語句。
            if let [inner,Stmt{kind:StmtKind::Return(term),..}]=body {
                if !returns_zero(term) {
                    return Err(reject("P16：不支援其他分支返回運算式"));
                }
                rt.enter_scope();
                match construct(&mut rt,inner,fail_inner)? {
                    Ok(())=>rt.leave_scope().map_err(|_|
                        reject("P16：提前 return 前區塊清理失敗"))?,
                    Err(StructRuntimeError::EvaluationFailed(_))=>
                        rt.leave_scope().map_err(|_|
                            reject("P16：分支失敗回滾區塊清理失敗"))?,
                    Err(_)=>return Err(reject("P16：區塊建構失敗")),
                }
                return rt.finish().map_err(|_|
                    reject("P16：提前 return 外層資源未反序清理"));
            }
            return Err(reject("P16：分支不能省略未驗證的控制流程"));
        };
        rt.enter_scope();
        match construct(&mut rt,inner,fail_inner)? {
            Ok(())=>{},
            Err(StructRuntimeError::EvaluationFailed(_))=>{},
            Err(_)=>return Err(reject("P16：分支建立錯誤")),
        }
        rt.leave_scope().map_err(|_|
            reject("P16：區塊作用域離開清理失敗"))?;
    } else if fail_inner.is_some() {
        return Err(reject("P16：未執行的 else 分支不能被當成失敗注入"));
    }
    rt.finish().map_err(|_|reject("P16：外層退出後資源帳簿未平衡"))
}

/// P16：真實 .voil 結構函式返回的私有所有權證據。
/// 限無參數 provider 單次建構與具名整筆 .move() 返回，
/// main 以唯一全新結構綁定接收，避免虛構任意函式執行期。
#[cfg(test)]
fn trace_persistent_struct_return_source(
    text: &str, provider: &str, receiver: &str, fail_after: Option<&str>,
) -> Result<(Option<String>, Vec<crate::owned_str_runtime::ResourceEvent>), Diagnostic> {
    use crate::struct_initialization::{
        BuildStatus, NamedString, OwnedStructRuntime, StringField, StructRuntimeError,
    };
    let unsupported = |message: &str| Diagnostic {
        kind: DiagnosticKind::Static, message: message.into(), span: None,
    };
    fn plain_literal<'a>(text: &'a str, term: &Term) -> Option<&'a str> {
        if !matches!(&term.kind, TermKind::Literal("str")) { return None; }
        let raw=text.get(term.start..term.end)?
            .strip_prefix('"')?.strip_suffix('"')?;
        (!raw.contains('\\') && !raw.contains('\n')
            && !raw.contains('\r')).then_some(raw)
    }
    let source=Source::new(text.as_bytes().to_vec());
    if let Some(error)=source.lexical_diagnostics().into_iter().next() {
        return Err(error);
    }
    let tokens=lexer::scan(&source);
    if let Some(error)=tokens.diagnostics.into_iter().next() {
        return Err(error);
    }
    let (structs,functions)=Parser {
        source:&source,tokens:&tokens.tokens,cursor:0,
    }.file()?;
    Checker {
        source:&source, structs:&structs, functions:&functions,
        ability:crate::ownership_policy::approved_leaf_copy_permission,
    }.all(&functions)?;
    let main=functions.iter().find(|function| function.name=="main")
        .ok_or_else(||unsupported("P16：缺少有效 main"))?;
    let callee=functions.iter().find(|function| function.name==provider
        && function.params.is_empty())
        .ok_or_else(||unsupported("P16：須有無參數資源建構函式"))?;
    let [declaration,return_final]=callee.body.as_slice() else {
        return Err(unsupported("P16：被呼叫函式僅接受一次建立與一次整筆返回"));
    };
    let (local,ty,named_args)=match &declaration.kind {
        StmtKind::Declare {
            name,ty,init:Some(Term {kind:TermKind::Construct(actual,args),..}), ..
        } if ty==actual && ty==&callee.result_ty =>
            (name.as_str(),ty.as_str(),args),
        _=>return Err(unsupported("P16：被呼叫端須先建立具名結構持有者")),
    };
    if !matches!(&return_final.kind,StmtKind::Return(Term {
        kind:TermKind::Move(name),..
    }) if name==local) {
        return Err(unsupported("P16：被呼叫端必須以具名整筆 .move() 返回結構"));
    }
    let [receive,main_return]=main.body.as_slice() else {
        return Err(unsupported("P16：main 須單次接收結構後直接返回"));
    };
    if !matches!(&receive.kind,StmtKind::Declare {
        name,ty:destination_type,init:Some(Term {
            kind:TermKind::Construct(called,args),..
        }),..
    } if name==receiver && destination_type==ty
        && called==provider && args.is_empty()) {
        return Err(unsupported("P16：main 須以新結構宣告接收無參數函式結果"));
    }
    if !matches!(&main_return.kind,StmtKind::Return(Term {
        kind:TermKind::Literal("int"),start,end,
    }) if text.get(*start..*end)==Some("0")) {
        return Err(unsupported("P16：main 的其他返回路徑尚未接通"));
    }
    let definition=structs.get(ty)
        .ok_or_else(||unsupported("P16：結構類別不存在"))?;
    let fields=definition.fields.iter().map(|field| {
        if field.ty!="str" {
            return Err(unsupported("P16：此返回資源橋接只支援 str 欄位"));
        }
        let default=field.default.as_ref().map(|term|
            plain_literal(text,term).ok_or_else(||unsupported(
                "P16：此返回資源橋接尚未實作轉義字串預設求值")))
            .transpose()?;
        Ok(StringField {
            name:field.name.as_str(),default,immutable:field.immutable,
        })
    }).collect::<Result<Vec<_>,Diagnostic>>()?;
    let args=named_args.iter().map(|(name,term)| {
        let value=plain_literal(text,term)
            .ok_or_else(||unsupported("P16：具名引數限未經轉義 str 常值"))?;
        Ok(NamedString{field:name.as_str(),value})
    }).collect::<Result<Vec<_>,Diagnostic>>()?;
    let observe=fields.last().map(|f|f.name)
        .ok_or_else(||unsupported("P16：來源須至少有一個可觀察的 str 欄位"))?;
    let mut runtime=OwnedStructRuntime::new();
    runtime.enter_scope();
    let status=runtime.construct(local,&fields,&args,fail_after);
    if matches!(&status,Err(StructRuntimeError::EvaluationFailed(_))) {
        runtime.leave_scope().map_err(|_|
            unsupported("P16：被呼叫端失敗清理作用域未關閉"))?;
        return Ok((None,runtime.finish().map_err(|_|
            unsupported("P16：失敗構造後所有資源未完整釋放"))?));
    }
    if !matches!(&status,Ok(BuildStatus::Complete)) {
        return Err(unsupported("P16：返回結構未完成初始化或失敗注入切點不合法"));
    }
    let transferred=runtime.return_whole(local)
        .map_err(|_|unsupported("P16：被呼叫端返回不能轉交整筆結構所有權"))?;
    runtime.leave_scope()
        .map_err(|_|unsupported("P16：被呼叫端作用域清理失敗"))?;
    runtime.accept_return(receiver,transferred)
        .map_err(|_|unsupported("P16：呼叫端無法取得返回結構的唯一所有權"))?;
    let value=runtime.read_field(receiver,observe)
        .map_err(|_|unsupported("P16：函式返回後原資源無效"))?.to_owned();
    let events=runtime.finish()
        .map_err(|_|unsupported("P16：呼叫端退出後資源清理身分不完整"))?;
    Ok((Some(value),events))
}

/// P16.2 內部來源／資源橋接：只模擬明確的「建立單一 str、
/// 隨即整筆 .move() 至新 var、函式正常返回」這個可證安全路徑。
/// 非一般執行器；先由真實 Lexer／Parser／Checker 保證合法性。
#[cfg(test)]
fn trace_named_string_move_source(
    text: &str,
) -> Result<Vec<crate::owned_str_runtime::ResourceEvent>, Diagnostic> {
    use crate::owned_str_runtime::OwnedStrRuntime;
    let unsupported = |message: &str| Diagnostic {
        kind: DiagnosticKind::Static, message: message.into(), span: None,
    };
    let source = Source::new(text.as_bytes().to_vec());
    if let Some(error) = source.lexical_diagnostics().into_iter().next() {
        return Err(error);
    }
    let tokens = lexer::scan(&source);
    if let Some(error) = tokens.diagnostics.into_iter().next() {
        return Err(error);
    }
    let (structs, funcs) = Parser {
        source: &source, tokens: &tokens.tokens, cursor: 0,
    }.file()?;
    Checker {
        source: &source, structs: &structs, functions: &funcs,
        ability: crate::ownership_policy::approved_leaf_copy_permission,
    }.all(&funcs)?;
    let main = funcs.iter().find(|fun| fun.name == "main")
        .ok_or_else(|| unsupported("P16：具名字串移動來源缺少 main"))?;
    let [first, second, trailing_return] = main.body.as_slice() else {
        return Err(unsupported("P16：具名字串移動只接受連續建立、移動、返回"));
    };
    if !matches!(&trailing_return.kind, StmtKind::Return(Term {
        kind: TermKind::Literal("int"), start, end,
    }) if text.get(*start..*end) == Some("0")) {
        return Err(unsupported("P16：尚未證明此返回或其他控制流程的資源事件"));
    }
    let (owner, mutable, literal) = match &first.kind {
        StmtKind::Declare {
            name, ty, mutable, init: Some(literal),
        } if ty == "str" && matches!(&literal.kind, TermKind::Literal("str")) =>
            (name.as_str(), *mutable, literal),
        _ => return Err(unsupported("P16：第一個來源必須是獨立 str 常值持有者")),
    };
    let (target, from) = match &second.kind {
        StmtKind::Declare {
            name, ty, mutable: true,
            init: Some(Term {kind: TermKind::Move(from), ..}),
        } if ty == "str" => (name.as_str(), from.as_str()),
        _ => return Err(unsupported("P16：第二個來源僅接受 var str 整筆移動")),
    };
    if from != owner {
        return Err(unsupported("P16：資源轉移來源必須是剛建立的具名持有者"));
    }
    let raw = text.get(literal.start..literal.end)
        .ok_or_else(|| unsupported("P16：字串常值位置不是合法 UTF-8 邊界"))?;
    let value = raw.strip_prefix('"').and_then(|text| text.strip_suffix('"'))
        .filter(|text| !text.contains('\\') && !text.contains('\n')
            && !text.contains('\r'))
        .ok_or_else(|| unsupported("P16：未實作字串轉義求值，僅可追蹤直接常值"))?;
    let mut runtime = OwnedStrRuntime::new();
    runtime.declare(owner, mutable, value)
        .map_err(|_| unsupported("P16：獨立 UTF-8 資源建立失敗"))?;
    runtime.move_named(owner, target)
        .map_err(|_| unsupported("P16：具名來源未能安全轉交資源"))?;
    runtime.finish()
        .map_err(|_| unsupported("P16：具名移動後資源最終清理責任不完整"))
}

fn inspect_with_ability(text: &str, ability: impl Fn(&str) -> CopyPermission)
    -> Result<(), Diagnostic> {
    let source = Source::new(text.as_bytes().to_vec());
    if let Some(error) = source.lexical_diagnostics().into_iter().next() { return Err(error); }
    let tokens = lexer::scan(&source);
    if let Some(error) = tokens.diagnostics.into_iter().next() { return Err(error); }
    let (structs, funcs) = Parser { source: &source, tokens: &tokens.tokens, cursor: 0 }.file()?;
    Checker { source: &source, structs: &structs, functions: &funcs, ability }.all(&funcs)
}

/** P13 已核准 HA13-A/HB13-A 的真實來源受限靜態入口。
 * 目前僅內部使用；若有尚未實作的語意，仍必須保守拒絕。
 * 外部無法傳入臨時型別權限覆寫已核准 Copyable 政策。
 */
pub(crate) fn inspect(text: &str) -> Result<(), Diagnostic> {
    inspect_with_ability(text, crate::ownership_policy::approved_leaf_copy_permission)
}

/// 歷史測試保留假設能力注入，僅用於驗證 S19 合取及未知能力；
/// 正式核准入口一律使用上方 inspect，不接受外部能力覆寫。
#[cfg(test)]
fn inspect_with_test_ability(text: &str, ability: impl Fn(&str) -> CopyPermission)
    -> Result<(), Diagnostic> {
    inspect_with_ability(text, ability)
}


// 同層引入原有測試模組，以保留 Rust 測試完整識別名稱及驗收腳本過濾條件。
#[cfg(test)]
include!("struct_stage/tests_ownership.rs");
#[cfg(test)]
include!("struct_stage/tests_lifetime_resources.rs");
