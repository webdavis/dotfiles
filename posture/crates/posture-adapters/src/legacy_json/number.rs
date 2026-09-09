pub fn display_number(token: &str) -> Option<String> {
    let negative = token.starts_with('-');
    let unsigned = token.strip_prefix(['+', '-']).unwrap_or(token);
    let lower = unsigned.to_ascii_lowercase();
    if let Some(payload) = lower
        .strip_prefix("nan")
        .or_else(|| lower.strip_prefix("snan"))
    {
        return payload
            .bytes()
            .all(|byte| byte == b'0')
            .then(|| "null".to_owned());
    }
    if lower == "inf" || lower == "infinity" {
        return Some(infinity(negative));
    }
    let (coefficient, explicit_exponent) = match unsigned.split_once(['e', 'E']) {
        Some((coefficient, exponent)) => (coefficient, exponent_value(exponent)?),
        None => (unsigned, 0),
    };
    let (integer, fraction) = coefficient.split_once('.').unwrap_or((coefficient, ""));
    if integer.len() + fraction.len() == 0
        || !integer
            .bytes()
            .chain(fraction.bytes())
            .all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let joined = format!("{integer}{fraction}");
    let nonzero = joined.trim_start_matches('0');
    let mut digits = if nonzero.is_empty() { "0" } else { nonzero }.to_owned();
    let mut exponent = explicit_exponent.saturating_sub(fraction.len() as i64);
    // These are the measured jq 1.8.2 decimal context bounds, not new refusal limits.
    const MAX_EXPONENT: i64 = 999_999_999;
    const PRECISION: usize = 147_483_648;
    const TINY_EXPONENT: i64 = -1_147_483_646;
    if digits.len() > PRECISION {
        let removed = digits.len() - PRECISION;
        round(&mut digits, removed as i64);
        exponent = exponent.saturating_add(removed as i64);
    }
    if exponent < TINY_EXPONENT {
        round(&mut digits, TINY_EXPONENT.saturating_sub(exponent));
        exponent = TINY_EXPONENT;
    }
    if digits == "0" {
        exponent = exponent.min(MAX_EXPONENT);
    } else if exponent.saturating_add(digits.len() as i64 - 1) > MAX_EXPONENT {
        return Some(infinity(negative));
    }
    let adjusted = exponent + digits.len() as i64 - 1;
    let mut body = if exponent <= 0 && adjusted >= -6 {
        let point = digits.len() as i64 + exponent;
        if point <= 0 {
            format!("0.{}{digits}", "0".repeat((-point) as usize))
        } else if (point as usize) < digits.len() {
            let (left, right) = digits.split_at(point as usize);
            format!("{left}.{right}")
        } else {
            digits
        }
    } else {
        let rest = if digits.len() > 1 {
            format!(".{}", &digits[1..])
        } else {
            String::new()
        };
        format!("{}{rest}E{adjusted:+}", &digits[..1])
    };
    if negative {
        body.insert(0, '-');
    }
    Some(body)
}

fn infinity(negative: bool) -> String {
    format!("{}1.7976931348623157e+308", if negative { "-" } else { "" })
}

fn exponent_value(text: &str) -> Option<i64> {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let magnitude = digits.bytes().fold(0i64, |value, byte| {
        value
            .saturating_mul(10)
            .saturating_add(i64::from(byte - b'0'))
    });
    Some(if text.starts_with('-') {
        -magnitude
    } else {
        magnitude
    })
}

fn round(digits: &mut String, removed: i64) {
    if removed > digits.len() as i64 {
        *digits = "0".to_owned();
        return;
    }
    let keep = digits.len() - removed as usize;
    let up = digits.as_bytes()[keep] >= b'5';
    digits.truncate(keep);
    if up {
        let mut bytes = digits.as_bytes().to_vec();
        let mut position = bytes.len();
        while position > 0 && bytes[position - 1] == b'9' {
            bytes[position - 1] = b'0';
            position -= 1;
        }
        if position == 0 {
            bytes.insert(0, b'1');
        } else {
            bytes[position - 1] += 1;
        }
        *digits = bytes.into_iter().map(char::from).collect();
    } else if digits.is_empty() {
        *digits = "0".to_owned();
    }
}

#[cfg(test)]
mod tests;
