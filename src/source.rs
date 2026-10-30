//! 保留原始 UTF-8 位元組的來源掃描與座標轉換。
//! 本模組僅供核心內部使用；真正詞彙／語法分析留待後續 Phase。

use crate::{Diagnostic, DiagnosticKind, SourcePosition, SourceSpan};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnitKind {
    Valid(char),
    /// 一個非法原始 UTF-8 位元組，不是可用於合法記號的 U+FFFD。
    Invalid(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Unit {
    pub(crate) kind: UnitKind,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Debug)]
pub(crate) struct Source {
    pub(crate) bytes: Vec<u8>,
    pub(crate) units: Vec<Unit>,
    line_starts: Vec<usize>,
}

impl Source {
    pub(crate) fn new(bytes: Vec<u8>) -> Self {
        let mut units = Vec::new();
        let mut index = 0;
        while index < bytes.len() {
            match std::str::from_utf8(&bytes[index..]) {
                Ok(valid) => {
                    Self::push_valid(&mut units, valid, index);
                    break;
                }
                Err(error) => {
                    let valid_end = error.valid_up_to();
                    if valid_end > 0 {
                        // 安全：UTF-8 錯誤 API 保證此前的前綴是合法 UTF-8。
                        let prefix = std::str::from_utf8(&bytes[index..index + valid_end])
                            .expect("已經由 UTF-8 解碼器確認的合法前綴");
                        Self::push_valid(&mut units, prefix, index);
                    }
                    index += valid_end;
                    // 不把單一不可解碼序列合併成一筆：每個無效原始
                    // 位元組各保存一個標記及一個單位元組原始跨度。
                    let invalid_count = error
                        .error_len()
                        .unwrap_or(bytes.len() - index)
                        .min(bytes.len() - index);
                    for _ in 0..invalid_count {
                        units.push(Unit {
                            kind: UnitKind::Invalid(bytes[index]),
                            start: index,
                            end: index + 1,
                        });
                        index += 1;
                    }
                }
            }
        }
        // 預先建立每一行的起點，避免對每筆診斷重掃整份來源。
        // CRLF 只在 LF 後換行；單獨 CR 與 LF 均各自換行。
        let mut line_starts = vec![0];
        for (index, byte) in bytes.iter().enumerate() {
            if *byte == b'\n' || (*byte == b'\r' && bytes.get(index + 1) != Some(&b'\n')) {
                line_starts.push(index + 1);
            }
        }
        Self { bytes, units, line_starts }
    }

    fn push_valid(units: &mut Vec<Unit>, text: &str, base: usize) {
        for (offset, ch) in text.char_indices() {
            let start = base + offset;
            units.push(Unit {
                kind: UnitKind::Valid(ch),
                start,
                end: start + ch.len_utf8(),
            });
        }
    }

    /// 原始偏移為零基；對外行欄均為一基，欄按位元組計算。
    /// CRLF 的中間偏移仍停留於 CR 所在行、CR 後一欄。
    pub(crate) fn position(&self, offset: usize) -> SourcePosition {
        assert!(offset <= self.bytes.len(), "位置不可超出原始來源");
        // 每個行起點都已排序；行號仍為一基，欄位仍按原始位元組計數。
        let line = self.line_starts.partition_point(|start| *start <= offset);
        let column = offset - self.line_starts[line - 1] + 1;
        SourcePosition { line, column }
    }

    pub(crate) fn span(&self, start: usize, end: usize) -> SourceSpan {
        assert!(start <= end && end <= self.bytes.len());
        SourceSpan {
            start: self.position(start),
            end: self.position(end),
        }
    }

    /// 來源末尾錯誤定位：空檔 [0,0)，末尾 CRLF 含完整雙位元組，
    /// 其餘使用最後存在的字元／無效標記原始跨度。
    pub(crate) fn eof_range(&self) -> (usize, usize) {
        if self.bytes.is_empty() {
            (0, 0)
        } else if self.bytes.ends_with(b"\r\n") {
            (self.bytes.len() - 2, self.bytes.len())
        } else {
            let last = self.units.last().expect("非空來源必有原始單位");
            (last.start, last.end)
        }
    }

    pub(crate) fn lexical_diagnostics(&self) -> Vec<Diagnostic> {
        self.units
            .iter()
            .filter_map(|unit| {
                let message = match unit.kind {
                    UnitKind::Invalid(_) => "來源包含非法 UTF-8 位元組",
                    UnitKind::Valid('\u{FFFD}') => "來源包含首版不接受的 U+FFFD 字元",
                    _ => return None,
                };
                Some(Diagnostic {
                    kind: DiagnosticKind::Lexical,
                    message: Box::from(message),
                    span: Some(self.span(unit.start, unit.end)),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{Source, UnitKind};
    use crate::{DiagnosticKind, SourcePosition};

    #[test]
    fn crlf_中間位置及雙位元組終點皆正確() {
        let source = Source::new(b"A\r\nB".to_vec());
        assert_eq!(source.position(0), SourcePosition { line: 1, column: 1 });
        assert_eq!(source.position(1), SourcePosition { line: 1, column: 2 });
        assert_eq!(source.position(2), SourcePosition { line: 1, column: 3 });
        assert_eq!(source.position(3), SourcePosition { line: 2, column: 1 });
        assert_eq!(source.position(4), SourcePosition { line: 2, column: 2 });
        let span = source.span(1, 3);
        assert_eq!(span.start, SourcePosition { line: 1, column: 2 });
        assert_eq!(span.end, SourcePosition { line: 2, column: 1 });
    }

    #[test]
    fn 單獨_cr_與_lf_只增加一個行號() {
        for bytes in [b"A\rB".as_slice(), b"A\nB".as_slice()] {
            let source = Source::new(bytes.to_vec());
            assert_eq!(source.position(2), SourcePosition { line: 2, column: 1 });
        }
    }

    #[test]
    fn 多位元組漢字和_tab_皆以原始位元組欄計算() {
        let source = Source::new("中\tB\n".as_bytes().to_vec());
        assert_eq!(source.position(3), SourcePosition { line: 1, column: 4 });
        assert_eq!(source.position(4), SourcePosition { line: 1, column: 5 });
        assert_eq!(source.position(5), SourcePosition { line: 1, column: 6 });
        assert_eq!(source.position(6), SourcePosition { line: 2, column: 1 });
        assert_eq!(source.units[0].start, 0);
        assert_eq!(source.units[0].end, 3);
    }

    #[test]
    fn 非法來源各原始位元組獨立標記和診斷() {
        let source = Source::new(vec![0xFF, b'A', 0x80]);
        assert_eq!(source.units.len(), 3);
        assert_eq!(source.units[0].kind, UnitKind::Invalid(0xFF));
        assert_eq!(source.units[0].start, 0);
        assert_eq!(source.units[0].end, 1);
        assert_eq!(source.units[2].kind, UnitKind::Invalid(0x80));
        assert_eq!(source.units[2].start, 2);
        assert_eq!(source.units[2].end, 3);
        let diagnostics = source.lexical_diagnostics();
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].kind, DiagnosticKind::Lexical);
        assert_eq!(diagnostics[0].span.unwrap().start.column, 1);
        assert_eq!(diagnostics[1].span.unwrap().start.column, 3);
    }

    #[test]
    fn 非法三位元組序列逐位元組分離() {
        let source = Source::new(vec![0xED, 0xA0, 0x80]);
        assert_eq!(source.lexical_diagnostics().len(), 3);
        assert!(source.units.iter().all(|x| matches!(x.kind, UnitKind::Invalid(_))));
    }

    #[test]
    fn 合法原始_ufffd_僅單筆詞彙錯誤() {
        let source = Source::new("\u{FFFD}".as_bytes().to_vec());
        assert_eq!(source.units.len(), 1);
        assert_eq!(source.units[0].start, 0);
        assert_eq!(source.units[0].end, 3);
        let ds = source.lexical_diagnostics();
        assert_eq!(ds.len(), 1);
        let span = ds[0].span.unwrap();
        assert_eq!(span.start.column, 1);
        assert_eq!(span.end.column, 4);
    }

    #[test]
    fn eof_空檔與末尾_crlf_使用已核准完整來源跨度() {
        let empty = Source::new(Vec::new());
        assert_eq!(empty.eof_range(), (0, 0));
        assert_eq!(empty.position(0), SourcePosition { line: 1, column: 1 });
        let source = Source::new(b"A\r\n".to_vec());
        assert_eq!(source.eof_range(), (1, 3));
        assert_eq!(source.span(1, 3).end, SourcePosition { line: 2, column: 1 });
        let chinese = Source::new("中".as_bytes().to_vec());
        assert_eq!(chinese.eof_range(), (0, 3));
    }
    #[test]
    fn 行起點索引與原始逐位元組掃描對所有偏移等價() {
        let bytes = [b"A\r\nB\rC\n\r\n\t".as_slice(), "中🪼".as_bytes(), &[0xff, b'X', 0x80]]
            .concat();
        let source = Source::new(bytes.clone());
        for offset in 0..=bytes.len() {
            let mut line = 1;
            let mut column = 1;
            for index in 0..offset {
                match bytes[index] {
                    b'\r' if bytes.get(index + 1) == Some(&b'\n') => column += 1,
                    b'\r' | b'\n' => { line += 1; column = 1; }
                    _ => column += 1,
                }
            }
            assert_eq!(source.position(offset), SourcePosition { line, column }, "偏移 {offset}");
        }
    }

    #[test]
    fn 大量獨立非法位元組的行欄診斷保持正確() {
        let mut bytes = Vec::new();
        for _ in 0..512 {
            bytes.extend_from_slice(&[0xff, b'\r', b'\n', 0x80]);
        }
        let diagnostics = Source::new(bytes).lexical_diagnostics();
        assert_eq!(diagnostics.len(), 1024);
        assert_eq!(diagnostics[0].span.unwrap().start, SourcePosition { line: 1, column: 1 });
        assert_eq!(diagnostics[1].span.unwrap().start, SourcePosition { line: 2, column: 1 });
        assert_eq!(diagnostics.last().unwrap().span.unwrap().start,
            SourcePosition { line: 512, column: 1 });
    }

}
