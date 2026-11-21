fn resource_grapheme(text: &str, index: i64) -> Option<&str> {
    text.get(indexed_range(&boundaries(text), index)?)
}
fn resource_byte(text: &str, index: i64, byte: i64) -> Option<i64> {
    let cluster = resource_grapheme(text, index)?.as_bytes();
    Some(i64::from(cluster[offset(byte, cluster.len())?]))
}
