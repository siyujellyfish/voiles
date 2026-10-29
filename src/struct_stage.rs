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
    Return(Term),
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
        } else if self.keyword("return") {
            self.cursor += 1;
            StmtKind::Return(self.term()?)
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
        if (kind == BorrowKind::ExclusiveWrite && !conflicts.is_empty())
            || (kind == BorrowKind::SharedRead
                && conflicts.contains(&BorrowKind::ExclusiveWrite)) {
            return Err(self.error("HC14：共享／獨占借用與現有別名衝突", term));
        }
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
            StmtKind::Return(term) => Self::term_mentions(term, name),
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
    fn release_last_uses(vars: &mut Scopes, rest: &[Stmt], continuation: &[Stmt]) {
        let mut younger_names = HashSet::new();
        for scope in vars.iter_mut().rev() {
            for (name, binding) in scope.iter_mut() {
                if let Some(loan) = binding.loan.as_mut() {
                    let needed_here = Self::scope_mentions_binding(
                        rest, name, younger_names.contains(name));
                    let needed_later = continuation.iter()
                        .any(|stmt| Self::statement_mentions(stmt, name));
                    if loan.active && !needed_here && !needed_later {
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
                // 引數在 self 借用前求值：明確移動 Copyable 來源合法，
                // 但下方將再次查核 self，禁止移交接收者後仍呼叫方法。
                for (arg, parameter) in args.iter().zip(&method.params) {
                    if !Self::method_argument_safe_before_borrow(arg) {
                        return Err(self.error(
                            "P13：方法引數僅接受已證明安全的 Copyable 值或具名整筆移動", arg));
                    }
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
                // 明確移動非 Copyable 具名來源；其餘資源運算式
                // 需由自身求值產生獨立所有權，不能隱含複製。
                // 所有參數預先核對型別與移交形式，避免型別錯誤
                // 才發現時已消耗前面參數的來源。
                for (arg, parameter) in args.iter().zip(&function.params) {
                    let expected = &parameter.ty;
                    if self.permission(expected) != CopyPermission::Allowed {
                        match &arg.kind {
                            TermKind::Literal("str") | TermKind::Move(_)
                                | TermKind::Construct(_, _) | TermKind::Call(_, _) => {}
                            _ => return Err(self.error(
                                "P13：非 Copyable 函式引數須明確移動或產生獨立擁有值", arg)),
                        }
                    }
                }
                for (arg, parameter) in args.iter().zip(&function.params) {
                    let expected = &parameter.ty;
                    let (actual, initialized) = self.evaluate(arg, vars)?;
                    if actual != *expected || !initialized {
                        return Err(self.error("P13：函式引數型別不符或尚未初始化", arg));
                    }
                }
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
    /// true 表示分支仍會繼續執行；false 表示所有目前路徑已 return。
    fn statements(&self, body: &[Stmt], vars: &mut Scopes,
        expected_return: &str, new_scope: bool) -> Result<bool, Diagnostic> {
        self.statements_with_follow(body, vars, expected_return, new_scope, &[])
    }

    fn statements_with_follow(&self, body: &[Stmt], vars: &mut Scopes,
        expected_return: &str, new_scope: bool,
        continuation: &[Stmt]) -> Result<bool, Diagnostic> {
        if new_scope { vars.push(HashMap::new()); }
        let result = self.statement_body(body, vars, expected_return, continuation);
        if new_scope { vars.pop(); }
        result
    }

    fn statement_body(&self, body: &[Stmt], vars: &mut Scopes,
        expected_return: &str, continuation: &[Stmt]) -> Result<bool, Diagnostic> {
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
                    // 僅准對完整具名擁有者覆寫 Copyable var 欄位，或對可證明
                    // 尚未初始化的 var 欄位首次寫入；後者不涉及 P16 舊值清理。
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
                        if permission != CopyPermission::Allowed {
                            return Err(self.reject_noncopyable(permission,
                                "P13 欄位覆寫涉及未核准的原值資源清理", value));
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
                    return Ok(false);
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
                    let after_branch: Vec<Stmt> = body[position + 1..].iter()
                        .chain(continuation.iter()).cloned().collect();
                    let mut on_true = snapshot.clone();
                    let live_true = self.statements_with_follow(
                        yes, &mut on_true, expected_return, true, &after_branch)?;
                    let mut on_false = snapshot.clone();
                    let live_false = if let Some(otherwise) = no {
                        self.statements_with_follow(
                            otherwise, &mut on_false, expected_return, true, &after_branch)?
                    } else {
                        Self::release_last_uses(&mut on_false, &after_branch, &[]);
                        true
                    };
                    if !live_true && !live_false { return Ok(false); }
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
        Ok(true)
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
                pending_fields: None, loan: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ownership_model::CopyPermission::{Allowed, Forbidden, Undecided};

    const BASE: &str = "struct Sample{\nconst flag:bool = true\nvar number:int = 1\n}\nfn main()->int{\nvar a:Sample = Sample()\nvar b:Sample = a.move()\nreturn 0\n}\n";

    #[test]
    fn 真正_struct_ast_具名整筆移動_來源失效() {
        assert!(inspect_with_test_ability(BASE, |_| Allowed).is_ok());
        let invalid = BASE.replace("return 0", "var c:Sample = a\nreturn 0");
        let err = inspect_with_test_ability(&invalid, |_| Allowed).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("移動"));
    }

    #[test]
    fn 真正_struct_ast_整體複製資格由欄位逐一核對() {
        let source = BASE.replace("a.move()", "a");
        assert!(inspect_with_test_ability(&source, |_| Allowed).is_ok());
        for cap in [Forbidden, Undecided] {
            let err = inspect_with_test_ability(&source, |name| if name == "bool" { cap } else { Allowed })
                .unwrap_err();
            assert_eq!(err.kind, DiagnosticKind::Static);
            assert!(err.message.contains("複製") || err.message.contains("Copyable"));
        }
        // 即使複製資格未定，明確整筆移動亦不等於隱含複製。
        assert!(inspect_with_test_ability(BASE, |_| Undecided).is_ok());
    }

    #[test]
    fn 真正_struct_ast_移動來源重新初始化後可再用() {
        let source = BASE.replace("return 0", "a = Sample()\nvar c:Sample = a.move()\nreturn 0");
        assert!(inspect_with_test_ability(&source, |_| Undecided).is_ok());
    }

    #[test]
    fn 真正_struct_ast_拒絕欄位移動與方法消耗_self() {
        let bad_field = BASE.replace("a.move()", "a.number.move()");
        assert!(inspect_with_test_ability(&bad_field, |_| Allowed).unwrap_err().message.contains("S21"));
        let bad_method = "struct S{\nvar n:int = 1\nfn consume()->int{\nreturn self.move()\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect_with_test_ability(bad_method, |_| Allowed).unwrap_err().message.contains("S15"));
    }

    #[test]
    fn 真正_struct_ast_缺失欄位初始化不能假裝資源完整() {
        let source = "struct S{\nvar missing:int\nconst flag:bool = true\n}\nfn main()->int{\nvar a:S = S()\nvar b:S = a.move()\nreturn 0\n}\n";
        assert!(inspect_with_test_ability(source, |_| Allowed).is_err());
    }

    #[test]
    fn 真正_struct_ast_建立式具名欄位及型別() {
        let source = BASE.replace("Sample()", "Sample(number=3,flag=false,)");
        assert!(inspect_with_test_ability(&source, |_| Allowed).is_ok());
        for bad in ["Sample(extra=1)", "Sample(number=false)", "Sample(number=1,number=2)"] {
            assert!(inspect_with_test_ability(&BASE.replace("Sample()", bad), |_| Allowed).is_err());
        }
    }

    #[test]
    fn 不得把限制外的控制流程直接當成已驗證() {
        let source = BASE.replace("return 0", "while (true){\nreturn 0\n}\nreturn 0");
        assert!(inspect_with_test_ability(&source, |_| Allowed).is_err());
    }
    #[test]
    fn 實體_voil_素材須由完整受限_ast_及型別所有權檢查處理() {
        const GOOD: &[&str] = &[
            include_str!("../tests/fixtures/p13_stage/整筆移動.voil"),
            include_str!("../tests/fixtures/p13_stage/結構重新初始化.voil"),
        ];
        const BAD: &[&str] = &[
            include_str!("../tests/fixtures/p13_stage/移動後重複使用.voil"),
            include_str!("../tests/fixtures/p13_stage/禁止欄位移動.voil"),
        ];
        for source in GOOD {
            assert!(inspect_with_test_ability(source, |_| Undecided).is_ok(), "合法整筆移動案例");
        }
        for source in BAD {
            let error = inspect_with_test_ability(source, |_| Allowed).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::Static);
        }
        const UNKNOWN_COPY: &str =
            include_str!("../tests/fixtures/p13_stage/未知複製資格.voil");
        assert!(inspect_with_test_ability(UNKNOWN_COPY, |_| Undecided).is_err());
        assert!(inspect_with_test_ability(UNKNOWN_COPY, |_| Allowed).is_ok());
    }

    #[test]
    fn 不存在的結構欄位型別不得由假設能力誤認() {
        let src = "struct Sample{\nvar mystery:Unknown\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect_with_test_ability(src, |_| Allowed).is_err());
    }

    #[test]
    fn 真正結構來源_單分支移動合流後必須拒絕() {
        let text = include_str!("../tests/fixtures/p13_stage/單分支移動後讀取.voil");
        let error = inspect_with_test_ability(text, |_| Allowed).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("可能") || error.message.contains("無效"));
    }

    #[test]
    fn 真正結構來源_雙分支重新初始化合流後可合法移動() {
        let text = include_str!("../tests/fixtures/p13_stage/雙分支重新初始化.voil");
        assert!(inspect_with_test_ability(text, |_| Undecided).is_ok());
    }

    #[test]
    fn 真正結構來源_已提前_return_分支不得污染可達路徑() {
        let text = include_str!("../tests/fixtures/p13_stage/提前回傳分支.voil");
        assert!(inspect_with_test_ability(text, |_| Undecided).is_ok());
    }

    #[test]
    fn 分支區域內宣告不得外洩到外層() {
        let text = include_str!("../tests/fixtures/p13_stage/雙分支重新初始化.voil")
            .replace("var d:Sample = a.move()", "var d:Sample = b");
        assert!(inspect_with_test_ability(&text, |_| Allowed).is_err());
    }

    #[test]
    fn 整數來源_非法進位與溢位均不得進入結構初始化() {
        for value in [
            "9223372036854775808",
            "18446744073709551616",
            "0x8000000000000000",
            "0b",
            "0o",
        ] {
            let source = BASE.replace("var number:int = 1",
                &format!("var number:int = {value}"));
            assert!(inspect_with_test_ability(&source, |_| Allowed).is_err(),
                "應拒絕不合法或超出 int64 上限的字面值：{value}");
        }
    }

    #[test]
    fn 整數來源_上限及合法多進位字面值仍可使用() {
        for value in ["9223372036854775807", "0x7fff_ffff_ffff_ffff", "0b101010", "0o777"] {
            let source = BASE.replace("var number:int = 1",
                &format!("var number:int = {value}"));
            assert!(inspect_with_test_ability(&source, |_| Undecided).is_ok());
        }
    }

    #[test]
    fn 尚未支援的浮點字面值不能繞過型別安全檢查() {
        let source = BASE.replace("var number:int = 1", "var number:float = 0.00000000000000001");
        assert!(inspect_with_test_ability(&source, |_| Allowed).is_err());
    }

}





#[cfg(test)]
mod readonly_self_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 已核准_copyable_欄位可藉共享唯讀_self_方法複製() {
        const CODE: &str =
            include_str!("../tests/fixtures/p13_stage/唯讀方法複製與呼叫後移動.voil");
        assert!(inspect(CODE).is_ok());
    }

    #[test]
    fn self_唯讀借用於方法結束後歸還_仍可明確整筆移動() {
        const CODE: &str =
            include_str!("../tests/fixtures/p13_stage/唯讀方法複製與呼叫後移動.voil");
        let invalid = CODE.replace("return copied", "var double:Counter = source.move()\nreturn copied");
        assert!(inspect(&invalid).unwrap_err().message.contains("已移動"));
    }

    #[test]
    fn 尚未核准_copyable_的_self_欄位不得從借用偷複製() {
        const CODE: &str =
            include_str!("../tests/fixtures/p13_stage/未知欄位複製能力的唯讀方法.voil");
        let error = inspect(CODE).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
        assert!(error.span.is_some());
    }

    #[test]
    fn 方法呼叫者在移動後不可再被借用() {
        const CODE: &str =
            include_str!("../tests/fixtures/p13_stage/移動後不可呼叫唯讀方法.voil");
        assert!(inspect(CODE).unwrap_err().message.contains("已移動"));
    }

    #[test]
    fn 尚未初始化的結構不可從方法讀取欄位() {
        const CODE: &str =
            include_str!("../tests/fixtures/p13_stage/未初始化欄位不可呼叫方法.voil");
        assert!(inspect(CODE).unwrap_err().message.contains("初始化"));
    }

    #[test]
    fn 方法只能讀取合法_self_欄位_回傳型別不符必須拒絕() {
        let wrong = "struct A{\nvar value:int = 1\nfn read()->bool{\nreturn self.value\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(wrong).unwrap_err().message.contains("回傳型別"));
        let missing = wrong.replace("->bool", "->int").replace("self.value", "self.missing");
        assert!(inspect(&missing).unwrap_err().message.contains("不存在"));
        let owner = wrong.replace("->bool", "->A").replace("self.value", "self");
        assert!(inspect(&owner).unwrap_err().message.contains("其他借用"));
    }

    #[test]
    fn 未驗證方法寫入及跨作用域語意維持保守拒絕() {
        let write = "struct A{\nvar value:int = 1\nfn change()->int{\nself.value = other.value\nreturn 0\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(write).is_err());
        let calls_self = "struct A{\nvar value:int = 1\nfn read()->int{\nreturn self.other()\n}\nfn other()->int{\nreturn self.value\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(calls_self).is_err());
    }
}

#[cfg(test)]
mod default_constructor_cycle_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 相同結構的預設建構不得無限遞迴() {
        let code = include_str!("../tests/fixtures/p13_stage/自身預設建構循環.voil");
        let error = inspect(code).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("循環"));
        assert!(error.span.is_some());
    }

    #[test]
    fn 多結構互相建立的預設值亦需保守拒絕() {
        let code = include_str!("../tests/fixtures/p13_stage/跨結構預設建構循環.voil");
        let error = inspect(code).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("循環"));
        assert!(error.span.is_some());
    }

    #[test]
    fn 合法無循環巢狀預設值保留受限子集可用性() {
        let code = include_str!("../tests/fixtures/p13_stage/無循環巢狀預設建構.voil");
        assert!(inspect(code).is_ok());
    }

    #[test]
    fn 具名建立引數內的遞迴預設值仍須拒絕() {
        let code = "struct Node{\nvar child:Node = Node(child=Node())\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(code).unwrap_err().message.contains("循環"));
    }
}

#[cfg(test)]
mod method_safety_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn s15_方法_if_內消耗_self_精準拒絕() {
        let code = include_str!("../tests/fixtures/p13_stage/方法分支內消耗_self.voil");
        let error = inspect(code).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("S15"));
        assert!(error.span.is_some());
    }

    #[test]
    fn s15_方法結構建立引數內消耗_self_亦應拒絕() {
        let code = include_str!("../tests/fixtures/p13_stage/方法建構引數內消耗_self.voil");
        let error = inspect(code).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("S15"));
        assert!(error.span.is_some());
    }

    #[test]
    fn s21_方法分支內局部欄位移動應保有來源位置() {
        let code = include_str!("../tests/fixtures/p13_stage/方法分支內局部欄位移動.voil");
        let error = inspect(code).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("S21"));
        assert!(error.span.is_some());
    }

    #[test]
    fn 無違規的合法借用方法仍不可在未驗證安全前誤收() {
        let code = "struct Sample{\nvar x:int = 1\nfn value()->int{\nvar temp:int = 1\nreturn temp\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error = inspect(code).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("未驗證"));
    }
}

