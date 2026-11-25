//! 私有自舉編譯器的宿主 ABI。沿用正式 lexer，宿主提供有界的記號與輸出緩衝區。
use crate::lexer::{self, Token, TokenKind};
use crate::source::Source;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug)]
enum Fault { Syntax(String), Overflow, Conversion, Limit }
type R = Result<i64, Fault>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MatchKind { Integer, Boolean }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExceptionKind { Evaluation, Conversion }

// A nested call's result kind is determined by the declared signature, not
// by its position in the source. Literal and moved arguments are already known.
#[derive(Debug)]
enum FactoryArgumentKind { Int, Str, Call(String) }

struct Machine {
    tokens: Vec<Token>,
    cursor: usize,
    output: String,
    steps: usize,
    functions: BTreeMap<String, Vec<String>>,
    function_param_kinds: BTreeMap<String, Vec<String>>,
    function_return_kinds: BTreeMap<String, String>,
    str_factory_calls: Vec<(String, Vec<FactoryArgumentKind>, Option<String>)>,
    str_factory_depth: usize,
    current_function: Option<String>,
    calls: Vec<(Option<String>, String, usize, Vec<String>)>,
    call_stack: Vec<usize>,
    module: Option<String>,
    imports: BTreeSet<String>,
    allowed_imports: BTreeSet<String>,
    external_functions: BTreeMap<(String, String), usize>,
    structures: BTreeMap<String, Vec<(String, i64)>>,
    current_structure: Option<String>,
    pending_field: Option<String>,
    locals: BTreeMap<String, String>,
    pending_local: Option<(String, String)>,
    pending_constructor: Option<String>,
    constructor_fields: BTreeSet<String>,
    list_items: usize,
    nested_rows: usize,
    nested_items: usize,
    dict_items: usize,
    dict_keys: BTreeSet<String>,
    pending_list_borrows: Vec<(String, Option<String>)>,
    pending_dict_borrow: Option<(String, Option<String>)>,
    pending_list_append: Option<String>,
    pending_dict_set: Option<(String, Option<String>)>,
    pending_matches: Vec<(Option<MatchKind>, BTreeSet<i64>, bool, usize, BTreeMap<String, String>)>,
    pending_if_scopes: Vec<(bool, BTreeMap<String, String>, Option<BTreeMap<String, String>>)>,
    pending_tries: Vec<(bool, Option<ExceptionKind>, Option<(String, Option<String>)>, BTreeMap<String, String>)>,
}

