impl ResourceText {
    fn new(text: String) -> Self { Self { text, starts: std::cell::OnceCell::new() } }
    fn range(&self, index: i64) -> Option<std::ops::Range<usize>> {
        indexed_range(self.starts.get_or_init(|| boundaries(&self.text)), index)
    }
    fn grapheme(&self, index: i64) -> Option<&str> {
        self.text.get(self.range(index)?)
    }
    fn byte(&self, index: i64, byte: i64) -> Option<i64> {
        let bytes = self.grapheme(index)?.as_bytes();
        Some(i64::from(bytes[offset(byte, bytes.len())?]))
    }
    fn replace(&mut self, index: i64, text: &str) -> Option<()> {
        let range = self.range(index)?;
        if boundaries(text).len() != 2 { return None; }
        self.text.replace_range(range, text);
        self.starts.take();
        Some(())
    }
}
