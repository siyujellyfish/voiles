/// P19 具型別常值只保存已驗證的純值。讀取借用原容器內的值。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ContainerAtom {
    Missing,
    Integer(i64),
    Boolean(bool),
    Text(String),
    Decimal(u64, i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ContainerItems {
    List(Vec<ContainerAtom>),
    Dictionary(BTreeMap<ContainerAtom, ContainerAtom>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ContainerLiteral {
    ty: String,
    items: ContainerItems,
}

impl ContainerLiteral {
    fn lookup(&self, selector: &ContainerAtom) -> Option<&ContainerAtom> {
        match (&self.items, selector) {
            (ContainerItems::List(items), ContainerAtom::Integer(index)) =>
                usize::try_from(*index).ok().and_then(|i| items.get(i)),
            (ContainerItems::Dictionary(items), key) => items.get(key),
            _ => None,
        }
    }
}

impl Parser<'_> {
    fn container_atom(&mut self, expected: &str) -> Result<ContainerAtom, Diagnostic> {
        let token = self.tok();
        let candidates = expected.strip_prefix("Optional[").and_then(|s| s.strip_suffix(']'));
        let (ty, atom) = match &token.kind {
            TokenKind::Keyword(word) if word == "none" && candidates.is_some() =>
                ("none", ContainerAtom::Missing),
            TokenKind::Integer(text) => ("int", ContainerAtom::Integer(
                crate::syntax_shared::integer_i64(text).map_err(|_| self.syntax("容器整數常值超出範圍"))?)),
            TokenKind::Keyword(word) if word == "true" || word == "false" =>
                ("bool", ContainerAtom::Boolean(word == "true")),
            TokenKind::StringLiteral(text) => ("str", ContainerAtom::Text(text.clone())),
            TokenKind::Decimal(text) => {
                let (coefficient, exponent) = Self::decimal64_checked_subset(text)
                    .ok_or_else(|| self.syntax("容器小數常值超出支援範圍"))?;
                ("float", ContainerAtom::Decimal(coefficient, exponent))
            }
            _ => return Err(self.syntax("此容器位置需要已核准的常值")),
        };
        if ty != "none" && ty != expected && !candidates.is_some_and(|c| c.split(',').any(|t| t == ty)) {
            return Err(Diagnostic { kind: DiagnosticKind::Static,
                message: "容器常值與宣告型別不符".into(),
                span: Some(self.source.span(token.start, token.end)) });
        }
        self.cursor += 1;
        Ok(atom)
    }

    fn container_literal(&mut self, ty: &str) -> Result<Term, Diagnostic> {
        let start = self.tok().start;
        let items = if let Some(element) = ty.strip_prefix("list[").and_then(|s| s.strip_suffix(']')) {
            self.take('[', "list 常值需要左方括號")?;
            let mut items = Vec::new();
            while !self.symbol(']') {
                items.push(self.container_atom(element)?);
                if self.symbol(']') { break; }
                self.take(',', "list 元素之間需要逗號")?;
                if self.symbol(']') { return Err(self.syntax("逗號之後需要元素")); }
            }
            self.take(']', "list 常值需要右方括號")?;
            ContainerItems::List(items)
        } else if let Some((key, value)) = ty.strip_prefix("dict{").and_then(|s|
            s.strip_suffix('}')).and_then(|s| s.split_once(',')) {
            self.take('{', "dict 常值需要左大括號")?;
            let mut items = BTreeMap::new();
            while !self.symbol('}') {
                let k = self.container_atom(key)?;
                self.take(':', "dict 鍵和值之間需要冒號")?;
                let v = self.container_atom(value)?;
                if items.insert(k, v).is_some() {
                    return Err(self.syntax("目前只支援鍵唯一的字典常值"));
                }
                if self.symbol('}') { break; }
                self.take(',', "dict 項目之間需要逗號")?;
                if self.symbol('}') { return Err(self.syntax("逗號之後需要鍵值")); }
            }
            self.take('}', "dict 常值需要右大括號")?;
            ContainerItems::Dictionary(items)
        } else {
            return Err(self.syntax("容器常值需要相符的容器型別宣告"));
        };
        Ok(Term { start, end: self.tokens[self.cursor - 1].end,
            kind: TermKind::Container(ContainerLiteral { ty: ty.to_owned(), items }) })
    }
}

fn checked_container_declaration(
    decl: &crate::syntax_shared::ParsedDeclaration<String, Term>, tokens: &[Token],
    audit: &mut crate::common_ir::SourceIdentityAudit<'_>,
) -> bool {
    use crate::common_ir::{NodeRole, NodeSpan};
    let Ok(cursor) = tokens.binary_search_by_key(&decl.start, |t| t.start) else { return false };
    let source = Source::new(audit.source.as_bytes().to_vec());
    let mut p = Parser { source: &source, tokens, cursor };
    if !p.keyword(if decl.mutable { "var" } else { "const" }) { return false; }
    p.cursor += 1;
    let name = p.tok().clone();
    if p.name().ok().as_ref() != Some(&decl.name) || p.take(':', "").is_err() { return false; }
    let type_start = p.tok().start;
    if p.local_type_annotation().ok().as_ref() != Some(&decl.ty) { return false; }
    let end = tokens[p.cursor - 1].end;
    let layout = match &decl.initializer {
        Some(init) => p.symbol('=') && tokens.get(p.cursor + 1).is_some_and(|t| t.start == init.start)
            && init.end == decl.end,
        None => end == decl.end && !p.symbol('='),
    };
    let span = Some(NodeSpan::new(decl.start, decl.end));
    layout && audit.record(NodeRole::Name, NodeSpan::new(name.start, name.end), span)
        && audit.record(NodeRole::Type, NodeSpan::new(type_start, end), span)
}

#[cfg(test)]
mod p19_literal_tests {
    use super::*;

    fn parsed(ty: &str, rhs: &str) -> Result<ContainerLiteral, Diagnostic> {
        let src = format!("fn main()->int{{\nvar values:{ty} = {rhs}\nreturn 0\n}}\n");
        let source = Source::new(src.into_bytes());
        let scan = lexer::scan(&source);
        assert!(scan.diagnostics.is_empty());
        let (_, functions) = Parser { source: &source, tokens: &scan.tokens, cursor: 0 }.file()?;
        let StmtKind::Declare(d) = &functions[0].body[0].kind else { panic!() };
        let TermKind::Container(value) = &d.initializer.as_ref().unwrap().kind else { panic!() };
        Ok(value.clone())
    }

    #[test]
    fn full_checker_accepts_typed_literal_and_rejects_implicit_copy() {
        let start = "fn main()->int{\nvar a:list[Optional[int,str]] = [1,none,\"文字\"]\n";
        inspect(&format!("{start}return 0\n}}\n")).unwrap();
        let error = inspect(&format!("{start}var b:list[Optional[str,int]] = a\nreturn 0\n}}\n")).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"), "{}", error.message);
    }

    #[test]
    fn optional_positions_distinguish_none_from_missing() {
        let list = parsed("list[Optional[int]]", "[none, 7]").unwrap();
        assert_eq!(list.lookup(&ContainerAtom::Integer(0)), Some(&ContainerAtom::Missing));
        assert_eq!(list.lookup(&ContainerAtom::Integer(1)), Some(&ContainerAtom::Integer(7)));
        assert_eq!(list.lookup(&ContainerAtom::Integer(2)), None);
        let dict = parsed("dict{str,Optional[int]}", r#"{"a": none}"#).unwrap();
        assert_eq!(dict.lookup(&ContainerAtom::Text("a".into())), Some(&ContainerAtom::Missing));
        assert_eq!(dict.lookup(&ContainerAtom::Text("b".into())), None);
    }

    #[test]
    fn declaration_drives_value_and_key_checks() {
        for (ty, rhs) in [("list[int]", "[true]"), ("list[Optional[bool]]", "[1]"),
                          ("dict{str,int}", "{2: 4}")] {
            assert_eq!(parsed(ty, rhs).unwrap_err().kind, DiagnosticKind::Static);
        }
    }
}

fn element_type(ty: &str) -> Option<(&str, &str)> {
    if let Some(value) = ty.strip_prefix("list[").and_then(|s| s.strip_suffix(']')) {
        Some(("int", value))
    } else {
        ty.strip_prefix("dict{")?.strip_suffix('}')?.split_once(',')
    }
}

impl<C: Fn(&str) -> CopyPermission> Checker<'_, C> {
    fn scoped_element_binding(&self, block: &ReadonlyElementBlock, vars: &mut Scopes)
        -> Result<Binding, Diagnostic> {
        let at = &block.selector;
        let TokenKind::Name(name) = &block.owner.kind else { return Err(self.error("需要容器名稱", at)); };
        let (selector, ready) = self.evaluate(at, vars)?;
        let owner = Self::lookup(vars, name).ok_or_else(|| self.error("找不到容器來源", at))?;
        let reference = self.checked_binding_use(owner, at)?;
        if owner.loan.is_some() { return Err(self.error("唯讀元素需要原始容器持有者", at)); }
        owner.value.read().map_err(|e| self.model_error(e, at))?;
        let (key, element) = element_type(reference.resolved.ty)
            .ok_or_else(|| self.error("唯讀綁定需要容器來源", at))?;
        if !ready || selector != key { return Err(self.error("容器索引或鍵的型別不符", at)); }
        self.check_borrow_conflicts(vars, owner.declaration_start, BorrowKind::SharedRead, at,
            "容器已有衝突借用")?;
        let mut binding = Binding::new(DeclarationId(block.alias.start), element.to_owned(), false,
            ValueSlot::initialized(self.permission(element)), None,
            Some(LoanBinding { fact: BorrowFact::new(owner.declaration_start, BorrowKind::SharedRead), active: true }));
        binding.scoped_element = true;
        Ok(binding)
    }
}

fn execute_element_body(statements: &[Stmt], alias: &str, element: &ContainerAtom)
    -> Result<Option<i64>, &'static str> {
    for stmt in statements {
        match &stmt.kind {
            StmtKind::Return(ret) => {
                let n = match &ret.kind {
                    TermKind::Name(name) if name == alias => match element {
                        ContainerAtom::Integer(n) => *n, _ => return Err("返回值需要 int"),
                    },
                    TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(n)) =>
                        crate::syntax_shared::integer_i64(n).map_err(|_| "整數返回值無效")?,
                    _ => return Err("此入口尚未支援該返回式"),
                };
                return Ok(Some(n));
            }
            StmtKind::If { branch, no } => {
                let test = match &branch.condition.kind {
                    TermKind::OptionalCondition(name, pred) if name == alias => match pred {
                        OptionalPredicate::IsMissing(equal) => matches!(element, ContainerAtom::Missing) == *equal,
                        OptionalPredicate::Candidate(Some(ty)) => matches!((element, ty.as_str()),
                            (ContainerAtom::Integer(_), "int") | (ContainerAtom::Text(_), "str")
                            | (ContainerAtom::Boolean(_), "bool") | (ContainerAtom::Decimal(..), "float")),
                        OptionalPredicate::Candidate(None) => return Err("候選型別缺失"),
                    },
                    TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Boolean(b)) => *b,
                    _ => return Err("此入口尚未支援該區塊條件"),
                };
                let selected = if test { Some(branch.body.statements.as_slice()) }
                    else { no.as_ref().map(|other| other.as_slice()) };
                if let Some(body) = selected {
                    if let Some(value) = execute_element_body(body, alias, element)? { return Ok(Some(value)); }
                }
            }
            _ => return Err("此入口尚未支援該區塊語句"),
        }
    }
    Ok(None)
}

