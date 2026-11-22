//! 公開 int 的有號 64 位元運算。直譯與原生產物共用此實作。
pub(super) fn arithmetic(left: i64, operator: char, right: i64) -> Option<i64> {
    match operator {
        '+' => left.checked_add(right),
        '-' => left.checked_sub(right),
        '*' => left.checked_mul(right),
        '/' => left.checked_div(right),
        '÷' | '%' => {
            let (a, b) = (i128::from(left), i128::from(right));
            let remainder = a.checked_rem(b)?;
            let adjust = remainder != 0 && (remainder < 0) != (b < 0);
            let value = if operator == '%' {
                if adjust { remainder + b } else { remainder }
            } else {
                a / b - i128::from(adjust)
            };
            i64::try_from(value).ok()
        }
        '^' if right >= 0 => match left {
            0 if right > 0 => Some(0),
            1 => Some(1),
            -1 => Some(if right % 2 == 0 { 1 } else { -1 }),
            _ => left.checked_pow(u32::try_from(right).ok()?),
        },
        _ => None,
    }
}
