#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeTrust {
    TrustedOrNotApplicable,
    Untrusted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enrichment {
    pub fact: Vec<u8>,
    pub trust: CodeTrust,
}

impl Enrichment {
    pub fn exit_code(&self) -> u8 {
        match self.trust {
            CodeTrust::TrustedOrNotApplicable => 0,
            CodeTrust::Untrusted => 10,
        }
    }
}

pub fn classify_signing(output: Option<&[u8]>) -> Enrichment {
    let untrusted = |fact: &[u8]| Enrichment {
        fact: fact.to_vec(),
        trust: CodeTrust::Untrusted,
    };
    let Some(output) = output else {
        return untrusted(b"UNSIGNED");
    };
    if contains_ascii_case(output, b"not signed") {
        return untrusted(b"UNSIGNED");
    }
    if contains_ascii_case(output, b"adhoc") {
        return untrusted(b"ad-hoc signature (untrusted)");
    }
    // Bash's first Authority line and awk field 2 are the compatibility contract.
    let authority = output
        .split(|byte| *byte == b'\n')
        .find_map(|line| line.strip_prefix(b"Authority="))
        .and_then(|value| value.split(|byte| *byte == b'=').next())
        .unwrap_or_default();
    if authority.is_empty() {
        return untrusted(b"signed, no authority (untrusted)");
    }
    let authority = if authority == b"Software Signing" || authority.starts_with(b"Apple") {
        b"Apple".as_slice()
    } else {
        authority
            .strip_prefix(b"Developer ID Application: ")
            .unwrap_or(authority)
    };
    let mut fact = b"signed: ".to_vec();
    fact.extend_from_slice(authority);
    Enrichment {
        fact,
        trust: CodeTrust::TrustedOrNotApplicable,
    }
}

fn contains_ascii_case(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|value| value.eq_ignore_ascii_case(needle))
}

pub fn is_interpreter(name: &[u8]) -> bool {
    matches!(
        name,
        b"sh"
            | b"bash"
            | b"zsh"
            | b"dash"
            | b"ksh"
            | b"python"
            | b"python2"
            | b"python3"
            | b"perl"
            | b"ruby"
            | b"node"
            | b"osascript"
            | b"php"
            | b"env"
    )
}

#[cfg(test)]
mod tests;
