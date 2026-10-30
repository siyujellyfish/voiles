//! P08 AST 內部位置檢視工具，僅供前端開發及 Rust 測試，不公開 CLI。
// 僅供 Rust 內部檢視與測試；非公開二進位不需連結所有展示輔助函式。
#![allow(dead_code)]

use crate::{SourcePosition};
use crate::expr::{Expr, ExprKind};
use crate::parser::{Binding, Block, File, Item, Stmt, StmtKind};
use crate::source::Source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NodeLocation {
    pub(crate) kind: &'static str,
    pub(crate) start: SourcePosition,
    pub(crate) end: SourcePosition,
}

fn record(out: &mut Vec<NodeLocation>, source: &Source,
    kind: &'static str, start: usize, end: usize) {
    let span = source.span(start, end);
    out.push(NodeLocation { kind, start: span.start, end: span.end });
}

fn expression(out: &mut Vec<NodeLocation>, source: &Source, expr: &Expr) {
    let name = match &expr.kind {
        ExprKind::IntLiteral(_) => "IntLiteral",
        ExprKind::BoolLiteral(_) => "BoolLiteral",
        ExprKind::DecimalLiteral(_) => "DecimalLiteral",
        ExprKind::StringLiteral(_) => "StringLiteral",
        ExprKind::Name(_) => "NameRef",
        ExprKind::ZeroArgCall(_) => "ZeroArgCall",
        ExprKind::Add(_, _) => "Add",
        ExprKind::Subtract(_, _) => "Subtract",
        ExprKind::Group(_) => "ArithmeticGroup",
        ExprKind::IndexAccess(_, _) => "IndexAccess",
        ExprKind::ZeroArgMethod(_, _) => "ZeroArgMethod",
        ExprKind::FirstIndexWrite(_, _) => "FirstIndexWrite",
    };
    record(out, source, name, expr.start, expr.end);
    match &expr.kind {
        ExprKind::Add(a,b) | ExprKind::Subtract(a,b) |
        ExprKind::IndexAccess(a,b) | ExprKind::FirstIndexWrite(a,b) => {
            expression(out, source, a);
            expression(out, source, b);
        }
        ExprKind::Group(a) | ExprKind::ZeroArgMethod(a,_) => expression(out,source,a),
        _ => {}
    }
}

fn binding(out: &mut Vec<NodeLocation>, source: &Source, decl: &Binding, kind: &'static str) {
    record(out,source,kind,decl.start,decl.end);
    if let Some(expr) = &decl.initializer { expression(out,source,expr); }
}

fn stmt(out: &mut Vec<NodeLocation>, source: &Source, s: &Stmt) {
    let name = match &s.kind {
        StmtKind::Binding(_) => "LocalBinding",
        StmtKind::Assignment { .. } => "Assignment",
        StmtKind::IfChain { .. } => "IfChain",
        StmtKind::Return(_) => "Return",
    };
    record(out,source,name,s.start,s.end);
    match &s.kind {
        StmtKind::Binding(b) => {
            // 語句與宣告節點是同一來源跨度，保留概念分組。
            binding(out,source,b,"LocalBinding");
        }
        StmtKind::Assignment { value, .. } | StmtKind::Return(value) =>
            expression(out,source,value),
        StmtKind::IfChain { branches, otherwise } => {
            for (cond,body) in branches {
                expression(out,source,cond);
                block(out,source,body);
            }
            if let Some(body)=otherwise { block(out,source,body); }
        }
    }
}

fn block(out: &mut Vec<NodeLocation>, source: &Source, b: &Block) {
    record(out,source,"Block",b.start,b.end);
    for s in &b.statements { stmt(out,source,s); }
}

/// AST 可在測試中檢視，但不允許額外對外 parser／lexer API。
pub(crate) fn locations(file: &File, source: &Source) -> Vec<NodeLocation> {
    let mut result=Vec::new();
    record(&mut result,source,"SourceFile",file.start,file.end);
    for item in &file.items {
        match item {
            Item::Function(f) => {
                record(&mut result,source,"FunctionDecl",f.start,f.end);
                block(&mut result,source,&f.body);
            }
            Item::Binding(b) => binding(&mut result,source,b,"TopLevelBinding"),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer::scan, parser::parse, source::Source, DiagnosticKind};

    #[test]
    fn ast_各必要節點含一基_utf8_位元組半開來源跨度() {
        let text = "# 中文\nfn main()->int{\nreturn 8 - (3 - 1)\n}\n";
        let source=Source::new(text.as_bytes().to_vec());
        let tokens=scan(&source).tokens;
        let file=parse(&source,&tokens).unwrap();
        let positions=locations(&file,&source);
        let fn_node=positions.iter().find(|n|n.kind=="FunctionDecl").unwrap();
        assert_eq!(fn_node.start,SourcePosition{line:2,column:1});
        let group=positions.iter().find(|n|n.kind=="ArithmeticGroup").unwrap();
        assert_eq!(group.start.line,3);
        assert_eq!(group.start.column,12);
        assert_eq!(group.end.column,19);
        assert!(positions.iter().any(|n|n.kind=="Subtract"));
        assert!(positions.iter().any(|n|n.kind=="Return"));
        assert!(format!("{file:#?}").contains("Subtract"));
    }

    #[test]
    fn ast_crlf_行欄及半開終點依原始來源計算() {
        let text = "fn main()->int{\r\nreturn 0\r\n}\r\n";
        let source=Source::new(text.as_bytes().to_vec());
        let file=parse(&source,&scan(&source).tokens).unwrap();
        let loc=locations(&file,&source);
        let ret=loc.iter().find(|n|n.kind=="Return").unwrap();
        assert_eq!(ret.start,SourcePosition{line:2,column:1});
        assert_eq!(ret.end,SourcePosition{line:2,column:9});
    }

    #[test]
    fn 解析失敗無須輸出部分_ast_但必保有定位診斷() {
        let source=Source::new(b"fn main()->int{\nreturn (1 +\n}\n".to_vec());
        let lex=scan(&source);
        let result=parse(&source,&lex.tokens);
        let e=result.unwrap_err();
        assert_eq!(e.kind, DiagnosticKind::Syntax);
        assert!(e.span.is_some());
    }
}
