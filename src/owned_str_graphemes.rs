//! P19-D，Linux x86-64 執行環境以 GNU libunistring 的 Unicode 資料分割字素。
use super::{OwnedStrRuntime, OwnedUtf8, StrRuntimeError};
use std::ops::Range;

#[link(name = "libunistring.so.5", kind = "dylib", modifiers = "+verbatim")]
extern "C" {
    fn u8_grapheme_breaks(input: *const u8, length: usize, breaks: *mut u8);
}

fn boundaries(text: &str) -> Vec<usize> {
    let mut flags = vec![0; text.len()];
    if !text.is_empty() {
        // GNU API 讀取 length 個 UTF-8 位元組，寫出等長的逐位元組邊界旗標。
        // 兩段記憶體在呼叫期間有效且不重疊；長度參數也容許內嵌 NUL。
        unsafe { u8_grapheme_breaks(text.as_ptr(), text.len(), flags.as_mut_ptr()) };
    }
    let mut starts: Vec<_> = flags.into_iter().enumerate()
        .filter_map(|(i, flag)| (flag != 0).then_some(i)).collect();
    starts.push(text.len());
    starts
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum IndexError {
    Owner(StrRuntimeError),
    OutOfBounds,
    ReplacementLength,
}

impl From<StrRuntimeError> for IndexError {
    fn from(error: StrRuntimeError) -> Self { Self::Owner(error) }
}

fn position(index: i64, count: usize) -> Result<usize, IndexError> {
    let distance = usize::try_from(index.unsigned_abs()).map_err(|_| IndexError::OutOfBounds)?;
    if index < 0 {
        count.checked_sub(distance).ok_or(IndexError::OutOfBounds)
    } else if distance < count {
        Ok(distance)
    } else {
        Err(IndexError::OutOfBounds)
    }
}

fn cluster(value: &OwnedUtf8, index: i64) -> Result<Range<usize>, IndexError> {
    let starts = value.grapheme_starts.get_or_init(|| boundaries(value.text()));
    let i = position(index, starts.len() - 1)?;
    Ok(starts[i]..starts[i + 1])
}

impl OwnedStrRuntime {
    fn indexed_value(&self, owner: &str) -> Result<&OwnedUtf8, StrRuntimeError> {
        let (scope, slot) = self.location(owner)?;
        self.scopes[scope].names[slot].owned.as_ref().ok_or(StrRuntimeError::MovedSource)
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
        if boundaries(rhs).len() != 2 { return Err(IndexError::ReplacementLength); }
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
