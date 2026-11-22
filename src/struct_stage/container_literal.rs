/// P19 具型別常值只保存已驗證的純值。讀取借用原容器內的值。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ContainerAtom {
    Nested(Box<ContainerLiteral>),
    ElementName(String),
    Missing,
    Integer(i64),
    Boolean(bool),
    Text(String),
    Decimal(u64, i32),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ContainerItems {
    List(Vec<ContainerAtom>),
    Dictionary(BTreeMap<ContainerAtom, ContainerAtom>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ContainerLiteral {
    ty: String,
    items: ContainerItems,
}

impl ContainerLiteral {
    fn values(&self) -> Box<dyn Iterator<Item = &ContainerAtom> + '_> {
        match &self.items {
            ContainerItems::List(items) => Box::new(items.iter()),
            ContainerItems::Dictionary(items) => Box::new(items.values()),
        }
    }

    fn visit_elements<E>(&self, f: &mut impl FnMut(&str, &ContainerAtom) -> Result<(), E>) -> Result<(), E> {
        let (_, ty) = element_type(&self.ty).expect("已解析容器型別");
        for atom in self.values() {
            match atom {
                ContainerAtom::Nested(child) => child.visit_elements(f)?,
                _ => f(ty, atom)?,
            }
        }
        Ok(())
    }

    fn fill_and_count(&mut self, resolve: &mut impl FnMut(&mut ContainerAtom) -> ContainerResult<()>)
        -> ContainerResult<usize> {
        let mut count = 1;
        let mut visit = |atom: &mut ContainerAtom| -> ContainerResult<()> {
            resolve(atom)?;
            count += match atom {
                ContainerAtom::Nested(child) => child.fill_and_count(resolve)?,
                ContainerAtom::Text(_) => 1,
                _ => 0,
            };
            Ok(())
        };
        match &mut self.items {
            ContainerItems::List(items) => items.iter_mut().try_for_each(&mut visit)?,
            ContainerItems::Dictionary(items) => {
                items.values_mut().try_for_each(&mut visit)?;
                count += items.keys().filter(|k| matches!(k, ContainerAtom::Text(_))).count();
            }
        }
        Ok(count)
    }

    fn lookup(&self, selector: &ContainerAtom) -> Option<&ContainerAtom> {
        match (&self.items, selector) {
            (ContainerItems::List(items), ContainerAtom::Integer(index)) =>
                usize::try_from(*index).ok().and_then(|i| items.get(i)),
            (ContainerItems::Dictionary(items), key) => items.get(key),
            _ => None,
        }
    }
}

impl<const PUBLIC:bool> LanguageParser<'_,PUBLIC> {
    fn container_atom(&mut self, expected: &str) -> Result<ContainerAtom, Diagnostic> {
        if element_type(expected).is_some() {
            let term = self.container_literal(expected)?;
            let TermKind::Container(inner) = term.kind else { unreachable!() };
            return Ok(ContainerAtom::Nested(Box::new(inner)));
        }
        let token = self.tok();
        let candidates = expected.strip_prefix("Optional[").and_then(|s| s.strip_suffix(']'));
        let (ty, atom) = match &token.kind {
            TokenKind::Keyword(word) if word == "none" && candidates.is_some() =>
                ("none", ContainerAtom::Missing),
            TokenKind::Name(name) if candidates.is_some() =>
                (expected, ContainerAtom::ElementName(name.clone())),
            TokenKind::Integer(text) => ("int", ContainerAtom::Integer(
                crate::syntax_shared::integer_i64(text).map_err(|_| self.syntax("容器整數常值超出範圍"))?)),
            TokenKind::Keyword(word) if word == "true" || word == "false" =>
                ("bool", ContainerAtom::Boolean(word == "true")),
            TokenKind::StringLiteral(text) => ("str", ContainerAtom::Text(text.clone())),
            TokenKind::Decimal(text) => {
                let (coefficient, exponent) = (if PUBLIC {decimal::parts(text)} else {Self::decimal64_checked_subset(text)})
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
                let key_token = self.tok().clone();
                let k = self.container_atom(key)?;
                if items.contains_key(&k) {
                    return Err(Diagnostic { kind: DiagnosticKind::Static,
                        span: Some(self.source.span(key_token.start, key_token.end)),
                        message: "字典常值的鍵重複".into() });
                }
                self.take(':', "dict 鍵和值之間需要冒號")?;
                let v = self.container_atom(value)?;
                items.insert(k, v);
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

type ContainerResult<T> = Result<T, &'static str>;

#[derive(Clone)]
enum ContainerLocal {
    Owner(DeclarationId),
    Element(DeclarationId, ContainerAtom),
    Integer(i64),
    Boolean(bool),
}

type LocalNames = HashMap<String, ContainerLocal>;

struct ContainerFrame<'a> {
    tokens: &'a [Token],
    builtin_len: bool,
    gate: crate::resource_borrow_pipeline::SafetyPipeline,
    values: BTreeMap<DeclarationId, (std::ops::Range<u64>, ContainerLiteral)>,
    next_resource: u64,
}

impl ContainerFrame<'_> {
    fn element(&self, names: &LocalNames, name: &str) -> ContainerResult<&ContainerAtom> {
        let Some(ContainerLocal::Element(id, key)) = names.get(name) else { return Err("找不到唯讀元素"); };
        self.values.get(id).and_then(|(_, v)| v.lookup(key)).ok_or("元素來源失效")
    }

    fn integer(&mut self, term: &Term, names: &LocalNames) -> ContainerResult<i64> {
        match &term.kind {
            TermKind::Group(inner) => self.integer(inner, names),
            TermKind::BinaryInt(binary) => {
                let left = self.integer(&binary.left, names)?;
                let right = self.integer(&binary.right, names)?;
                match binary.operator {
                    '+' => left.checked_add(right),
                    '-' => left.checked_sub(right),
                    _ => None,
                }.ok_or("整數運算溢位或運算子無效")
            }
            TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Integer(n)) =>
                crate::syntax_shared::integer_i64(n).map_err(|_| "整數值無效"),
            TermKind::Name(n) => match names.get(n) {
                Some(ContainerLocal::Integer(value)) => Ok(*value),
                Some(ContainerLocal::Element(..)) => match self.element(names, n)? {
                    ContainerAtom::Integer(value) => Ok(*value), _ => Err("返回值需要 int"),
                },
                _ => Err("找不到整數來源"),
            },
            TermKind::Call(call) if self.builtin_len && call.name == "len" && !call.awaited => {
                let [arg] = call.arguments.as_slice() else { return Err("len 引數數量不符"); };
                let TermKind::Name(n) = &arg.kind else { return Err("len 需要具名容器"); };
                let Some(ContainerLocal::Owner(id)) = names.get(n) else { return Err("len 容器缺失"); };
                let (_, value) = self.values.get(id).ok_or("容器已移動")?;
                let ticket = self.gate.borrow(*id, BorrowKind::SharedRead).map_err(|_| "len 借用衝突")?;
                let count = match &value.items {
                    ContainerItems::List(values) => values.len(),
                    ContainerItems::Dictionary(values) => values.len(),
                };
                self.gate.finish_method(&[ticket]).map_err(|_| "len 退借失敗")?;
                i64::try_from(count).map_err(|_| "容器長度超出 int")
            }
            _ => Err("此入口尚未支援該整數運算式"),
        }
    }

    fn condition(&self, term: &Term, names: &LocalNames) -> ContainerResult<bool> {
        match &term.kind {
            TermKind::OptionalCondition(name, pred) => {
                let v = self.element(names, name)?;
                match pred {
                    OptionalPredicate::IsMissing(equal) => Ok(matches!(v, ContainerAtom::Missing) == *equal),
                    OptionalPredicate::Candidate(Some(ty)) => Ok(matches!((v, ty.as_str()),
                        (ContainerAtom::Integer(_), "int") | (ContainerAtom::Text(_), "str") |
                        (ContainerAtom::Boolean(_), "bool") | (ContainerAtom::Decimal(..), "float"))),
                    _ => Err("候選檢查未獲授權"),
                }
            }
            TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Boolean(b)) => Ok(*b),
            TermKind::Name(n) => match names.get(n) {
                Some(ContainerLocal::Boolean(b)) => Ok(*b),
                _ => Err("條件需要 bool"),
            },
            _ => Err("此入口尚未支援該區塊條件"),
        }
    }

    fn selector(&mut self, term: &Term, names: &LocalNames) -> ContainerResult<ContainerAtom> {
        match &term.kind {
            TermKind::Literal("str") => {
                let i = self.tokens.binary_search_by_key(&term.start, |t| t.start).map_err(|_| "字串鍵來源缺失")?;
                let TokenKind::StringLiteral(s) = &self.tokens[i].kind else { return Err("字串鍵來源失配"); };
                Ok(ContainerAtom::Text(s.clone()))
            }
            TermKind::Scalar(crate::syntax_shared::ScalarLiteral::Boolean(b)) => Ok(ContainerAtom::Boolean(*b)),
            _ => self.integer(term, names).map(ContainerAtom::Integer),
        }
    }

    fn create(&mut self, id: DeclarationId, init: &Term, names: &LocalNames) -> ContainerResult<()> {
        let resource = match &init.kind {
            TermKind::Move(source) => {
                let Some(ContainerLocal::Owner(from)) = names.get(source) else { return Err("移動來源缺失"); };
                self.gate.move_into(*from, id).map_err(|_| "容器整筆移動失敗")?;
                self.values.remove(from).ok_or("容器已移動")?
            }
            TermKind::Container(literal) => {
                let mut value = literal.clone();
                let mut resolve = |atom: &mut ContainerAtom| -> ContainerResult<()> {
                    if let ContainerAtom::ElementName(name) = atom {
                        *atom = match self.element(names, name)? {
                            ContainerAtom::Missing => ContainerAtom::Missing,
                            ContainerAtom::Integer(n) => ContainerAtom::Integer(*n),
                            ContainerAtom::Boolean(b) => ContainerAtom::Boolean(*b),
                            ContainerAtom::Decimal(c,e) => ContainerAtom::Decimal(*c,*e),
                            _ => return Err("元素不能自共享借用複製"),
                        };
                    }
                    Ok(())
                };
                let count = value.fill_and_count(&mut resolve)?;
                let end = self.next_resource.checked_add(count as u64).ok_or("容器資源身分耗盡")?;
                let resources = self.next_resource..end;
                self.gate.declare(id, CopyPermission::Forbidden).map_err(|_| "容器登錄失敗")?;
                for resource in resources.clone() {
                    self.gate.register_resource(id, resource).map_err(|_| "容器資源登錄失敗")?;
                }
                self.next_resource = end;
                (resources, value)
            }
            _ => return Err("容器需要常值或明確移動"),
        };
        self.values.insert(id, resource);
        Ok(())
    }

    fn block(&mut self, body: &[Stmt], mut names: LocalNames) -> ContainerResult<Option<i64>> {
        let mut owners = Vec::new();
        let result = (|| {
            for stmt in body {
                let nested = match &stmt.kind {
                    StmtKind::Declare(d) => {
                        let init = d.initializer.as_ref().ok_or("宣告需要初始值")?;
                        let local = match d.ty.as_str() {
                            "int" => ContainerLocal::Integer(self.integer(init, &names)?),
                            "bool" => ContainerLocal::Boolean(self.condition(init, &names)?),
                            _ => {
                                let id = DeclarationId(stmt.start);
                                self.create(id, init, &names)?;
                                owners.push(id);
                                ContainerLocal::Owner(id)
                            }
                        };
                        names.insert(d.name.clone(), local);
                        None
                    }
                    StmtKind::ReadonlyElement(block) => {
                        let (TokenKind::Name(owner), TokenKind::Name(alias)) = (&block.owner.kind, &block.alias.kind)
                            else { return Err("綁定名稱失配") };
                        let Some(ContainerLocal::Owner(id)) = names.get(owner) else { return Err("容器來源缺失"); };
                        let key = self.selector(&block.selector, &names)?;
                        let (_, storage) = self.values.get(id).ok_or("容器已移動")?;
                        let ticket = self.gate.borrow(*id, BorrowKind::SharedRead).map_err(|_| "容器借用衝突")?;
                        let selected = if storage.lookup(&key).is_some() {
                            let mut inner = names.clone();
                            inner.insert(alias.clone(), ContainerLocal::Element(*id, key));
                            self.block(&block.body, inner)
                        } else { Err("容器索引越界或缺鍵") };
                        self.gate.finish_method(&[ticket]).map_err(|_| "容器借用歸還失敗")?;
                        selected?
                    }
                    StmtKind::If { branch, no } => {
                        if self.condition(&branch.condition, &names)? { self.block(&branch.body, names.clone())? }
                        else if let Some(other) = no { self.block(other.as_slice(), names.clone())? }
                        else { None }
                    }
                    StmtKind::Return(ret) => Some(self.integer(ret, &names)?),
                    _ => return Err("此入口尚未支援該語句"),
                };
                if nested.is_some() { return Ok(nested); }
            }
            Ok(None)
        })();
        for id in owners.into_iter().rev() {
            if let Some((resource, value)) = self.values.remove(&id) {
                drop(value);
                for part in resource.rev() {
                    self.gate.record_resource_cleanup(id, part).map_err(|_| "容器釋放來源失配")?;
                }
                self.gate.record_cleanup(id).map_err(|_| "容器所有權未閉合")?;
            }
        }
        result
    }
}