/// 真私有 Checker 已驗證的直線容器入口。遇到此執行器尚未支援的
/// 語句會失敗；查找只傳遞元素參考，區塊所有出口共用同一次退票。
fn execute_container_main(text: &str) -> Result<i64, String> {
    use crate::borrow_ownership_gate::BorrowOwnershipGate;
    let checked = verify_source_with_ability(text, crate::ownership_policy::approved_leaf_copy_permission)
        .map_err(|d| d.message.to_string())?;
    let main = checked.functions.iter().find(|f| f.name == "main" && f.params.is_empty()
        && f.result_ty == "int" && !f.is_async).ok_or("需要同步 main()->int")?;
    let mut containers = HashMap::new();
    let mut gate = BorrowOwnershipGate::new();
    for stmt in main.body.iter() {
        match &stmt.kind {
            StmtKind::Declare(d) => {
                let id = DeclarationId(stmt.start);
                let content = match d.initializer.as_ref().map(|term| &term.kind) {
                    Some(TermKind::Container(value)) => {
                        gate.declare(id, ValueSlot::initialized(CopyPermission::Forbidden))
                            .map_err(|_| "容器持有者登錄失敗")?;
                        // 字面值在每次執行建立獨立的容器與內部字串。
                        value.clone()
                    }
                    Some(TermKind::Move(name)) => {
                        let (owner, storage): &mut (DeclarationId, Option<ContainerLiteral>) =
                            containers.get_mut(name.as_str()).ok_or("移動來源缺失")?;
                        if storage.is_none() { return Err("容器已移動".into()); }
                        gate.move_into(*owner, id).map_err(|_| "容器整筆移動失敗")?;
                        storage.take().expect("來源已通過移動預檢")
                    }
                    _ => return Err("此入口只支援具常值或整筆移動的容器宣告".into()),
                };
                containers.insert(d.name.as_str(), (id, Some(content)));
            }
            StmtKind::ReadonlyElement(block) => {
                let (TokenKind::Name(owner), TokenKind::Name(alias)) = (&block.owner.kind, &block.alias.kind)
                    else { return Err("綁定名稱失配".into()) };
                let (id, storage) = containers.get(owner.as_str()).ok_or("容器來源缺失")?;
                let value = storage.as_ref().ok_or("容器已移動")?;
                let selector = match &block.selector.kind {
                    TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(text)) =>
                        ContainerAtom::Integer(crate::syntax_shared::integer_i64(text).map_err(|_| "索引無效")?),
                    TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Boolean(b)) => ContainerAtom::Boolean(*b),
                    TermKind::Literal("str") => {
                        let token = checked.tokens.binary_search_by_key(&block.selector.start, |t| t.start)
                            .ok().and_then(|i| checked.tokens.get(i)).ok_or("字串鍵來源缺失")?;
                        let TokenKind::StringLiteral(key) = &token.kind else { return Err("字串鍵來源失配".into()); };
                        ContainerAtom::Text(key.clone())
                    }
                    _ => return Err("此入口尚未支援該選擇式".into()),
                };
                let ticket = gate.borrow(*id, BorrowKind::SharedRead).map_err(|_| "容器借用衝突")?;
                let result = value.lookup(&selector).ok_or("容器索引越界或缺鍵")
                    .and_then(|element| execute_element_body(&block.body, alias, element));
                gate.finish_method_scope(&[ticket]).map_err(|_| "容器借用歸還失敗")?;
                if let Some(n) = result.map_err(str::to_owned)? { return Ok(n); }
            }
            StmtKind::Return(ret) => if let TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(n)) = &ret.kind {
                return crate::syntax_shared::integer_i64(n).map_err(|_| "整數返回值無效".into());
            } else { return Err("此入口尚未支援該返回式".into()); },
            _ => return Err("此入口尚未支援該語句".into()),
        }
    }
    Err("缺少返回值".into())
}

