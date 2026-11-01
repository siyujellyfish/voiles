//! F00：跨解析器接受／拒絕與來源診斷回歸基準。
//! 只使用已核准的公開 MVP-A 和私有 P13～P16 交集；不創造新語言語意。

use crate::{Diagnostic, DiagnosticKind};
use crate::{flow, functions, lexer, names, parser, source::Source, struct_stage, types};

fn 公開正式檢查(原始: &str) -> Result<(), Diagnostic> {
    let source = Source::new(原始.as_bytes().to_vec());
    if let Some(error) = source.lexical_diagnostics().into_iter().next() {
        return Err(error);
    }
    let scanned = lexer::scan(&source);
    if let Some(error) = scanned.diagnostics.into_iter().next() {
        return Err(error);
    }
    let ast = parser::parse(&source, &scanned.tokens)?;
    let resolved = names::resolve_with_facts(&ast, &source)?;
    types::check_with_names(&ast, &source, &resolved)?;
    flow::check_with_names(&ast, &source, &resolved)?;
    functions::check(&ast, &source)
}

#[test]
fn 公私共有的整數與分支語法仍經兩條正式檢查路徑接受() {
    for code in [
        "fn main()->int{\nvar n:int = 1\nreturn n\n}\n",
        "fn main()->int{\nvar n:int = 1\nif(true){\nn = 2\n}else{\nn = 3\n}\nreturn n\n}\n",
        "fn main()->int{\nreturn 2 + 3 - 1\n}\n",
    ] {
        assert!(公開正式檢查(code).is_ok(), "公開 MVP-A 交集應合法：{code}");
        assert!(struct_stage::inspect(code).is_ok(), "私有 P13 交集應合法：{code}");
    }
}

#[test]
fn 不存在的宣告不可因兩套作用域模型不同而通過() {
    let code = "fn main()->int{\nvar n:int = 1\nreturn missing\n}\n";
    let public = 公開正式檢查(code).unwrap_err();
    let private = struct_stage::inspect(code).unwrap_err();
    assert_eq!(public.kind, DiagnosticKind::Static);
    assert_eq!(private.kind, DiagnosticKind::Static);
    assert!(public.span.is_some());
    assert!(private.span.is_some());
}

#[test]
fn 公開頂層變數與私有字串語法不因共通_ast_而互相開放() {
    let public_only = "var n:int = 3\nfn main()->int{\nreturn n\n}\n";
    assert!(公開正式檢查(public_only).is_ok());
    assert!(struct_stage::inspect(public_only).is_err());
    let private_only = "fn main()->int{\nvar value:str = \"內容\"\nreturn 0\n}\n";
    assert!(公開正式檢查(private_only).is_err(),
        "公開 MVP-A 不得因共用解析或 HIR 放行 str 宣告");
}

#[test]
fn 來源位置必須保留_utf8_半開位元組及一基欄位() {
    let code = "甲\n乙";
    let source = Source::new(code.as_bytes().to_vec());
    let start = code.find('乙').unwrap();
    assert_eq!(start, 4);
    let span = source.span(start, code.len());
    assert_eq!((span.start.line, span.start.column), (2, 1));
    assert_eq!((span.end.line, span.end.column), (2, 4));
}