fn execute_container_main(text: &str) -> Result<i64, String> {
    let checked = verify_source_with_ability(text, crate::ownership_policy::approved_leaf_copy_permission)
        .map_err(|d| d.message.to_string())?;
    let main = checked.functions.iter().find(|f| f.name == "main" && f.params.is_empty()
        && f.result_ty == "int" && !f.is_async).ok_or("需要同步 main()->int")?;
    let mut frame = ContainerFrame {
        tokens: &checked.tokens, builtin_len: !checked.functions.iter().any(|f| f.name == "len"),
        gate: crate::resource_borrow_pipeline::SafetyPipeline::new(), values: BTreeMap::new(), next_resource: 1,
    };
    let result = frame.block(&main.body, HashMap::new());
    frame.gate.verify_static_borrows(&[]).map_err(|_| "容器作用域仍有未歸還借用")?;
    frame.gate.audit_finished().map_err(|_| "容器資源未全部釋放")?;
    result?.ok_or_else(|| "缺少返回值".into())
}

#[cfg(test)]
mod p19_scoped_execution {
    use super::*;

    #[test]
    fn cross_container_copy_uses_the_whole_declared_set() {
        let code = include_str!("../../tests/fixtures/p19/跨容器元素複製.voil");
        for (value, expected) in [("7",1), ("true",0), ("none",0)] {
            assert_eq!(execute_container_main(&code.replace("[7]", &format!("[{value}]"))), Ok(expected));
        }
        for value in ["7", "none"] {
            let source = code.replace("bool", "str").replace("[7]", &format!("[{value}]"));
            assert!(inspect(&source).unwrap_err().message.contains("Copyable"));
        }
        assert!(inspect(&code.replace("Optional[bool,int]", "Optional[int]")).is_err());
        let bad = code.replace("dest[\"值\"]", "dest[\"缺\"]");
        assert_eq!(execute_container_main(&bad), Err("容器索引越界或缺鍵".into()));
    }

