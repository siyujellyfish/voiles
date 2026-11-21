//! P19-D，voiles 執行期內建 Unicode 15.1 字素分割資料。
use super::{OwnedStrRuntime, OwnedUtf8, StrRuntimeError};
use std::ops::Range;

include!("grapheme_data.rs");

fn property(table: &[(u32,u32,&'static str)], cp: u32) -> &'static str {
    let end = table.partition_point(|row| row.0 <= cp);
    match end.checked_sub(1).map(|i| table[i]) {
        Some((_, hi, prop)) if cp <= hi => prop,
        _ => "Other",
    }
}

fn grapheme_property(cp: u32) -> &'static str {
    if (0xac00..=0xd7a3).contains(&cp) {
        if (cp - 0xac00) % 28 == 0 { "LV" } else { "LVT" }
    } else { property(GCB, cp) }
}

// UAX #29 extended grapheme clusters. RI、emoji ZWJ、Indic linker
// 三個跨碼位條件均以從左到右的狀態維護，無需重掃前綴。
fn boundaries(text: &str) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut prev = "start";
    let mut ri_odd = false;
    let mut emoji_run = false;
    let mut emoji_zwj = false;
    let mut indic_base = false;
    let mut indic_linked = false;
    for (offset, ch) in text.char_indices() {
        let cp = ch as u32;
        let next = grapheme_property(cp);
        let pict = property(EP, cp) == "Extended_Pictographic";
        let indic = property(INCB, cp);
        let join = if prev == "CR" && next == "LF" {
            true
        } else if prev == "start" || matches!(prev, "Control" | "CR" | "LF")
            || matches!(next, "Control" | "CR" | "LF") {
            false
        } else {
            (prev == "L" && matches!(next, "L" | "V" | "LV" | "LVT"))
                || (matches!(prev, "V" | "LV") && matches!(next, "V" | "T"))
                || (matches!(prev, "LVT" | "T") && next == "T")
                || matches!(next, "Extend" | "ZWJ" | "SpacingMark") || prev == "Prepend"
                || (indic_linked && indic == "Consonant") || (emoji_zwj && pict)
                || (ri_odd && next == "Regional_Indicator")
        };
        if !join { starts.push(offset); }
        ri_odd = next == "Regional_Indicator" && !ri_odd;
        emoji_zwj = next == "ZWJ" && emoji_run;
        emoji_run = pict || (emoji_run && next == "Extend");
        match indic {
            "Consonant" => { indic_base = true; indic_linked = false; }
            "Linker" => indic_linked = indic_base,
            "Extend" => {}
            _ => { indic_base = false; indic_linked = false; }
        }
        prev = next;
    }
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

pub(crate) fn single_grapheme(text: &str) -> bool { boundaries(text).len() == 2 }

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
