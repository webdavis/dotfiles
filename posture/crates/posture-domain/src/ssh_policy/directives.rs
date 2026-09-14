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

pub fn ssh_directive_count() -> usize {
    DIRECTIVES.len()
}

/// The `refuseconnection` directive is deliberately NOT one of `DIRECTIVES`: it
/// resolves `no` outside the drop-in's Match block, which is correct, so the
/// unconditioned checks must not judge it. It is judged only against a resolved
/// local address.
const REFUSE_CONNECTION: &str = "refuseconnection";

/// The arrival addresses the verification resolves, each paired with the
/// `refuseconnection` value the drop-in's Match block must produce for it.
///
/// One allowed sample per negated term of that block, so a typo in any single
/// term turns a sample red instead of silently refusing a path the operator
/// depends on: the two ranges Tailscale documents for every tailnet, over both
/// address families, and loopback over both. Then one refused sample per
/// family. Every address is documented for universal use and none is one
/// host's own.
///
/// `fd00::1` is the refused IPv6 sample because it is a unique-local address
/// OUTSIDE the tailnet's own unique-local prefix, which is what proves the
/// IPv6 negation is prefix precise rather than admitting `fc00::/7` wholesale.
pub const SSH_LOCAL_ADDRESS_SAMPLES: [(&str, &str); 6] = [
    ("100.64.0.1", "no"),
    ("fd7a:115c:a1e0::1", "no"),
    ("127.0.0.1", "no"),
    ("::1", "no"),
    ("192.168.0.1", "yes"),
    ("fd00::1", "yes"),
];

/// The refusal verdict for one resolved connection, as a judgment list so it
/// composes with `judge_ssh_output` behind a single check.
pub fn judge_ssh_refusal(output: &[u8], required: &'static str) -> Vec<SshJudgment> {
    let actual =
        first_value(output, REFUSE_CONNECTION.as_bytes()).filter(|value| !value.is_empty());
    judgment(REFUSE_CONNECTION, required, actual)
        .into_iter()
        .collect()
}

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