impl Machine {
    fn enter(&mut self, depth: usize) -> Result<(), Fault> {
        self.steps += 1;
        if depth >= 192 || self.steps > 400_000 {
            return Err(Fault::Limit);
        }
        Ok(())
    }
    fn token(&self) -> &TokenKind { &self.tokens[self.cursor].kind }
    fn matched_width(&self, text: &str) -> usize {
        if matches!(text, "!=" | "==" | "<=" | ">=") {
            let first = &self.tokens[self.cursor];
            let Some(second) = self.tokens.get(self.cursor + 1) else {return 0};
            return if first.end == second.start
                && first.kind == TokenKind::Symbol(char::from(text.as_bytes()[0]))
                && second.kind == TokenKind::Symbol('=') {2} else {0};
        }
        usize::from(match self.token() {
            TokenKind::Keyword(n) | TokenKind::Name(n) => n == text,
            TokenKind::Symbol(c) => text.len() == c.len_utf8() && text.starts_with(*c),
            TokenKind::Arrow => text == "->",
            _ => false,
        })
    }
    fn at(&self, text: &str) -> R { Ok(i64::from(self.matched_width(text) != 0)) }
    fn at_end(&self) -> R { Ok(i64::from(matches!(self.token(), TokenKind::End))) }
    fn at_integer(&self) -> R { Ok(i64::from(matches!(self.token(), TokenKind::Integer(_)))) }
    fn at_str_local(&self) -> R {
        let mutable = self.matched_width("var") != 0;
        let declarator = mutable || self.matched_width("const") != 0;
        let head = declarator
            && self.tokens.get(self.cursor + 1).is_some_and(|t| matches!(t.kind, TokenKind::Name(_)))
            && self.tokens.get(self.cursor + 2).is_some_and(|t| t.kind == TokenKind::Symbol(':'))
            && self.tokens.get(self.cursor + 3).is_some_and(|t|
                matches!(&t.kind, TokenKind::Name(n) | TokenKind::Keyword(n) if n == "str"))
            && self.tokens.get(self.cursor + 4).is_some_and(|t| t.kind == TokenKind::Symbol('='));
        let literal = self.tokens.get(self.cursor + 5)
            .is_some_and(|t| matches!(t.kind, TokenKind::StringLiteral(_)));
        let moved = declarator
            && self.tokens.get(self.cursor + 5).is_some_and(|t| matches!(t.kind, TokenKind::Name(_)))
            && self.tokens.get(self.cursor + 6).is_some_and(|t| t.kind == TokenKind::Symbol('.'))
            && self.tokens.get(self.cursor + 7).is_some_and(|t|
                matches!(&t.kind, TokenKind::Name(n) | TokenKind::Keyword(n) if n == "move"))
            && self.tokens.get(self.cursor + 8).is_some_and(|t| t.kind == TokenKind::Symbol('('))
            && self.tokens.get(self.cursor + 9).is_some_and(|t| t.kind == TokenKind::Symbol(')'));
        let factory_call = declarator
            && self.tokens.get(self.cursor + 5).is_some_and(|t| matches!(t.kind, TokenKind::Name(_)))
            && self.tokens.get(self.cursor + 6).is_some_and(|t| t.kind == TokenKind::Symbol('('));
        Ok(i64::from(head && (literal || moved || factory_call)))
    }
    fn at_str_assignment(&self) -> R {
        // 由已登錄的字串宣告辨識賦值語句；const 也進入此路徑以明確拒絕。
        Ok(i64::from(matches!(self.token(), TokenKind::Name(name)
                if matches!(self.locals.get(name).map(String::as_str), Some("const-owned:str" | "owned:str" | "moved:str" | "moved:const:str")))
            && self.tokens.get(self.cursor + 1)
                .is_some_and(|t| t.kind == TokenKind::Symbol('='))))
    }
    fn at_conversion(&self) -> R {
        Ok(i64::from(matches!(self.token(), TokenKind::Name(ref name)
                if matches!(self.locals.get(name).map(String::as_str), Some("const-owned:str" | "owned:str")))
            && self.tokens.get(self.cursor + 1).is_some_and(|t| t.kind == TokenKind::Symbol('.'))
            && self.tokens.get(self.cursor + 2).is_some_and(|t| matches!(&t.kind, TokenKind::Name(n) if n == "to_int"))
            && self.tokens.get(self.cursor + 3).is_some_and(|t| t.kind == TokenKind::Symbol('('))
            && self.tokens.get(self.cursor + 4).is_some_and(|t| t.kind == TokenKind::Symbol(')'))))
    }
    fn at_call(&self) -> R {
        Ok(i64::from(matches!(self.token(), TokenKind::Name(_))
            && self.tokens.get(self.cursor + 1).is_some_and(|token|
                token.kind == TokenKind::Symbol('('))))
    }
    fn at_module_call(&self) -> R {
        Ok(i64::from(matches!(self.token(), TokenKind::Name(_))
            && self.tokens.get(self.cursor + 1).is_some_and(|t| t.kind == TokenKind::Symbol('.'))
            && self.tokens.get(self.cursor + 2).is_some_and(|t| matches!(t.kind, TokenKind::Name(_)))
            && self.tokens.get(self.cursor + 3).is_some_and(|t| t.kind == TokenKind::Symbol('('))))
    }
    fn at_field(&self) -> R {
        Ok(i64::from(matches!(self.token(), TokenKind::Name(_))
            && self.tokens.get(self.cursor + 1).is_some_and(|t| t.kind == TokenKind::Symbol('.'))
            && self.tokens.get(self.cursor + 2).is_some_and(|t| matches!(t.kind, TokenKind::Name(_)))
            && !self.tokens.get(self.cursor + 3).is_some_and(|t| t.kind == TokenKind::Symbol('('))))
    }
    fn at_field_write(&self) -> R {
        Ok(i64::from(matches!(self.token(), TokenKind::Name(_))
            && self.tokens.get(self.cursor + 1).is_some_and(|t| t.kind == TokenKind::Symbol('.'))
            && self.tokens.get(self.cursor + 2).is_some_and(|t| matches!(t.kind, TokenKind::Name(_)))
            && self.tokens.get(self.cursor + 3).is_some_and(|t| t.kind == TokenKind::Symbol('='))))
    }
    fn at_list_local(&self) -> R {
        Ok(i64::from(matches!(self.token(), TokenKind::Keyword(k) if k == "var")
            && self.tokens.get(self.cursor + 1).is_some_and(|t| matches!(t.kind, TokenKind::Name(_)))
            && self.tokens.get(self.cursor + 2).is_some_and(|t| t.kind == TokenKind::Symbol(':'))
            && self.tokens.get(self.cursor + 3).is_some_and(|t| matches!(&t.kind, TokenKind::Name(k) | TokenKind::Keyword(k) if k == "list"))))
    }
    fn at_nested_list_local(&self) -> R {
        Ok(i64::from(matches!(self.token(), TokenKind::Keyword(k) if k == "var")
            && self.tokens.get(self.cursor + 1).is_some_and(|t| matches!(t.kind, TokenKind::Name(_)))
            && self.tokens.get(self.cursor + 2).is_some_and(|t| t.kind == TokenKind::Symbol(':'))
            && self.tokens.get(self.cursor + 3).is_some_and(|t| matches!(&t.kind, TokenKind::Name(k) | TokenKind::Keyword(k) if k == "list"))
            && self.tokens.get(self.cursor + 4).is_some_and(|t| t.kind == TokenKind::Symbol('['))
            && self.tokens.get(self.cursor + 5).is_some_and(|t| matches!(&t.kind, TokenKind::Name(k) | TokenKind::Keyword(k) if k == "list"))))
    }
    fn at_nested_dict_local(&self) -> R {
        Ok(i64::from(matches!(self.token(), TokenKind::Keyword(k) if k == "var")
            && self.tokens.get(self.cursor + 1).is_some_and(|t| matches!(t.kind, TokenKind::Name(_)))
            && self.tokens.get(self.cursor + 2).is_some_and(|t| t.kind == TokenKind::Symbol(':'))
            && self.tokens.get(self.cursor + 3).is_some_and(|t| matches!(&t.kind, TokenKind::Name(k) | TokenKind::Keyword(k) if k == "dict"))
            && self.tokens.get(self.cursor + 4).is_some_and(|t| t.kind == TokenKind::Symbol('{'))
            && self.tokens.get(self.cursor + 5).is_some_and(|t| matches!(&t.kind, TokenKind::Name(k) | TokenKind::Keyword(k) if k == "str"))
            && self.tokens.get(self.cursor + 7).is_some_and(|t| matches!(&t.kind, TokenKind::Name(k) | TokenKind::Keyword(k) if k == "list"))))
    }
    fn at_dict_local(&self) -> R {
        Ok(i64::from(matches!(self.token(), TokenKind::Keyword(k) if k == "var")
            && self.tokens.get(self.cursor + 1).is_some_and(|t| matches!(t.kind, TokenKind::Name(_)))
            && self.tokens.get(self.cursor + 2).is_some_and(|t| t.kind == TokenKind::Symbol(':'))
            && self.tokens.get(self.cursor + 3).is_some_and(|t| matches!(&t.kind, TokenKind::Name(k) | TokenKind::Keyword(k) if k == "dict"))))
    }
    fn at_dict_borrow(&self) -> R {
        let Some(Token { kind: TokenKind::Name(owner), .. }) = self.tokens.get(self.cursor + 1) else {
            return Ok(0);
        };
        Ok(i64::from(self.matched_width("with") != 0
            && matches!(self.locals.get(owner).map(String::as_str),
                Some("dict:str:int" | "dict:str:list:int"))
            && self.tokens.get(self.cursor + 2).is_some_and(|t| t.kind == TokenKind::Symbol('['))
            && self.tokens.get(self.cursor + 3).is_some_and(|t| matches!(t.kind, TokenKind::StringLiteral(_)))
            && self.tokens.get(self.cursor + 4).is_some_and(|t| t.kind == TokenKind::Symbol(']'))))
    }
    fn at_len(&self) -> R {
        Ok(i64::from(self.matched_width("len") != 0
            && self.tokens.get(self.cursor + 1).is_some_and(|t| t.kind == TokenKind::Symbol('('))
            && self.tokens.get(self.cursor + 2).is_some_and(|t| matches!(t.kind, TokenKind::Name(_)))
            && self.tokens.get(self.cursor + 3).is_some_and(|t| t.kind == TokenKind::Symbol(')'))))
    }
    fn at_list_append(&self) -> R { self.at_list_method("append", false) }
    fn at_list_pop(&self) -> R { self.at_list_method("pop", true) }
    fn at_dict_set(&self) -> R { self.at_list_method("set", false) }
    fn at_list_method(&self, method: &str, closed: bool) -> R {
        let head = matches!(self.token(), TokenKind::Name(_))
            && self.tokens.get(self.cursor + 1).is_some_and(|t| t.kind == TokenKind::Symbol('.'))
            && self.tokens.get(self.cursor + 2).is_some_and(|t| matches!(&t.kind, TokenKind::Name(n) if n == method))
            && self.tokens.get(self.cursor + 3).is_some_and(|t| t.kind == TokenKind::Symbol('('));
        let close = !closed || self.tokens.get(self.cursor + 4).is_some_and(|t| t.kind == TokenKind::Symbol(')'));
        Ok(i64::from(head && close))
    }
    fn expect(&mut self, text: &str) -> R {
        let width = self.matched_width(text);
        if width == 0 { return Err(Fault::Syntax(format!("預期 {text}"))); }
        self.cursor += width;
        Ok(0)
    }
    fn emit(&mut self, text: &str) -> R {
        self.output.push_str(text);
        Ok(0)
    }
    fn name(&mut self) -> R {
        let name = self.take_name()?;
        self.emit_name(&name)
    }
    fn take_name(&mut self) -> Result<String, Fault> {
        let TokenKind::Name(name) = self.token() else {
            return Err(Fault::Syntax("預期函式名稱".into()));
        };
        let name = name.clone();
        self.cursor += 1;
        Ok(name)
    }
    fn emit_name(&mut self, name: &str) -> R {
        let ident = match &self.module {
            Some(module) => format!("v_{module}_{name}"),
            None => format!("v_{name}"),
        };
        self.emit(&ident)
    }
    fn struct_ident(&self, name: &str) -> String {
        self.module.as_ref().map_or_else(|| format!("S_{name}"), |module| format!("S_{module}_{name}"))
    }
    fn structure(&mut self) -> R {
        let name = self.take_name()?;
        if self.structures.contains_key(&name) { return Err(Fault::Syntax(format!("重複結構 {name}"))); }
        self.structures.insert(name.clone(), Vec::new());
        self.current_structure = Some(name.clone());
        let ident = self.struct_ident(&name);
        self.emit(&ident)
    }
    fn field(&mut self) -> R {
        let name = self.take_name()?;
        let Some(structure) = self.current_structure.as_ref() else {
            return Err(Fault::Syntax("欄位不在結構中".into()));
        };
        if self.structures.get(structure).expect("目前結構已登錄").iter().any(|(field, _)| field == &name)
            || self.pending_field.as_ref() == Some(&name) {
            return Err(Fault::Syntax(format!("重複欄位 {name}")));
        }
        self.pending_field = Some(name.clone());
        self.emit(&format!("f_{name}: i64"))
    }
    fn field_default(&mut self) -> R {
        let TokenKind::Integer(raw) = self.token() else { return Err(Fault::Syntax("int 欄位需要整數預設值".into())); };
        let value = crate::syntax_shared::integer_i64(raw)
            .map_err(|_| Fault::Syntax("整數常值無效".into()))?;
        self.cursor += 1;
        let field = self.pending_field.take().ok_or_else(|| Fault::Syntax("欄位預設值邊界無效".into()))?;
        let structure = self.current_structure.as_ref().expect("目前結構已登錄");
        self.structures.get_mut(structure).expect("目前結構已登錄").push((field, value));
        Ok(0)
    }
    fn finish_structure(&mut self) -> R {
        if self.pending_field.is_some() { return Err(Fault::Syntax("欄位缺少預設值".into())); }
        self.current_structure.take().ok_or_else(|| Fault::Syntax("結構邊界無效".into()))?;
        Ok(0)
    }
    fn module_import(&mut self) -> R {
        let name = self.take_name()?;
        if self.module.is_none() {
            if name != "host" { return Err(Fault::Syntax("種子來源只能匯入 host".into())); }
        } else if !self.allowed_imports.contains(&name) {
            return Err(Fault::Syntax(format!("未知模組 {name}")));
        }
        if !self.imports.insert(name.clone()) {
            return Err(Fault::Syntax(format!("重複匯入 {name}")));
        }
        Ok(0)
    }
    fn function(&mut self) -> R {
        let name = self.take_name()?;
        if self.functions.contains_key(&name) {
            return Err(Fault::Syntax(format!("重複函式 {name}")));
        }
        self.functions.insert(name.clone(), Vec::new());
        self.function_param_kinds.insert(name.clone(), Vec::new());
        self.function_return_kinds.insert(name.clone(), "int".into());
        self.current_function = Some(name.clone());
        self.emit_name(&name)
    }
    fn parameter(&mut self) -> R {
        let name = self.take_name()?;
        let Some(function) = self.current_function.as_ref() else {
            return Err(Fault::Syntax("參數不在函式中".into()));
        };
        let parameters = self.functions.get_mut(function).expect("目前函式已登錄");
        if parameters.contains(&name) {
            return Err(Fault::Syntax(format!("重複參數 {name}")));
        }
        // 在語法消耗 :int／:str 前取得參數型別，讓前向呼叫於 finish_names
        // 可核對完整簽章；第一批 str 參數為不可重新賦值的擁有 String。
        let kind = if self.tokens.get(self.cursor + 1).is_some_and(|t|
            matches!(&t.kind, TokenKind::Name(k) | TokenKind::Keyword(k) if k == "str")) {
            "str"
        } else { "int" };
        parameters.push(name.clone());
        self.function_param_kinds.get_mut(function).expect("函式參數型別已登錄")
            .push(kind.into());
        if kind == "str" {
            self.locals.insert(name.clone(), "const-owned:str".into());
        }
        self.emit_name(&name)
    }
    fn value(&mut self) -> R {
        let name = self.take_name()?;
        let parameter_kind = self.current_function.as_ref().and_then(|function| {
            self.functions.get(function)?.iter().position(|candidate| candidate == &name)
                .and_then(|index| self.function_param_kinds.get(function)?.get(index))
        }).map(String::as_str);
        let parameter = parameter_kind == Some("int");
        let local_int = self.locals.get(&name).is_some_and(|kind| kind == "int");
        if !parameter && !local_int {
            return Err(Fault::Syntax(format!("{name} 不是可用的 int 名稱")));
        }
        if parameter { self.emit_name(&name) } else { self.emit(&format!("v_{name}")) }
    }
    fn mark_str_return(&mut self) -> R {
        let name = self.current_function.as_ref()
            .ok_or_else(|| Fault::Syntax("str 返回型別不在函式中".into()))?;
        if name == "main" {
            return Err(Fault::Syntax("main 必須回傳 int".into()));
        }
        self.function_return_kinds.insert(name.clone(), "str".into());
        Ok(0)
    }
    fn is_str_return(&self) -> R {
        Ok(i64::from(self.current_function.as_ref()
            .and_then(|name| self.function_return_kinds.get(name))
            .is_some_and(|kind| kind == "str")))
    }
    fn str_return_statement(&mut self) -> R {
        if self.is_str_return()? == 0 {
            return Err(Fault::Syntax("此函式必須回傳 int".into()));
        }
        if !self.pending_if_scopes.is_empty() || !self.pending_matches.is_empty()
            || !self.pending_tries.is_empty() || !self.pending_list_borrows.is_empty()
            || self.pending_dict_borrow.is_some() {
            return Err(Fault::Syntax("str 返回暫不支援未收斂控制流程或借用".into()));
        }
        self.expect("return")?;
        match self.token() {
            TokenKind::StringLiteral(value) => {
                let value = value.clone();
                self.cursor += 1;
                self.emit(&format!("Ok(String::from({value:?}))\n"))
            }
            TokenKind::Name(name) => {
                let name = name.clone();
                if self.tokens.get(self.cursor + 1)
                    .is_some_and(|t| t.kind == TokenKind::Symbol('(')) {
                    self.cursor += 1;
                    self.emit("Ok(")?;
                    self.str_factory_call(&name)?;
                    return self.emit(")\n");
                }
                let moved_kind = match self.locals.get(&name).map(String::as_str) {
                    Some("owned:str") => "moved:str",
                    Some("const-owned:str") => "moved:const:str",
                    _ => return Err(Fault::Syntax(format!("{name} 不是可返回的擁有字串"))),
                };
                self.cursor += 1;
                self.expect(".")?;
                self.expect("move")?;
                self.expect("(")?;
                self.expect(")")?;
                self.locals.insert(name.clone(), moved_kind.into());
                self.emit(&format!("Ok(v_{name})\n"))
            }
            _ => Err(Fault::Syntax("str 返回需要字串常值或明確 move".into())),
        }
    }
    fn finish_function(&mut self) -> R {
        self.current_function.take().ok_or_else(|| Fault::Syntax("函式邊界無效".into()))?;
        self.locals.clear();
        self.pending_local = None;
        self.pending_constructor = None;
        self.constructor_fields.clear();
        self.list_items = 0;
        self.nested_rows = 0;
        self.nested_items = 0;
        self.dict_items = 0;
        self.dict_keys.clear();
        self.pending_list_borrows.clear();
        self.pending_dict_borrow = None;
        self.pending_list_append = None;
        self.pending_dict_set = None;
        self.pending_matches.clear();
        self.pending_if_scopes.clear();
        self.pending_tries.clear();
        Ok(0)
    }
    fn if_has_continuation(&self) -> R {
        // 自舉第一批允許 fallthrough if，但不能把終端 return 路徑誤當成合流。
        // 從 if 的詞彙起點尋找兩個完整的 {}，包括巢狀括號。
        if self.matched_width("if") == 0 { return Ok(0); }
        let Some(start) = (self.cursor..self.tokens.len())
            .find(|&i| self.tokens[i].kind == TokenKind::Symbol('{')) else {
            return Ok(0);
        };
        let matching_end = |opening: usize| -> Option<usize> {
            let mut depth = 0usize;
            for i in opening..self.tokens.len() {
                match self.tokens[i].kind {
                    TokenKind::Symbol('{') => depth += 1,
                    TokenKind::Symbol('}') => {
                        depth = depth.checked_sub(1)?;
                        if depth == 0 { return Some(i); }
                    }
                    _ => {}
                }
            }
            None
        };
        let Some(then_end) = matching_end(start) else { return Ok(0); };
        let Some(else_token) = self.tokens.get(then_end + 1) else { return Ok(0); };
        if !matches!(&else_token.kind, TokenKind::Keyword(s) | TokenKind::Name(s) if s == "else") {
            return Ok(0);
        }
        if !self.tokens.get(then_end + 2).is_some_and(|t| t.kind == TokenKind::Symbol('{')) {
            return Ok(0);
        }
        let Some(else_end) = matching_end(then_end + 2) else { return Ok(0); };
        Ok(i64::from(self.tokens.get(else_end + 1).is_some_and(|next|
            !matches!(next.kind, TokenKind::Symbol('}') | TokenKind::End))))
    }
    fn if_scope_start(&mut self) -> R {
        // 保存外層與兩個互斥臂的獨立擁有權狀態。
        self.pending_if_scopes.push((false, self.locals.clone(), None));
        Ok(0)
    }
    fn if_scope_else(&mut self) -> R {
        let completed_then = self.locals.clone();
        let Some((switched, outer_locals, then_locals)) = self.pending_if_scopes.last_mut() else {
            return Err(Fault::Syntax("if 分支邊界無效".into()));
        };
        if *switched { return Err(Fault::Syntax("if 分支重複切換".into())); }
        *switched = true;
        *then_locals = Some(completed_then);
        self.locals = outer_locals.clone();
        Ok(0)
    }
    fn if_scope_finish(&mut self) -> R {
        let Some((switched, outer_locals, completed_then)) = self.pending_if_scopes.pop() else {
            return Err(Fault::Syntax("if 分支邊界無效".into()));
        };
        if !switched { return Err(Fault::Syntax("if 缺少 else 分支".into())); }
        let completed_then = completed_then.ok_or_else(|| Fault::Syntax("if true 分支狀態缺失".into()))?;
        let completed_else = self.locals.clone();
        let mut merged = outer_locals.clone();
        for (name, original_kind) in &outer_locals {
            let Some(then_kind) = completed_then.get(name) else {
                return Err(Fault::Syntax(format!("if true 分支遺失外層變數 {name}")));
            };
            let Some(else_kind) = completed_else.get(name) else {
                return Err(Fault::Syntax(format!("if else 分支遺失外層變數 {name}")));
            };
            if then_kind == else_kind {
                merged.insert(name.clone(), then_kind.clone());
                continue;
            }
            let mutable_owner = matches!(original_kind.as_str(), "owned:str" | "moved:str")
                && matches!(then_kind.as_str(), "owned:str" | "moved:str")
                && matches!(else_kind.as_str(), "owned:str" | "moved:str");
            let immutable_owner = matches!(original_kind.as_str(), "const-owned:str" | "moved:const:str")
                && matches!(then_kind.as_str(), "const-owned:str" | "moved:const:str")
                && matches!(else_kind.as_str(), "const-owned:str" | "moved:const:str");
            if mutable_owner {
                // 兩臂只要有一臂失去所有權，合流後便不可讀；
                // 明確重新初始化後才可再次使用。
                merged.insert(name.clone(), "moved:str".into());
            } else if immutable_owner {
                merged.insert(name.clone(), "moved:const:str".into());
            } else {
                return Err(Fault::Syntax(format!("if 分支的 {name} 資源狀態無法安全合流")));
            }
        }
        // 分支內的新區域名稱不提升為外層。
        self.locals = merged;
        Ok(0)
    }
    fn try_start(&mut self) -> R {
        // 每個 try 臂以進入前的詞彙環境為基準；區塊內區域不可逃逸到 except／後續語句。
        self.pending_tries.push((false, None, None, self.locals.clone()));
        self.emit("match (|| -> R {\n")
    }
    fn try_handler(&mut self) -> R {
        let Some((handler, _, _, outer_locals)) = self.pending_tries.last_mut() else {
            return Err(Fault::Syntax("try 邊界無效".into()));
        };
        if *handler { return Err(Fault::Syntax("try 處理區塊重複".into())); }
        *handler = true;
        // Rust 生成物的保護臂由封閉 closure 隔離；靜態名稱解析必須同樣隔離。
        self.locals = outer_locals.clone();
        self.emit("})() {\nOk(value) => return Ok(value),\n")
    }
    fn exception_category(&mut self) -> R {
        let (kind, fault) = if self.matched_width("EvaluationError") != 0 {
            (ExceptionKind::Evaluation, "Overflow")
        } else if self.matched_width("TypeConversationError") != 0 {
            (ExceptionKind::Conversion, "Conversion")
        } else {
            return Err(Fault::Syntax("未知的例外型別".into()));
        };
        let Some((handler, category, _, _)) = self.pending_tries.last_mut() else {
            return Err(Fault::Syntax("except 型別不在處理區塊中".into()));
        };
        if !*handler { return Err(Fault::Syntax("except 型別邊界無效".into())); }
        if category.is_some() { return Err(Fault::Syntax("except 型別重複".into())); }
        *category = Some(kind);
        self.cursor += 1;
        self.emit(&format!("Err(Fault::{fault}) => {{\n"))
    }
    fn exception_binding(&mut self) -> R {
        let name = self.take_name()?;
        let Some((handler, category, binding, _)) = self.pending_tries.last_mut() else {
            return Err(Fault::Syntax("except 綁定不在處理區塊中".into()));
        };
        if !*handler { return Err(Fault::Syntax("except 綁定邊界無效".into())); }
        let message = match category {
            Some(ExceptionKind::Evaluation) => "運算失敗",
            Some(ExceptionKind::Conversion) => "整數轉換失敗",
            None => return Err(Fault::Syntax("except 綁定缺少例外型別".into())),
        };
        if binding.is_some() { return Err(Fault::Syntax("except 綁定重複".into())); }
        let previous = self.locals.insert(name.clone(), "error:str".into());
        *binding = Some((name.clone(), previous));
        self.emit(&format!("let v_{name}: &'static str = {message:?};\n"))
    }
    fn finish_try(&mut self) -> R {
        let (handler, category, _binding, outer_locals) = self.pending_tries.pop()
            .ok_or_else(|| Fault::Syntax("try 邊界無效".into()))?;
        if !handler { return Err(Fault::Syntax("try 缺少具型別的 except".into())); }
        if category.is_none() { return Err(Fault::Syntax("try 缺少例外型別".into())); }
        // except 綁定及處理臂內宣告的字串只在處理臂內有效。
        self.locals = outer_locals;
        self.emit("},\nErr(error) => return Err(error),\n}\n")
    }
    fn conversion(&mut self) -> R {
        let name = self.take_name()?;
        if !matches!(self.locals.get(&name).map(String::as_str), Some("const-owned:str" | "owned:str")) {
            return Err(Fault::Syntax("to_int 需要 str 區域名稱".into()));
        }
        self.expect(".")?;
        self.expect("to_int")?;
        self.expect("(")?;
        self.expect(")")?;
        self.emit(&format!("v_{name}.parse::<i64>().map_err(|_| Fault::Conversion)?"))
    }
    fn str_assignment(&mut self) -> R {
        let name = self.take_name()?;
        if !matches!(self.locals.get(&name).map(String::as_str), Some("owned:str" | "moved:str")) {
            return Err(Fault::Syntax(format!("{name} 不是可變的擁有字串")));
        }
        self.expect("=")?;
        match self.token() {
            TokenKind::StringLiteral(value) => {
                let value = value.clone();
                self.cursor += 1;
                // moved:str 可重新初始化；既有 owner 則先建立新字串，再釋放舊值。
                self.locals.insert(name.clone(), "owned:str".into());
                self.emit(&format!("v_{name} = String::from({value:?});\n"))
            }
            TokenKind::Name(source) => {
                let source = source.clone();
                if self.tokens.get(self.cursor + 1)
                    .is_some_and(|t| t.kind == TokenKind::Symbol('(')) {
                    self.cursor += 1;
                    self.emit(&format!("v_{name} = "))?;
                    self.str_factory_call(&source)?;
                    // 若來源透過 target.move() 轉移，Rust 允許在呼叫後重新初始化 target。
                    self.locals.insert(name.clone(), "owned:str".into());
                    return self.emit(";\n");
                }
                if source == name {
                    return Err(Fault::Syntax("字串不能以自身作為 move 來源".into()));
                }
                if !matches!(self.locals.get(&source).map(String::as_str), Some("owned:str" | "const-owned:str")) {
                    return Err(Fault::Syntax(format!("{source} 不是可移交的擁有字串")));
                }
                // 沿用宣告移交已核准的直線、無有效借用安全子集。
                if !self.pending_if_scopes.is_empty() || !self.pending_matches.is_empty()
                    || !self.pending_tries.is_empty() || !self.pending_list_borrows.is_empty()
                    || self.pending_dict_borrow.is_some() {
                    return Err(Fault::Syntax("字串移交暫不允許跨控制流程或有效借用".into()));
                }
                self.cursor += 1;
                self.expect(".")?;
                self.expect("move")?;
                self.expect("(")?;
                self.expect(")")?;
                let moved_kind = if self.locals.get(&source).map(String::as_str) == Some("const-owned:str") {
                    "moved:const:str"
                } else {
                    "moved:str"
                };
                self.locals.insert(source.clone(), moved_kind.into());
                self.locals.insert(name.clone(), "owned:str".into());
                // 目標先前有效：Rust 賦值釋放其舊 String；已移交則重新取得所有權。
                self.emit(&format!("v_{name} = v_{source};\n"))
            }
            _ => Err(Fault::Syntax("str 重新賦值需要字串常值或明確 move".into())),
        }
    }
    fn str_factory_call(&mut self, function: &str) -> R {
        self.typed_factory_call(function, Some("str"))
    }
    fn typed_factory_call(&mut self, function: &str, return_kind: Option<&str>) -> R {
        // 遞迴深度不能依賴 Rust 執行緒堆疊無限成長。
        if self.str_factory_depth >= 32 {
            return Err(Fault::Syntax("str 返回函式巢狀呼叫超過 32 層".into()));
        }
        self.str_factory_depth += 1;
        let result = self.typed_factory_call_inner(function, return_kind);
        self.str_factory_depth -= 1;
        result
    }
    fn typed_factory_call_inner(&mut self, function: &str, return_kind: Option<&str>) -> R {
        // 先依詞彙序逐項求值，保留 move 來源失效；完成全文後才核對
        // 前向函式的參數型別及返回型別。
        self.expect("(")?;
        // 依序將所有實參先執行為區塊暫存，再借用 &mut Machine 呼叫外層。
        // 避免 v_outer(s, depth+1, v_inner(s,...)) 的 Rust E0499 衝突。
        self.emit("{\n")?;
        let mut kinds: Vec<FactoryArgumentKind> = Vec::new();
        while self.matched_width(")") == 0 {
            if kinds.len() >= 64 {
                return Err(Fault::Syntax("str 返回函式實參過多".into()));
            }
            if !kinds.is_empty() { self.expect(",")?; }
            let slot = format!("__voiles_str_arg_{}_{}", self.str_factory_depth, kinds.len());
            self.emit(&format!("let {slot} = "))?;
            match self.token().clone() {
                TokenKind::Integer(_) => {
                    self.literal()?;
                    kinds.push(FactoryArgumentKind::Int);
                }
                TokenKind::StringLiteral(value) => {
                    self.cursor += 1;
                    self.emit(&format!("String::from({value:?})"))?;
                    kinds.push(FactoryArgumentKind::Str);
                }
                TokenKind::Name(source) if self.tokens.get(self.cursor + 1)
                    .is_some_and(|t| t.kind == TokenKind::Symbol('(')) => {
                    // 巢狀結果先保留 callee 名稱，待全文的正式簽章建立後
                    // 再解析型別。生成的 Rust 暫存不需要猜測 int 或 str。
                    self.cursor += 1;
                    self.typed_factory_call(&source, None)?;
                    kinds.push(FactoryArgumentKind::Call(source));
                }
                TokenKind::Name(source) => {
                    let explicit_move = self.tokens.get(self.cursor + 1)
                        .is_some_and(|t| t.kind == TokenKind::Symbol('.'))
                        && self.tokens.get(self.cursor + 2)
                            .is_some_and(|t| matches!(&t.kind,
                                TokenKind::Name(n) | TokenKind::Keyword(n) if n == "move"))
                        && self.tokens.get(self.cursor + 3)
                            .is_some_and(|t| t.kind == TokenKind::Symbol('('))
                        && self.tokens.get(self.cursor + 4)
                            .is_some_and(|t| t.kind == TokenKind::Symbol(')'));
                    if explicit_move {
                        if !self.pending_if_scopes.is_empty() || !self.pending_matches.is_empty()
                            || !self.pending_tries.is_empty() || !self.pending_list_borrows.is_empty()
                            || self.pending_dict_borrow.is_some() {
                            return Err(Fault::Syntax(
                                "str 返回函式實參移交暫不允許跨控制流程或有效借用".into()));
                        }
                        let moved_kind = match self.locals.get(&source).map(String::as_str) {
                            Some("owned:str") => "moved:str",
                            Some("const-owned:str") => "moved:const:str",
                            _ => return Err(Fault::Syntax(
                                format!("{source} 不是可移交的 str 實參"))),
                        };
                        self.cursor += 1;
                        self.expect(".")?;
                        self.expect("move")?;
                        self.expect("(")?;
                        self.expect(")")?;
                        self.locals.insert(source.clone(), moved_kind.into());
                        self.emit(&format!("v_{source}"))?;
                        kinds.push(FactoryArgumentKind::Str);
                    } else {
                        // 保留 int 形參／局部既有的名稱解析，拒絕對 str 的隱含複製。
                        self.value()?;
                        kinds.push(FactoryArgumentKind::Int);
                    }
                }
                _ => return Err(Fault::Syntax(
                    "str 返回函式實參僅支援 int、字串常值或明確 move".into())),
            }
            self.emit(";\n")?;
            if self.matched_width(")") == 0 && self.matched_width(",") == 0 {
                return Err(Fault::Syntax("str 返回函式實參缺少逗號或右括號".into()));
            }
        }
        self.expect(")")?;
        self.emit(&format!("v_{function}(s, depth + 1"))?;
        for index in 0..kinds.len() {
            self.emit(&format!(", __voiles_str_arg_{}_{}", self.str_factory_depth, index))?;
        }
        self.str_factory_calls.push((function.to_owned(), kinds, return_kind.map(str::to_owned)));
        self.emit(")?\n}")
    }
    fn str_local(&mut self) -> R {
        let mutable = self.matched_width("var") != 0;
        self.expect(if mutable { "var" } else { "const" })?;
        let name = self.take_name()?;
        let parameter = self.current_function.as_ref().and_then(|f| self.functions.get(f))
            .is_some_and(|parameters| parameters.contains(&name));
        if parameter || self.locals.contains_key(&name) {
            return Err(Fault::Syntax(format!("重複區域名稱 {name}")));
        }
        self.expect(":")?;
        self.expect("str")?;
        self.expect("=")?;
        match self.token() {
            TokenKind::StringLiteral(value) => {
                let value = value.clone();
                self.cursor += 1;
                if mutable {
                    self.locals.insert(name.clone(), "owned:str".into());
                    self.emit(&format!("let mut v_{name}: String = String::from({value:?});\n"))
                } else {
                    self.locals.insert(name.clone(), "const-owned:str".into());
                    self.emit(&format!("let v_{name}: String = String::from({value:?});\n"))
                }
            }
            TokenKind::Name(source) => {
                let source = source.clone();
                if self.tokens.get(self.cursor + 1)
                    .is_some_and(|t| t.kind == TokenKind::Symbol('(')) {
                    self.cursor += 1;
                    let modifier = if mutable { "mut " } else { "" };
                    self.emit(&format!("let {modifier}v_{name}: String = "))?;
                    self.str_factory_call(&source)?;
                    self.locals.insert(name.clone(), if mutable {
                        "owned:str".into()
                    } else {
                        "const-owned:str".into()
                    });
                    return self.emit(";\n");
                }
                if !matches!(self.locals.get(&source).map(String::as_str), Some("owned:str" | "const-owned:str")) {
                    return Err(Fault::Syntax(format!("{source} 不是可移交的擁有字串")));
                }
                // 第一批明確移交限直線語句：分支合流與借用票據需後續獨立檢查。
                if !self.pending_if_scopes.is_empty() || !self.pending_matches.is_empty()
                    || !self.pending_tries.is_empty() || !self.pending_list_borrows.is_empty()
                    || self.pending_dict_borrow.is_some() {
                    return Err(Fault::Syntax("字串移交暫不允許跨控制流程或有效借用".into()));
                }
                self.cursor += 1;
                self.expect(".")?;
                self.expect("move")?;
                self.expect("(")?;
                self.expect(")")?;
                let moved_kind = if self.locals.get(&source).map(String::as_str) == Some("const-owned:str") {
                    "moved:const:str"
                } else {
                    "moved:str"
                };
                self.locals.insert(source.clone(), moved_kind.into());
                if mutable {
                    self.locals.insert(name.clone(), "owned:str".into());
                    self.emit(&format!("let mut v_{name}: String = v_{source};\n"))
                } else {
                    self.locals.insert(name.clone(), "const-owned:str".into());
                    self.emit(&format!("let v_{name}: String = v_{source};\n"))
                }
            }
            _ => Err(Fault::Syntax("str 初始化需要字串常值或明確 move".into())),
        }
    }
    fn match_start(&mut self) -> R {
        // 同一 match 的各 case 共用外層名稱，但不共用分支內宣告。
        self.pending_matches.push((None, BTreeSet::new(), false, 0, self.locals.clone()));
        self.emit("match ")
    }
    fn match_integer_subject(&mut self) -> R {
        let Some((kind, _, _, _, _)) = self.pending_matches.last_mut() else {
            return Err(Fault::Syntax("match 邊界無效".into()));
        };
        *kind = Some(MatchKind::Integer);
        Ok(0)
    }
    fn match_boolean_subject(&mut self) -> R {
        let value = if self.matched_width("true") != 0 { "true" }
            else if self.matched_width("false") != 0 { "false" }
            else { return Err(Fault::Syntax("match 需要布林常值".into())); };
        let Some((kind, _, _, _, _)) = self.pending_matches.last_mut() else {
            return Err(Fault::Syntax("match 邊界無效".into()));
        };
        *kind = Some(MatchKind::Boolean);
        self.cursor += 1;
        self.emit(value)
    }
    fn match_pattern(&mut self) -> R {
        let Some(kind) = self.pending_matches.last().and_then(|(kind, _, _, _, _)| *kind) else {
            return Err(Fault::Syntax("match 邊界無效".into()));
        };
        let (value, emitted) = match kind {
            MatchKind::Integer => {
                let (negative, raw) = if self.matched_width("-") != 0 {
                    self.cursor += 1;
                    let TokenKind::Integer(raw) = self.token() else {
                        return Err(Fault::Syntax("case 需要整數常值".into()));
                    };
                    (true, raw.clone())
                } else {
                    let TokenKind::Integer(raw) = self.token() else {
                        return Err(Fault::Syntax("case 需要整數常值".into()));
                    };
                    (false, raw.clone())
                };
                let value = if negative {
                    crate::syntax_shared::negative_integer_i64(&raw)
                } else {
                    crate::syntax_shared::integer_i64(&raw)
                }.map_err(|_| Fault::Syntax("case 整數常值無效".into()))?;
                (value, format!("{value}i64"))
            }
            MatchKind::Boolean => {
                let value = if self.matched_width("true") != 0 { 1 }
                    else if self.matched_width("false") != 0 { 0 }
                    else { return Err(Fault::Syntax("case 需要布林常值".into())); };
                (value, if value == 1 { "true".into() } else { "false".into() })
            }
        };
        self.cursor += 1;
        let Some((_, seen, wildcard, _, _)) = self.pending_matches.last_mut() else {
            return Err(Fault::Syntax("match 邊界無效".into()));
        };
        if *wildcard { return Err(Fault::Syntax("預設分支只能位於末尾".into())); }
        if !seen.insert(value) { return Err(Fault::Syntax("case 常值重複".into())); }
        self.emit(&format!("{emitted} => {{\n"))
    }
    fn match_wildcard(&mut self) -> R {
        self.expect("_")?;
        let Some((kind, seen, wildcard, _, _)) = self.pending_matches.last_mut() else {
            return Err(Fault::Syntax("match 邊界無效".into()));
        };
        if *wildcard { return Err(Fault::Syntax("預設分支重複".into())); }
        if *kind == Some(MatchKind::Boolean) && seen.len() == 2 {
            return Err(Fault::Syntax("所有布林值都已有分支".into()));
        }
        *wildcard = true;
        self.emit("_ => {\n")
    }
    fn finish_match_arm(&mut self) -> R {
        let Some((_, _, _, arms, outer_locals)) = self.pending_matches.last_mut() else {
            return Err(Fault::Syntax("match 邊界無效".into()));
        };
        *arms += 1;
        // 真 Rust 的 case 區塊結束後區域名稱亦到期。
        self.locals = outer_locals.clone();
        self.emit("},\n")
    }
    fn finish_match(&mut self) -> R {
        let (kind, seen, wildcard, arms, outer_locals) = self.pending_matches.pop()
            .ok_or_else(|| Fault::Syntax("match 邊界無效".into()))?;
        if kind.is_none() { return Err(Fault::Syntax("match 缺少比對值".into())); }
        if arms == 0 { return Err(Fault::Syntax("match 需要至少一個 case".into())); }
        self.locals = outer_locals;
        let exhaustive_boolean = kind == Some(MatchKind::Boolean) && seen.len() == 2;
        if !wildcard && !exhaustive_boolean { self.emit("_ => {},\n")?; }
        self.emit("}\n")
    }
    fn local(&mut self) -> R {
        let name = self.take_name()?;
        let parameter = self.current_function.as_ref().and_then(|f| self.functions.get(f))
            .is_some_and(|parameters| parameters.contains(&name));
        if parameter || self.locals.contains_key(&name) { return Err(Fault::Syntax(format!("重複區域名稱 {name}"))); }
        self.pending_local = Some((name.clone(), String::new()));
        self.emit(&format!("let mut v_{name}"))
    }
    fn local_type(&mut self) -> R {
        let kind = self.take_name()?;
        if !self.structures.contains_key(&kind) { return Err(Fault::Syntax(format!("未知結構 {kind}"))); }
        let name = {
            let Some((name, pending)) = self.pending_local.as_mut() else { return Err(Fault::Syntax("區域型別邊界無效".into())); };
            *pending = kind.clone();
            name.clone()
        };
        self.locals.insert(name, format!("struct:{kind}"));
        let ident = self.struct_ident(&kind);
        self.emit(&format!(": {ident}"))
    }
    fn constructor(&mut self) -> R {
        let kind = self.take_name()?;
        let Some((_, expected)) = self.pending_local.take() else { return Err(Fault::Syntax("建立式邊界無效".into())); };
        if kind != expected { return Err(Fault::Syntax(format!("建立式型別應為 {expected}"))); }
        self.pending_constructor = Some(kind.clone());
        self.constructor_fields.clear();
        let ident = self.struct_ident(&kind);
        self.emit(&format!(" = {ident}{{"))
    }
    fn constructor_field(&mut self) -> R {
        let field = self.take_name()?;
        let kind = self.pending_constructor.as_ref().ok_or_else(|| Fault::Syntax("具名欄位不在建立式中".into()))?;
        if !self.structures.get(kind).expect("建立式結構已驗證").iter().any(|(name, _)| name == &field) {
            return Err(Fault::Syntax(format!("未知欄位 {field}")));
        }
        if !self.constructor_fields.insert(field.clone()) { return Err(Fault::Syntax(format!("重複建立欄位 {field}"))); }
        let comma = if self.constructor_fields.len() > 1 { "," } else { "" };
        self.emit(&format!("{comma}f_{field}:"))
    }
    fn finish_constructor(&mut self) -> R {
        let kind = self.pending_constructor.take().ok_or_else(|| Fault::Syntax("建立式邊界無效".into()))?;
        let omitted: Vec<_> = self.structures.get(&kind).expect("建立式結構已驗證").iter()
            .filter(|(name, _)| !self.constructor_fields.contains(name)).cloned().collect();
        for (name, value) in omitted {
            let comma = if self.constructor_fields.is_empty() { "" } else { "," };
            self.emit(&format!("{comma}f_{name}:{value}i64"))?;
            self.constructor_fields.insert(name);
        }
        self.constructor_fields.clear();
        self.emit("}")
    }
    fn field_value(&mut self) -> R {
        let local = self.take_name()?;
        self.expect(".")?;
        let field = self.take_name()?;
        let kind = self.locals.get(&local).and_then(|kind| kind.strip_prefix("struct:"))
            .ok_or_else(|| Fault::Syntax(format!("{local} 不是結構")))?;
        if !self.structures.get(kind).expect("區域結構已驗證").iter().any(|(name, _)| name == &field) {
            return Err(Fault::Syntax(format!("未知欄位 {field}")));
        }
        self.emit(&format!("v_{local}.f_{field}"))
    }
    fn field_target(&mut self) -> R {
        let local = self.take_name()?;
        self.expect(".")?;
        let field = self.take_name()?;
        let kind = self.locals.get(&local).and_then(|kind| kind.strip_prefix("struct:"))
            .ok_or_else(|| Fault::Syntax(format!("{local} 不是結構")))?;
        if !self.structures.get(kind).expect("區域結構已驗證").iter().any(|(name, _)| name == &field) {
            return Err(Fault::Syntax(format!("未知欄位 {field}")));
        }
        self.emit(&format!("v_{local}.f_{field} = "))
    }
    fn list_local(&mut self) -> R {
        let name = self.take_name()?;
        let parameter = self.current_function.as_ref().and_then(|f| self.functions.get(f))
            .is_some_and(|parameters| parameters.contains(&name));
        if parameter || self.locals.contains_key(&name) { return Err(Fault::Syntax(format!("重複區域名稱 {name}"))); }
        self.locals.insert(name.clone(), "list:int".into());
        self.list_items = 0;
        self.emit(&format!("let mut v_{name}: Vec<i64> = vec!["))
    }
    fn list_item(&mut self) -> R {
        if self.list_items != 0 { self.emit(",")?; }
        self.list_items += 1;
        Ok(0)
    }
    fn finish_list(&mut self) -> R {
        self.list_items = 0;
        self.emit("]")
    }
    fn nested_list_local(&mut self) -> R {
        let name = self.take_name()?;
        let parameter = self.current_function.as_ref().and_then(|f| self.functions.get(f))
            .is_some_and(|parameters| parameters.contains(&name));
        if parameter || self.locals.contains_key(&name) {
            return Err(Fault::Syntax(format!("重複區域名稱 {name}")));
        }
        self.locals.insert(name.clone(), "list:list:int".into());
        self.nested_rows = 0;
        self.nested_items = 0;
        self.emit(&format!("let v_{name}: Vec<Vec<i64>> = vec!["))
    }
    fn nested_row(&mut self) -> R {
        if self.nested_rows != 0 { self.emit(",")?; }
        self.nested_rows += 1;
        self.nested_items = 0;
        self.emit("vec![")
    }
    fn nested_item(&mut self) -> R {
        if self.nested_items != 0 { self.emit(",")?; }
        self.nested_items += 1;
        Ok(0)
    }
    fn finish_nested_row(&mut self) -> R {
        self.nested_items = 0;
        self.emit("]")
    }
    fn finish_nested_list(&mut self) -> R {
        self.nested_rows = 0;
        self.nested_items = 0;
        self.emit("]")
    }
    fn dict_local(&mut self) -> R {
        let name = self.take_name()?;
        let parameter = self.current_function.as_ref().and_then(|f| self.functions.get(f))
            .is_some_and(|parameters| parameters.contains(&name));
        if parameter || self.locals.contains_key(&name) {
            return Err(Fault::Syntax(format!("重複區域名稱 {name}")));
        }
        self.locals.insert(name.clone(), "dict:str:int".into());
        self.dict_items = 0;
        self.dict_keys.clear();
        self.emit(&format!("let mut v_{name}: Vec<(&'static str, i64)> = vec!["))
    }
    fn dict_key(&mut self) -> R {
        let TokenKind::StringLiteral(key) = self.token() else {
            return Err(Fault::Syntax("dict{str,int} 的鍵必須是 str 常值".into()));
        };
        let key = key.clone();
        if !self.dict_keys.insert(key.clone()) {
            return Err(Fault::Syntax(format!("字典常值的鍵重複：{key}")));
        }
        self.cursor += 1;
        self.expect(":")?;
        let comma = if self.dict_items == 0 { "" } else { "," };
        self.dict_items += 1;
        self.emit(&format!("{comma}({key:?},"))
    }
    fn finish_dict(&mut self) -> R {
        self.dict_items = 0;
        self.dict_keys.clear();
        self.emit("]")
    }
    fn nested_dict_local(&mut self) -> R {
        let name = self.take_name()?;
        let parameter = self.current_function.as_ref().and_then(|f| self.functions.get(f))
            .is_some_and(|parameters| parameters.contains(&name));
        if parameter || self.locals.contains_key(&name) {
            return Err(Fault::Syntax(format!("重複區域名稱 {name}")));
        }
        self.locals.insert(name.clone(), "dict:str:list:int".into());
        self.dict_items = 0;
        self.dict_keys.clear();
        self.nested_items = 0;
        self.emit(&format!("let v_{name}: Vec<(&'static str, Vec<i64>)> = vec!["))
    }
    fn nested_dict_key(&mut self) -> R {
        let TokenKind::StringLiteral(key) = self.token() else {
            return Err(Fault::Syntax("dict{str,list[int]} 的鍵必須是 str 常值".into()));
        };
        let key = key.clone();
        if !self.dict_keys.insert(key.clone()) {
            return Err(Fault::Syntax(format!("字典常值的鍵重複：{key}")));
        }
        self.cursor += 1;
        self.expect(":")?;
        let comma = if self.dict_items == 0 { "" } else { "," };
        self.dict_items += 1;
        self.nested_items = 0;
        self.emit(&format!("{comma}({key:?},vec!["))
    }
    fn finish_nested_dict_value(&mut self) -> R {
        self.nested_items = 0;
        self.emit("])")
    }
    fn finish_nested_dict(&mut self) -> R {
        self.dict_items = 0;
        self.dict_keys.clear();
        self.nested_items = 0;
        self.emit("]")
    }
    fn list_len(&mut self) -> R {
        self.expect("len")?;
        self.expect("(")?;
        let name = self.take_name()?;
        self.expect(")")?;
        if matches!(self.locals.get(&name).map(String::as_str), Some("const-owned:str" | "owned:str" | "error:str")) {
            return self.emit(&format!("(v_{name}.chars().count() as i64)"));
        }
        if !matches!(self.locals.get(&name).map(String::as_str),
            Some("list:int" | "borrow:list:int" | "list:list:int" | "dict:str:int" | "dict:str:list:int")) {
            return Err(Fault::Syntax(format!("{name} 不是已支援的容器")));
        }
        self.emit(&format!("(v_{name}.len() as i64)"))
    }
    fn list_append(&mut self) -> R {
        let name = self.take_name()?;
        if self.locals.get(&name).map(String::as_str) != Some("list:int") {
            return Err(Fault::Syntax(format!("{name} 不是 list[int]")));
        }
        if self.pending_list_borrows.iter().any(|(owner, _)| owner == &name) {
            return Err(Fault::Syntax("有效唯讀元素借用期間不能修改清單".into()));
        }
        self.expect(".")?;
        self.expect("append")?;
        self.expect("(")?;
        self.pending_list_append = Some(name.clone());
        self.emit(&format!("{{ v_{name}.push("))
    }
    fn finish_list_append(&mut self) -> R {
        let name = self.pending_list_append.take()
            .ok_or_else(|| Fault::Syntax("append 邊界無效".into()))?;
        self.emit(&format!("); v_{name}.len() as i64 }}"))
    }
    fn list_pop(&mut self) -> R {
        let name = self.take_name()?;
        if self.locals.get(&name).map(String::as_str) != Some("list:int") {
            return Err(Fault::Syntax(format!("{name} 不是 list[int]")));
        }
        if self.pending_list_borrows.iter().any(|(owner, _)| owner == &name) {
            return Err(Fault::Syntax("有效唯讀元素借用期間不能修改清單".into()));
        }
        self.expect(".")?;
        self.expect("pop")?;
        self.expect("(")?;
        self.expect(")")?;
        self.emit(&format!("v_{name}.pop().ok_or_else(|| Fault::Syntax(\"清單為空\".into()))?"))
    }
    fn dict_set(&mut self) -> R {
        let name = self.take_name()?;
        if self.locals.get(&name).map(String::as_str) != Some("dict:str:int") {
            return Err(Fault::Syntax(format!("{name} 不是 dict{{str,int}}")));
        }
        if self.pending_dict_borrow.as_ref().is_some_and(|(owner, _)| owner == &name) {
            return Err(Fault::Syntax("有效唯讀元素借用期間不能修改字典".into()));
        }
        if self.pending_dict_set.is_some() {
            return Err(Fault::Syntax("巢狀字典 set 尚未開放".into()));
        }
        self.expect(".")?;
        self.expect("set")?;
        self.expect("(")?;
        self.pending_dict_set = Some((name, None));
        Ok(0)
    }
    fn dict_set_key(&mut self) -> R {
        let TokenKind::StringLiteral(key) = self.token() else {
            return Err(Fault::Syntax("dict{str,int}.set 的鍵必須是 str 常值".into()));
        };
        let key = key.clone();
        self.cursor += 1;
        let Some((_, pending_key)) = self.pending_dict_set.as_mut() else {
            return Err(Fault::Syntax("字典 set 邊界無效".into()));
        };
        *pending_key = Some(key);
        self.emit("{ let v_dict_value = ")
    }
    fn finish_dict_set(&mut self) -> R {
        let (owner, key) = self.pending_dict_set.take()
            .ok_or_else(|| Fault::Syntax("字典 set 邊界無效".into()))?;
        let key = key.ok_or_else(|| Fault::Syntax("字典 set 缺少鍵".into()))?;
        self.emit(&format!("; if let Some((_, current)) = v_{owner}.iter_mut().find(|(k, _)| *k == {key:?}) {{ *current = v_dict_value; }} else {{ v_{owner}.push(({key:?}, v_dict_value)); }} v_{owner}.len() as i64 }}"))
    }
    fn return_start(&mut self) -> R {
        if !self.pending_list_borrows.is_empty() || self.pending_dict_borrow.is_some()
            || !self.pending_matches.is_empty() || !self.pending_tries.is_empty() {
            self.emit("return Ok(")
        } else {
            self.emit("Ok(")
        }
    }
    fn list_owner(&mut self) -> R {
        let name = self.take_name()?;
        if !matches!(self.locals.get(&name).map(String::as_str),
            Some("list:int" | "borrow:list:int" | "list:list:int")) {
            return Err(Fault::Syntax(format!("{name} 不是可選取的清單")));
        }
        if self.pending_dict_borrow.is_some()
            && self.locals.get(&name).map(String::as_str) != Some("borrow:list:int") {
            return Err(Fault::Syntax("清單借用邊界無效".into()));
        }
        self.pending_list_borrows.push((name.clone(), None));
        self.emit("match usize::try_from(")
    }
    fn list_alias(&mut self) -> R {
        let alias = self.take_name()?;
        let parameter = self.current_function.as_ref().and_then(|f| self.functions.get(f))
            .is_some_and(|parameters| parameters.contains(&alias));
        if parameter || self.locals.contains_key(&alias) {
            return Err(Fault::Syntax(format!("重複區域名稱 {alias}")));
        }
        let (owner, element_kind) = {
            let Some((owner, pending_alias)) = self.pending_list_borrows.last_mut() else {
                return Err(Fault::Syntax("清單借用邊界無效".into()));
            };
            *pending_alias = Some(alias.clone());
            let element_kind = if self.locals.get(owner).map(String::as_str) == Some("list:list:int") {
                "borrow:list:int"
            } else {
                "int"
            };
            (owner.clone(), element_kind)
        };
        self.locals.insert(alias.clone(), element_kind.into());
        let binding = if element_kind == "int" {
            format!("let v_{alias}: i64 = *v_{alias};\n")
        } else {
            format!("let v_{alias}: &Vec<i64> = v_{alias};\n")
        };
        self.emit(&format!(").ok().and_then(|i| v_{owner}.get(i)) {{\nSome(v_{alias}) => {{\n{binding}"))
    }
    fn finish_list_borrow(&mut self) -> R {
        let (_, alias) = self.pending_list_borrows.pop()
            .ok_or_else(|| Fault::Syntax("清單借用邊界無效".into()))?;
        let alias = alias.ok_or_else(|| Fault::Syntax("清單借用缺少元素名稱".into()))?;
        self.locals.remove(&alias);
        self.emit("}\nNone => return Err(Fault::Syntax(\"清單索引越界\".into()))\n};\n")
    }
    fn dict_owner(&mut self) -> R {
        let name = self.take_name()?;
        if !matches!(self.locals.get(&name).map(String::as_str),
            Some("dict:str:int" | "dict:str:list:int")) {
            return Err(Fault::Syntax(format!("{name} 不是可選取的字典")));
        }
        if self.pending_dict_borrow.is_some() || !self.pending_list_borrows.is_empty() {
            return Err(Fault::Syntax("字典借用邊界無效".into()));
        }
        self.pending_dict_borrow = Some((name.clone(), None));
        self.emit(&format!("match v_{name}.iter().find(|(k, _)| *k == "))
    }
    fn dict_selector(&mut self) -> R {
        let TokenKind::StringLiteral(key) = self.token() else {
            return Err(Fault::Syntax("dict{str,int} 查找鍵必須是 str 常值".into()));
        };
        let key = key.clone();
        self.cursor += 1;
        self.emit(&format!("{key:?}"))
    }
    fn dict_alias(&mut self) -> R {
        let alias = self.take_name()?;
        let parameter = self.current_function.as_ref().and_then(|f| self.functions.get(f))
            .is_some_and(|parameters| parameters.contains(&alias));
        if parameter || self.locals.contains_key(&alias) {
            return Err(Fault::Syntax(format!("重複區域名稱 {alias}")));
        }
        let element_kind = {
            let Some((owner, pending_alias)) = self.pending_dict_borrow.as_mut() else {
                return Err(Fault::Syntax("字典借用邊界無效".into()));
            };
            *pending_alias = Some(alias.clone());
            if self.locals.get(owner).map(String::as_str) == Some("dict:str:list:int") {
                "borrow:list:int"
            } else {
                "int"
            }
        };
        self.locals.insert(alias.clone(), element_kind.into());
        let binding = if element_kind == "int" {
            format!("let v_{alias}: i64 = *v_{alias};\n")
        } else {
            format!("let v_{alias}: &Vec<i64> = v_{alias};\n")
        };
        self.emit(&format!(").map(|(_, value)| value) {{\nSome(v_{alias}) => {{\n{binding}"))
    }
    fn finish_dict_borrow(&mut self) -> R {
        let (_, alias) = self.pending_dict_borrow.take()
            .ok_or_else(|| Fault::Syntax("字典借用邊界無效".into()))?;
        let alias = alias.ok_or_else(|| Fault::Syntax("字典借用缺少元素名稱".into()))?;
        self.locals.remove(&alias);
        self.emit("}\nNone => return Err(Fault::Syntax(\"字典查找缺少鍵\".into()))\n};\n")
    }
    fn call(&mut self) -> R {
        let name = self.take_name()?;
        let index = self.calls.len();
        self.calls.push((None, name.clone(), 0, Vec::new()));
        self.call_stack.push(index);
        self.emit_name(&name)
    }
    fn module_call(&mut self) -> R {
        let module = self.take_name()?;
        self.expect(".")?;
        let name = self.take_name()?;
        let index = self.calls.len();
        self.calls.push((Some(module.clone()), name.clone(), 0, Vec::new()));
        self.call_stack.push(index);
        self.emit(&format!("v_{module}_{name}"))
    }
    fn argument(&mut self) -> R {
        let Some(index) = self.call_stack.last().copied() else {
            return Err(Fault::Syntax("實參不在呼叫中".into()));
        };
        self.calls[index].2 += 1;
        self.calls[index].3.push("int".into());
        Ok(0)
    }
    fn at_str_move_argument(&self) -> R {
        Ok(i64::from(matches!(self.token(), TokenKind::Name(name)
                if matches!(self.locals.get(name).map(String::as_str), Some("owned:str" | "const-owned:str")))
            && self.tokens.get(self.cursor + 1).is_some_and(|t| t.kind == TokenKind::Symbol('.'))
            && self.tokens.get(self.cursor + 2).is_some_and(|t|
                matches!(&t.kind, TokenKind::Name(k) | TokenKind::Keyword(k) if k == "move"))
            && self.tokens.get(self.cursor + 3).is_some_and(|t| t.kind == TokenKind::Symbol('('))
            && self.tokens.get(self.cursor + 4).is_some_and(|t| t.kind == TokenKind::Symbol(')'))))
    }
    fn str_move_argument(&mut self) -> R {
        let index = self.call_stack.last().copied()
            .ok_or_else(|| Fault::Syntax("字串實參不在呼叫中".into()))?;
        if self.calls[index].0.is_some() {
            return Err(Fault::Syntax("跨模組字串移交尚未支援".into()));
        }
        if !self.pending_if_scopes.is_empty() || !self.pending_matches.is_empty()
            || !self.pending_tries.is_empty() || !self.pending_list_borrows.is_empty()
            || self.pending_dict_borrow.is_some() {
            return Err(Fault::Syntax("字串實參移交暫不允許跨控制流程或有效借用".into()));
        }
        let name = self.take_name()?;
        let kind = self.locals.get(&name).map(String::as_str).ok_or_else(||
            Fault::Syntax(format!("未知字串擁有者 {name}")))?;
        let moved_kind = match kind {
            "owned:str" => "moved:str",
            "const-owned:str" => "moved:const:str",
            _ => return Err(Fault::Syntax(format!("{name} 不是可移交的擁有字串"))),
        };
        self.expect(".")?;
        self.expect("move")?;
        self.expect("(")?;
        self.expect(")")?;
        self.locals.insert(name.clone(), moved_kind.into());
        self.calls[index].2 += 1;
        self.calls[index].3.push("str".into());
        self.emit(&format!(", v_{name}"))
    }
    fn finish_call(&mut self) -> R {
        self.call_stack.pop().ok_or_else(|| Fault::Syntax("呼叫邊界無效".into()))?;
        Ok(0)
    }
    fn finish_names(&mut self) -> R {
        if self.current_function.is_some() || !self.call_stack.is_empty() {
            return Err(Fault::Syntax("名稱解析邊界無效".into()));
        }
        for (module, name, actual, actual_kinds) in &self.calls {
            let expected = if let Some(module) = module {
                if !self.imports.contains(module) {
                    return Err(Fault::Syntax(format!("未匯入模組 {module}")));
                }
                self.external_functions.get(&(module.clone(), name.clone())).copied()
            } else {
                self.functions.get(name).map(Vec::len)
            };
            let Some(expected) = expected else {
                let qualified = module.as_ref().map_or_else(|| name.clone(), |m| format!("{m}.{name}"));
                return Err(Fault::Syntax(format!("未知函式 {qualified}")));
            };
            if *actual != expected {
                return Err(Fault::Syntax(format!("函式 {name} 預期 {expected} 個實參，收到 {actual} 個")));
            }
            if module.is_none() {
                let expected_kinds = self.function_param_kinds.get(name)
                    .ok_or_else(|| Fault::Syntax(format!("未知函式型別 {name}")))?;
                if expected_kinds != actual_kinds {
                    return Err(Fault::Syntax(format!("函式 {name} 的 str／int 實參型別不符")));
                }
                if self.function_return_kinds.get(name).map(String::as_str) != Some("int") {
                    return Err(Fault::Syntax(format!("函式 {name} 不回傳 int")));
                }
            } else if actual_kinds.iter().any(|kind| kind == "str") {
                return Err(Fault::Syntax("跨模組 str 實參仍未移植".into()));
            }
        }
        for (name, actual_kinds, return_kind) in &self.str_factory_calls {
            let Some(params) = self.functions.get(name) else {
                return Err(Fault::Syntax(format!("未知巢狀函式 {name}")));
            };
            let expected_kinds = self.function_param_kinds.get(name)
                .ok_or_else(|| Fault::Syntax(format!("未知巢狀函式簽章 {name}")))?;
            let declared_return = self.function_return_kinds.get(name)
                .ok_or_else(|| Fault::Syntax(format!("未知巢狀函式返回型別 {name}")))?;
            if params.len() != actual_kinds.len()
                || return_kind.as_ref().is_some_and(|kind| kind != declared_return) {
                return Err(Fault::Syntax(
                    format!("巢狀函式 {name} 的參數數量、型別或返回型別不符")));
            }
            for (expected, actual) in expected_kinds.iter().zip(actual_kinds) {
                let actual_kind = match actual {
                    FactoryArgumentKind::Int => "int",
                    FactoryArgumentKind::Str => "str",
                    FactoryArgumentKind::Call(callee) => self.function_return_kinds
                        .get(callee).map(String::as_str).ok_or_else(||
                            Fault::Syntax(format!("未知巢狀函式返回型別 {callee}")))?,
                };
                if expected != actual_kind {
                    return Err(Fault::Syntax(format!(
                        "巢狀函式 {name} 的實參型別不符：預期 {expected}，實際 {actual_kind}")));
                }
            }
        }
        Ok(0)
    }
    fn method(&mut self) -> R {
        let TokenKind::Name(name) = self.token() else {
            return Err(Fault::Syntax("預期自舉宿主函式".into()));
        };
        if self.module.is_some() { return Err(Fault::Syntax("一般模組不能使用自舉宿主".into())); }
        if !matches!(name.as_str(), "at" | "at_end" | "at_integer" | "at_call" | "at_module_call" | "at_field" | "at_field_write" | "at_list_local" | "at_nested_list_local" | "at_nested_dict_local" | "at_dict_local" | "at_dict_borrow" | "at_len" | "at_list_append" | "at_list_pop" | "at_dict_set" | "expect" |
                    "emit" | "name" | "function" | "parameter" | "value" |
                    "finish_function" | "call" | "argument" | "finish_call" |
                    "finish_names" | "module_import" | "module_call" | "structure" | "field" |
                    "field_default" | "finish_structure" | "local" | "local_type" | "constructor" |
                    "constructor_field" | "finish_constructor" | "field_value" | "field_target" |
                    "list_local" | "list_item" | "finish_list" | "nested_list_local" |
                    "nested_row" | "nested_item" | "finish_nested_row" | "finish_nested_list" |
                    "nested_dict_local" | "nested_dict_key" | "finish_nested_dict_value" | "finish_nested_dict" |
                    "dict_local" | "dict_key" |
                    "finish_dict" | "list_len" | "list_append" |
                    "finish_list_append" | "list_pop" | "list_owner" |
                    "list_alias" | "finish_list_borrow" | "dict_owner" | "dict_selector" |
                    "dict_alias" | "finish_dict_borrow" | "dict_set" | "dict_set_key" |
                    "finish_dict_set" | "return_start" | "if_scope_start" | "if_scope_else" |
                    "if_scope_finish" | "if_has_continuation" | "try_start" | "try_handler" |
                    "exception_category" | "exception_binding" | "finish_try" |
                    "match_start" | "match_integer_subject" |
                    "match_boolean_subject" | "match_pattern" |
                    "match_wildcard" | "finish_match_arm" | "finish_match" | "method" |
                    "literal" | "at_str_local" | "str_local" | "at_str_move_argument" |
                    "str_move_argument" | "mark_str_return" | "is_str_return" |
                    "str_return_statement" | "at_str_assignment" |
                    "str_assignment" | "at_conversion" | "conversion") {
            return Err(Fault::Syntax("未知的自舉宿主函式".into()));
        }
        let ident = format!("s.{name}");
        self.cursor += 1;
        self.emit(&ident)
    }
    fn literal(&mut self) -> R {
        let value = match self.token() {
            TokenKind::Integer(n) => {
                let v = crate::syntax_shared::integer_i64(n)
                    .map_err(|_| Fault::Syntax("整數常值無效".into()))?;
                format!("{v}i64")
            }
            TokenKind::StringLiteral(s) => format!("{s:?}"),
            _ => return Err(Fault::Syntax("預期整數或字串常值".into())),
        };
        self.cursor += 1;
        self.emit(&value)
    }
}

