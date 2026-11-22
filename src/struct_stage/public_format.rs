//! 以同一個解析器檢查插值中的純量，保留呼叫端的詞彙作用域。
use super::*;

pub(super) fn logic_types(operator: &str, ty: &str) -> bool {
    match operator {
        "and" | "or" => ty == "bool",
        "!=" => matches!(ty, "bool" | "int" | "float"),
        "==" | "<" | "<=" | ">" | ">=" => ty == "int",
        _ => false,
    }
}

fn format_scalar(t:&Term,tokens:&[Token],resolve:&impl Fn(&Term)->Option<(String,Number)>)->Option<(String,Number)> {
    let (ty,n)=match &t.kind {
        TermKind::Group(inner)=>return format_scalar(inner,tokens,resolve),
        TermKind::Scalar(ScalarLiteral::Integer(v))=>("int",Number::Constant(integer_i64(v).ok()?)),
        TermKind::Scalar(ScalarLiteral::Boolean(v))=>("bool",Number::Constant(i64::from(*v))),
        TermKind::Literal("float")=>{
            let i=tokens.binary_search_by_key(&t.start,|t|t.start).ok()?;
            let TokenKind::Decimal(text)=&tokens[i].kind else {return None};
            let (c,e)=decimal::parts(text)?;
            ("float",Number::Constant(decimal::encode(c,e)))
        }
        TermKind::BinaryInt(node)=>{
            let (ty,a)=format_scalar(&node.left,tokens,resolve)?;
            let (other,b)=format_scalar(&node.right,tokens,resolve)?;
            if ty!=other {return None;}
            let (a,b)=(Box::new(a),Box::new(b));
            match ty.as_str() { "int"=>("int",Number::Binary(a,node.operator,b)),
                "float" if matches!(node.operator, '+' | '-')=>("float",Number::Decimal(a,node.operator,b)), _=>return None }
        }
        TermKind::Not(rhs)=>{
            let (ty,n)=format_scalar(rhs,tokens,resolve)?;
            if ty!="bool" {return None;}
            ("bool",Number::Logic("==",Box::new(n),Box::new(Number::Constant(0))))
        }
        TermKind::Logic(op,a,b)=>{
            let (ty,a)=format_scalar(a,tokens,resolve)?;
            let (other,b)=format_scalar(b,tokens,resolve)?;
            if other!=ty || !logic_types(op,&ty) {return None;}
            ("bool",Number::Logic(op,Box::new(a),Box::new(b)))
        }
        TermKind::Name(_) | TermKind::UnresolvedZeroArgs(_) => return resolve(t),
        _ => return None,
    };
    Some((ty.into(),n))
}

pub(super) fn format_parts(tokens:&[Token],term:&Term,text:impl Fn(&str)->Option<String>,
    scalar:impl Fn(&Term)->Option<(String,Number)>)->Option<Vec<FormatPart>> {
    let index=tokens.binary_search_by_key(&term.start,|t|t.start).ok()?;
    let TokenKind::StringLiteral(template)=&tokens.get(index+1)?.kind else {return None};
    let mut chars = template.chars().peekable();
    let mut parts = Vec::new();
    let mut literal = String::new();
    while let Some(ch) = chars.next() {
        if ch != '{' && ch != '}' {
            literal.push(ch);
        } else if chars.next_if_eq(&ch).is_some() {
            literal.push(ch);
        } else if ch == '}' {
            return None;
        } else {
            if !literal.is_empty() {
                parts.push(FormatPart::Literal(std::mem::take(&mut literal)));
            }
            let mut field = String::new();
            loop {
                let c = chars.next()?;
                if c == '}' { break; }
                field.push(c);
            }
            let (name, spec) = field.split_once(':').unwrap_or((&field, ""));
            let name=name.trim();
            let part = if let Some(slot)=text(name) {
                if !matches!(spec,""|"s") {return None;}
                FormatPart::Text(slot)
            } else {
                if name.is_empty() {return None;}
                let source=Source::new(name.as_bytes().to_vec());
                let scan=lexer::scan(&source);
                if !scan.diagnostics.is_empty() {return None;}
                let mut parser=LanguageParser::<true> {source:&source,tokens:&scan.tokens,cursor:0};
                let expression=parser.term().ok()?;
                if !parser.eof() {return None;}
                let (ty,value)=format_scalar(&expression,&scan.tokens,&scalar)?;
                match (ty.as_str(),spec) {
                    ("bool","") => FormatPart::Boolean(value),
                    ("float","") => FormatPart::Float(value,None),
                    ("float",format) => {
                        let places=format.strip_prefix('.')?.strip_suffix('f')?.parse::<usize>().ok()?;
                        if places>398 {return None;}
                        FormatPart::Float(value,Some(places))
                    }
                    ("int","" | "d") => FormatPart::Decimal(value),
                    ("int","x") => FormatPart::Hex(value),
                    _ => return None,
                }
            };
            parts.push(part);
        }
    }
    parts.push(FormatPart::Literal(literal));
    Some(parts)
}