#[cfg(test)]
mod lexical_ownership_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn h06_內層同名結構移動不得毀損外層所有權() {
        let code = include_str!("../tests/fixtures/p13_stage/內層遮蔽移動不傷外層.voil");
        assert!(inspect(code).is_ok());
    }

    #[test]
    fn 內層同名移動後不能透過同名重新複製() {
        let code = include_str!("../tests/fixtures/p13_stage/內層移動後誤讀.voil");
        let err = inspect(code).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("移動"));
    }

    #[test]
    fn 外層移動後不可藉內層重新宣告偷恢復來源() {
        let code = include_str!("../tests/fixtures/p13_stage/外層已移動不被內層遮蔽復原.voil");
        let err = inspect(code).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("移動"));
    }

    #[test]
    fn 同作用域重複宣告繼續拒絕() {
        let code = include_str!("../tests/fixtures/p13_stage/同層重複宣告拒絕.voil");
        let err = inspect(code).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("重複宣告"));
    }

    #[test]
    fn 雙分支同名內層資源都不該污染同一外層來源() {
        let code = "struct S{\nvar v:int = 1\n}\nfn main()->int{\nvar a:S = S()\nif (true){\nvar a:S = S()\nvar x:S = a.move()\n}else{\nvar a:S = S()\nvar y:S = a.move()\n}\nvar last:S = a.move()\nreturn 0\n}\n";
        assert!(inspect(code).is_ok());
    }

    #[test]
    fn 真正內層移動外層來源仍必須讓外層失效() {
        let code = "struct S{\nvar v:int = 1\n}\nfn main()->int{\nvar a:S = S()\nif (true){\nvar moved:S = a.move()\n}\nvar last:S = a\nreturn 0\n}\n";
        assert!(inspect(code).unwrap_err().message.contains("可能"));
    }
}

#[cfg(test)]
mod approved_policy_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn ha13_核准_int_bool_且所有欄位可複製的_struct_可獨立複製() {
        const SOURCE: &str =
            include_str!("../tests/fixtures/p13_approved/整體自動複製.voil");
        assert!(inspect(SOURCE).is_ok());
    }

    #[test]
    fn str_已禁止_copyable_及含_str_欄位的_struct_都不可隱含複製() {
        const SOURCE: &str =
            include_str!("../tests/fixtures/p13_approved/未知字串資格拒絕複製.voil");
        let error = inspect(SOURCE).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
        let simple = "fn main()->int{\nvar s:str = \"ok\"\nvar t:str = s\nreturn 0\n}\n";
        assert!(inspect(simple).unwrap_err().message.contains("Copyable"));
    }

    #[test]
    fn hb13_未知複製資格不應擋住明確的整筆具名移動() {
        const SOURCE: &str =
            include_str!("../tests/fixtures/p13_approved/未知字串允許具名整筆移動.voil");
        assert!(inspect(SOURCE).is_ok());
        let double_move = SOURCE.replace("return 0", "var c:Note = a.move()\nreturn 0");
        assert!(inspect(&double_move).unwrap_err().message.contains("已移動"));
    }

    #[test]
    fn hb13_不允許單欄位_未綁定來源或方法消耗_self() {
        const FIELD: &str =
            include_str!("../tests/fixtures/p13_approved/禁止欄位單獨移動.voil");
        assert!(inspect(FIELD).unwrap_err().message.contains("S21"));
        let missing = "fn main()->int{\nvar b:int = not_declared.move()\nreturn 0\n}\n";
        assert!(inspect(missing).unwrap_err().message.contains("找不到"));
        let method = "struct Pair{\nvar value:int = 1\nfn take()->int{\nreturn self.move()\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(method).unwrap_err().message.contains("S15"));
    }

    #[test]
    fn hb13_移動後必須重新初始化才允許再次使用() {
        let source = "fn main()->int{\nvar a:int = 1\nvar b:int = a.move()\na = 2\nreturn a\n}\n";
        assert!(inspect(source).is_ok());
        let invalid = source.replace("a = 2\n", "");
        assert!(inspect(&invalid).unwrap_err().message.contains("已移動"));
    }

    #[test]
    fn mvp_a_公開解析器仍禁止_move_語法() {
        let source = crate::source::Source::new(b"fn main()->int{\nvar a:int = 1\nreturn a.move()\n}\n".to_vec());
        let lexed = crate::lexer::scan(&source);
        assert!(crate::parser::parse(&source, &lexed.tokens).is_err());
    }
}

#[cfg(test)]
mod automatic_self_borrow_tests {
    use super::{inspect, Parser, Checker};
    use crate::DiagnosticKind;
    use crate::borrow_safety::BorrowKind;

    #[test]
    fn 方案_a_依欄位寫入推導方法共享或獨占借用() {
        let text = include_str!("../tests/fixtures/p13_stage/自動獨占與共享方法.voil");
        assert!(inspect(text).is_ok());
        let source = crate::source::Source::new(text.as_bytes().to_vec());
        let scan = crate::lexer::scan(&source);
        let (structs, _) = Parser {source: &source, tokens: &scan.tokens, cursor: 0}.file().unwrap();
        let methods = &structs["Counter"].methods;
        let checker = Checker {source: &source, structs: &structs, functions: &[],
            ability: crate::ownership_policy::approved_leaf_copy_permission};
        assert_eq!(checker.check_method("Counter", methods.iter().find(|m| m.name == "get").unwrap()).unwrap(),
            BorrowKind::SharedRead);
        assert_eq!(checker.check_method("Counter", methods.iter().find(|m| m.name == "set").unwrap()).unwrap(),
            BorrowKind::ExclusiveWrite);
    }

    #[test]
    fn s18_const_實例不得呼叫自動推導的獨占方法() {
        let code = include_str!("../tests/fixtures/p13_stage/const實例禁止可變方法.voil");
        let err = inspect(code).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("const 結構實例"));
    }

    #[test]
    fn s18_const_欄位不得被方法修改() {
        let code = include_str!("../tests/fixtures/p13_stage/const欄位禁止方法寫入.voil");
        let err = inspect(code).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("const 欄位"));
    }

    #[test]
    fn 方法對_self_欄位的錯誤型別必須拒絕() {
        let code = include_str!("../tests/fixtures/p13_stage/可變方法錯誤型別.voil");
        assert!(inspect(code).unwrap_err().message.contains("型別"));
    }

    #[test]
    fn 已移動來源不得藉可變方法存取() {
        let code = include_str!("../tests/fixtures/p13_stage/移動後禁止可變方法.voil");
        assert!(inspect(code).unwrap_err().message.contains("已移動"));
    }

    #[test]
    fn 不推定不同方法重入與運算式寫入的借用生命週期() {
        let src = "struct X{\nvar n:int = 1\nfn set()->int{\nself.n = other.n\nreturn 0\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).is_err());
        let nested = src.replace("self.n = other.n", "if (self.n){\nself.n = 2\n}");
        assert!(inspect(&nested).is_err());
    }

    #[test]
    fn 一般函式在完整持有來源上可合法寫入已核准_copyable_var_欄位() {
        let src = "struct X{\nvar n:int = 1\n}\nfn main()->int{\nvar x:X = X()\nx.n = 2\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod conditional_self_inference_tests {
    use super::{inspect, Parser, Checker};
    use crate::borrow_safety::BorrowKind;

    #[test]
    fn 任一分支修改_self_即推導獨占借用() {
        let text = include_str!("../tests/fixtures/p13_stage/分支寫入推導獨占方法.voil");
        assert!(inspect(text).is_ok());
        let source = crate::source::Source::new(text.as_bytes().to_vec());
        let scanned = crate::lexer::scan(&source);
        let (types, _) = Parser {source: &source, tokens: &scanned.tokens, cursor: 0}.file().unwrap();
        let checker = Checker {source: &source, structs: &types, functions: &[],
            ability: crate::ownership_policy::approved_leaf_copy_permission};
        let method = &types["Counter"].methods[0];
        assert_eq!(checker.check_method("Counter", method).unwrap(), BorrowKind::ExclusiveWrite);
    }

    #[test]
    fn const_實例不得呼叫只在某條分支寫入的方法() {
        let text = include_str!("../tests/fixtures/p13_stage/const實例禁止分支寫入.voil");
        assert!(inspect(text).unwrap_err().message.contains("const 結構實例"));
    }

    #[test]
    fn 自動借用推導也必須檢查兩條分支的欄位寫入型別() {
        let text = include_str!("../tests/fixtures/p13_stage/分支寫入推導獨占方法.voil");
        let bad = text.replace("self.value = 3", "self.value = false");
        assert!(inspect(&bad).unwrap_err().message.contains("型別"));
    }

    #[test]
    fn self_非布林欄位不可用作條件() {
        let text = include_str!("../tests/fixtures/p13_stage/分支寫入推導獨占方法.voil");
        let bad = text.replace("if (self.active)", "if (self.value)");
        assert!(inspect(&bad).unwrap_err().message.contains("bool"));
    }

    #[test]
    fn 無副作用常值_巢狀條件可保守推導獨占而非放行別名() {
        let text = include_str!("../tests/fixtures/p13_stage/分支寫入推導獨占方法.voil");
        let nested = text.replace("self.value = 2", "if (true){\nself.value = 2\n}");
        assert!(inspect(&nested).is_ok());
        let unknown = text.replace("if (self.active)", "if (other())");
        assert!(inspect(&unknown).is_err());
    }

    #[test]
    fn 方法分支提前返回已核准_未知接收者寫入仍拒絕() {
        let text = include_str!("../tests/fixtures/p13_stage/分支寫入推導獨占方法.voil");
        let early = text.replace("self.value = 2", "return 1");
        // 依已核准的分支提前回傳語意，此來源現在是合法正例。
        assert!(inspect(&early).is_ok());
        let unknown = text.replace("self.value = 2", "other.value = 2");
        assert!(inspect(&unknown).is_err());
    }
}

#[cfg(test)]
mod copyable_self_field_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 已核准可複製的_self_欄位可作方法獨占寫入來源() {
        let code = include_str!("../tests/fixtures/p13_stage/方法複製自身可複製欄位.voil");
        assert!(inspect(code).is_ok());
    }

    #[test]
    fn 已禁止_str_複製能力不可在方法裡隱含複製() {
        let code = include_str!("../tests/fixtures/p13_stage/方法不可複製未知欄位.voil");
        let err = inspect(code).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("Copyable"));
        assert!(err.span.is_some());
    }

    #[test]
    fn 已核准可複製來源若型別不一致亦不可寫入() {
        let code = include_str!("../tests/fixtures/p13_stage/方法來源欄位錯誤型別.voil");
        assert!(inspect(code).unwrap_err().message.contains("型別"));
    }

    #[test]
    fn 不存在或非法_self_欄位不能假借複製模式跳過解析() {
        let code = include_str!("../tests/fixtures/p13_stage/方法複製自身可複製欄位.voil");
        let invalid = code.replace("self.left = self.right", "self.left = self.missing");
        assert!(inspect(&invalid).unwrap_err().message.contains("不存在"));
    }

    #[test]
    fn s19_結構欄位若全可複製仍能整體複製作為_欄位_來源() {
        let src = "struct Leaf{\nvar n:int = 1\n}\nstruct Box{\nvar a:Leaf = Leaf()\nvar b:Leaf = Leaf()\nfn copy()->int{\nself.a = self.b\nreturn 0\n}\n}\nfn main()->int{\nvar box:Box = Box()\nreturn box.copy()\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod nonempty_struct_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn k06_空結構與只有註解的結構均必須拒絕() {
        for src in [
            "struct Empty{\n}\nfn main()->int{\nreturn 0\n}\n",
            "struct Empty{\n# 沒有實際欄位或方法\n}\nfn main()->int{\nreturn 0\n}\n",
            "struct Empty{\n\n}\nfn main()->int{\nreturn 0\n}\n",
        ] {
            let error = inspect(src).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::Syntax);
            assert!(error.message.contains("K06"));
            assert!(error.span.is_some());
        }
    }

    #[test]
    fn k06_只有欄位或只有方法都滿足結構非空條件() {
        let fields = "struct OnlyField{\nvar n:int = 1\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(fields).is_ok());
        let methods = "struct OnlyMethod{\nfn get()->int{\nreturn 1\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(methods).is_ok());
    }
}

