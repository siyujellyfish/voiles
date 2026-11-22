//! 有限 decimal64。以十進位係數及量子指數計算，最近值、半數取偶。
const RADIX: i64 = 10_000_000_000_000_000;
const MIN_EXP: i32 = -398;
const MAX_EXP: i32 = 369;

pub(super) fn parts(literal: &str) -> Option<(u64, i32)> {
    let clean = literal.replace('_', "");
    let (integer, fraction) = clean.split_once('.')?;
    if integer.is_empty() || !integer.bytes().chain(fraction.bytes()).all(|c| c.is_ascii_digit()) {
        return None;
    }
    let digits = format!("{integer}{fraction}");
    let significant = digits.trim_start_matches('0');
    if significant.is_empty() { return Some((0, 0)); }
    let adjusted = i64::try_from(significant.len()).ok()? - i64::try_from(fraction.len()).ok()? - 1;
    if adjusted > 384 { return None; }
    if adjusted < -399 { return Some((0, 0)); }
    let exponent = (adjusted - 15).max(i64::from(MIN_EXP)) as i32;
    let kept = (adjusted - i64::from(exponent) + 1) as usize;
    let digits = significant.as_bytes();
    let mut coefficient = 0;
    for i in 0..kept {
        coefficient = coefficient * 10 + u64::from(digits.get(i).copied().unwrap_or(b'0') - b'0');
    }
    if let Some(&next) = digits.get(kept) {
        if next > b'5' || next == b'5' && (coefficient % 2 == 1 || digits[kept+1..].iter().any(|&d| d != b'0')) {
            coefficient += 1;
        }
    }
    normalize(coefficient, exponent)
}

fn normalize(mut coefficient: u64, mut exponent: i32) -> Option<(u64,i32)> {
    if coefficient == 0 { return Some((0,0)); }
    if coefficient == RADIX as u64 {
        coefficient /= 10;
        exponent += 1;
    }
    while coefficient < RADIX as u64 / 10 && exponent > MIN_EXP {
        coefficient *= 10;
        exponent -= 1;
    }
    (exponent <= MAX_EXP).then_some((coefficient, exponent))
}

// 每個有限值在 i64 純量槽中有唯一編碼；不對外輸出此內部表示。
pub(super) fn encode(coefficient: u64, exponent: i32) -> i64 {
    if coefficient == 0 { 0 } else { i64::from(exponent - MIN_EXP) * RADIX + coefficient as i64 }
}

fn unpack(value: i64) -> (i128, i32) {
    (i128::from(value % RADIX), (value.abs() / RADIX) as i32 + MIN_EXP)
}

pub(super) fn arithmetic(left: i64, operator: char, right: i64) -> Option<i64> {
    let right = if operator == '-' { -right } else { right };
    if left == 0 { return Some(right); }
    if right == 0 { return Some(left); }
    let (a, ea) = unpack(left);
    let (b, eb) = unpack(right);
    // 相差超過 17 個十進位位數時，較小者不足以改變最近的可表示值。
    if ea - eb > 17 { return Some(left); }
    if eb - ea > 17 { return Some(right); }
    let mut exponent = ea.min(eb);
    let total = a * 10i128.pow((ea-exponent) as u32) + b * 10i128.pow((eb-exponent) as u32);
    if total == 0 { return Some(0); }
    let magnitude = total.unsigned_abs();
    let discarded = (magnitude.ilog10() + 1).saturating_sub(16);
    let scale = 10u128.pow(discarded);
    let mut coeff = magnitude / scale;
    let tail = magnitude % scale;
    if tail * 2 > scale || tail * 2 == scale && coeff % 2 == 1 { coeff += 1; }
    exponent += discarded as i32;
    let (c,e) = normalize(coeff as u64, exponent)?;
    Some(encode(c,e) * total.signum() as i64)
}

pub(super) fn display(value:i64, precision:Option<usize>) -> String {
    let (coefficient, exponent) = unpack(value);
    let mut digits = coefficient.abs().to_string();
    let mut scale = exponent;
    if let Some(places) = precision {
        let target = -(places as i32);
        if scale < target {
            let discarded = (target-scale) as u32;
            let rounded = if discarded > 16 { 0 } else {
                let divisor = 10i128.pow(discarded);
                let magnitude = coefficient.abs();
                let retained = magnitude / divisor;
                let twice = (magnitude % divisor)*2;
                retained + i128::from(twice > divisor || twice == divisor && retained%2 == 1)
            };
            digits = rounded.to_string();
            scale = target;
        }
    }
    if value == 0 && precision.is_none() { return "0".into(); }
    let point = digits.len() as i32 + scale;
    let (integer,mut fraction) = if point <= 0 {
        ("0".into(), "0".repeat((-point) as usize) + &digits)
    } else if point as usize >= digits.len() {
        (digits.clone() + &"0".repeat(point as usize-digits.len()), String::new())
    } else {
        (digits[..point as usize].into(),digits[point as usize..].into())
    };
    if let Some(count)=precision {
        fraction.truncate(count);
        fraction.push_str(&"0".repeat(count-fraction.len()));
    } else { fraction.truncate(fraction.trim_end_matches('0').len()); }
    let sign = if value<0 { "-" } else { "" };
    if fraction.is_empty() { format!("{sign}{integer}") } else { format!("{sign}{integer}.{fraction}") }
}