#[cfg(not(voiles_bootstrap))]
include!("../bootstrap/compiler_seed.inc");
#[cfg(voiles_bootstrap)]
include!(env!("VOILES_COMPILER_RS"));

pub fn compile_bootstrap_source(path: &Path) -> Result<String, String> {
    let source = Source::new(std::fs::read(path).map_err(|e| format!("{}：{e}", path.display()))?);
    if let Some(d) = source.lexical_diagnostics().first() {
        return Err(format!("{}：{}", path.display(), d.message));
    }
    let scanned = lexer::scan(&source);
    if let Some(d) = scanned.diagnostics.first() {
        return Err(format!("{}：{}", path.display(), d.message));
    }
    compile_tokens(path, &source, scanned.tokens)
}

/// 以自舉解析器編譯入口及其直接／間接匯入，供種子與兩代產物交叉驗證。
/// 一般公開專案仍由 `EntryProgram` 與既有四個命令處理。
pub fn compile_bootstrap_project(entry: &Path) -> Result<String, String> {
    fn visit(path: PathBuf, root: &Path, visiting: &mut Vec<PathBuf>, done: &mut BTreeSet<PathBuf>,
        signatures: &mut BTreeMap<(String, String), usize>, output: &mut String) -> Result<(), String> {
        let path = path.canonicalize().map_err(|e| format!("{}：{e}", path.display()))?;
        if done.contains(&path) { return Ok(()); }
        if let Some(start) = visiting.iter().position(|p| p == &path) {
            let mut cycle: Vec<_> = visiting[start..].iter().map(|p| p.display().to_string()).collect();
            cycle.push(path.display().to_string());
            return Err(format!("模組循環：{}", cycle.join(" -> ")));
        }
        visiting.push(path.clone());
        let module = path.file_stem().and_then(|s| s.to_str())
            .filter(|s| is_module_name(s)).ok_or_else(|| format!("{}：模組名稱無效", path.display()))?.to_string();
        let source = Source::new(std::fs::read(&path).map_err(|e| format!("{}：{e}", path.display()))?);
        if let Some(d) = source.lexical_diagnostics().first() { return Err(format!("{}：{}", path.display(), d.message)); }
        let scanned = lexer::scan(&source);
        if let Some(d) = scanned.diagnostics.first() { return Err(format!("{}：{}", path.display(), d.message)); }
        let imports = module_imports(&path, &scanned.tokens)?;
        for imported in &imports {
            visit(root.join(format!("{imported}.voil")), root, visiting, done, signatures, output)?;
        }
        let (generated, local) = compile_tokens_in_module(&path, &source, scanned.tokens,
            module.clone(), imports.into_iter().collect(), signatures.clone())?;
        output.push_str(&generated);
        for (name, parameters) in local {
            if !name.starts_with('_') { signatures.insert((module.clone(), name), parameters.len()); }
        }
        visiting.pop();
        done.insert(path);
        Ok(())
    }

    let root = entry.parent().unwrap_or_else(|| Path::new(".")).canonicalize()
        .map_err(|e| format!("{}：{e}", entry.display()))?;
    let entry_module = entry.file_stem().and_then(|s| s.to_str())
        .filter(|s| is_module_name(s)).ok_or_else(|| format!("{}：模組名稱無效", entry.display()))?.to_string();
    let mut output = String::new();
    visit(entry.to_path_buf(), &root, &mut Vec::new(), &mut BTreeSet::new(),
        &mut BTreeMap::new(), &mut output)?;
    output.push_str(&format!("fn v_main(s: &mut Machine, depth: usize) -> R {{ v_{entry_module}_main(s, depth) }}\n"));
    Ok(output)
}

