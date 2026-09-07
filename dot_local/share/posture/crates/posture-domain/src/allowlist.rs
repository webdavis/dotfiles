use crate::LaunchdIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllowlistEntry<'a> {
    pub identity: LaunchdIdentity<'a>,
    pub sha256: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowlistVerdict {
    Suppress,
    NotAllowlisted,
    ReusedLabel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allowlist<'a> {
    Read(&'a [AllowlistEntry<'a>]),
    Unreadable,
}

pub fn allowlist_verdict(
    home: &str,
    list_path: &str,
    entries: Allowlist<'_>,
    finding: LaunchdIdentity<'_>,
    current_hash: Option<&str>,
    mut vouch: impl FnMut(&str) -> bool,
) -> AllowlistVerdict {
    let Allowlist::Read(entries) = entries else {
        return AllowlistVerdict::NotAllowlisted;
    };
    let Some(entry) = entries
        .iter()
        .find(|entry| entry.identity.label == finding.label)
    else {
        return AllowlistVerdict::NotAllowlisted;
    };
    // Bash replaces embedded ~/ too, including tokens in program arguments.
    let home = format!("{home}/");
    let path = entry.identity.path.replace("~/", &home);
    let program = entry.identity.program.replace("~/", &home);
    if path.is_empty() || program.is_empty() {
        return AllowlistVerdict::NotAllowlisted;
    }
    if path != finding.path || program != finding.program {
        return AllowlistVerdict::ReusedLabel;
    }
    if !entry.sha256.is_empty() {
        if current_hash != Some(entry.sha256) {
            return AllowlistVerdict::ReusedLabel;
        }
    } else if !vouch(finding.path) {
        return AllowlistVerdict::NotAllowlisted;
    }
    // Misses and reused labels already page; only suppression spends this vouch.
    if !vouch(list_path) {
        return AllowlistVerdict::NotAllowlisted;
    }
    AllowlistVerdict::Suppress
}

pub fn valid_allowlist_label(label: &str) -> bool {
    let bytes = label.as_bytes();
    if bytes.len() < 2 || !bytes[0].is_ascii_alphanumeric() {
        return false;
    }
    if !bytes[1..]
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || b"._@-".contains(byte))
    {
        return false;
    }
    let lower = label.to_ascii_lowercase();
    lower != "com.apple" && !lower.starts_with("com.apple.")
}

pub fn relativize_allowlist_identity(home: &str, path: &str, program: &str) -> (String, String) {
    let home = format!("{home}/");
    let path = path
        .strip_prefix(&home)
        .map_or_else(|| path.to_owned(), |tail| format!("~/{tail}"));
    (path, program.replace(&home, "~/"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowlistLine<'a> {
    Preserved(&'a str),
    Object {
        label: Option<&'a str>,
        raw: &'a str,
    },
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowlistChange<'a> {
    Allow(AllowlistEntry<'a>),
    Deny(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CuratedLine<'a> {
    Preserved(&'a str),
    Added(AllowlistEntry<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurationRefusal {
    InvalidLabel,
    InvalidLine(usize),
}

pub fn curate_allowlist<'a>(
    lines: &[AllowlistLine<'a>],
    change: AllowlistChange<'a>,
) -> Result<Vec<CuratedLine<'a>>, CurationRefusal> {
    let label = match change {
        AllowlistChange::Allow(entry) => entry.identity.label,
        AllowlistChange::Deny(label) => label,
    };
    if !valid_allowlist_label(label) {
        return Err(CurationRefusal::InvalidLabel);
    }
    let mut result = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        match line {
            AllowlistLine::Invalid => return Err(CurationRefusal::InvalidLine(index + 1)),
            AllowlistLine::Preserved(raw) => result.push(CuratedLine::Preserved(raw)),
            AllowlistLine::Object { label: stored, raw } => {
                if *stored != Some(label) {
                    result.push(CuratedLine::Preserved(raw));
                }
            }
        }
    }
    if let AllowlistChange::Allow(entry) = change {
        result.push(CuratedLine::Added(entry));
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