    #[test]
    fn nested_shared_blocks_return_separate_tickets_and_allow_subsequent_move() {
        let code = "fn main()->int{\nvar a:list[int] = [1,2]\nwith a[0] as first {\nwith a[1] as second {\nconst n:int = second\n}\nconst m:int = first\n}\nvar b:list[int] = a.move()\nreturn len(b)\n}\n";
        assert_eq!(execute_container_main(code), Ok(2));
        assert!(inspect(&code.replace("return len(b)", "return first")).is_err());
        assert!(inspect(&code.replace("const m:int = first", "var stolen:list[int] = a.move()")).is_err());
    }

    #[test]
    fn nested_container_move_and_missing_lookup_clean_every_owned_part() {
        let code = "fn main()->int{\nvar x:list[dict{str,str}] = [{\"鍵\":\"值\"},{}]\nvar y:list[dict{str,str}] = x.move()\nwith y[0] as inner {\nreturn len(y)\n}\nreturn 0\n}\n";
        assert_eq!(execute_container_main(code), Ok(2));
        assert_eq!(execute_container_main(&code.replace("y[0]", "y[2]")), Err("容器索引越界或缺鍵".into()));
        assert!(inspect(&code.replace("x.move()", "x")).is_err());
    }

    #[test]
    fn builtin_length_reads_list_and_dictionary_without_consuming() {
        for (ty, literal, n) in [("list[int]", "[]", 0), ("list[int]", "[1,1,3]", 3),
            ("dict{str,int}", "{}", 0), ("dict{str,int}", "{\"一\":7,\"二\":7}", 2)] {
            let code = format!("fn main()->int{{\nvar xs:{ty} = {literal}\nconst count:int = len(xs)\nvar next:{ty} = xs.move()\nreturn len(next)\n}}\n");
            assert_eq!(execute_container_main(&code), Ok(n));
            assert_eq!(execute_container_main(&code.replace("return len(next)", "return count")), Ok(n));
            assert!(inspect(&code.replace("return len(next)", "return len(xs)")).is_err());
        }
    }