fn is_module_name(name: &str) -> bool {
    name.as_bytes().first().is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn module_imports(path: &Path, tokens: &[Token]) -> Result<Vec<String>, String> {
    let filtered: Vec<_> = tokens.iter().filter(|t| !matches!(t.kind, TokenKind::Newline)).collect();
    let mut cursor = 0;
    let mut imports = Vec::new();
    while filtered.get(cursor).is_some_and(|t| matches!(&t.kind, TokenKind::Keyword(k) if k == "import")) {
        cursor += 1;
        let Some(Token {kind: TokenKind::Name(name), ..}) = filtered.get(cursor).map(|t| *t) else {
            return Err(format!("{}：import 後需要模組名稱", path.display()));
        };
        imports.push(name.clone());
        cursor += 1;
    }
    Ok(imports)
}

/// 自舉時由上一代的公開 run 入口執行 scanner.voil。Rust 只從已定位的
/// 原始切片取得詞值，並拒絕順序、字元邊界或種類不相容的記號資料。
pub fn compile_bootstrap_tokens(path: &Path, rows: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}：{e}", path.display()))?;
    let source = Source::new(bytes);
    if let Some(d) = source.lexical_diagnostics().first() {
        return Err(format!("{}：{}", path.display(), d.message));
    }
    let input = std::str::from_utf8(&source.bytes).map_err(|e| e.to_string())?;
    let mut tokens = Vec::new();
    let mut previous = 0;
    let mut ended = false;
    for row in rows.lines() {
        let invalid = || format!("{}：自舉記號資料無效", path.display());
        let mut fields = row.split_ascii_whitespace();
        let mut number = || fields.next().and_then(|s| s.parse::<usize>().ok()).ok_or_else(invalid);
        let (code, start, end) = (number()?, number()?, number()?);
        if fields.next().is_some() || ended || start < previous { return Err(invalid()); }
        let gap = input.get(previous..start).ok_or_else(invalid)?;
        if !gap.bytes().all(|c| matches!(c, b' ' | b'\t' | b'\n' | b'\r')) { return Err(invalid()); }
        let raw = input.get(start..end).ok_or_else(invalid)?;
        let kind = match code {
            1 | 2 if raw.starts_with(|c:char| c.is_ascii_alphabetic() || c == '_')
                && raw.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') => {
                if code == 1 { TokenKind::Keyword(raw.into()) } else { TokenKind::Name(raw.into()) }
            }
            3 if !raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()) => TokenKind::Integer(raw.into()),
            4 if raw.starts_with('"') => {
                // 字串跳脫仍由正式字串解碼器處理，只讀取此一記號的切片。
                let literal = lexer::scan(&Source::new(raw.as_bytes().to_vec()));
                if let Some(d) = literal.diagnostics.first() {return Err(format!("{}：{}",path.display(),d.message));}
                match literal.tokens.as_slice() {
                    [Token {kind: TokenKind::StringLiteral(value),end:len,..}, Token {kind:TokenKind::End,..}] if *len == raw.len() =>
                        TokenKind::StringLiteral(value.clone()),
                    _ => return Err(invalid()),
                }
            }
            5 if raw.chars().count() == 1 => TokenKind::Symbol(raw.chars().next().unwrap()),
            6 if raw == "->" => TokenKind::Arrow,
            7 if matches!(raw, "\r" | "\n" | "\r\n") => TokenKind::Newline,
            9 if start == input.len() && raw.is_empty() => { ended = true; TokenKind::End }
            _ => return Err(invalid()),
        };
        previous = end;
        tokens.push(Token { kind, start, end });
    }
    if !ended { return Err(format!("{}：自舉記號缺少檔尾", path.display())); }
    compile_tokens(path, &source, tokens)
}

