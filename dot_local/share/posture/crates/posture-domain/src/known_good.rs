#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestDigest<'a> {
    Built(&'a str),
    Unbuilt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownGoodTuple<'a> {
    pub digest: ManifestDigest<'a>,
    pub mode: &'a str,
    pub uid: &'a str,
    pub path: &'a str,
}

impl<'a> KnownGoodTuple<'a> {
    pub fn parse_line(line: &'a str) -> Option<Self> {
        let mut fields = line.splitn(4, ' ');
        let hash = fields.next()?;
        let mode = fields.next()?;
        let uid = fields.next()?;
        let path = fields.next()?;
        let digest = if hash == "unbuilt" {
            ManifestDigest::Unbuilt
        } else if valid_digest(hash) {
            ManifestDigest::Built(hash)
        } else {
            return None;
        };
        if mode.len() != 4
            || !mode.bytes().all(|byte| (b'0'..=b'7').contains(&byte))
            || !(1..=10).contains(&uid.len())
            || !uid.bytes().all(|byte| byte.is_ascii_digit())
            || !path.starts_with('/')
            || path.len() < 2
        {
            return None;
        }
        Some(Self {
            digest,
            mode,
            uid,
            path,
        })
    }

    pub fn matches(self, observed: Self) -> bool {
        let (ManifestDigest::Built(expected), ManifestDigest::Built(actual)) =
            (self.digest, observed.digest)
        else {
            return false;
        };
        valid_digest(actual)
            && expected.eq_ignore_ascii_case(actual)
            && !observed.mode.is_empty()
            && !observed.uid.is_empty()
            && !observed.path.is_empty()
            && self.mode == observed.mode
            && self.uid == observed.uid
            && self.path == observed.path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Manifest<'a> {
    Trusted(&'a [KnownGoodTuple<'a>]),
    Missing,
    Unreadable,
    Empty,
    Untrustworthy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestAuthority {
    Protected,
    ExplicitOverride,
}

pub fn manifest_trustworthy(
    authority: ManifestAuthority,
    uid: Option<&str>,
    mode: Option<u16>,
) -> bool {
    authority == ManifestAuthority::ExplicitOverride
        || (uid == Some("0") && mode.is_some_and(|mode| mode & 0o022 == 0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestKind {
    Pipeline,
    ManagedBin,
}

pub fn manifest_for(home: &str, target: &str) -> ManifestKind {
    if pipeline_path(home, target) || !bin_path(home, target) {
        ManifestKind::Pipeline
    } else {
        ManifestKind::ManagedBin
    }
}

#[derive(Debug, Clone, Copy)]
pub struct KnownGood<'a> {
    pub home: &'a str,
    pub pipeline: Manifest<'a>,
    pub managed_bin: Manifest<'a>,
}

impl KnownGood<'_> {
    pub fn is_tracked(self, target: &str) -> bool {
        if pipeline_path(self.home, target)
            || (target.starts_with(&format!(
                "{}/Library/LaunchAgents/com.webdavis.osquery-",
                self.home
            )) && target.ends_with(".plist"))
            || target == format!("{}/.config/osquery/page-launchd-allowlist.txt", self.home)
        {
            return true;
        }
        if !bin_path(self.home, target) {
            return false;
        }
        match self.managed_bin {
            Manifest::Trusted(entries) => entries.iter().any(|entry| entry.path == target),
            Manifest::Missing
            | Manifest::Unreadable
            | Manifest::Empty
            | Manifest::Untrustworthy => true,
        }
    }

    pub fn vouches(self, observed: KnownGoodTuple<'_>) -> bool {
        let manifest = match manifest_for(self.home, observed.path) {
            ManifestKind::Pipeline => self.pipeline,
            ManifestKind::ManagedBin => self.managed_bin,
        };
        match manifest {
            Manifest::Trusted(entries) => entries.iter().any(|entry| entry.matches(observed)),
            Manifest::Missing
            | Manifest::Unreadable
            | Manifest::Empty
            | Manifest::Untrustworthy => false,
        }
    }
}

fn valid_digest(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn pipeline_path(home: &str, target: &str) -> bool {
    target.starts_with(&format!("{home}/.local/libexec/osquery/"))
        || target.starts_with(&format!("{home}/.local/libexec/posture/"))
}

fn bin_path(home: &str, target: &str) -> bool {
    target.starts_with(&format!("{home}/.local/bin/"))
        || target.starts_with(&format!("{home}/.local/libexec/"))
}

#[cfg(test)]
mod tests;