    #[test]
    fn composed_length_and_out_of_bounds_execute_from_source() {
        let source = "fn main()->int{\nvar a:list[int] = [4,4]\nreturn (len(a) + len(a)) - 1\n}\n";
        assert_eq!(execute_container_main(source), Ok(3));
        let missing = source.replace("return (len(a) + len(a)) - 1",
            "with a[2] as item {\nreturn item\n}\nreturn 0");
        assert_eq!(execute_container_main(&missing), Err("容器索引越界或缺鍵".into()));
        assert!(inspect(&source.replace("len(a) + len(a)", "len(1)")).is_err());
    }

    #[test]
    fn length_inside_readonly_scope_returns_only_its_own_ticket() {
        let code = include_str!("../../tests/fixtures/p19/容器長度.voil");
        assert_eq!(execute_container_main(code), Ok(2));
        assert!(inspect(&code.replace("len(entries)", "len(item)")).is_err());
    }

    #[test]
    fn repeated_key_points_to_the_second_literal() {
        let code = "fn main()->int{\nvar d:dict{str,int} = {\"鍵\":1,\"鍵\":2}\nreturn 0\n}\n";
        let d = inspect(code).unwrap_err();
        let at = code.rfind("\"鍵\"").unwrap();
        assert_eq!(d.kind, DiagnosticKind::Static);
        assert_eq!(d.span, Some(Source::new(code.as_bytes().to_vec()).span(at, at + "\"鍵\"".len())));
    }