#[cfg(test)]
mod p13_deferred_borrowed_return_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_a_非_copyable_self_字串欄位借用回傳必須拒絕() {
        let src = include_str!("../tests/fixtures/p13_stage/禁止回傳未決字串欄位.voil");
        let error = inspect(src).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
        assert!(error.message.contains("P15"));
        assert!(error.span.is_some());
    }

    #[test]
    fn p13_a_巢狀結構含未決能力欄位不得從_self_逸出() {
        let src = include_str!("../tests/fixtures/p13_stage/禁止回傳包含未決欄位的結構.voil");
        let error = inspect(src).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("P15"));
    }

    #[test]
    fn p13_a_全部欄位已核准_copyable_的巢狀結構可返回獨立複本() {
        let src = include_str!("../tests/fixtures/p13_stage/允許回傳完全Copyable結構副本.voil");
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_a_與_self_無關的字串常值是新值_不是借用回傳() {
        let src = include_str!("../tests/fixtures/p13_stage/合法獨立字串常值回傳.voil");
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn s21_即使回傳型別標註正確_仍不可移動單一_self_欄位() {
        let src = include_str!("../tests/fixtures/p13_stage/拒絕移動self單一欄位回傳.voil");
        let error = inspect(src).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("S21"));
    }

    #[test]
    fn p13_a_未知_copyable_欄位不會因方法有寫入而可借用回傳() {
        let src = "struct X{\nvar label:str = \"A\"\nvar n:int = 1\nfn get()->str{\nself.n = 2\nreturn self.label\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).unwrap_err().message.contains("P15"));
    }
}

#[cfg(test)]
mod nested_default_initialization_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn s07_有預設運算式不代表子結構已完整初始化() {
        let src = include_str!("../tests/fixtures/p13_stage/拒絕巢狀預設不完整值.voil");
        let err = inspect(src).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("未初始化"));
        assert!(err.span.is_some());
    }

    #[test]
    fn f01_const_欄位預設值也不可包含不完整子結構() {
        let src = include_str!("../tests/fixtures/p13_stage/拒絕const巢狀預設不完整值.voil");
        let err = inspect(src).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("未初始化"));
    }

    #[test]
    fn s05_具名提供所有必要欄位的巢狀預設值可通過() {
        let src = include_str!("../tests/fixtures/p13_stage/已初始化巢狀具名預設值.voil");
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn s07_var_欄位沒有預設依舊合法_只要求在使用前完成初始化() {
        let src = include_str!("../tests/fixtures/p13_stage/沒有預設值的var欄位仍合法宣告.voil");
        assert!(inspect(src).is_ok());
        let incomplete = src.replace("Parent(child=Child(count=5))", "Parent()");
        // 可建立未完成初始化的具名 var，但不能將其誤當作合法回傳值。
        let returned = incomplete.replace("return 0", "var q:Parent = p.move()\nreturn 0");
        assert!(inspect(&returned).is_err());
    }

    #[test]
    fn 多層巢狀預設建立亦不得繞過最內層未初始化狀態() {
        let src = "struct Leaf{\nvar n:int\n}\nstruct Mid{\nvar leaf:Leaf = Leaf()\n}\nstruct Root{\nvar mid:Mid = Mid()\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).unwrap_err().message.contains("未初始化"));
    }
}

#[cfg(test)]
mod str_noncopyable_policy_tests {
    use super::inspect;
    use crate::{DiagnosticKind, ownership_model::CopyPermission};
    use crate::ownership_policy::approved_leaf_copy_permission;

    #[test]
    fn str_明確禁止名稱隱含複製並保留來源位置() {
        let source =
            include_str!("../tests/fixtures/p13_stage/禁止字串名稱隱含複製.voil");
        let error = inspect(source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable 已明確禁止"));
        assert!(error.span.is_some());
    }

    #[test]
    fn str_具名整筆移動及重新初始化不需_copyable() {
        let source =
            include_str!("../tests/fixtures/p13_stage/字串具名整筆移動與重新初始化.voil");
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn str_移動後的原來源讀取必須失敗() {
        let source =
            include_str!("../tests/fixtures/p13_stage/移動字串後不能再次讀取.voil");
        let error = inspect(source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("已移動"));
    }

    #[test]
    fn s19_包含非copyable字串欄位的結構確定禁止隱含複製() {
        let source =
            include_str!("../tests/fixtures/p13_stage/包含字串欄位的結構禁止複製.voil");
        let error = inspect(source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable 已明確禁止"));
    }

    #[test]
    fn s19_包含字串欄位的結構仍可合法整筆移動() {
        let source =
            include_str!("../tests/fixtures/p13_stage/包含字串欄位的結構允許整筆移動.voil");
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn p13_禁止借用回傳已確定非copyable的字串欄位_不提前p15() {
        let source =
            include_str!("../tests/fixtures/p13_stage/禁止方法複製確定非Copyable字串欄位.voil");
        let error = inspect(source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable 已明確禁止"));
        assert!(error.message.contains("P15"));
    }

    #[test]
    fn str_明確非_copyable_float_明確_copyable_容器仍未決() {
        assert_eq!(approved_leaf_copy_permission("str"), CopyPermission::Forbidden);
        assert_eq!(approved_leaf_copy_permission("float"), CopyPermission::Allowed);
        assert_eq!(approved_leaf_copy_permission("list[float]"), CopyPermission::Undecided);
        assert_eq!(approved_leaf_copy_permission("int"), CopyPermission::Allowed);
        assert_eq!(approved_leaf_copy_permission("bool"), CopyPermission::Allowed);
    }
}

#[cfg(test)]
mod str_ownership_branch_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 非copyable字串雙分支皆重新初始化可再次整筆移動() {
        let source =
            include_str!("../tests/fixtures/p13_stage/字串雙分支移動後重新初始化.voil");
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn 非copyable字串只有一條路徑移動合流後必須拒絕() {
        let source =
            include_str!("../tests/fixtures/p13_stage/字串單分支移動合流後不得使用.voil");
        let err = inspect(source).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("可能"));
        assert!(err.span.is_some());
    }

    #[test]
    fn 已提前回傳的字串移動路徑不污染後續可達路徑() {
        let source =
            include_str!("../tests/fixtures/p13_stage/字串提前回傳後合流可合法移動.voil");
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn 分支內遮蔽的字串所有權不得混入外層來源() {
        let source =
            include_str!("../tests/fixtures/p13_stage/字串遮蔽來源移動不影響外層.voil");
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn str_巢狀提前_return_不可達移動不污染可達合流() {
        let source = include_str!(
            "../tests/fixtures/p13_stage/字串巢狀分支提前回傳不污染合流.voil"
        );
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn str_巢狀分支有可達移動時合流後須拒絕再次使用() {
        let source = include_str!(
            "../tests/fixtures/p13_stage/字串巢狀分支有可達移動不得再次使用.voil"
        );
        // VA16-03-A 已正式核准可證明安全的 str 常值覆寫。
        // 分支另一側仍可能已移動，不能因覆寫路徑合法就擅自宣稱
        // 合流之後來源一定有效，仍應回報「可能已移動」。
        let err = inspect(source).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("可能"));
        // 未覆寫的另一條路徑同樣必須維持此失效證明。
        let merged = source.replace("original = \"alternate\"", "var untouched2:int = 1");
        let err = inspect(&merged).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("可能"));
        assert!(err.span.is_some());
    }

    #[test]
    fn 字串在重新初始化前仍不得隱含複製且禁止讀取已移動來源() {
        let source =
            include_str!("../tests/fixtures/p13_stage/字串雙分支移動後重新初始化.voil");
        let illegal = source.replace(
            "original = \"true\"", "var illegal:str = original");
        assert!(inspect(&illegal).unwrap_err().message.contains("已移動"));
        let illegal_copy = source.replace(
            "var last:str = original.move()", "var last:str = original");
        assert!(inspect(&illegal_copy).unwrap_err()
            .message.contains("Copyable 已明確禁止"));
    }
}

#[cfg(test)]
mod float_copyable_approved_tests {
    use super::{inspect, Checker, Parser};
    use crate::ownership_model::CopyPermission;
    use crate::ownership_policy::approved_leaf_copy_permission;
    use crate::source::Source;
    use crate::lexer;

    /// 結構體的 float 欄位可在無具體 decimal64 常值求值的情況下驗證 Copyable。
    #[test]
    fn float_與遞迴全部_copyable_結構已核准_但包含_str_的結構不准複製() {
        let source_text = include_str!("../tests/fixtures/p13_stage/浮點與混合結構複製能力.voil");
        assert!(inspect(source_text).is_ok());
        let source = Source::new(source_text.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        let (structs, _) = Parser {
            source: &source, tokens: &tokens.tokens, cursor: 0
        }.file().unwrap();
        let checker = Checker {
            source: &source, structs: &structs, functions: &[],
            ability: approved_leaf_copy_permission,
        };
        assert_eq!(checker.permission("FloatBox"), CopyPermission::Allowed);
        assert_eq!(checker.permission("NestedFloat"), CopyPermission::Allowed);
        assert_eq!(checker.permission("MixedString"), CopyPermission::Forbidden);
    }

    #[test]
    fn float_具名來源尚未初始化不能因_copyable_就略過安全檢查() {
        let source_text = "fn main()->int{\nvar number:float\nvar copy:float = number\nreturn 0\n}\n";
        let error = inspect(source_text).unwrap_err();
        assert!(error.message.contains("初始化"));
        assert!(error.span.is_some());
    }

    #[test]
    fn 受限前端沒有完整_decimal64_範圍證明時仍須保守拒絕() {
        let source_text = "fn main()->int{\nvar number:float = 0.00000000000000001\nreturn 0\n}\n";
        let error = inspect(source_text).unwrap_err();
        assert!(error.message.contains("decimal64"));
    }
}

#[cfg(test)]
mod float_copyable_method_tests {
    use super::{inspect, Checker, Parser};
    use crate::borrow_safety::BorrowKind;
    use crate::lexer;
    use crate::ownership_policy::approved_leaf_copy_permission;
    use crate::source::Source;

    #[test]
    fn float_已准_copyable_故唯讀_self_欄位方法可以靜態驗證() {
        let text = include_str!("../tests/fixtures/p13_stage/浮點欄位唯讀方法型別能力.voil");
        assert!(inspect(text).is_ok());
        let source = Source::new(text.as_bytes().to_vec());
        let scan = lexer::scan(&source);
        let (types, _) = Parser {
            source: &source, tokens: &scan.tokens, cursor: 0
        }.file().unwrap();
        let checker = Checker {
            source: &source, structs: &types, functions: &[],
            ability: approved_leaf_copy_permission,
        };
        assert_eq!(
            checker.check_method("FloatHolder", &types["FloatHolder"].methods[0]).unwrap(),
            BorrowKind::SharedRead);
    }

    #[test]
    fn float_欄位之間複製已准但仍必須推導獨占_self_借用() {
        let text = include_str!("../tests/fixtures/p13_stage/浮點欄位複製至同型別欄位.voil");
        assert!(inspect(text).is_ok());
        let source = Source::new(text.as_bytes().to_vec());
        let scan = lexer::scan(&source);
        let (types, _) = Parser {
            source: &source, tokens: &scan.tokens, cursor: 0
        }.file().unwrap();
        let checker = Checker {
            source: &source, structs: &types, functions: &[],
            ability: approved_leaf_copy_permission,
        };
        assert_eq!(
            checker.check_method("FloatPair", &types["FloatPair"].methods[0]).unwrap(),
            BorrowKind::ExclusiveWrite);
    }

    #[test]
    fn float_可複製並不解除方法回傳型別一致的要求() {
        let text = include_str!("../tests/fixtures/p13_stage/浮點欄位回傳型別不一致.voil");
        assert!(inspect(text).unwrap_err().message.contains("回傳型別"));
    }

    #[test]
    fn float_未初始化的接收者仍不能直接呼叫方法() {
        let text = include_str!("../tests/fixtures/p13_stage/浮點欄位唯讀方法型別能力.voil");
        let invalid = text.replace("return 0\n}", "var value:FloatHolder = FloatHolder()\nvar copy:float = value.read()\nreturn 0\n}");
        let err = inspect(&invalid).unwrap_err();
        assert!(err.message.contains("初始化"));
    }
}

#[cfg(test)]
mod conservative_decimal64_tests {
    use super::{inspect, Parser};
    use crate::DiagnosticKind;

    #[test]
    fn decimal64_有限子集與尾端小數點_前導零_底線可解析() {
        let source = include_str!("../tests/fixtures/p13_stage/decimal64有限合法常值.voil");
        assert!(inspect(source).is_ok());
        for text in ["0.", "00.5", "1._2", "1.25", "12.345_678_901_234_5"] {
            assert!(Parser::decimal64_checked_subset(text).is_some(), "{text}");
        }
    }

    #[test]
    fn decimal64_平手時選尾數偶數_超過一半需進位() {
        assert_eq!(Parser::decimal64_checked_subset("1.2345678901234565"),
            Some((1_234_567_890_123_456, -15)));
        assert_eq!(Parser::decimal64_checked_subset("1.2345678901234555"),
            Some((1_234_567_890_123_456, -15)));
        assert_eq!(Parser::decimal64_checked_subset("1.2345678901234565001"),
            Some((1_234_567_890_123_457, -15)));
        assert_eq!(Parser::decimal64_checked_subset("1.2345678901234566"),
            Some((1_234_567_890_123_457, -15)));
        assert_eq!(Parser::decimal64_checked_subset("0.5"),
            Some((5_000_000_000_000_000, -16)));
    }

    #[test]
    fn decimal64_極端下溢與超出驗證範圍均保守拒絕() {
        let source = include_str!("../tests/fixtures/p13_stage/decimal64超出安全子集.voil");
        let error = inspect(source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Syntax);
        assert!(error.message.contains("decimal64"));
        assert!(error.span.is_some());
        for text in ["0.00000000000000001", "99999999999999999.", "10000000000000000."] {
            assert!(Parser::decimal64_checked_subset(text).is_none(), "{text}");
        }
    }

    #[test]
    fn decimal64_copyable_只允許型別相符而非隱含轉成_int() {
        assert!(inspect("fn main()->int{\nvar x:float = 1.25\nvar y:float = x\nreturn 0\n}\n").is_ok());
        let bad = "fn main()->int{\nvar x:int = 1.25\nreturn 0\n}\n";
        assert!(inspect(bad).unwrap_err().message.contains("型別"));
    }
}

#[cfg(test)]
mod vb14_vb15_vb16_approved_source_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn vb16_具名引數可任意排列且未覆蓋預設值仍合法() {
        let source="struct Profile{\nvar first:str = \"甲\"\nvar second:str = \"乙\"\nconst id:int = 1\n}\nfn main()->int{\nvar source:str = \"新\"\nvar x:Profile = Profile(second=source.move(),first=\"覆蓋\")\nreturn 0\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn vb16_同一欄位重複具名引數先靜態拒絕() {
        let source="struct Item{\nvar text:str = \"預設\"\n}\nfn main()->int{\nvar x:Item=Item(text=\"甲\",text=\"乙\")\nreturn 0\n}\n";
        assert_eq!(inspect(source).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn vb14_既有借用別名再借用必須保守拒絕() {
        let source="fn main()->int{\nconst value:int = 1\nconst alias:int = value.borrow()\nconst second:int = alias.borrow()\nreturn second\n}\n";
        let err=inspect(source).unwrap_err();
        assert_eq!(err.kind,DiagnosticKind::Static);
        assert!(err.message.contains("再次借用"));
    }

    #[test]
    fn vb15_尚未支援的跨_await_借用與例外不可被私有檢查器放行() {
        for source in [
            "fn main()->int{\nvar x:int=1\nvar alias:int=x.borrow()\nawait alias\nreturn x\n}\n",
            "fn main()->int{\nvar x:int=1\nvar alias:int=x.borrow()\nthrow alias\nreturn x\n}\n",
        ] {
            let err=inspect(source).unwrap_err();
            assert!(matches!(err.kind,DiagnosticKind::Syntax|DiagnosticKind::Static));
        }
    }
}

#[cfg(test)]
mod va15_path_sensitive_last_use_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn va15_分支中真正最後使用後同分支恢復原來源直接讀取() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nif (true){\nvar copy:int = alias\nvar direct:int = owner\n}else{\nvar direct:int = owner\n}\nreturn owner\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va15_外層仍會使用借用時分支內不可提前解除() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nif (true){\nvar direct:int = owner\n}else{\nvar other:int = 2\n}\nreturn alias\n}\n";
        let err = inspect(source).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("HC14"));
    }

    #[test]
    fn va15_無_else_的隱含分支也須和已分析分支安全合流() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nif (true){\nvar copy:int = alias\nvar direct:int = owner\n}\nreturn owner\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va15_巢狀分支各自完成最後使用後來源可用() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nif (true){\nif (false){\nvar a:int = alias\n}else{\nvar b:int = alias\n}\nvar direct:int = owner\n}else{\nvar direct:int = owner\n}\nreturn owner\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va15_返回路徑不能把借用活性污染可達的其他路徑() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nif (true){\nreturn alias\n}else{\nvar copy:int = owner\n}\nreturn owner\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va15_跨分支仍有效的獨占借用不得允許重新借用() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nif (true){\nvar first:int = alias\n}else{\nvar second:int = 2\n}\nvar other:int = owner.borrow()\nreturn alias\n}\n";
        let err = inspect(source).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("HC14"));
    }
}

