//! 直接使用正式 lexer，逐記號輸出自舉子集的種類與原始位元組範圍。
use voiles_core::{Diagnostic, DiagnosticKind, SourcePosition, SourceSpan};
#[path = "../../src/source.rs"]
mod source;
#[path = "../../src/lexer.rs"]
mod lexer;

fn main() {
    let path = std::env::args_os().nth(1).unwrap();
    let source = source::Source::new(std::fs::read(path).unwrap());
    for token in lexer::scan(&source).tokens {
        use lexer::TokenKind::*;
        let kind = match token.kind {
            Keyword(_) => 1, Name(_) => 2, Integer(_) => 3, StringLiteral(_) => 4,
            Symbol(_) => 5, Arrow => 6, Newline => 7, Invalid => 8, End => 9,
            other => panic!("超出當前自舉子集：{other:?}"),
        };
        println!("{kind} {} {}", token.start, token.end);
    }
}
