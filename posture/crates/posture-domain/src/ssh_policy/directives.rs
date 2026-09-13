use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub struct SshJudgment {
    pub keyword: &'static str,
    pub required: &'static str,
    pub actual: Option<Vec<u8>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PasswordChannel {
    Open,
    Blocked,
    Unreadable,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SshScan {
    Include(Vec<Vec<u8>>),
    Violation(SshJudgment),
    None,
}

const DIRECTIVES: [(&str, &str); 7] = [
    ("passwordauthentication", "no"),
    ("kbdinteractiveauthentication", "no"),
    ("usepam", "yes"),
    ("pubkeyauthentication", "yes"),
    ("permitrootlogin", "no"),
    ("gssapiauthentication", "no"),
    ("hostbasedauthentication", "no"),
];

pub fn ssh_config() -> &'static str {
    include_str!("dropin.conf")
}
pub fn ssh_dropin_path(directory: &Path) -> PathBuf {
    directory.join("000-ssh-hardening.conf")
}

pub fn judge_ssh_output(output: &[u8]) -> Vec<SshJudgment> {
    DIRECTIVES
        .iter()
        .filter_map(|&(keyword, required)| {
            let actual = first_value(output, keyword.as_bytes()).filter(|value| !value.is_empty());
            judgment(keyword, required, actual)
        })
        .collect()
}

pub fn password_channel(output: &[u8]) -> PasswordChannel {
    let mut open = false;
    for key in [
        b"passwordauthentication".as_slice(),
        b"kbdinteractiveauthentication",
    ] {
        match first_value(output, key) {
            Some(value) if value.eq_ignore_ascii_case(b"yes") => open = true,
            Some(value) if value.eq_ignore_ascii_case(b"no") => {}
            _ => return PasswordChannel::Unreadable,
        }
    }
    if open {
        PasswordChannel::Open
    } else {
        PasswordChannel::Blocked
    }
}

pub fn scan_ssh_line(line: &[u8], in_match: &mut bool) -> SshScan {
    let Some(line) = super::parse_ssh_line(line) else {
        return SshScan::None;
    };
    if line.keyword.eq_ignore_ascii_case(b"match") {
        *in_match = true;
    } else if line.keyword.eq_ignore_ascii_case(b"include") {
        return SshScan::Include(line.arguments);
    } else if *in_match {
        let key = match line.keyword.to_ascii_lowercase().as_slice() {
            b"challengeresponseauthentication" | b"skeyauthentication" => {
                b"kbdinteractiveauthentication".as_slice()
            }
            b"dsaauthentication" => b"pubkeyauthentication",
            _ => line.keyword.as_slice(),
        };
        if let Some(&(keyword, required)) = DIRECTIVES
            .iter()
            .find(|(name, _)| key.eq_ignore_ascii_case(name.as_bytes()))
            && let Some(failure) =
                judgment(keyword, required, line.arguments.first().map(Vec::as_slice))
        {
            return SshScan::Violation(failure);
        }
    }
    SshScan::None
}

fn judgment(
    keyword: &'static str,
    required: &'static str,
    actual: Option<&[u8]>,
) -> Option<SshJudgment> {
    (!actual.is_some_and(|value| value.eq_ignore_ascii_case(required.as_bytes()))).then(|| {
        SshJudgment {
            keyword,
            required,
            actual: actual.map(<[u8]>::to_vec),
        }
    })
}

fn first_value<'a>(output: &'a [u8], key: &[u8]) -> Option<&'a [u8]> {
    output.split(|byte| *byte == b'\n').find_map(|line| {
        let mut fields = line
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|part| !part.is_empty());
        (fields.next()? == key).then(|| fields.next().unwrap_or_default())
    })
}

#[cfg(test)]
mod tests;