#[cfg(test)]
mod va14_va15_borrow_source_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn va15_合法最後使用後來源恢復可用_不必等區塊出口() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nvar copy:int = alias\nreturn owner\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va14_來源借用尚有效時不得直接隱含複製() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nvar illegal:int = owner\nreturn alias\n}\n";
        let error = inspect(source).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("HC14"));
    }

    #[test]
    fn va14_const_共享借用可同時存在() {
        let source = "fn main()->int{\nconst owner:int = 1\nconst first:int = owner.borrow()\nconst second:int = owner.borrow()\nvar read:int = first\nreturn second\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va14_var_獨占借用不可重疊() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar first:int = owner.borrow()\nvar second:int = owner.borrow()\nreturn first\n}\n";
        let error = inspect(source).unwrap_err();
        assert!(error.message.contains("HC14"));
    }

    #[test]
    fn va14_來源與借用宣告可變性不符必須拒絕() {
        for source in [
            "fn main()->int{\nconst owner:int = 1\nvar alias:int = owner.borrow()\nreturn 0\n}\n",
            "fn main()->int{\nvar owner:int = 1\nconst alias:int = owner.borrow()\nreturn 0\n}\n",
        ] {
            assert!(inspect(source).unwrap_err().message.contains("VA14-01"));
        }
    }

    #[test]
    fn va14_非擁有借用不得被明確_move_轉換成所有者() {
        let source = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nvar moved:int = alias.move()\nreturn owner\n}\n";
        let error = inspect(source).unwrap_err();
        assert!(error.message.contains("VA14-01"));
    }

    #[test]
    fn va15_借用回傳沒有獨立存活證明時拒絕_但_copyable_值可獨立複製() {
        let bad = "fn get_str()->str{\nvar owner:str = \"str\"\nvar alias:str = owner.borrow()\nreturn alias\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(bad).unwrap_err().message.contains("Copyable"));
        let direct = "fn main()->int{\nvar owner:int = 1\nreturn owner.borrow()\n}\n";
        assert!(inspect(direct).unwrap_err().message.contains("逃逸"));
        let copy = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nreturn alias\n}\n";
        assert!(inspect(copy).is_ok());
    }

    #[test]
    fn va14_借用原非_copyable_資源期間不能偷偷移動來源() {
        let source = "fn main()->int{\nvar owner:str = \"str\"\nvar alias:str = owner.borrow()\nvar stolen:str = owner.move()\nreturn 0\n}\n";
        // 沒有實際用到 alias 會即時歸還，因此另加最後使用以確保此處仍借用。
        let source = source.replace("return 0", "var copy:str = alias\nreturn 0");
        assert!(inspect(&source).unwrap_err().message.contains("HC14"));
    }

    #[test]
    fn va15_分支內借用於分支最後使用後歸還外層來源() {
        let source = "fn main()->int{\nvar owner:int = 1\nif (true){\nvar alias:int = owner.borrow()\nvar copy:int = alias\n}else{\nvar other:int = 2\n}\nreturn owner\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va14_借用接收者合法唯讀方法且最後使用後解除封鎖() {
        let source = "struct S{\nvar n:int = 1\nfn get()->int{\nreturn self.n\n}\n}\nfn main()->int{\nconst owner:S = S()\nconst alias:S = owner.borrow()\nvar independent:int = alias.get()\nreturn owner.get()\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va14_var_獨占借用可呼叫合法可寫方法且原來源於最後使用後恢復() {
        let source = "struct S{\nvar n:int = 1\nfn update()->int{\nself.n = 2\nreturn self.n\n}\nfn get()->int{\nreturn self.n\n}\n}\nfn main()->int{\nvar owner:S = S()\nvar alias:S = owner.borrow()\nvar independent:int = alias.update()\nreturn owner.get()\n}\n";
        assert!(inspect(source).is_ok());
    }

    #[test]
    fn va14_再借用_在缺乏別名關係證明前保守拒絕() {
        let source = "fn main()->int{\nconst owner:int = 1\nconst alias:int = owner.borrow()\nconst twice:int = alias.borrow()\nreturn twice\n}\n";
        assert!(inspect(source).unwrap_err().message.contains("再次借用"));
    }
}

#[cfg(test)]
mod p13_resource_overwrite_barrier_tests {
    use super::inspect;
    #[test]
    fn va16_已核准有效_str_新常值原子覆寫為合法來源() {
        let src = "fn main()->int{\nvar s:str = \"old\"\ns = \"new\"\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn va16_另一合法具名_str_整筆_move_可覆寫有效目標() {
        let src = "fn main()->int{\nvar s:str = \"old\"\nvar incoming:str = \"new\"\ns = incoming.move()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn va16_新資源覆寫前先檢查右側型別與原來源借用限制() {
        let mismatch = "fn main()->int{\nvar s:str = \"old\"\ns = false\nreturn 0\n}\n";
        assert!(inspect(mismatch).unwrap_err().message.contains("型別"));
        let borrowed = "fn main()->int{\nvar s:str = \"old\"\nvar alias:str = s.borrow()\ns = \"new\"\nvar invalid:str = alias\nreturn 0\n}\n";
        assert!(inspect(borrowed).unwrap_err().message.contains("HC14"));
    }

    #[test]
    fn 已存在非_copyable_struct_不得整筆資源覆寫() {
        let src = "struct S{\nvar text:str = \"old\"\n}\nfn main()->int{\nvar a:S = S()\na = S()\nreturn 0\n}\n";
        assert!(inspect(src).unwrap_err().message.contains("P16"));
    }

    #[test]
    fn 已存在_str_方法欄位寫入亦不可繞過_p16() {
        let src = "struct S{\nvar text:str = \"old\"\nfn change()->int{\nself.text = \"new\"\nreturn 0\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).unwrap_err().message.contains("P16"));
    }

    #[test]
    fn copyable_一般賦值與方法覆寫仍合法() {
        let src = "struct S{\nvar value:int = 1\nfn update()->int{\nself.value = 2\nreturn self.value\n}\n}\nfn main()->int{\nvar s:S = S()\nvar n:int = 1\nn = 2\nvar m:int = s.update()\nreturn m\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn str_移動後重新初始化保留已核准語意() {
        let src = "fn main()->int{\nvar s:str = \"original\"\nvar t:str = s.move()\ns = \"new\"\nvar result:str = s.move()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 同名自身_move_回寫暫緩至求值次序安全核准() {
        let src = "fn main()->int{\nvar n:int = 1\nn = n.move()\nreturn n\n}\n";
        assert!(inspect(src).unwrap_err().message.contains("同一名稱"));
    }

    #[test]
    fn 合流仍可能保有資源時禁止整筆覆寫() {
        let src = "fn main()->int{\nvar s:str = \"old\"\nif (true){\nvar taken:str = s.move()\n}\ns = \"new\"\nreturn 0\n}\n";
        assert!(inspect(src).unwrap_err().message.contains("可能"));
    }
}

#[cfg(test)]
mod p13_formal_partial_field_completion_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    const HEADER: &str = "struct S{\nvar x:int\nvar text:str\nconst flag:bool = true\n}\nfn main()->int{\nvar a:S = S()\n";

    #[test]
    fn p13_s07_var_逐欄首次初始化後方可整筆明確移動() {
        let source = format!("{HEADER}a.x = 2\na.text = \"text\"\nvar b:S = a.move()\nreturn b.x\n}}\n");
        assert!(inspect(&source).is_ok());
    }

    #[test]
    fn p13_s07_仍缺少一欄時不得讀取_複製_移動或呼叫方法() {
        let moved = format!("{HEADER}a.x = 2\nvar b:S = a.move()\nreturn 0\n}}\n");
        assert!(inspect(&moved).unwrap_err().message.contains("初始化"));
        let read = format!("{HEADER}a.x = 2\nreturn a.x\n}}\n");
        assert!(inspect(&read).unwrap_err().message.contains("初始化"));
        let copy = format!("{HEADER}a.x = 2\nvar b:S = a\nreturn 0\n}}\n");
        assert!(inspect(&copy).unwrap_err().message.contains("初始化"));
    }

    #[test]
    fn p13_s07_分支兩側同樣補齊欄位才能在合流後使用() {
        let both = format!(
            "{HEADER}if (true){{\na.x = 3\n}}else{{\na.x = 4\n}}\na.text = \"yes\"\nvar b:S = a.move()\nreturn b.x\n}}\n"
        );
        assert!(inspect(&both).is_ok());
        let divergent = format!(
            "{HEADER}if (true){{\na.x = 3\n}}else{{\na.text = \"str\"\n}}\na.x = 2\na.text = \"text\"\nreturn a.x\n}}\n"
        );
        let err = inspect(&divergent).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("初始化"));
    }

    #[test]
    fn p13_d02_const_部分實例不可透過欄位寫入補完() {
        let src = "struct S{\nvar n:int\n}\nfn main()->int{\nconst a:S = S()\na.n = 5\nreturn 0\n}\n";
        assert!(inspect(src).unwrap_err().message.contains("const"));
    }

    #[test]
    fn p13_s07_未建構的_var_宣告不可偽裝成可寫的結構實例() {
        let src = "struct S{\nvar n:int\n}\nfn main()->int{\nvar a:S\na.n = 5\nreturn 0\n}\n";
        assert!(inspect(src).unwrap_err().message.contains("初始化"));
    }

    #[test]
    fn p13_s07_已有非_copyable_欄位不可二次覆寫_避免偷定_p16() {
        let src = format!("{HEADER}a.x = 2\na.text = \"hi\"\na.text = \"bye\"\nreturn 0\n}}\n");
        let err = inspect(&src).unwrap_err();
        assert!(err.message.contains("Copyable 已明確禁止"));
    }
}