#[cfg(test)]
mod p19_scoped_execution {
    use super::*;

    #[test]
    fn moved_dictionary_owns_original_values() {
        let text = "fn main()->int{\nvar first:dict{str,int} = {\"key\":17}\nvar next:dict{str,int} = first.move()\nwith next[\"key\"] as it {\nreturn it\n}\nreturn 0\n}\n";
        assert_eq!(execute_container_main(text), Ok(17));
        assert!(inspect(&text.replace("with next", "with first")).is_err());
    }
    #[test]
    fn 繁中來源從解析到查找與區塊返回() {
        let text = include_str!("../../tests/fixtures/p19/唯讀字典元素.voil");
        assert_eq!(execute_container_main(text), Ok(9));
    }

    #[test]
    fn const_and_var_read_the_selected_integer_and_exit_scope() {
        for binding in ["const", "var"] {
            let code = format!("fn main()->int{{\n{binding} xs:list[int] = [3,8]\nwith xs[1] as item {{\nreturn item\n}}\nreturn 0\n}}\n");
            assert_eq!(execute_container_main(&code), Ok(8));
        }
    }

    #[test]
    fn present_none_enters_body_and_absent_key_fails() {
        let source = |key| format!("fn main()->int{{\nvar xs:dict{{str,Optional[int]}} = {{\"k\":none}}\nwith xs[\"{key}\"] as item {{\nreturn 9\n}}\nreturn 0\n}}\n");
        assert_eq!(execute_container_main(&source("k")), Ok(9));
        assert_eq!(execute_container_main(&source("missing")), Err("容器索引越界或缺鍵".into()));
    }