    #[test]
    fn moved_dictionary_owns_original_values() {
        let text = include_str!("../../tests/fixtures/p19/移動後讀取.voil");
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
    IndexedCandidate { selector_type: &'static str, candidate: Option<String> },
    IsMissing(bool),
    Candidate(Option<String>),
}

impl<const PUBLIC:bool> LanguageParser<'_,PUBLIC> {
    fn optional_condition(&mut self) -> Result<Option<Term>, Diagnostic> {
        let start = self.tok().start;
        let TokenKind::Name(name) = self.tok().kind.clone() else { return Ok(None) };
        let op = self.cursor + 1;
        // 保留非具名索引形狀供 Checker 區分 AQ03、AN04；絕不核准其使用。
        if crate::syntax_shared::symbol(self.tokens, op, '[')
            && crate::syntax_shared::symbol(self.tokens, op + 2, ']')
            && matches!(self.tokens.get(op + 3).map(|t| &t.kind), Some(TokenKind::Keyword(s)) if s == "is") {
            let selector_type = match &self.tokens[op + 1].kind {
                TokenKind::Integer(_) => "int",
                TokenKind::StringLiteral(_) => "str",
                TokenKind::Keyword(b) if b == "true" || b == "false" => "bool",
                _ => return Ok(None),
            };
            self.cursor = op + 4;
            let candidate = if self.symbol(')') { None } else { Some(self.name()?) };
            return Ok(Some(Term { start, end: self.tokens[self.cursor - 1].end,
                kind: TermKind::OptionalCondition(name, OptionalPredicate::IndexedCandidate { selector_type, candidate }) }));
        }
        let pred = if matches!(self.tokens.get(op).map(|t| &t.kind), Some(TokenKind::Keyword(s)) if s == "is") {
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

fn visit_candidate_sites(name: &str, statements: &[Stmt], mut permit: impl FnMut(usize)) {
    let mut next = statements.first();
    while let Some(Stmt { kind: StmtKind::If { branch, no }, .. }) = next {
        match &branch.condition.kind {
            TermKind::OptionalCondition(target, OptionalPredicate::Candidate(_)) if target == name => {
                permit(branch.condition.start);

            }
            TermKind::Name(_) | TermKind::Scalar(_) => {}
            // 未知副作用的普通條件可能使非缺失證明失效。
            _ => break,
        }
        next = match no {
            Some(crate::syntax_shared::ParsedElse::ElseIf(arm)) => Some(arm.as_ref()),
            _ => None,
        };
    }
}

impl<C: Fn(&str) -> CopyPermission> Checker<'_, C> {
    fn permit_candidates(vars: &mut Scopes, name: &str, statements: &[Stmt]) {
        let Some(binding) = Self::lookup_mut(vars, name) else { return; };
        visit_candidate_sites(name, statements, |site| { binding.candidate_sites.insert(site); });
    }
}

#[cfg(test)]
mod p19_candidate_execution {
    use super::*;
    fn program(value: &str, body: &str) -> String {
        format!("fn main()->int{{\nvar xs:list[Optional[int,str]] = [{value}]\nwith xs[0] as x {{\n{body}\n}}\nreturn 0\n}}\n")
    }
    #[test]
    fn arbitrary_candidate_order_and_interleaved_ordinary_branches() {
        let body = "if(x != none){\nif(false){\nreturn 0\n} else if(x is str){\nreturn 1\n} else if(false){\nreturn 3\n} else if(x is int){\nreturn 2\n}\n}\nreturn 9";
        assert_eq!(execute_container_main(&program("5", body)), Ok(2));
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
    fn optional_element_expression_is_a_static_type_error() {
        let src = program("1", "if(xs[0] is int){\nreturn 1\n}");
        let err = inspect(&src).unwrap_err();
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("具名"));
        let src = src.replace("list[Optional[int,str]]", "list[int]");
        assert_eq!(inspect(&src).unwrap_err().kind, DiagnosticKind::Syntax);
    }
    #[test]
    fn nested_repeat_reports_only_the_redundant_check_and_still_executes() {
        let source = program("5", "if(x != none){\nif(x is int){\nif(x != none){\nif(x is int){\nreturn 7\n}\n}\n}\n}\nreturn 0");
        let start = source.rfind("x is int").unwrap();
        let text = Source::new(source.as_bytes().to_vec());
        assert_eq!(inspect_redundant_candidate_warnings(&source).unwrap(),
            vec![text.span(start, start + "x is int".len())]);
        assert_eq!(execute_container_main(&source), Ok(7));
    }
    #[test]
    fn repeat_after_local_statement_is_scoped_and_calls_invalidate_it() {
        let body = "if(x != none){\nif(x is int){\nconst n:int = 1\nif(x != none){\nif(x is int){\nreturn 7\n}\n}\n}\n}\nreturn 0";
        let source = program("5", body);
        assert_eq!(inspect_redundant_candidate_warnings(&source).unwrap().len(), 1);
        let called = format!("fn step()->int{{\nreturn 1\n}}\n{}", source.replace("= 1\nif", "= step()\nif"));
        assert!(inspect_redundant_candidate_warnings(&called).unwrap().is_empty());
        assert!(inspect(&source.replace("n:int", "is:int")).is_err());
    }

    #[test]
    fn candidate_chain_has_no_small_fixed_count_and_does_not_escape() {
        for count in [1, 8, 32] {
            let chain = "if(x is str){\nreturn 3\n} else ".repeat(count);
            let body = format!("if(x != none){{\n{chain}if(x is int){{\nreturn 2\n}}\n}}\nreturn 9");
            assert_eq!(execute_container_main(&program("7", &body)), Ok(2));
            let outside = format!("if(x != none){{\n{chain}if(x is int){{\nreturn 2\n}}\n}}\nif(x is int){{\nreturn 4\n}}");
            assert!(inspect(&program("7", &outside)).unwrap_err().message.contains("非缺失證明"));
        }
    }

    #[test]
    fn candidate_needs_same_name_immediately_after_proof() {
        for body in ["if(x is int){\nreturn 1\n}",
            "if(x != none){\nvar n:int = 0\nif(x is int){\nreturn 1\n}\n}"] {
            assert!(inspect(&program("1", body)).unwrap_err().message.contains("非缺失證明"));
        }
    }
}
