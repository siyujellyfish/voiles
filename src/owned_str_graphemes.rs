//! P19-D，voiles 執行期內建 Unicode 15.1 字素分割資料。
use super::{OwnedStrRuntime, OwnedUtf8, StrRuntimeError};
use std::ops::Range;

include!("grapheme_data.rs");

include!("grapheme_boundaries.rs");

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum IndexError {
    Owner(StrRuntimeError),
    OutOfBounds,
    ReplacementLength,
}

impl From<StrRuntimeError> for IndexError {
    fn from(error: StrRuntimeError) -> Self { Self::Owner(error) }
}

pub(crate) fn single_grapheme(text: &str) -> bool { boundaries(text).len() == 2 }

fn position(index: i64, count: usize) -> Result<usize, IndexError> {
    offset(index, count).ok_or(IndexError::OutOfBounds)
}

fn cluster_range(starts: &[usize], index: i64) -> Result<Range<usize>, IndexError> {
    indexed_range(starts, index).ok_or(IndexError::OutOfBounds)
}

fn cluster(value: &OwnedUtf8, index: i64) -> Result<Range<usize>, IndexError> {
    cluster_range(value.grapheme_starts.get_or_init(|| boundaries(value.text())), index)
}

pub(crate) fn literal_grapheme(text: &str, index: i64) -> Option<&str> {
    let range = cluster_range(&boundaries(text), index).ok()?;
    text.get(range)
}

pub(crate) fn literal_byte(text: &str, index: i64, byte: i64) -> Option<i64> {
    let bytes = literal_grapheme(text, index)?.as_bytes();
    Some(i64::from(bytes[position(byte, bytes.len()).ok()?]))
}

pub(crate) fn literal_index_valid(text: &str, index: i64, byte: Option<i64>) -> bool {
    cluster_range(&boundaries(text), index).is_ok_and(|range|
        byte.is_none_or(|offset| position(offset, range.len()).is_ok()))
}

impl OwnedStrRuntime {
    fn indexed_value(&self, owner: &str) -> Result<&OwnedUtf8, StrRuntimeError> {
        let (scope, slot) = self.location(owner)?;
        self.scopes[scope].names[slot].owned.as_ref().ok_or(StrRuntimeError::MovedSource)
    }

    pub(crate) fn grapheme_count(&self, owner:&str)->Result<i64,StrRuntimeError> {
        let value=self.indexed_value(owner)?;
        Ok((value.grapheme_starts.get_or_init(||boundaries(value.text())).len()-1) as i64)
    }

    pub(crate) fn read_grapheme(&mut self, owner: &str, index: i64)
        -> Result<OwnedUtf8, IndexError> {
        let value = self.indexed_value(owner)?;
        let copy = value.text()[cluster(value, index)?].to_owned();
        Ok(self.allocate(copy)?)
    }

    pub(crate) fn read_grapheme_byte(&self, owner: &str, index: i64, byte: i64)
        -> Result<i64, IndexError> {
        let value = self.indexed_value(owner)?;
        let bytes = &value.text().as_bytes()[cluster(value, index)?];
        Ok(i64::from(bytes[position(byte, bytes.len())?]))
    }

    pub(crate) fn replace_grapheme(&mut self, owner: &str, index: i64, rhs: &str)
        -> Result<OwnedUtf8, IndexError> {
        let (scope, slot) = self.location(owner)?;
        let binding = &self.scopes[scope].names[slot];
        if !binding.mutable { return Err(StrRuntimeError::ImmutableBinding.into()); }
        let value = binding.owned.as_ref().ok_or(StrRuntimeError::MovedSource)?;
        let range = cluster(value, index)?;
        if !single_grapheme(rhs) { return Err(IndexError::ReplacementLength); }
        // 預先配置獨立結果，任何可回報錯誤均發生在寫入前。更新後立即重算整份字串的字素邊界。
        let result = self.allocate(rhs.to_owned())?;
        let value = self.scopes[scope].names[slot].owned.as_mut().unwrap();
        value.text.replace_range(range, rhs);
        value.grapheme_starts = boundaries(value.text()).into();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::owned_str_runtime::event_trace_balanced;

    #[test]
    fn unicode_15_1_grapheme_conformance() {
        let mut cases = 0;
        for line in include_str!("../tests/fixtures/GraphemeBreakTest-15.1.txt").lines() {
            let data = line.split('#').next().unwrap().trim();
            if data.is_empty() { continue; }
            let mut text = String::new();
            let mut expected = Vec::new();
            for token in data.split_whitespace() {
                match token {
                    "÷" => expected.push(text.len()),
                    "×" => {}
                    hex => text.push(char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap()),
                }
            }
            assert_eq!(boundaries(&text), expected, "{data}");
            cases += 1;
        }
        assert_eq!(cases, 1187);
    }

    #[test]
    fn changed_boundaries_and_independent_results() {
        let mut rt = OwnedStrRuntime::new();
        rt.declare("s", true, "AB").unwrap();
        let before = rt.read_grapheme("s", -1).unwrap();
        let written = rt.replace_grapheme("s", 1, "\u{301}").unwrap();
        assert_eq!(rt.read("s").unwrap(), "A\u{301}");
        assert_eq!(before.text(), "B");
        assert_eq!(written.text(), "\u{301}");
        assert_ne!(before.resource_id(), written.resource_id());
        assert_ne!(rt.resource_id("s").unwrap(), written.resource_id());
        assert_eq!(rt.read_grapheme_byte("s", 0, -1), Ok(129));
        assert_eq!(rt.read_grapheme_byte("s", 1, 0), Err(IndexError::OutOfBounds));
        rt.release_detached(before);
        rt.release_detached(written);
        assert!(event_trace_balanced(&rt.finish().unwrap()));
    }

    #[test]
    fn invalid_indexes_and_replacements_are_atomic() {
        let mut rt = OwnedStrRuntime::new();
        rt.declare("s", true, "🇹🇼👩‍💻\0").unwrap();
        rt.declare("c", false, "").unwrap();
        let events = rt.events().to_vec();
        for (i, b) in [(i64::MIN, 0), (3, 0), (-4, 0), (0, 8), (1, i64::MIN)] {
            assert_eq!(rt.read_grapheme_byte("s", i, b), Err(IndexError::OutOfBounds));
        }
        assert_eq!(rt.read_grapheme_byte("s", -1, 0), Ok(0));
        assert_eq!(rt.read_grapheme_byte("s", -3, -8), Ok(240));
        for invalid in ["", "ab"] {
            assert_eq!(rt.replace_grapheme("s", 1, invalid).unwrap_err(), IndexError::ReplacementLength);
        }
        assert_eq!(rt.replace_grapheme("c", 0, "x").unwrap_err(),
                   IndexError::Owner(StrRuntimeError::ImmutableBinding));
        assert_eq!(rt.events(), events.as_slice());
        assert_eq!(rt.read("s").unwrap(), "🇹🇼👩‍💻\0");
    }
}