    #[test]
    fn readonly_owner_cannot_move_even_after_last_alias_use() {
        let code = "fn main()->int{\nvar xs:list[int] = [9]\nwith xs[0] as item {\nvar n:int = item\nvar stolen:list[int] = xs.move()\nreturn n\n}\nreturn 0\n}\n";
        assert!(inspect(code).unwrap_err().message.contains("借用"));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OptionalPredicate {
    IsMissing(bool),
    Candidate(Option<String>),
}

impl Parser<'_> {
    fn optional_condition(&mut self) -> Result<Option<Term>, Diagnostic> {
        let start = self.tok().start;
        let TokenKind::Name(name) = self.tok().kind.clone() else { return Ok(None) };
        let op = self.cursor + 1;
        let pred = if matches!(self.tokens.get(op).map(|t| &t.kind), Some(TokenKind::Name(s)) if s == "is") {
            self.cursor += 2;
            OptionalPredicate::Candidate(if self.symbol(')') { None } else { Some(self.name()?) })
        } else if (crate::syntax_shared::symbol(self.tokens, op, '!') || crate::syntax_shared::symbol(self.tokens, op, '='))
            && crate::syntax_shared::symbol(self.tokens, op+1, '=')
            && self.tokens[op].end == self.tokens[op+1].start
            && crate::syntax_shared::keyword(self.tokens, op+2, "none") {
            self.cursor += 4;
            OptionalPredicate::IsMissing(self.tokens[op].kind == TokenKind::Symbol('='))
        } else { return Ok(None) };
        Ok(Some(Term { start, end: self.tokens[self.cursor-1].end,
            kind: TermKind::OptionalCondition(name, pred) }))
    }
}

impl<C: Fn(&str) -> CopyPermission> Checker<'_, C> {
    fn permit_first_candidate(vars: &mut Scopes, name: &str, statements: &[Stmt]) {
        if let Some(Stmt { kind: StmtKind::If { branch, .. }, .. }) = statements.first() {
            if let Some(binding) = Self::lookup_mut(vars, name) {
                binding.candidate_check_at = Some(branch.condition.start);
            }
        }
    }
}