fn compile_tokens(path: &Path, source: &Source, tokens: Vec<Token>) -> Result<String,String> {
    compile_tokens_with_context(path, source, tokens, None, BTreeSet::new(), BTreeMap::new())
        .map(|(output, _)| output)
}

fn compile_tokens_in_module(path: &Path, source: &Source, tokens: Vec<Token>, module: String,
    imports: BTreeSet<String>, external_functions: BTreeMap<(String, String), usize>)
    -> Result<(String, BTreeMap<String, Vec<String>>), String> {
    compile_tokens_with_context(path, source, tokens, Some(module), imports, external_functions)
}

fn compile_tokens_with_context(path: &Path, source: &Source, tokens: Vec<Token>, module: Option<String>,
    allowed_imports: BTreeSet<String>, external_functions: BTreeMap<(String, String), usize>)
    -> Result<(String, BTreeMap<String, Vec<String>>), String> {
    let mut machine = Machine {
        tokens: tokens.into_iter().filter(|t| !matches!(t.kind, TokenKind::Newline)).collect(),
        cursor: 0, steps: 0, output: String::new(), functions: BTreeMap::new(),
        function_param_kinds: BTreeMap::new(),
        function_return_kinds: BTreeMap::new(), str_factory_calls: Vec::new(), str_factory_depth: 0, current_function: None, calls: Vec::new(), call_stack: Vec::new(),
        module, imports: BTreeSet::new(), allowed_imports, external_functions,
        structures: BTreeMap::new(), current_structure: None, pending_field: None,
        locals: BTreeMap::new(), pending_local: None, pending_constructor: None,
        constructor_fields: BTreeSet::new(), list_items: 0, nested_rows: 0,
        nested_items: 0, dict_items: 0,
        dict_keys: BTreeSet::new(), pending_list_borrows: Vec::new(), pending_dict_borrow: None,
        pending_list_append: None, pending_dict_set: None,
        pending_matches: Vec::new(), pending_if_scopes: Vec::new(), pending_tries: Vec::new(),
    };
    v_main(&mut machine, 0).map_err(|e| {
        let pos = source.position(machine.tokens[machine.cursor].start);
        let message = match e { Fault::Syntax(s) => s, Fault::Limit => "自舉編譯預算耗盡".into(),
            Fault::Overflow => "自舉編譯整數溢位".into(),
            Fault::Conversion => "自舉編譯轉換失敗".into() };
        format!("{}:{}:{}: {message}", path.display(), pos.line, pos.column)
    })?;
    Ok((machine.output, machine.functions))
}
