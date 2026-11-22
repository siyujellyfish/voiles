//! 私有自舉編譯器的宿主 ABI。沿用正式 lexer，宿主提供有界的記號與輸出緩衝區。
use crate::lexer::{self, Token, TokenKind};
use crate::source::Source;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug)]
enum Fault { Syntax(String), Overflow, Limit }
type R = Result<i64, Fault>;

struct Machine {
    tokens: Vec<Token>,
    cursor: usize,
    output: String,
    steps: usize,
    functions: BTreeMap<String, Vec<String>>,
    current_function: Option<String>,
    calls: Vec<(Option<String>, String, usize)>,
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
    dict_items: usize,
    dict_keys: BTreeSet<String>,
    pending_list_borrow: Option<(String, Option<String>)>,
    pending_dict_borrow: Option<(String, Option<String>)>,
    pending_list_append: Option<String>,
    pending_dict_set: Option<(String, Option<String>)>,
}

impl Machine {
    fn enter(&mut self, depth: usize) -> Result<(), Fault> {
        let budget = crate::execution::ExecutionBudget::default();
        self.steps += 1;
        if depth >= budget.max_call_frames || self.steps > budget.max_steps {
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
            && self.locals.get(owner).map(String::as_str) == Some("dict:str:int")
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
        parameters.push(name.clone());
        self.emit_name(&name)
    }
    fn value(&mut self) -> R {
        let name = self.take_name()?;
        let parameter = self.current_function.as_ref()
            .and_then(|function| self.functions.get(function))
            .is_some_and(|parameters| parameters.contains(&name));
        let local_int = self.locals.get(&name).is_some_and(|kind| kind == "int");
        if !parameter && !local_int { return Err(Fault::Syntax(format!("未知名稱 {name}"))); }
        if parameter { self.emit_name(&name) } else { self.emit(&format!("v_{name}")) }
    }
    fn finish_function(&mut self) -> R {
        self.current_function.take().ok_or_else(|| Fault::Syntax("函式邊界無效".into()))?;
        self.locals.clear();
        self.pending_local = None;
        self.pending_constructor = None;
        self.constructor_fields.clear();
        self.list_items = 0;
        self.dict_items = 0;
        self.dict_keys.clear();
        self.pending_list_borrow = None;
        self.pending_dict_borrow = None;
        self.pending_list_append = None;
        self.pending_dict_set = None;
        Ok(0)
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
    fn list_len(&mut self) -> R {
        self.expect("len")?;
        self.expect("(")?;
        let name = self.take_name()?;
        self.expect(")")?;
        if !matches!(self.locals.get(&name).map(String::as_str),
            Some("list:int" | "dict:str:int")) {
            return Err(Fault::Syntax(format!("{name} 不是已支援的容器")));
        }
        self.emit(&format!("(v_{name}.len() as i64)"))
    }
    fn list_append(&mut self) -> R {
        let name = self.take_name()?;
        if self.locals.get(&name).map(String::as_str) != Some("list:int") {
            return Err(Fault::Syntax(format!("{name} 不是 list[int]")));
        }
        if self.pending_list_borrow.as_ref().is_some_and(|(owner, _)| owner == &name) {
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
        if self.pending_list_borrow.as_ref().is_some_and(|(owner, _)| owner == &name) {
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
        if self.pending_list_borrow.is_some() || self.pending_dict_borrow.is_some() {
            self.emit("return Ok(")
        } else {
            self.emit("Ok(")
        }
    }
    fn list_owner(&mut self) -> R {
        let name = self.take_name()?;
        if self.locals.get(&name).map(String::as_str) != Some("list:int") {
            return Err(Fault::Syntax(format!("{name} 不是 list[int]")));
        }
        if self.pending_list_borrow.is_some() {
            return Err(Fault::Syntax("清單借用邊界無效".into()));
        }
        self.pending_list_borrow = Some((name.clone(), None));
        self.emit("match usize::try_from(")
    }
    fn list_alias(&mut self) -> R {
        let alias = self.take_name()?;
        let parameter = self.current_function.as_ref().and_then(|f| self.functions.get(f))
            .is_some_and(|parameters| parameters.contains(&alias));
        if parameter || self.locals.contains_key(&alias) {
            return Err(Fault::Syntax(format!("重複區域名稱 {alias}")));
        }
        let owner = {
            let Some((owner, pending_alias)) = self.pending_list_borrow.as_mut() else {
                return Err(Fault::Syntax("清單借用邊界無效".into()));
            };
            *pending_alias = Some(alias.clone());
            owner.clone()
        };
        self.locals.insert(alias.clone(), "int".into());
        self.emit(&format!(").ok().and_then(|i| v_{owner}.get(i)) {{\nSome(v_{alias}) => {{\nlet v_{alias}: i64 = *v_{alias};\n"))
    }
    fn finish_list_borrow(&mut self) -> R {
        let (_, alias) = self.pending_list_borrow.take()
            .ok_or_else(|| Fault::Syntax("清單借用邊界無效".into()))?;
        let alias = alias.ok_or_else(|| Fault::Syntax("清單借用缺少元素名稱".into()))?;
        self.locals.remove(&alias);
        self.emit("}\nNone => return Err(Fault::Syntax(\"清單索引越界\".into()))\n};\n")
    }
    fn dict_owner(&mut self) -> R {
        let name = self.take_name()?;
        if self.locals.get(&name).map(String::as_str) != Some("dict:str:int") {
            return Err(Fault::Syntax(format!("{name} 不是 dict{{str,int}}")));
        }
        if self.pending_dict_borrow.is_some() || self.pending_list_borrow.is_some() {
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
        let Some((_, pending_alias)) = self.pending_dict_borrow.as_mut() else {
            return Err(Fault::Syntax("字典借用邊界無效".into()));
        };
        *pending_alias = Some(alias.clone());
        self.locals.insert(alias.clone(), "int".into());
        self.emit(&format!(").map(|(_, value)| value) {{\nSome(v_{alias}) => {{\nlet v_{alias}: i64 = *v_{alias};\n"))
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
        self.calls.push((None, name.clone(), 0));
        self.call_stack.push(index);
        self.emit_name(&name)
    }
    fn module_call(&mut self) -> R {
        let module = self.take_name()?;
        self.expect(".")?;
        let name = self.take_name()?;
        let index = self.calls.len();
        self.calls.push((Some(module.clone()), name.clone(), 0));
        self.call_stack.push(index);
        self.emit(&format!("v_{module}_{name}"))
    }
    fn argument(&mut self) -> R {
        let Some(index) = self.call_stack.last().copied() else {
            return Err(Fault::Syntax("實參不在呼叫中".into()));
        };
        self.calls[index].2 += 1;
        Ok(0)
    }
    fn finish_call(&mut self) -> R {
        self.call_stack.pop().ok_or_else(|| Fault::Syntax("呼叫邊界無效".into()))?;
        Ok(0)
    }
    fn finish_names(&mut self) -> R {
        if self.current_function.is_some() || !self.call_stack.is_empty() {
            return Err(Fault::Syntax("名稱解析邊界無效".into()));
        }
        for (module, name, actual) in &self.calls {
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
        }
        Ok(0)
    }
    fn method(&mut self) -> R {
        let TokenKind::Name(name) = self.token() else {
            return Err(Fault::Syntax("預期自舉宿主函式".into()));
        };
        if self.module.is_some() { return Err(Fault::Syntax("一般模組不能使用自舉宿主".into())); }
        if !matches!(name.as_str(), "at" | "at_end" | "at_integer" | "at_call" | "at_module_call" | "at_field" | "at_field_write" | "at_list_local" | "at_dict_local" | "at_dict_borrow" | "at_len" | "at_list_append" | "at_list_pop" | "at_dict_set" | "expect" |
                    "emit" | "name" | "function" | "parameter" | "value" |
                    "finish_function" | "call" | "argument" | "finish_call" |
                    "finish_names" | "module_import" | "module_call" | "structure" | "field" |
                    "field_default" | "finish_structure" | "local" | "local_type" | "constructor" |
                    "constructor_field" | "finish_constructor" | "field_value" | "field_target" |
                    "list_local" | "list_item" | "finish_list" | "dict_local" | "dict_key" |
                    "finish_dict" | "list_len" | "list_append" |
                    "finish_list_append" | "list_pop" | "list_owner" |
                    "list_alias" | "finish_list_borrow" | "dict_owner" | "dict_selector" |
                    "dict_alias" | "finish_dict_borrow" | "dict_set" | "dict_set_key" |
                    "finish_dict_set" | "return_start" | "method" | "literal") {
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
        current_function: None, calls: Vec::new(), call_stack: Vec::new(),
        module, imports: BTreeSet::new(), allowed_imports, external_functions,
        structures: BTreeMap::new(), current_structure: None, pending_field: None,
        locals: BTreeMap::new(), pending_local: None, pending_constructor: None,
        constructor_fields: BTreeSet::new(), list_items: 0, dict_items: 0,
        dict_keys: BTreeSet::new(), pending_list_borrow: None, pending_dict_borrow: None,
        pending_list_append: None, pending_dict_set: None,
    };
    v_main(&mut machine, 0).map_err(|e| {
        let pos = source.position(machine.tokens[machine.cursor].start);
        let message = match e { Fault::Syntax(s) => s, Fault::Limit => "自舉編譯預算耗盡".into(),
            Fault::Overflow => "自舉編譯整數溢位".into() };
        format!("{}:{}:{}: {message}", path.display(), pos.line, pos.column)
    })?;
    Ok((machine.output, machine.functions))
}