#[cfg(test)]
mod p19_candidate_execution {
    use super::*;
    fn program(value: &str, body: &str) -> String {
        format!("fn main()->int{{\nvar xs:list[Optional[int,str]] = [{value}]\nwith xs[0] as x {{\n{body}\n}}\nreturn 0\n}}\n")
    }
    #[test]
    fn named_alias_is_checked_after_direct_nonmissing_guard() {
        let body = "if(x != none){\nif(x is str){\nreturn 7\n} else {\nreturn 2\n}\n}\nreturn 9";
        for (value, expected) in [("none", 9), ("1", 2), ("\"值\"", 7)] {
            assert_eq!(execute_container_main(&program(value, body)), Ok(expected));
        }
    }
    #[test]
    fn missing_candidate_and_missing_name_prioritize_name_resolution() {
        let error = inspect(&program("none", "if(missing is){\nreturn 2\n}")).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("未宣告"));
        assert_eq!(inspect(&program("none", "if(x is){\nreturn 2\n}")).unwrap_err().kind,
            DiagnosticKind::Syntax);
    }
    #[test]
    fn candidate_needs_same_name_immediately_after_proof() {
        for body in ["if(x is int){\nreturn 1\n}",
            "if(x != none){\nvar n:int = 0\nif(x is int){\nreturn 1\n}\n}"] {
            assert!(inspect(&program("1", body)).unwrap_err().message.contains("非缺失證明"));
        }
    }
}
