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
