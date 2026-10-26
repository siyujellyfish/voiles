//! P13 受限結構體所有權前端：真正 .voil → 私有 AST → 型別 → 所有權狀態。
//! 只接受可以完整驗證的子集，未支援的語法拒絕，不影響 MVP-A check_file。
//! 欄位型別 Copyable 由呼叫端提供，不自行宣告任何完整語言型別的資格。
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use crate::{Diagnostic, DiagnosticKind};
use crate::lexer::{self, Token, TokenKind};
use crate::ownership_model::{CopyPermission, ModelError, ValueSlot};
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
    fn term(&mut self) -> Result<Term, Diagnostic> {
        let start = self.tok().start;
        let (kind, mut end) = match self.tok().kind.clone() {
            TokenKind::Integer(_) => {
                let end = self.tok().end; self.cursor += 1;
                (TermKind::Literal("int"), end)
            }
            TokenKind::Decimal(_) => {
                let end = self.tok().end; self.cursor += 1;
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
                    } else {
                        return Err(self.syntax("此子集尚不接受一般欄位讀取或方法呼叫"));
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
    ty: String,
    mutable: bool,
    value: ValueSlot,
}

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
    fn evaluate(&self, term: &Term, vars: &mut HashMap<String, Binding>)
        -> Result<(String, bool), Diagnostic> {
        match &term.kind {
            TermKind::Literal(ty) => Ok(((*ty).to_string(), true)),
            TermKind::Name(name) => {
                let binding = vars.get(name)
                    .ok_or_else(|| self.error("找不到合法的來源宣告", term))?;
                binding.value.duplicate().map_err(|err| self.model_error(err, term))?;
                Ok((binding.ty.clone(), true))
            }
            TermKind::Move(name) => {
                if name == "self" {
                    return Err(self.error("S15：方法不可消耗或移動整個 self", term));
                }
                let binding = vars.get_mut(name)
                    .ok_or_else(|| self.error("找不到可移動的具名來源", term))?;
                binding.value.move_whole().map_err(|err| self.model_error(err, term))?;
                Ok((binding.ty.clone(), true))
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
    fn statements(&self, body: &[Stmt], vars: &mut HashMap<String, Binding>,
        expected_return: &str) -> Result<bool, Diagnostic> {
        for statement in body {
            match &statement.kind {
                StmtKind::Declare { name, ty, mutable, init } => {
                    if vars.contains_key(name) {
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
                    vars.insert(name.clone(), Binding {
                        ty: ty.clone(), mutable: *mutable, value: slot
                    });
                }
                StmtKind::Assign { name, value } => {
                    let expected = vars.get(name).ok_or_else(|| self.error(
                        "無法賦值給尚未宣告的名稱", value))?;
                    if !expected.mutable { return Err(self.error("const 不可重新賦值", value)); }
                    let ty = expected.ty.clone();
                    let (actual, ready) = self.evaluate(value, vars)?;
                    if actual != ty || !ready {
                        return Err(self.error("賦值型別錯誤或來源尚未完成初始化", value));
                    }
                    vars.get_mut(name).unwrap().value.mark_reinitialized();
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
                    let live_true = self.statements(yes, &mut on_true, expected_return)?;
                    let mut on_false = snapshot.clone();
                    let live_false = if let Some(otherwise) = no {
                        self.statements(otherwise, &mut on_false, expected_return)?
                    } else { true };
                    if !live_true && !live_false { return Ok(false); }
                    // 僅合流分支進入前可見的真正名稱。分支私有宣告不得洩漏。
                    for (name, original) in vars.iter_mut() {
                        let left = if live_true {
                            Some(on_true.get(name).expect("原來源必須存在").value)
                        } else { None };
                        let right = if live_false {
                            Some(on_false.get(name).expect("原來源必須存在").value)
                        } else { None };
                        original.value = ValueSlot::join_reachable(&[left, right])
                            .map_err(|err| self.model_error(err, condition))?;
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
        let mut vars = HashMap::new();
        self.statements(&fun.body, &mut vars, &fun.result_ty)?;
        Ok(())
    }
    fn all(&self, funcs: &[Function]) -> Result<(), Diagnostic> {
        for definition in self.structs.values() {
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
                    let (ty, _) = self.evaluate(default, &mut HashMap::new())?;
                    if ty != field.ty {
                        return Err(self.error("結構預設欄位型別不一致", default));
                    }
                }
                // immutable 僅保存 AST，不在此決定後續 S18 可變借用。
                let _ = field.immutable;
            }
            for method in &definition.methods {
                for stmt in &method.body {
                    let term = match &stmt.kind {
                        StmtKind::Declare { init, .. } => init.as_ref(),
                        StmtKind::Assign { value, .. } => Some(value),
                        StmtKind::Return(value) => Some(value),
                        StmtKind::If { condition, .. } => Some(condition),
                    };
                    if let Some(term) = term {
                        if matches!(&term.kind, TermKind::Move(name) if name == "self") {
                            let _ = struct_ownership::check_method_self(SelfAction::ConsumeWhole);
                            return Err(self.error("S15：方法禁止消耗整個 self", term));
                        }
                        if matches!(term.kind, TermKind::FieldMove(_, _)) {
                            return Err(self.error("S21：方法不得單獨移動欄位", term));
                        }
                    }
                }
                // 方法其餘語意需要 P09-F02/P14；不把純形狀測試當作方法已驗收。
                return Err(Diagnostic { kind: DiagnosticKind::Static,
                    message: "含方法的 struct 尚待完整方法借用與名稱／型別檢查".into(),
                    span: None });
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
pub(crate) fn inspect(text: &str, ability: impl Fn(&str) -> CopyPermission)
    -> Result<(), Diagnostic> {
    let source = Source::new(text.as_bytes().to_vec());
    if let Some(error) = source.lexical_diagnostics().into_iter().next() { return Err(error); }
    let tokens = lexer::scan(&source);
    if let Some(error) = tokens.diagnostics.into_iter().next() { return Err(error); }
    let (structs, funcs) = Parser { source: &source, tokens: &tokens.tokens, cursor: 0 }.file()?;
    Checker { source: &source, structs: &structs, ability }.all(&funcs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ownership_model::CopyPermission::{Allowed, Forbidden, Undecided};

    const BASE: &str = "struct Sample{\nconst flag:bool = true\nvar number:int = 1\n}\nfn main()->int{\nvar a:Sample = Sample()\nvar b:Sample = a.move()\nreturn 0\n}\n";

    #[test]
    fn 真正_struct_ast_具名整筆移動_來源失效() {
        assert!(inspect(BASE, |_| Allowed).is_ok());
        let invalid = BASE.replace("return 0", "var c:Sample = a\nreturn 0");
        let err = inspect(&invalid, |_| Allowed).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("移動"));
    }

    #[test]
    fn 真正_struct_ast_整體複製資格由欄位逐一核對() {
        let source = BASE.replace("a.move()", "a");
        assert!(inspect(&source, |_| Allowed).is_ok());
        for cap in [Forbidden, Undecided] {
            let err = inspect(&source, |name| if name == "bool" { cap } else { Allowed })
                .unwrap_err();
            assert_eq!(err.kind, DiagnosticKind::Static);
            assert!(err.message.contains("複製") || err.message.contains("Copyable"));
        }
        // 即使複製資格未定，明確整筆移動亦不等於隱含複製。
        assert!(inspect(BASE, |_| Undecided).is_ok());
    }

    #[test]
    fn 真正_struct_ast_移動來源重新初始化後可再用() {
        let source = BASE.replace("return 0", "a = Sample()\nvar c:Sample = a.move()\nreturn 0");
        assert!(inspect(&source, |_| Undecided).is_ok());
    }

    #[test]
    fn 真正_struct_ast_拒絕欄位移動與方法消耗_self() {
        let bad_field = BASE.replace("a.move()", "a.number.move()");
        assert!(inspect(&bad_field, |_| Allowed).unwrap_err().message.contains("S21"));
        let bad_method = "struct S{\nvar n:int = 1\nfn consume()->int{\nreturn self.move()\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(bad_method, |_| Allowed).unwrap_err().message.contains("S15"));
    }

    #[test]
    fn 真正_struct_ast_缺失欄位初始化不能假裝資源完整() {
        let source = "struct S{\nvar missing:int\nconst flag:bool = true\n}\nfn main()->int{\nvar a:S = S()\nvar b:S = a.move()\nreturn 0\n}\n";
        assert!(inspect(source, |_| Allowed).is_err());
    }

    #[test]
    fn 真正_struct_ast_建立式具名欄位及型別() {
        let source = BASE.replace("Sample()", "Sample(number=3,flag=false,)");
        assert!(inspect(&source, |_| Allowed).is_ok());
        for bad in ["Sample(extra=1)", "Sample(number=false)", "Sample(number=1,number=2)"] {
            assert!(inspect(&BASE.replace("Sample()", bad), |_| Allowed).is_err());
        }
    }

    #[test]
    fn 不得把限制外的控制流程直接當成已驗證() {
        let source = BASE.replace("return 0", "if (true){\nreturn 0\n}\nreturn 0");
        assert!(inspect(&source, |_| Allowed).is_err());
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
            assert!(inspect(source, |_| Undecided).is_ok(), "合法整筆移動案例");
        }
        for source in BAD {
            let error = inspect(source, |_| Allowed).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::Static);
        }
        const UNKNOWN_COPY: &str =
            include_str!("../tests/fixtures/p13_stage/未知複製資格.voil");
        assert!(inspect(UNKNOWN_COPY, |_| Undecided).is_err());
        assert!(inspect(UNKNOWN_COPY, |_| Allowed).is_ok());
    }

    #[test]
    fn 不存在的結構欄位型別不得由假設能力誤認() {
        let src = "struct Sample{\nvar mystery:Unknown\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src, |_| Allowed).is_err());
    }

}