#[cfg(test)]
mod p13_formal_field_write_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    const BODY: &str =
        "struct S{\nvar n:int = 1\nconst fixed:bool = true\nvar text:str = \"ok\"\n}\nfn main()->int{\nvar a:S = S()\n";

    #[test]
    fn p13_s13_s18_完整_var_接收者可覆寫_copyable_var_欄位() {
        let src = format!("{BODY}a.n = 2\nvar copy:int = a.n\nreturn copy\n}}\n");
        assert!(inspect(&src).is_ok());
        let in_branch = format!(
            "{BODY}if (true){{\na.n = 3\n}}else{{\na.n = 4\n}}\nreturn a.n\n}}\n"
        );
        assert!(inspect(&in_branch).is_ok());
    }

    #[test]
    fn p13_s18_const_實例及_const_欄位都不能寫入() {
        let const_owner = "struct S{\nvar n:int = 1\n}\nfn main()->int{\nconst a:S = S()\na.n = 2\nreturn 0\n}\n";
        let err = inspect(const_owner).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("S18"));
        let const_field = format!("{BODY}a.fixed = false\nreturn 0\n}}\n");
        assert!(inspect(&const_field).unwrap_err().message.contains("S18"));
    }

    #[test]
    fn p13_資源欄位覆寫仍依_p16_前置安全保守拒絕() {
        let src = format!("{BODY}a.text = \"new\"\nreturn 0\n}}\n");
        let err = inspect(&src).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("Copyable 已明確禁止"));
    }

    #[test]
    fn p13_移動後與分支不確定接收者仍拒絕_有建構基礎可首次補欄位() {
        let moved = format!(
            "{BODY}var b:S = a.move()\na.n = 2\nreturn 0\n}}\n"
        );
        assert!(inspect(&moved).unwrap_err().message.contains("已移動"));
        let uncertain = format!(
            "{BODY}if (true){{\nvar b:S = a.move()\n}}\na.n = 2\nreturn 0\n}}\n"
        );
        assert!(inspect(&uncertain).unwrap_err().message.contains("可能"));
        let partial = "struct S{\nvar n:int\n}\nfn main()->int{\nvar a:S = S()\na.n = 2\nreturn 0\n}\n";
        assert!(inspect(partial).is_ok());
    }

    #[test]
    fn p13_欄位型別不符與原來源仍受檢查() {
        let type_mismatch = format!("{BODY}a.n = false\nreturn 0\n}}\n");
        assert!(inspect(&type_mismatch).unwrap_err().message.contains("型別"));
        let missing = format!("{BODY}a.other = 5\nreturn 0\n}}\n");
        assert!(inspect(&missing).unwrap_err().message.contains("不存在"));
        let source_move = format!("{BODY}var n:int = 5\na.n = n.move()\nreturn n\n}}\n");
        assert!(inspect(&source_move).unwrap_err().message.contains("已移動"));
    }
}

#[cfg(test)]
mod p13_formal_implicit_move_matrix_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    fn forbidden_copy(code: &str) {
        let error = inspect(code).expect_err("已繫結 str 的隱含複製不得被當成移動");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable 已明確禁止"),
            "應清楚區分明確禁止與未決：{}", error.message);
        assert!(error.span.is_some());
    }

    #[test]
    fn p13_2_宣告初始化不允許把_str_名稱當成隱式移動() {
        forbidden_copy("fn main()->int{\nvar a:str = \"x\"\nvar b:str = a\nreturn 0\n}\n");
    }

    #[test]
    fn p13_2_重新賦值不允許把_str_名稱當成隱式移動() {
        forbidden_copy("fn main()->int{\nvar a:str = \"x\"\nvar b:str\nb = a\nreturn 0\n}\n");
    }

    #[test]
    fn p13_2_函式回傳已繫結_str_必須明確_move() {
        forbidden_copy("fn give()->str{\nvar a:str = \"x\"\nreturn a\n}\nfn main()->int{\nreturn 0\n}\n");
    }

    #[test]
    fn p13_2_結構具名欄位建立不得隱式移動_str() {
        forbidden_copy("struct Holder{\nvar value:str\n}\nfn main()->int{\nvar a:str = \"x\"\nvar h:Holder = Holder(value=a)\nreturn 0\n}\n");
    }

    #[test]
    fn p13_3_明確整筆_move_在宣告賦值回傳及具名建構仍合法() {
        let sources = [
            "fn main()->int{\nvar a:str = \"x\"\nvar b:str = a.move()\nreturn 0\n}\n",
            "fn main()->int{\nvar a:str = \"x\"\nvar b:str\nb = a.move()\nreturn 0\n}\n",
            "fn give()->str{\nvar a:str = \"x\"\nreturn a.move()\n}\nfn main()->int{\nreturn 0\n}\n",
            "struct Holder{\nvar value:str\n}\nfn main()->int{\nvar a:str = \"x\"\nvar h:Holder = Holder(value=a.move())\nreturn 0\n}\n",
        ];
        for source in sources {
            assert!(inspect(source).is_ok(), "合法的具名整筆明確移動不應被誤拒：{source}");
        }
    }

    #[test]
    fn p13_4_建構引數明確移動後原來源不得二次移動() {
        let source = "struct Holder{\nvar value:str\n}\nfn main()->int{\nvar a:str = \"x\"\nvar h:Holder = Holder(value=a.move())\nvar again:str = a.move()\nreturn 0\n}\n";
        let error = inspect(source).expect_err("已移動來源不得二次消耗");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("已移動"));
        assert!(error.span.is_some());
    }
}

#[cfg(test)]
mod p13_d02_const_partial_safety_tests {
    use super::inspect;
    use crate::DiagnosticKind;

    fn partial() -> &'static str {
        include_str!("../tests/fixtures/p13_stage/D02_const結構允許保留未初始化var欄位.voil")
    }

    #[test]
    fn d02_a_const_實例可以建立但不能誤判var欄位全部初始化() {
        assert!(inspect(partial()).is_ok());
    }

    #[test]
    fn d02_a_const_來源未完整初始化禁止欄位讀取() {
        let bad = partial().replace("return 0", "var copy:int = item.value\nreturn 0");
        let err = inspect(&bad).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("初始化"));
        assert!(err.span.is_some());
    }

    #[test]
    fn d02_a_const_來源未完整初始化即使全部欄位copyable也禁止隱含複製() {
        let bad = partial().replace("return 0", "var duplicate:Sample = item\nreturn 0");
        assert!(inspect(&bad).unwrap_err().message.contains("初始化"));
    }

    #[test]
    fn d02_a_const_即使欄位有預設值_未完整初始化結構仍不得讀取欄位() {
        let bad = partial().replace("return 0", "var flag:bool = item.ready\nreturn 0");
        let err = inspect(&bad).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("初始化"));
        assert!(err.span.is_some());
    }

    #[test]
    fn d02_a_const_不完整來源不可透過新的_const_綁定偷渡複製() {
        let bad = partial().replace("return 0", "const alias:Sample = item\nreturn 0");
        let err = inspect(&bad).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("初始化"));
        assert!(err.span.is_some());
    }

    #[test]
    fn d02_a_const_來源未完整初始化禁止整筆_move() {
        let bad = partial().replace("return 0", "var moved:Sample = item.move()\nreturn 0");
        assert!(inspect(&bad).unwrap_err().message.contains("初始化"));
    }

    #[test]
    fn d02_a_const_來源未完整初始化禁止方法借用讀取() {
        let bad = include_str!("../tests/fixtures/p13_stage/D02_const結構未初始化欄位不得藉方法使用.voil");
        assert!(inspect(bad).unwrap_err().message.contains("初始化"));
    }

    #[test]
    fn d02_a_const_來源不得透過賦值或欄位寫入補足初始化() {
        let reassignment = partial().replace("return 0", "item = Sample(value=9)\nreturn 0");
        assert!(inspect(&reassignment).unwrap_err().message.contains("const"));
        let field_write = partial().replace("return 0", "item.value = 9\nreturn 0");
        assert!(inspect(&field_write).unwrap_err().message.contains("const"));
    }

    #[test]
    fn d02_a_已完整初始化的_const_結構仍可安全複製讀取及使用唯讀方法() {
        let positive =
            include_str!("../tests/fixtures/p13_stage/D02_const結構完整初始化允許安全使用.voil");
        assert!(inspect(positive).is_ok());
    }

    #[test]
    fn d02_a_完整初始化_const_可讀取有預設值欄位並合法複製至另一_const() {
        let ready = include_str!(
            "../tests/fixtures/p13_stage/D02_const結構完整初始化允許安全使用.voil"
        );
        let positive = ready.replace(
            "return result",
            "const alias:Sample = item\nvar flag:bool = alias.ready\nreturn result"
        );
        assert!(inspect(&positive).is_ok());
    }

    #[test]
    fn d02_a_const_須有建立式且const欄位仍須提供預設值() {
        let no_init = partial().replace("const item:Sample = Sample()", "const item:Sample");
        assert!(inspect(&no_init).unwrap_err().message.contains("const"));
        let no_default = partial().replace("const ready:bool = true", "const ready:bool");
        assert!(inspect(&no_default).unwrap_err().message.contains("const"));
    }
}

#[cfg(test)]
mod p13_完整來源閉包推進測試 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_2_已核准_copyable_一般賦值及函式回傳不需隱式移動() {
        let cases = [
            "fn main()->int{\nvar a:int = 1\nvar b:int = a\nreturn a\n}\n",
            "fn main()->int{\nvar a:bool = true\nvar b:bool = a\nreturn 0\n}\n",
            "fn give()->int{\nvar a:int = 7\nreturn a\n}\nfn main()->int{\nreturn 0\n}\n",
        ];
        for source in cases {
            assert!(inspect(source).is_ok(), "合法 Copyable 來源不應誤拒：{source}");
        }
    }

    #[test]
    fn p13_3_明確移動後_原來源不可再次作為隱含複製或移動() {
        let suffixes = [
            "var again:str = a",
            "var again:str = a.move()",
        ];
        for suffix in suffixes {
            let source=format!(
                "fn main()->int{{\nvar a:str = \"原始\"\nvar b:str = a.move()\n{suffix}\nreturn 0\n}}\n"
            );
            let error=inspect(&source).expect_err("移動後來源不可再次使用");
            assert_eq!(error.kind,DiagnosticKind::Static);
            assert!(error.span.is_some());
        }
    }

    #[test]
    fn p13_4_移動後重新初始化可以再次明確移動_但不得隱含複製() {
        let valid="fn main()->int{\nvar a:str = \"原始\"\nvar b:str = a.move()\na = \"重新初始化\"\nvar c:str = a.move()\nreturn 0\n}\n";
        assert!(inspect(valid).is_ok());
        let invalid=valid.replace("var c:str = a.move()", "var c:str = a");
        let error=inspect(&invalid).expect_err("str 重新初始化後仍禁止隱含複製");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"),"{}",error.message);
    }
}

#[cfg(test)]
mod p13_分支合流所有權來源矩陣 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_4_單側移動合流後不能讀取或再移動來源() {
        for operation in ["var copy:str = source", "var taken:str = source.move()"] {
            let source=format!(
                "fn main()->int{{\nvar source:str = \"text\"\nif (true){{\nvar inner:str = source.move()\n}}else{{\nvar keep:int = 1\n}}\n{operation}\nreturn 0\n}}\n"
            );
            let error=inspect(&source).expect_err("單側移動後合流不得視為仍持有");
            assert_eq!(error.kind,DiagnosticKind::Static);
            assert!(error.span.is_some());
        }
    }

    #[test]
    fn p13_4_雙側移動合流後仍不得再次消耗() {
        let source="fn main()->int{\nvar source:str = \"text\"\nif (true){\nvar left:str = source.move()\n}else{\nvar right:str = source.move()\n}\nvar after:str = source.move()\nreturn 0\n}\n";
        let error=inspect(source).expect_err("兩側皆移動後不得重用");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.span.is_some());
    }

    #[test]
    fn p13_4_分支內合法_copyable_讀取不應令合流來源失效() {
        let source="fn main()->int{\nvar original:int = 3\nif (true){\nvar left:int = original\n}else{\nvar right:int = original\n}\nreturn original\n}\n";
        assert!(inspect(source).is_ok());
    }
}

