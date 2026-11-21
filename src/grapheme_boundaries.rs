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

fn offset(index: i64, count: usize) -> Option<usize> {
    let distance = usize::try_from(index.unsigned_abs()).ok()?;
    if index < 0 { count.checked_sub(distance) } else { (distance < count).then_some(distance) }
}

fn indexed_range(starts: &[usize], index: i64) -> Option<std::ops::Range<usize>> {
    let i = offset(index, starts.len().checked_sub(1)?)?;
    Some(starts[i]..starts[i + 1])
}
