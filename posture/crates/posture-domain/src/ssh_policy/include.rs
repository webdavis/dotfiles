#[derive(Debug, PartialEq, Eq)]
pub struct IncludePattern {
    pub pattern: Vec<u8>,
    pub literal: Vec<u8>,
    pub has_glob: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum IncludeRefusal {
    MissingPath,
    CaretBracket(Vec<u8>),
}

pub fn analyze_include(
    directory: &[u8],
    arguments: &[Vec<u8>],
) -> Result<Vec<IncludePattern>, IncludeRefusal> {
    if arguments.is_empty() {
        return Err(IncludeRefusal::MissingPath);
    }
    arguments
        .iter()
        .map(|argument| {
            let pattern = if argument.starts_with(b"/") {
                argument.clone()
            } else {
                [directory, b"/", argument].concat()
            };
            analyze_pattern(pattern)
        })
        .collect()
}

fn analyze_pattern(pattern: Vec<u8>) -> Result<IncludePattern, IncludeRefusal> {
    let mut rest = pattern.as_slice();
    let mut literal = Vec::new();
    let mut has_glob = false;
    while let Some(&byte) = rest.first() {
        if byte == b'\\' && rest.len() > 1 {
            literal.push(rest[1]);
            rest = &rest[2..];
            continue;
        }
        match byte {
            b'*' | b'?' => has_glob = true,
            b'[' if bracket_set(&rest[1..]) => {
                has_glob = true;
                if rest.get(1) == Some(&b'^') {
                    return Err(IncludeRefusal::CaretBracket(pattern));
                }
            }
            _ => {}
        }
        literal.push(byte);
        rest = &rest[1..];
    }
    Ok(IncludePattern {
        pattern,
        literal,
        has_glob,
    })
}

fn bracket_set(mut rest: &[u8]) -> bool {
    if rest.starts_with(b"!") {
        rest = &rest[1..];
    }
    // The first member may itself be ']'; only a later unescaped ']' closes the set.
    let first = if rest.starts_with(b"\\") { 2 } else { 1 };
    rest = rest.get(first..).unwrap_or_default();
    while let Some(&byte) = rest.first() {
        if byte == b']' {
            return true;
        }
        let width = if byte == b'\\' { 2 } else { 1 };
        rest = rest.get(width..).unwrap_or_default();
    }
    false
}

#[cfg(test)]
mod tests;