#[cfg(test)]
mod p13_s19_s21_結構所有權來源矩陣 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_6_結構整體複製僅在全部欄位可複製時合法() {
        let valid="struct S{\nvar n:int = 1\nconst flag:bool = true\n}\nfn main()->int{\nvar original:S = S()\nvar copy:S = original\nreturn original.n\n}\n";
        assert!(inspect(valid).is_ok());
        let invalid="struct S{\nvar n:int = 1\nvar text:str = \"resource\"\n}\nfn main()->int{\nvar original:S = S()\nvar copy:S = original\nreturn 0\n}\n";
        let error=inspect(invalid).expect_err("含非 Copyable 欄位的結構不可整筆隱含複製");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.span.is_some());
    }

    #[test]
    fn p13_6_禁止從結構個別欄位明確移動() {
        let source="struct S{\nvar text:str = \"resource\"\n}\nfn main()->int{\nvar owner:S = S()\nvar stolen:str = owner.text.move()\nreturn 0\n}\n";
        let error=inspect(source).expect_err("S21 不允許只移動結構單一欄位");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.span.is_some());
    }
}

#[cfg(test)]
mod p13_5_資源與可複製值來源對照矩陣 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_5_資源_str_允許明確整筆移動_但不允許隱含複製() {
        let allowed="fn main()->int{\nvar resource:str = \"resource\"\nvar transferred:str = resource.move()\nreturn 0\n}\n";
        assert!(inspect(allowed).is_ok());
        let forbidden="fn main()->int{\nvar resource:str = \"resource\"\nvar duplicated:str = resource\nreturn 0\n}\n";
        let error=inspect(forbidden).expect_err("資源不得透過一般 = 隱含複製");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }

    #[test]
    fn p13_5_可複製值_允許複製與明確移動_移動後來源失效() {
        let copied="fn main()->int{\nvar source:int = 3\nvar copied:int = source\nreturn source\n}\n";
        assert!(inspect(copied).is_ok());
        let moved="fn main()->int{\nvar source:int = 3\nvar transferred:int = source.move()\nreturn source\n}\n";
        let error=inspect(moved).expect_err("即使是 Copyable，明確移動仍須使來源失效");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.span.is_some());
    }

    #[test]
    fn p13_5_資源重新初始化後可再次明確移動() {
        let source="fn main()->int{\nvar resource:str = \"first\"\nvar first:str = resource.move()\nresource = \"second\"\nvar second:str = resource.move()\nreturn 0\n}\n";
        assert!(inspect(source).is_ok());
    }
}

#[cfg(test)]
mod p13_函式參數來源安全 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_可複製型別參數允許函式內讀取與回傳() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_非可複製參數須由明確移交的呼叫端供應() {
        let src="fn identity(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar source:str = \"hello\"\nvar result:str = identity(source.move())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_參數名稱不可重複() {
        let src="fn identity(value:int,value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).is_err());
    }
}

#[cfg(test)]
mod p13_函式參數型別閉包 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_函式參數不得以不存在的型別建立虛構所有權() {
        let src="fn identity(value:Missing)->int{\nreturn 0\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("參數型別必須在現有型別環境中可解析");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("不存在的型別"));
    }

    #[test]
    fn p13_可複製的已宣告結構可作為受限函式參數() {
        let src="struct Item{\nvar n:int = 1\n}\nfn identity(value:Item)->int{\nreturn value.n\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_一般函式呼叫解析邊界 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_位置引數呼叫通過已驗證_copyable_路徑() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn identity(1)\n}\n";
        assert!(inspect(src).is_ok(), "已核准 Copyable 函式呼叫應通過完整引數檢查");
    }

    #[test]
    fn p13_位置引數巢狀移動不得繞過方法禁令() {
        let src="struct Item{\nvar n:int = 1\nfn value()->int{\nreturn identity(self.move())\n}\n}\nfn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("方法中巢狀 self.move 不得通過");
        assert_eq!(error.kind,DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p13_可複製函式呼叫 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_可複製參數與回傳可安全呼叫() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn identity(1)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_巢狀可複製呼叫可通過() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn identity(identity(1))\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_呼叫參數數量不符必須拒絕() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn identity()\n}\n";
        let err=inspect(src).expect_err("引數數量不符");
        assert_eq!(err.kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_呼叫參數型別不符必須拒絕() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn identity(true)\n}\n";
        let err=inspect(src).expect_err("引數型別不符");
        assert_eq!(err.kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_資源參數呼叫不得隱含複製() {
        let src="fn identity(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar source:str = \"hello\"\nvar result:str = identity(source)\nreturn 0\n}\n";
        assert!(inspect(src).is_err());
    }
}

#[cfg(test)]
mod p13_函式簽章唯一性 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_重名函式不可讓呼叫端選擇錯誤簽章() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn identity(value:bool)->bool{\nreturn value\n}\nfn main()->int{\nreturn identity(1)\n}\n";
        let error=inspect(src).expect_err("重名函式不得被任意挑選");
        assert_eq!(error.kind,DiagnosticKind::Syntax);
        assert!(error.message.contains("不得重複宣告函式名稱"));
    }

    #[test]
    fn p13_main_不可接受未經入口供應的參數() {
        let src="fn main(value:int)->int{\nreturn value\n}\n";
        let error=inspect(src).expect_err("main 參數不可被視為已初始化來源");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("main 入口不可宣告參數"));
    }

    #[test]
    fn p13_唯一函式簽章與合法入口仍可呼叫() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn identity(1)\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_跨函式資源移交 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_資源參數明確移動並由函式回傳() {
        let src="fn take(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar item:str = \"hello\"\nvar received:str = take(item.move())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_移交後原來源不得再用() {
        let src="fn take(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar item:str = \"hello\"\nvar received:str = take(item.move())\nvar invalid:str = item.move()\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_非_copyable_具名引數不能隱含複製() {
        let src="fn take(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar item:str = \"hello\"\nvar invalid:str = take(item)\nreturn 0\n}\n";
        let error=inspect(src).expect_err("資源不能隱含複製");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("明確移動"));
    }

    #[test]
    fn p13_資源函式呼叫結果可再移交() {
        let src="fn take(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar item:str = \"hello\"\nvar received:str = take(take(item.move()))\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_跨函式資源分支合流 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_單側移交後合流不得再次使用來源() {
        let src="fn take(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar source:str = \"hello\"\nif (true){\nvar result:str = take(source.move())\n}\nvar invalid:str = source.move()\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_雙分支皆移交後合流不得再次使用來源() {
        let src="fn take(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar source:str = \"hello\"\nif (true){\nvar result:str = take(source.move())\n}else{\nvar result:str = take(source.move())\n}\nvar invalid:str = source.move()\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_分支內遮蔽資源移交不影響外層來源() {
        let src="fn take(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar source:str = \"outer\"\nif (true){\nvar source:str = \"inner\"\nvar result:str = take(source.move())\n}\nvar result:str = take(source.move())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_雙分支重新初始化後可再次移交() {
        let src="fn take(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar source:str = \"hello\"\nif (true){\nvar result:str = take(source.move())\nsource = \"again\"\n}else{\nvar result:str = take(source.move())\nsource = \"again\"\n}\nvar result:str = take(source.move())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_函式呼叫所有權預檢 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_非可複製參數拒絕具名隱含複製() {
        let src="fn consume(value:str)->int{\nreturn 0\n}\nfn main()->int{\nvar source:str = \"hello\"\nreturn consume(source)\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_呼叫引數數量錯誤不應消耗資源() {
        let src="fn consume(value:str)->int{\nreturn 0\n}\nfn main()->int{\nvar source:str = \"hello\"\nreturn consume(source.move(),source.move())\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_明確資源移交與可複製參數可混用() {
        let src="fn consume(value:str,count:int)->int{\nreturn count\n}\nfn main()->int{\nvar source:str = \"hello\"\nreturn consume(source.move(),1)\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_零引數一般函式 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_零參數函式可回傳可複製值() {
        let src="fn answer()->int{\nreturn 42\n}\nfn main()->int{\nreturn answer()\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_零參數函式可建立獨立資源() {
        let src="fn make()->str{\nreturn \"hello\"\n}\nfn main()->int{\nvar result:str = make()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_缺少函式引數必須拒絕() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn identity()\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p13_巢狀結構資源正反來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    const PREFIX: &str = "struct Inner{\nvar content:str = \"hello\"\n}\nstruct Outer{\nvar inner:Inner = Inner()\n}\n";

    #[test]
    fn p13_巢狀非可複製結構可明確整筆移交() {
        let source=format!("{PREFIX}fn main()->int{{\nvar source:Outer = Outer()\nvar target:Outer = source.move()\nreturn 0\n}}\n");
        assert!(inspect(&source).is_ok());
    }

    #[test]
    fn p13_巢狀資源不可隱含複製() {
        let source=format!("{PREFIX}fn main()->int{{\nvar source:Outer = Outer()\nvar target:Outer = source\nreturn 0\n}}\n");
        assert_eq!(inspect(&source).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_巢狀資源移交後不得再次使用() {
        let source=format!("{PREFIX}fn main()->int{{\nvar source:Outer = Outer()\nvar target:Outer = source.move()\nvar again:Outer = source.move()\nreturn 0\n}}\n");
        assert_eq!(inspect(&source).unwrap_err().kind,DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p13_h_巢狀函式資源來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_h_巢狀函式可逐層移交新建資源() {
        let src="fn make()->str{\nreturn \"hello\"\n}\nfn forward(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar owned:str = forward(make())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_h_巢狀函式不可隱含複製資源() {
        let src="fn forward(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar owned:str = \"hello\"\nvar next:str = forward(owned)\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_h_巢狀函式移交後不得再次使用來源() {
        let src="fn forward(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar owned:str = \"hello\"\nvar next:str = forward(owned.move())\nvar invalid:str = owned.move()\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_h_巢狀函式引數型別必須匹配() {
        let src="fn forward(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar owned:str = forward(1)\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p13_i_方法與_self_所有權來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_i_合法唯讀方法複製_copyable_欄位() {
        let src="struct Counter{\nvar count:int = 1\nfn get()->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar value:Counter = Counter()\nreturn value.get()\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_i_禁止方法移動整個_self() {
        let src="struct Counter{\nvar count:int = 1\nfn take()->int{\nreturn self.move()\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).unwrap_err();
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("S15"));
    }

    #[test]
    fn p13_i_禁止方法回傳非_copyable_self_欄位() {
        let src="struct Resource{\nvar content:str = \"hello\"\nfn get()->str{\nreturn self.content\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_i_禁止方法單欄位移動() {
        let src="struct Counter{\nvar count:int = 1\nfn take()->int{\nreturn self.count.move()\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).unwrap_err();
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("S21"));
    }
}

#[cfg(test)]
mod p13_j_分支移交與重新初始化 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn p13_j_兩條分支皆移交來源則合流不可再次使用() {
        let src="fn main()->int{\nvar source:str = \"hello\"\nif (true){\nvar left:str = source.move()\n}else{\nvar right:str = source.move()\n}\nvar invalid:str = source.move()\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_j_單側移交後合流不可假定來源有效() {
        let src="fn main()->int{\nvar source:str = \"hello\"\nif (true){\nvar left:str = source.move()\n}\nvar invalid:str = source.move()\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn p13_j_分支合流後重新初始化恢復有效所有權() {
        let src="fn main()->int{\nvar source:str = \"hello\"\nif (true){\nvar left:str = source.move()\n}else{\nvar right:str = source.move()\n}\nsource = \"again\"\nvar valid:str = source.move()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn p13_j_分支區域變數不得逃逸作用域() {
        let src="fn main()->int{\nif (true){\nvar local:int = 1\n}\nreturn local\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p13_函式參數綁定唯一性 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 重複_copyable_參數名稱必須靜態拒絕() {
        let src="fn same(value:int,value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).unwrap_err();
        assert_eq!(error.kind,DiagnosticKind::Syntax);
        assert!(error.message.contains("函式參數名稱不得重複"));
    }

    #[test]
    fn 重複資源參數名稱不可掩蓋持有者() {
        let src="fn same(value:str,value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).unwrap_err();
        assert_eq!(error.kind,DiagnosticKind::Syntax);
        assert!(error.message.contains("函式參數名稱不得重複"));
    }

    #[test]
    fn 不同名稱的資源參數可各自明確移交() {
        let src="fn choose(first:str,second:str)->str{\nreturn first.move()\n}\nfn main()->int{\nvar left:str = \"left\"\nvar right:str = \"right\"\nvar result:str = choose(left.move(),right.move())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_函式與方法回傳型別封閉 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 未呼叫的函式也不得宣告不存在的回傳型別() {
        let src="fn unused()->Unknown{\nreturn 1\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).unwrap_err();
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("函式回傳型別不存在"));
    }

    #[test]
    fn 未呼叫的方法也不得宣告不存在的回傳型別() {
        let src="struct Boxed{\nvar value:int = 1\nfn get()->Unknown{\nreturn self.value\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).unwrap_err();
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("方法回傳型別不存在"));
    }

    #[test]
    fn 合法結構回傳型別保持允許整筆移交() {
        let src="struct Boxed{\nvar value:int = 1\n}\nfn make()->Boxed{\nreturn Boxed()\n}\nfn main()->int{\nvar item:Boxed = make()\nreturn item.value\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_方法回傳與借用權限交叉來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 合法唯讀方法回傳_copyable_欄位() {
        let src="struct Counter{\nvar count:int = 1\nfn read()->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar counter:Counter = Counter()\nreturn counter.read()\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 不可寫接收者不得呼叫寫入方法() {
        let src="struct Counter{\nvar count:int = 1\nfn reset()->int{\nself.count = 0\nreturn self.count\n}\n}\nfn main()->int{\nconst counter:Counter = Counter()\nreturn counter.reset()\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn 非_copyable_方法參數合法宣告但呼叫不得隱含複製() {
        let src="struct Counter{\nvar count:int = 1\nfn add(amount:str)->int{\nreturn self.count\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
        let invalid=src.replace("return 0\n}\n",
            "var value:str = \"資源\"\nconst counter:Counter = Counter()\nreturn counter.add(value)\n}\n");
        let error=inspect(&invalid).expect_err("具名 str 不得隱含複製移交");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }
}

