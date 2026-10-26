//! P13 受限結構體所有權前端：真正 .voil → 私有 AST → 型別 → 所有權狀態。
//! 只接受可以完整驗證的子集，未支援的語法拒絕，不影響 MVP-A check_file。
//! 欄位型別 Copyable 由呼叫端提供，不自行宣告任何完整語言型別的資格。
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use crate::{Diagnostic, DiagnosticKind};
use crate::lexer::{self, Token, TokenKind};
use crate::ownership_model::{CopyPermission, ModelError, ValueSlot};
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
    result_ty: String,
    body: Vec<Stmt>,
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
    Construct(String, Vec<(String, Term)>),
    Move(String),
    FieldMove(String, String),
    /// 只讀欄位值的複製；需另外通過已初始化與 HA13/S19 Copyable 檢查。
    FieldRead(String, String),
    /// S16 已核准的 instance.method() 零參數子集；此處不推定其他簽章。
    MethodCall(String, String),
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
    fn term(&mut self) -> Result<Term, Diagnostic> {
        let start = self.tok().start;
        let (kind, mut end) = match self.tok().kind.clone() {
            TokenKind::Integer(text) => {
                if !Self::valid_integer(&text) {
                    return Err(self.syntax("整數字面值無效或超出有號 64 位元範圍"));
                }
                let end = self.tok().end; self.cursor += 1;
                (TermKind::Literal("int"), end)
            }
            TokenKind::Decimal(_) => {
                return Err(self.syntax(
                    "此 P13 受限檢查尚未完成 decimal64 字面值合法性驗證"
                ));
            }
            TokenKind::StringLiteral(_) => {
                let end = self.tok().end; self.cursor += 1;
                (TermKind::Literal("str"), end)
            }
            TokenKind::Keyword(s) if s == "true" || s == "false" => {
                let end = self.tok().end; self.cursor += 1;
                (TermKind::Literal("bool"), end)
            }
            TokenKind::Name(s) => {
                let end = self.tok().end; self.cursor += 1;
                if self.symbol('(') {
                    self.cursor += 1;
                    let mut args = Vec::new();
                    let mut names = HashSet::new();
                    while !self.symbol(')') {
                        let field = self.name()?;
                        if !names.insert(field.clone()) {
                            return Err(self.syntax("建立結構時不得重複指定同一欄位"));
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
                } else if self.symbol('.') {
                    self.cursor += 1;
                    let field_or_move = self.name()?;
                    if field_or_move == "move" && self.symbol('(') {
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
                        self.take(')', "此受限子集方法呼叫只接受空引數列表")?;
                        (TermKind::MethodCall(s, field_or_move),
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
            self.cursor += 1;
            self.take('(', "if 條件需要左括號")?;
            let condition = self.term()?;
            self.take(')', "if 條件需要右括號")?;
            let yes = self.body()?;
            let saved = self.cursor;
            self.lines();
            let no = if self.keyword("else") {
                self.cursor += 1;
                Some(self.body()?)
            } else {
                self.cursor = saved;
                None
            };
            StmtKind::If { condition, yes, no }
        } else if self.keyword("return") {
            self.cursor += 1;
            StmtKind::Return(self.term()?)
        } else {
            let name = self.name()?;
            self.take('=', "此階段具名語句必須為賦值")?;
            let value = self.term()?;
            StmtKind::Assign { name, value }
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
        self.take(')', "受限驗證子集僅接受零參數函式")?;
        if !matches!(self.tok().kind, TokenKind::Arrow) {
            return Err(self.syntax("函式需要回傳型別標註"));
        }
        self.cursor += 1;
        let result_ty = self.name()?;
        let body = self.body()?;
        Ok(Function { name, result_ty, body })
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
struct Binding {
    declaration_start: usize,
    ty: String,
    mutable: bool,
    value: ValueSlot,
}

/// 各大括號區塊都有獨立詞彙作用域；內層同名宣告不得覆蓋外層所有權來源。
type Scopes = Vec<HashMap<String, Binding>>;

struct Checker<'a, C> {
    source: &'a Source,
    structs: &'a HashMap<String, StructDecl>,
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
    fn lookup<'b>(vars: &'b Scopes, name: &str) -> Option<&'b Binding> {
        vars.iter().rev().find_map(|scope| scope.get(name))
    }
    fn lookup_mut<'b>(vars: &'b mut Scopes, name: &str) -> Option<&'b mut Binding> {
        vars.iter_mut().rev().find_map(|scope| scope.get_mut(name))
    }

    /// 這裡僅核准完全唯讀且可靜態證明安全的 self 方法子集。
    fn check_readonly_method(&self, owner: &str, method: &Function)
        -> Result<(), Diagnostic> {
        self.reject_method_statements(&method.body)?;
        if method.body.len() != 1 {
            return Err(Diagnostic {kind: DiagnosticKind::Static,
                message: "目前只能驗證單一 return 的唯讀 self 方法".into(), span: None});
        }
        let StmtKind::Return(term) = &method.body[0].kind else {
            return Err(Diagnostic {kind: DiagnosticKind::Static,
                message: "唯讀 self 方法必須只有 return".into(), span: None});
        };
        let actual = match &term.kind {
            TermKind::Literal(ty) => *ty,
            TermKind::FieldRead(receiver, name) if receiver == "self" => {
                let field = self.structs.get(owner)
                    .and_then(|def| def.fields.iter().find(|f| f.name == *name))
                    .ok_or_else(|| self.error("self 欄位不存在", term))?;
                if self.permission(&field.ty) != CopyPermission::Allowed {
                    return Err(self.error(
                        "self 欄位 Copyable 能力未核准，不能從共享借用複製回傳", term));
                }
                field.ty.as_str()
            }
            _ => return Err(self.error(
                "此階段方法只支援唯讀 self.欄位或常值回傳，其他借用尚未驗證", term)),
        };
        if actual != method.result_ty {
            return Err(self.error("方法回傳型別與已驗證運算式不一致", term));
        }
        Ok(())
    }

    fn evaluate(&self, term: &Term, vars: &mut Scopes)
        -> Result<(String, bool), Diagnostic> {
        match &term.kind {
            TermKind::Literal(ty) => Ok(((*ty).to_string(), true)),
            TermKind::Name(name) => {
                let binding = Self::lookup(vars, name)
                    .ok_or_else(|| self.error("找不到合法的來源宣告", term))?;
                binding.value.duplicate().map_err(|err| self.model_error(err, term))?;
                Ok((binding.ty.clone(), true))
            }
            TermKind::Move(name) => {
                if name == "self" {
                    return Err(self.error("S15：方法不可消耗或移動整個 self", term));
                }
                let binding = Self::lookup_mut(vars, name)
                    .ok_or_else(|| self.error("找不到可移動的具名來源", term))?;
                binding.value.move_whole().map_err(|err| self.model_error(err, term))?;
                Ok((binding.ty.clone(), true))
            }
            TermKind::FieldRead(owner, name) => {
                if owner == "self" {
                    return Err(self.error("只有方法內才能讀取 self 欄位", term));
                }
                let bound = Self::lookup(vars, owner)
                    .ok_or_else(|| self.error("找不到欄位來源宣告", term))?;
                bound.value.read().map_err(|err| self.model_error(err, term))?;
                let field = self.structs.get(&bound.ty)
                    .and_then(|def| def.fields.iter().find(|f| f.name == *name))
                    .ok_or_else(|| self.error("來源不是具有此欄位的結構", term))?;
                if self.permission(&field.ty) != CopyPermission::Allowed {
                    return Err(self.error("欄位 Copyable 能力尚未核准，不可直接複製", term));
                }
                Ok((field.ty.clone(), true))
            }
            TermKind::MethodCall(receiver, name) => {
                if receiver == "self" {
                    return Err(self.error("重入 self 方法的借用安全尚未驗證", term));
                }
                let bound = Self::lookup(vars, receiver)
                    .ok_or_else(|| self.error("找不到方法接收者來源", term))?;
                bound.value.read().map_err(|err| self.model_error(err, term))?;
                let method = self.structs.get(&bound.ty)
                    .and_then(|def| def.methods.iter().find(|m| m.name == *name))
                    .ok_or_else(|| self.error("找不到結構上的合法方法", term))?;
                self.check_readonly_method(&bound.ty, method)?;
                let mut loans = BorrowSafety::new();
                let loan = loans.acquire(DeclarationId(bound.declaration_start),
                    BorrowKind::SharedRead)
                    .map_err(|_| self.error("方法 self 借用與現有借用衝突", term))?;
                // 唯讀無重入子集，不進行來源操作；方法結束後歸還 self 借用。
                loans.finish_scope(&[loan])
                    .map_err(|_| self.error("方法 self 借用未安全歸還", term))?;
                Ok((method.result_ty.clone(), true))
            }
            TermKind::FieldMove(_owner, _field) => {
                let _ = struct_ownership::check_struct_move_target(MoveTarget::SingleField);
                Err(self.error("S21：結構欄位不能單獨移動", term))
            }
            TermKind::Construct(name, args) => {
                let definition = self.structs.get(name)
                    .ok_or_else(|| self.error("找不到結構型別或不支援一般函式呼叫", term))?;
                let mut written = HashSet::new();
                for (field, expr) in args {
                    let expected = definition.fields.iter().find(|f| f.name == *field)
                        .ok_or_else(|| self.error("建立結構時指定了不存在的欄位", term))?;
                    if !written.insert(field.as_str()) {
                        return Err(self.error("建立結構時不得重複指定欄位", term));
                    }
                    let (actual, ready) = self.evaluate(expr, vars)?;
                    if actual != expected.ty || !ready {
                        return Err(self.error("結構具名欄位初值型別不一致或尚未初始化", expr));
                    }
                }
                // const 欄位依法必有預設；未給初值的 var 欄位可延後初始化，
                // 但本保守子集不允許把尚未全部初始化的實例複製／移動。
                let complete = definition.fields.iter()
                    .all(|field| written.contains(field.name.as_str()) || field.default.is_some());
                Ok((name.clone(), complete))
            }
        }
    }
    fn model_error(&self, err: ModelError, term: &Term) -> Diagnostic {
        let message = match err {
            ModelError::MovedRead => "來源已移動，禁止再次使用",
            ModelError::UninitializedRead => "來源尚未完成初始化，不可使用",
            ModelError::UncertainRead => "存在可達路徑使來源可能無效",
            ModelError::CopyForbidden => "型別不允許預設複製",
            ModelError::CopyUndecided => "型別 Copyable 能力未核准，拒絕預設複製",
            _ => "所有權狀態無法獲得合法證明",
        };
        self.error(message, term)
    }
    /// true 表示分支仍會繼續執行；false 表示所有目前路徑已 return。
    fn statements(&self, body: &[Stmt], vars: &mut Scopes,
        expected_return: &str, new_scope: bool) -> Result<bool, Diagnostic> {
        if new_scope { vars.push(HashMap::new()); }
        let result = self.statement_body(body, vars, expected_return);
        if new_scope { vars.pop(); }
        result
    }

    fn statement_body(&self, body: &[Stmt], vars: &mut Scopes,
        expected_return: &str) -> Result<bool, Diagnostic> {
        for statement in body {
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
                    let initialized = if let Some(expr) = init {
                        let (actual, ready) = self.evaluate(expr, vars)?;
                        if &actual != ty {
                            return Err(self.error("初始化運算式與宣告型別不一致", expr));
                        }
                        ready
                    } else { false };
                    let permission = self.permission(ty);
                    let slot = if initialized {
                        ValueSlot::initialized(permission)
                    } else {
                        ValueSlot::uninitialized(permission)
                    };
                    vars.last_mut().expect("應有詞彙作用域").insert(name.clone(), Binding {
                        declaration_start: statement.start, ty: ty.clone(),
                        mutable: *mutable, value: slot
                    });
                }
                StmtKind::Assign { name, value } => {
                    let expected = Self::lookup(vars, name).ok_or_else(|| self.error(
                        "無法賦值給尚未宣告的名稱", value))?;
                    if !expected.mutable { return Err(self.error("const 不可重新賦值", value)); }
                    let ty = expected.ty.clone();
                    let (actual, ready) = self.evaluate(value, vars)?;
                    if actual != ty || !ready {
                        return Err(self.error("賦值型別錯誤或來源尚未完成初始化", value));
                    }
                    Self::lookup_mut(vars, name).expect("已確認來源").value.mark_reinitialized();
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
                    let mut on_true = snapshot.clone();
                    let live_true = self.statements(yes, &mut on_true, expected_return, true)?;
                    let mut on_false = snapshot.clone();
                    let live_false = if let Some(otherwise) = no {
                        self.statements(otherwise, &mut on_false, expected_return, true)?
                    } else { true };
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
                        }
                    }
                }
            }
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
            _ => Ok(()),
        }
    }

    fn reject_method_statements(&self, body: &[Stmt]) -> Result<(), Diagnostic> {
        for stmt in body {
            match &stmt.kind {
                StmtKind::Declare { init, .. } => {
                    if let Some(value) = init { self.reject_method_term(value)?; }
                }
                StmtKind::Assign { value, .. } | StmtKind::Return(value) => {
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
        if let TermKind::Construct(name, args) = &term.kind {
            if active.contains(name) {
                return Err(self.error("結構欄位預設建構形成循環，無法安全完成初始化", term));
            }
            self.check_default_type(name, active, checked)?;
            // 具名欄位實際求值時，也不得在巢狀建立引數中形成循環。
            for (_, value) in args {
                self.check_default_term(value, active, checked)?;
            }
        }
        Ok(())
    }

    fn all(&self, funcs: &[Function]) -> Result<(), Diagnostic> {
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
                    let (ty, _) = self.evaluate(default, &mut vec![HashMap::new()])?;
                    if ty != field.ty {
                        return Err(self.error("結構預設欄位型別不一致", default));
                    }
                }
                // immutable 僅保存 AST，不在此決定後續 S18 可變借用。
                let _ = field.immutable;
            }
            for method in &definition.methods {
                self.check_readonly_method(struct_name, method)?;
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
    Checker { source: &source, structs: &structs, ability }.all(&funcs)
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
        let source = BASE.replace("var number:int = 1", "var number:float = 1.25");
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
        let write = "struct A{\nvar value:int = 1\nfn change()->int{\nself.value = 2\nreturn 0\n}\n}\nfn main()->int{\nreturn 0\n}\n";
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
        assert!(error.message.contains("唯讀 self 方法"));
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
    fn ha13_str_資格未決_及含_str_欄位的_struct_都不可隱含複製() {
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
