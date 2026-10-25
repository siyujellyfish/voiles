//! 診斷穩定排序與位置處置，沿用 BC03／BF05／BG03／BN01。
//! AQ06 的 Optional 專用優先級只在將來真正解析 is 時適用。

use crate::{Diagnostic, DiagnosticKind};

pub(crate) fn sort_diagnostics(items: &mut Vec<Diagnostic>) {
    // Rust 穩定排序：同一來源、同一階段保持原有確認順序。
    // 無來源位置的診斷後置，不得編造行列。
    items.sort_by_key(|d| {
        let (line, column) = d.span.map(|s| (s.start.line, s.start.column))
            .unwrap_or((usize::MAX, usize::MAX));
        let phase = match d.kind {
            DiagnosticKind::Lexical => 0,
            DiagnosticKind::Syntax => 1,
            DiagnosticKind::Static => 2,
        };
        (line, column, phase)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SourcePosition, SourceSpan};

    fn at(kind: DiagnosticKind, line: usize, col: usize, text: &str) -> Diagnostic {
        Diagnostic {
            kind,
            message: Box::from(text),
            span: Some(SourceSpan {
                start: SourcePosition { line, column: col },
                end: SourcePosition { line, column: col + 1 },
            }),
        }
    }

    #[test]
    fn 不同位置先原始來源順序_同位置先詞彙語法靜態() {
        let mut diagnostics = vec![
            at(DiagnosticKind::Static, 1, 1, "third"),
            at(DiagnosticKind::Syntax, 2, 1, "later"),
            at(DiagnosticKind::Syntax, 1, 1, "second"),
            at(DiagnosticKind::Lexical, 1, 1, "first"),
        ];
        sort_diagnostics(&mut diagnostics);
        assert_eq!(diagnostics.iter().map(|d| &*d.message).collect::<Vec<_>>(),
            ["first","second","third","later"]);
    }

    #[test]
    fn 相同位置同階段保持原始確認順序_無位置後置() {
        let mut diagnostics = vec![
            Diagnostic { kind: DiagnosticKind::Static, message: "none".into(), span: None },
            at(DiagnosticKind::Syntax, 1, 1, "A"),
            at(DiagnosticKind::Syntax, 1, 1, "B"),
        ];
        sort_diagnostics(&mut diagnostics);
        assert_eq!(&*diagnostics[0].message, "A");
        assert_eq!(&*diagnostics[1].message, "B");
        assert_eq!(&*diagnostics[2].message, "none");
        assert!(diagnostics[2].span.is_none());
    }
}