#[cfg(test)]
mod p13_函式呼叫簽章與資源移交交叉驗證 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 函式呼叫不得引用不存在的名稱() {
        let src="fn main()->int{\nreturn missing()\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn 資源引數不可透過普通名稱隱含複製() {
        let src="fn use_it(item:str)->int{\nreturn 1\n}\nfn main()->int{\nvar item:str = \"x\"\nreturn use_it(item)\n}\n";
        let error=inspect(src).unwrap_err();
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("非 Copyable 函式引數"));
    }

    #[test]
    fn 資源引數明確移交後不可再次使用() {
        let src="fn use_it(item:str)->int{\nreturn 1\n}\nfn main()->int{\nvar item:str = \"x\"\nvar result:int = use_it(item.move())\nvar again:str = item.move()\nreturn result\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn 資源引數明確移交與合法回傳保持可用() {
        let src="fn use_it(item:str)->int{\nreturn 1\n}\nfn main()->int{\nvar item:str = \"x\"\nreturn use_it(item.move())\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_預設建構巢狀函式引數循環 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 函式引數中的預設結構建構循環必須拒絕() {
        let src="struct Loop{\nvar next:Loop = Loop()\n}\nfn identity(item:Loop)->Loop{\nreturn item.move()\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).unwrap_err();
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("循環"));
    }

    #[test]
    fn 合法非循環的巢狀預設建構仍可使用() {
        let src="struct Leaf{\nvar count:int = 1\n}\nstruct Holder{\nvar leaf:Leaf = Leaf()\n}\nfn main()->int{\nvar holder:Holder = Holder()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_預設值來源跨型別循環 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 兩種結構的預設建構互相依賴必須拒絕() {
        let src="struct Alpha{\nvar other:Beta = Beta()\n}\nstruct Beta{\nvar other:Alpha = Alpha()\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).unwrap_err();
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("循環"));
    }

    #[test]
    fn 未使用的非循環預設結構可以宣告() {
        let src="struct Leaf{\nvar value:int = 1\n}\nstruct Outer{\nvar inner:Leaf = Leaf()\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_1_copyable_完整能力分類 {
    use super::{Checker, Parser};
    use crate::ownership_model::CopyPermission;
    use crate::ownership_policy::approved_leaf_copy_permission;
    use crate::source::Source;

    fn permission(src: &str, ty: &str) -> CopyPermission {
        let source = Source::new(src.as_bytes().to_vec());
        let tokens = crate::lexer::scan(&source);
        let (structs, functions) = Parser { source: &source, tokens: &tokens.tokens, cursor: 0 }.file().expect("來源語法合法");
        let checker = Checker {
            source: &source, structs: &structs, functions: &functions,
            ability: approved_leaf_copy_permission,
        };
        checker.permission(ty)
    }

    #[test]
    fn 已核准葉型別及容器各自維持三態() {
        let src="fn main()->int{\nreturn 0\n}\n";
        for ty in ["int", "bool", "float"] {
            assert_eq!(permission(src,ty),CopyPermission::Allowed);
        }
        assert_eq!(permission(src,"str"),CopyPermission::Forbidden);
        for ty in ["list[int]","dict{int,bool}","list[list[float]]","Unknown"] {
            assert_eq!(permission(src,ty),CopyPermission::Undecided);
        }
    }

    #[test]
    fn 巢狀結構全欄位允許時才允許複製() {
        let src="struct Leaf{\nvar value:float = 1.0\n}\nstruct Middle{\nvar leaf:Leaf = Leaf()\n}\nstruct Root{\nvar middle:Middle = Middle()\n}\nfn main()->int{\nreturn 0\n}\n";
        for ty in ["Leaf","Middle","Root"] {
            assert_eq!(permission(src,ty),CopyPermission::Allowed);
        }
    }

    #[test]
    fn 巢狀結構含資源欄位整體明確禁止複製() {
        let src="struct Resource{\nvar text:str = \"x\"\n}\nstruct Wrapper{\nvar resource:Resource = Resource()\n}\nfn main()->int{\nreturn 0\n}\n";
        for ty in ["Resource","Wrapper"] {
            assert_eq!(permission(src,ty),CopyPermission::Forbidden);
        }
    }
}

#[cfg(test)]
mod p13_1_正式來源閉包 {
    use super::{inspect, Checker, Parser};
    use crate::ownership_model::CopyPermission;
    use crate::ownership_policy::approved_leaf_copy_permission;
    use crate::{DiagnosticKind, lexer, source::Source};

    fn ability(src: &str, ty: &str) -> CopyPermission {
        let source = Source::new(src.as_bytes().to_vec());
        let tokens = lexer::scan(&source);
        assert!(tokens.diagnostics.is_empty());
        let (structs, functions) = Parser {
            source: &source, tokens: &tokens.tokens, cursor: 0,
        }.file().expect("來源語法應可解析");
        Checker {
            source: &source, structs: &structs, functions: &functions,
            ability: approved_leaf_copy_permission,
        }.permission(ty)
    }

    #[test]
    fn 全部欄位可複製的三層結構可實際隱含複製() {
        let src = "struct Leaf{\nvar amount:float = 1.0\n}\nstruct Middle{\nvar leaf:Leaf = Leaf()\n}\nstruct Root{\nvar middle:Middle = Middle()\n}\nfn main()->int{\nvar first:Root = Root()\nvar second:Root = first\nvar third:Root = first\nreturn 0\n}\n";
        assert_eq!(ability(src, "Root"), CopyPermission::Allowed);
        assert!(inspect(src).is_ok(), "可複製來源的多次使用必須合法");
    }

    #[test]
    fn 三層結構內含字串時整體確定禁止隱含複製() {
        let src = "struct Leaf{\nvar title:str = \"文字\"\n}\nstruct Middle{\nvar leaf:Leaf = Leaf()\n}\nstruct Root{\nvar middle:Middle = Middle()\n}\nfn main()->int{\nvar first:Root = Root()\nvar second:Root = first\nreturn 0\n}\n";
        assert_eq!(ability(src, "Root"), CopyPermission::Forbidden);
        let error = inspect(src).expect_err("非 Copyable 結構不得隱含複製");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }

    #[test]
    fn 容器未決與未知型別不誤報為永久不可複製() {
        let src = "fn main()->int{\nreturn 0\n}\n";
        for ty in ["list[int]", "list[list[float]]", "dict{str,int}", "Optional[int]", "Unknown"] {
            assert_eq!(ability(src, ty), CopyPermission::Undecided, "{ty}");
        }
        assert_eq!(ability(src, "str"), CopyPermission::Forbidden);
        for ty in ["int", "bool", "float"] {
            assert_eq!(ability(src, ty), CopyPermission::Allowed);
        }
    }
}

#[cfg(test)]
mod p13_p14_函式參數真實宣告身分 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 借用第一參數不應封鎖另一個獨立參數() {
        let src = "fn calculate(first:int, second:int)->int{\nconst borrowed:int = first.borrow()\nvar other:int = second\nreturn borrowed\n}\nfn main()->int{\nreturn calculate(1, 2)\n}\n";
        assert!(inspect(src).is_ok(), "不同參數的來源身分必須各自獨立");
    }

    #[test]
    fn 借用第一參數時仍禁止直接使用同一參數() {
        let src = "fn calculate(first:int, second:int)->int{\nconst borrowed:int = first.borrow()\nvar other:int = first\nreturn borrowed\n}\nfn main()->int{\nreturn calculate(1, 2)\n}\n";
        let error=inspect(src).expect_err("來源仍被借用時應拒絕直接使用");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }
}

#[cfg(test)]
mod p13_2_一般加減及所有權來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 已核准括號加減可使用多個可複製來源() {
        let src = "fn sum(x:int, y:int)->int{\nvar first:int = (x + y) - 1\nreturn first + (y - 2)\n}\nfn main()->int{\nreturn sum(4, 5)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 明確移動值參與加減後來源必須失效() {
        let src = "fn main()->int{\nvar number:int = 3\nvar result:int = number.move() + 1\nreturn number\n}\n";
        let error=inspect(src).expect_err("明確移交的 int 來源已失效");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("移動"));
    }

    #[test]
    fn 加減不能藉括號隱含複製字串資源() {
        let src = "fn main()->int{\nvar word:str = \"資源\"\nvar number:int = (word + 1)\nreturn number\n}\n";
        let error=inspect(src).expect_err("非 Copyable 來源不得隱含複製");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }

    #[test]
    fn 加減不能默許布林型別混入() {
        let src = "fn main()->int{\nreturn true + 2\n}\n";
        let error=inspect(src).expect_err("型別不符合 int 加減");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("int"));
    }
}

#[cfg(test)]
mod p13_2_有參數合法方法來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn copyable_參數可寫入_var_self_欄位並返回獨立值() {
        let src = "struct Counter{\nvar count:int = 1\nfn replace(value:int)->int{\nself.count = value\nreturn self.count\n}\n}\nfn main()->int{\nvar counter:Counter = Counter()\nreturn counter.replace(3)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 唯讀方法可回傳_copyable_參數() {
        let src = "struct Sample{\nvar number:int = 0\nfn choose(value:int)->int{\nreturn value\n}\n}\nfn main()->int{\nconst sample:Sample = Sample()\nreturn sample.choose(7)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn copyable_參數錯誤型別必須在來源檢查階段拒絕() {
        let src = "struct Sample{\nvar number:int = 0\nfn choose(value:int)->int{\nreturn value\n}\n}\nfn main()->int{\nvar sample:Sample = Sample()\nreturn sample.choose(true)\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind, DiagnosticKind::Static);
    }

    #[test]
    fn const_方法接收者不得透過參數修改_self() {
        let src = "struct Counter{\nvar count:int = 1\nfn replace(value:int)->int{\nself.count = value\nreturn self.count\n}\n}\nfn main()->int{\nconst counter:Counter = Counter()\nreturn counter.replace(3)\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind, DiagnosticKind::Static);
    }

    #[test]
    fn 資源方法參數使用已核准擁有權移交時可合法宣告() {
        let src = "struct Holder{\nvar count:int = 0\nfn use_text(text:str)->int{\nreturn 1\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_4_else_if_來源合流 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 已核准_else_if_中各分支_copyable_賦值都可安全合流() {
        let src = "fn main()->int{\nvar value:int = 1\nif(true){\nvalue = value + 1\n} else if(false){\nvalue = value + 2\n} else {\nvalue = value + 3\n}\nreturn value\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn else_if_其中一路徑移動資源後合流不能再使用() {
        let src = "fn main()->int{\nvar text:str = \"abc\"\nif(false){\nvar taken:str = text.move()\n} else if(true){\nvar keep:int = 1\n} else {\nvar keep:int = 2\n}\nvar again:str = text.move()\nreturn 0\n}\n";
        let error=inspect(src).expect_err("存在可達路徑已移動來源，不得使用");
        assert_eq!(error.kind,DiagnosticKind::Static);
    }

    #[test]
    fn else_if_全部分支提前回傳不應被不可達移動干擾() {
        let src = "fn main()->int{\nvar text:str = \"safe\"\nif(true){\nreturn 1\n} else if(false){\nreturn 2\n} else {\nreturn 3\n}\nreturn 4\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p15_遮蔽借用最後使用點精度 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 分支內新別名遮蔽後不再引用外層借用_可歸還原來源() {
        let src = "fn main()->int{\nconst source:int = 1\nconst alias:int = source.borrow()\nif(true){\nvar alias:int = 2\nvar copy:int = source\nreturn alias\n}\nreturn 0\n}\n";
        assert!(inspect(src).is_ok(), "遮蔽後的別名使用不得誤延長外層借用");
    }

    #[test]
    fn 分支內仍引用外層借用時不得提前解鎖來源() {
        let src = "fn main()->int{\nconst source:int = 1\nconst alias:int = source.borrow()\nif(true){\nvar copy:int = source\nreturn alias\n}\nreturn 0\n}\n";
        let error=inspect(src).expect_err("外層別名仍有效不得直接使用來源");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }
}

#[cfg(test)]
mod p13_方法純值運算式統一檢查 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 讀取_self_欄位與參數的加減可獨立回傳() {
        let src = "struct Counter{\nvar count:int = 4\nfn plus(amount:int)->int{\nreturn (self.count + amount) - 1\n}\n}\nfn main()->int{\nconst counter:Counter = Counter()\nreturn counter.plus(3)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 可寫方法以純值加減更新_self_欄位() {
        let src = "struct Counter{\nvar count:int = 4\nfn plus(amount:int)->int{\nself.count = self.count + amount\nreturn self.count\n}\n}\nfn main()->int{\nvar counter:Counter = Counter()\nreturn counter.plus(3)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 可寫方法的_self_原欄位與加減型別必須匹配() {
        let src = "struct Counter{\nvar count:int = 4\nfn wrong(amount:bool)->int{\nself.count = self.count + amount\nreturn 0\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("方法加減必須驗證 int 運算元");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("int"));
    }

    #[test]
    fn 方法不能把非_copyable_self_欄位藏進加減中() {
        let src = "struct Counter{\nvar resource:str = \"x\"\nfn bad()->int{\nreturn self.resource + 1\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("巢狀資源欄位仍不得隱含複製");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }
}

