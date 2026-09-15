#[derive(Debug, PartialEq, Eq)]
pub struct SshLine {
    pub keyword: Vec<u8>,
    pub arguments: Vec<Vec<u8>>,
}

pub fn parse_ssh_line(line: &[u8]) -> Option<SshLine> {
    let mut rest = line;
    while rest.last().is_some_and(|byte| b" \t\r\x0c".contains(byte)) {
        rest = &rest[..rest.len() - 1];
    }
    let mut keyword = keyword(&mut rest)?;
    if keyword.is_empty() {
        keyword = self::keyword(&mut rest)?;
    }
    if keyword.is_empty() || keyword.starts_with(b"#") {
        return None;
    }
    let mut arguments = Vec::new();
    while let Some(argument) = argument(&mut rest) {
        arguments.push(argument);
    }
    Some(SshLine { keyword, arguments })
}

fn skip(rest: &mut &[u8], separators: &[u8]) {
    while rest.first().is_some_and(|byte| separators.contains(byte)) {
        *rest = &rest[1..];
    }
}

fn keyword(rest: &mut &[u8]) -> Option<Vec<u8>> {
    skip(rest, b" \t\r");
    if rest.is_empty() {
        return None;
    }
    let mut token = Vec::new();
    while let Some(&byte) = rest.first() {
        match byte {
            b' ' | b'\t' | b'\r' | b'=' => {
                skip(rest, b" \t\r");
                if rest.starts_with(b"=") {
                    *rest = &rest[1..];
                    skip(rest, b" \t\r");
                }
                break;
            }
            b'"' => {
                *rest = &rest[1..];
                let closing = rest.iter().position(|byte| *byte == b'"')?;
                token.extend_from_slice(&rest[..closing]);
                *rest = &rest[closing + 1..];
                skip(rest, b" \t\r");
                break;
            }
            _ => {
                token.push(byte);
                *rest = &rest[1..];
            }
        }
    }
    Some(token)
}

fn argument(rest: &mut &[u8]) -> Option<Vec<u8>> {
    skip(rest, b" \t");
    if rest.is_empty() || rest.starts_with(b"#") {
        return None;
    }
    let mut token = Vec::new();
    let mut quote = None;
    while let Some(&byte) = rest.first() {
        if byte == b'\\' {
            if let Some(&escaped) = rest.get(1)
                && (matches!(escaped, b'"' | b'\'' | b'\\') || (quote.is_none() && escaped == b' '))
            {
                token.push(escaped);
                *rest = &rest[2..];
                continue;
            }
        } else if quote == Some(byte) {
            quote = None;
            *rest = &rest[1..];
            continue;
        } else if quote.is_none() {
            if matches!(byte, b' ' | b'\t') {
                break;
            }
            if matches!(byte, b'"' | b'\'') {
                quote = Some(byte);
                *rest = &rest[1..];
                continue;
            }
        }
        token.push(byte);
        *rest = &rest[1..];
    }
    quote.is_none().then_some(token)
}

#[cfg(test)]
mod tests;