#[cfg(test)]
mod p13_方法呼叫純_copyable_一般函式 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 方法回傳可引用合法純值函式計算與_self_欄位() {
        let src = "fn add_one(value:int)->int{\nreturn value + 1\n}\nstruct Counter{\nvar amount:int = 5\nfn calculated(value:int)->int{\nreturn add_one(self.amount + value)\n}\n}\nfn main()->int{\nconst counter:Counter = Counter()\nreturn counter.calculated(3)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 方法能呼叫返回_copyable_的零引數一般函式() {
        let src = "fn base()->int{\nreturn 9\n}\nstruct Box{\nvar count:int = 0\nfn read()->int{\nreturn base()\n}\n}\nfn main()->int{\nconst instance:Box = Box()\nreturn instance.read()\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 方法內函式的資源引數不可隱含複製() {
        let src = "fn use_text(text:str)->int{\nreturn 1\n}\nstruct Box{\nvar text:str = \"資源\"\nfn bad()->int{\nreturn use_text(self.text)\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("方法不能偷複製資源欄位");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }

    #[test]
    fn 方法內一般函式新建資源可直接回傳獨立所有權() {
        let src = "fn make_text()->str{\\nreturn \\"資源\\"\\n}\\nstruct Box{\\nvar count:int = 0\\nfn fresh()->str{\\nreturn make_text()\\n}\\n}\\nfn main()->int{\\nreturn 0\\n}\\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_方法可達分支提前回傳 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 方法分支可合法提前回傳_copyable_欄位及參數運算式() {
        let src="struct Sample{\nvar amount:int = 4\nfn value(flag:bool, delta:int)->int{\nif(flag){\nreturn self.amount + delta\n}\nreturn self.amount\n}\n}\nfn main()->int{\nconst sample:Sample = Sample()\nreturn sample.value(true, 3)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 可寫方法的分支提前回傳仍須自動推導獨占_self() {
        let src="struct Sample{\nvar amount:int = 4\nfn value(flag:bool, delta:int)->int{\nif(flag){\nself.amount = delta\nreturn self.amount\n}\nreturn self.amount\n}\n}\nfn main()->int{\nvar sample:Sample = Sample()\nreturn sample.value(true, 3)\n}\n";
        assert!(inspect(src).is_ok());
        let invalid=src.replace("var sample:Sample", "const sample:Sample");
        let error=inspect(&invalid).expect_err("可寫方法要求獨占可變 self");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("const"));
    }

    #[test]
    fn 提前回傳錯誤型別不得因最後_return_正確而漏檢() {
        let src="struct Sample{\nvar amount:int = 4\nfn bad(flag:bool)->int{\nif(flag){\nreturn true\n}\nreturn self.amount\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("提前返回型別錯誤");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("回傳型別"));
    }

    #[test]
    fn 提前回傳不得隱含複製非_copyable_self_欄位() {
        let src="struct Sample{\nvar text:str = \"資源\"\nfn bad(flag:bool)->str{\nif(flag){\nreturn self.text\n}\nreturn \"ok\"\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("資源欄位不允許隱含複製");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }
}

#[cfg(test)]
mod p13_方法位置引數明確整筆移動 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn copyable_具名整筆_move_可作方法位置引數() {
        let src="struct Counter{\nvar count:int = 1\nfn use_value(value:int)->int{\nreturn value\n}\n}\nfn main()->int{\nvar input:int = 5\nconst counter:Counter = Counter()\nreturn counter.use_value(input.move())\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 方法引數移動後來源不可再讀() {
        let src="struct Counter{\nvar count:int = 1\nfn use_value(value:int)->int{\nreturn value\n}\n}\nfn main()->int{\nvar input:int = 5\nconst counter:Counter = Counter()\nvar result:int = counter.use_value(input.move())\nreturn input\n}\n";
        let error=inspect(src).expect_err("輸入來源已明確移動");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("移動"));
    }

    #[test]
    fn 方法引數不得移動同一接收者後仍借用_self() {
        let src="struct Counter{\nvar count:int = 1\nfn compare(other:Counter)->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar counter:Counter = Counter()\nreturn counter.compare(counter.move())\n}\n";
        let error=inspect(src).expect_err("方法借用前接收者已被移動");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("移動"));
    }

    #[test]
    fn 方法引數仍不可移動已借出來源() {
        let src="struct Counter{\nvar count:int = 1\nfn use_value(value:int)->int{\nreturn value\n}\n}\nfn main()->int{\nconst source:int = 1\nconst alias:int = source.borrow()\nconst counter:Counter = Counter()\nvar result:int = counter.use_value(source.move())\nreturn alias\n}\n";
        let error=inspect(src).expect_err("有效別名期間不得移動來源");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }
}

#[cfg(test)]
mod p13_3_所有權移動來源閉包 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn str_資源經函式引數及回傳可連續明確轉交() {
        let src="fn relay(source:str)->str{\nreturn source.move()\n}\nfn main()->int{\nvar source:str = \"中文\"\nvar result:str = relay(source.move())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 含資源結構可整筆移動但不可隱含複製() {
        let src="struct Box{\nvar content:str = \"內容\"\n}\nfn main()->int{\nvar source:Box = Box()\nvar destination:Box = source.move()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
        let invalid=src.replace("source.move()", "source");
        let error=inspect(&invalid).expect_err("資源結構不可隱含複製");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }

    #[test]
    fn 具名欄位建立式明確轉交資源不得重複使用舊來源() {
        let src="struct Box{\nvar content:str = \"預設\"\n}\nfn main()->int{\nvar source:str = \"獨立\"\nvar destination:Box = Box(content=source.move())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
        let invalid=src.replace("return 0", "var again:str = source.move()\nreturn 0");
        let error=inspect(&invalid).expect_err("資源建立後原持有者失效");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("移動"));
    }

    #[test]
    fn 移動借用別名不得偷轉成所有者() {
        let src="fn main()->int{\nconst source:int = 1\nconst alias:int = source.borrow()\nvar moved:int = alias.move()\nreturn 0\n}\n";
        let error=inspect(src).expect_err("借用別名不得移動");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }

    #[test]
    fn 有效借用未歸還前不得移動原持有者() {
        let src="fn main()->int{\nconst source:int = 1\nconst alias:int = source.borrow()\nvar moved:int = source.move()\nreturn alias\n}\n";
        let error=inspect(src).expect_err("原持有者借用有效期間禁止 move");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }

    #[test]
    fn 結構單欄位移動即使資源型別也不被接受() {
        let src="struct Box{\nvar content:str = \"文字\"\n}\nfn main()->int{\nvar box:Box = Box()\nvar moved:str = box.content.move()\nreturn 0\n}\n";
        let error=inspect(src).expect_err("S21 禁止部分欄位移動");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("S21"));
    }
}

#[cfg(test)]
mod p13_非_copyable_方法參數與回傳移交 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn str_可經明確整筆_move_交給方法參數() {
        let src="struct Holder{\nvar count:int = 1\nfn consume(text:str)->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar source:str = \"資源\"\nconst holder:Holder = Holder()\nreturn holder.consume(source.move())\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 非_copyable_方法參數禁止由具名來源隱含複製() {
        let src="struct Holder{\nvar count:int = 1\nfn consume(text:str)->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar source:str = \"資源\"\nconst holder:Holder = Holder()\nreturn holder.consume(source)\n}\n";
        let error=inspect(src).expect_err("非 Copyable 參數不可隱含複製");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }

    #[test]
    fn 方法可將取得的_str_參數再次明確轉交回傳值() {
        let src="struct Holder{\nvar count:int = 1\nfn relay(text:str)->str{\nreturn text.move()\n}\n}\nfn main()->int{\nvar source:str = \"資源\"\nconst holder:Holder = Holder()\nvar result:str = holder.relay(source.move())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 方法不能藉一般名稱回傳_str_參數形成隱含複製() {
        let src="struct Holder{\nvar count:int = 1\nfn invalid(text:str)->str{\nreturn text\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("資源參數不可隱含複製回傳");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }

    #[test]
    fn 方法仍禁止從_self_資源欄位明確偷移動() {
        let src="struct Holder{\nvar text:str = \"資源\"\nfn invalid()->str{\nreturn self.text.move()\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("S21 禁止個別移動結構欄位");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("S21"));
    }

    #[test]
    fn str_方法參數可由新的字串常值直接建立獨立所有權() {
        let src="struct Holder{\nvar count:int = 1\nfn consume(text:str)->int{\nreturn self.count\n}\n}\nfn main()->int{\nconst holder:Holder = Holder()\nreturn holder.consume(\"新資源\")\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_新建資源函式位置引數 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 獨立字串常值可直接移交非_copyable_函式參數() {
        let src = "fn use_it(item:str)->int{\nreturn 1\n}\nfn main()->int{\nreturn use_it(\"新建資源\")\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 巢狀呼叫中新建字串可移交且不隱含複製既有來源() {
        let src = "fn consume(item:str)->int{\nreturn 1\n}\nfn relay(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn relay(consume(\"獨立資源\"))\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 既有字串名稱作為參數仍禁止隱含複製() {
        let src = "fn consume(item:str)->int{\nreturn 1\n}\nfn main()->int{\nvar item:str = \"原有資源\"\nreturn consume(item)\n}\n";
        let error=inspect(src).expect_err("不得將新值例外擴張到名稱隱含複製");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("非 Copyable"));
    }

    #[test]
    fn 明確移動既有字串仍合法() {
        let src = "fn consume(item:str)->int{\nreturn 1\n}\nfn main()->int{\nvar item:str = \"原有資源\"\nreturn consume(item.move())\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_2_一般賦值正式來源閉包 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn int_bool_float_名稱複製與一般重新賦值均保持來源有效() {
        let src="fn main()->int{\nvar a:int = 4\nvar b:int = a\nb = a\nvar flag:bool = true\nvar another:bool = flag\nanother = flag\nvar measure:float = 1.0\nvar measure_copy:float = measure\nmeasure_copy = measure\nreturn b\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn copyable_巢狀結構可以複製給不同變數及覆寫並讀取欄位() {
        let src="struct Inner{\nvar count:int = 1\n}\nstruct Outer{\nvar inner:Inner = Inner()\n}\nfn main()->int{\nvar first:Outer = Outer()\nvar second:Outer = first\nsecond = first\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn copyable_函式參數與回傳以及方法參數可重用同一來源() {
        let src="struct Boxed{\nvar count:int = 1\nfn choose(value:int)->int{\nreturn value\n}\n}\nfn echo(value:int)->int{\nreturn value\n}\nfn main()->int{\nvar amount:int = 3\nvar receiver:Boxed = Boxed()\nvar first:int = echo(amount)\nvar second:int = receiver.choose(amount)\nreturn first + second + amount\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 已繫結_str_不能經一般宣告複製() {
        let src="fn main()->int{\nvar source:str = \"字串\"\nvar duplicate:str = source\nreturn 0\n}\n";
        let err=inspect(src).expect_err("已存在資源不能隱含複製");
        assert_eq!(err.kind,DiagnosticKind::Static);
        assert!(err.message.contains("Copyable"));
    }

    #[test]
    fn 已繫結_str_不能經一般賦值隱含複製() {
        let src="fn main()->int{\nvar source:str = \"原始\"\nvar target:str = \"目標\"\ntarget = source\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn str_獨立新建值可合法重新賦值且不偷渡隱含移動() {
        let src="fn main()->int{\nvar source:str = \"原始\"\nsource = \"新值\"\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 未初始化_copyable_值依然不能複製() {
        let src="fn main()->int{\nvar source:int\nvar duplicate:int = source\nreturn duplicate\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p13_4_分支部分結構安全合流 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 不同分支各完成不同_copyable_欄位後可安全補完() {
        let src = "struct Pair{\nvar first:int\nvar second:int\n}\nfn main()->int{\nvar pair:Pair = Pair()\nif(true){\npair.first = 1\n}else{\npair.second = 2\n}\npair.first = 3\npair.second = 4\nvar copy:Pair = pair\nreturn 0\n}\n";
        assert!(inspect(src).is_ok(), "所有可能覆寫的欄位皆 Copyable，允許聯集補完");
    }

    #[test]
    fn 差異欄位包含資源時仍需保守拒絕未證明的舊值清理() {
        let src = "struct Pair{\nvar text:str\nvar number:int\n}\nfn main()->int{\nvar pair:Pair = Pair()\nif(true){\npair.text = \"a\"\n}else{\npair.number = 1\n}\npair.text = \"b\"\npair.number = 2\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind, DiagnosticKind::Static);
    }

    #[test]
    fn 某分支移動整個結構而另一分支未移動時不能靠欄位寫入復活() {
        let src = "struct Pair{\nvar first:int\nvar second:int\n}\nfn main()->int{\nvar pair:Pair = Pair()\npair.first = 1\npair.second = 2\nif(true){\nvar taken:Pair = pair.move()\n}\npair.first = 3\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind, DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p13_6_方法獨立資源回傳 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 唯讀方法可回傳一般函式新建的獨立_str_資源() {
        let src="fn fresh()->str{\nreturn \"獨立資源\"\n}\nstruct Holder{\nvar count:int = 1\nfn acquire()->str{\nreturn fresh()\n}\n}\nfn main()->int{\nconst holder:Holder = Holder()\nvar result:str = holder.acquire()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 方法可將純_copyable_參數計算交給新建資源函式() {
        let src="fn fresh(code:int)->str{\nreturn \"獨立資源\"\n}\nstruct Holder{\nvar count:int = 1\nfn acquire(increment:int)->str{\nreturn fresh(self.count + increment)\n}\n}\nfn main()->int{\nconst holder:Holder = Holder()\nvar result:str = holder.acquire(3)\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 方法不得將已繫結_str_當成純值引數偷偷複製() {
        let src="fn passthrough(value:str)->str{\nreturn value.move()\n}\nstruct Holder{\nvar count:int = 1\nfn bad(value:str)->str{\nreturn passthrough(value)\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let err=inspect(src).expect_err("方法新建資源的引數不得隱含複製 str");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("Copyable"));
    }
}
