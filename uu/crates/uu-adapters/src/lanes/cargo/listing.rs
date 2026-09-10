//! Reading what `cargo` says: the install list, and one search result.
//!
//! BOTH FORMATS ARE HUMAN OUTPUT, not a stable interface. `cargo install
//! --list` and `cargo search` have no JSON mode, so a line whose shape is not
//! the one below is SKIPPED rather than half read. Guessing at a line means
//! reporting a version nobody has, and this lane's whole output is versions.

/// Where a crate came from, which decides whether the registry can answer for
/// it at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Source {
    Registry,
    /// Installed from a git remote, pinned at this short revision.
    Git {
        rev: String,
    },
}

/// One crate `cargo install --list` reports, with the binaries it put on PATH.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Installed {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) source: Source,
    pub(crate) binaries: Vec<String>,
}

/// Read `cargo install --list`.
///
/// Its shape is a header line at column zero naming the crate and its version,
/// then one indented line per binary:
///
/// ```text
/// fd-find v8.4.0:
///     fd
/// herdr-navigator v0.1.0 (https://example.git?rev=deed8496#deed8496):
///     herdr-navigator
/// ```
pub(crate) fn parse_install_list(stdout: &str) -> Vec<Installed> {
    let mut crates: Vec<Installed> = Vec::new();
    for line in stdout.lines() {
        if line.starts_with(char::is_whitespace) {
            // A BINARY BELONGS TO THE CRATE ABOVE IT. An indented line before
            // any header is output this does not understand, so it is dropped
            // rather than attached to nothing.
            if let Some(current) = crates.last_mut() {
                let binary = line.trim();
                if !binary.is_empty() {
                    current.binaries.push(binary.to_string());
                }
            }
            continue;
        }
        if let Some(header) = parse_header(line) {
            crates.push(header);
        }
    }
    crates
}

/// `<name> v<version>[ (<source>)]:` and nothing else.
fn parse_header(line: &str) -> Option<Installed> {
    let line = line.strip_suffix(':')?;
    let (name, rest) = line.split_once(' ')?;
    if name.is_empty() {
        return None;
    }
    let (version, source) = match rest.split_once(' ') {
        Some((version, origin)) => (version, git_rev(origin)?),
        None => (rest, Source::Registry),
    };
    let version = version.strip_prefix('v')?;
    if version.is_empty() {
        return None;
    }
    Some(Installed {
        name: name.to_string(),
        version: version.to_string(),
        source,
        binaries: Vec::new(),
    })
}

/// The revision out of a parenthesised git origin, which is the fragment after
/// its `#`. A parenthesised origin this does not recognize makes the whole
/// header unreadable rather than passing as a registry crate: calling a git
/// install a registry one would search crates.io for a version it never had.
fn git_rev(origin: &str) -> Option<Source> {
    let inner = origin.strip_prefix('(')?.strip_suffix(')')?;
    let (_, rev) = inner.rsplit_once('#')?;
    if rev.is_empty() {
        return None;
    }
    Some(Source::Git {
        rev: rev.to_string(),
    })
}

/// The newest version `cargo search` knows for exactly this crate.
///
/// MATCHED BY NAME, never by position. A search for `ripgrep` answers with
/// `gist-search` and `cgx-core` below it, whose descriptions merely mention
/// ripgrep, so reading the first line would report a neighbour's version as
/// this crate's.
pub(crate) fn parse_search(stdout: &str, crate_name: &str) -> Option<String> {
    for line in stdout.lines() {
        let (name, rest) = line.split_once(" = ")?;
        if name.trim() != crate_name {
            continue;
        }
        let quoted = rest.trim_start().strip_prefix('"')?;
        let (version, _) = quoted.split_once('"')?;
        if version.is_empty() {
            return None;
        }
        return Some(version.to_string());
    }
    None
}

/// What the operator is told when a crate is behind and this lane will not
/// compile it for them.
///
/// It names the command to run, because a report that says only "behind" leaves
/// the reader to work out that `fd` comes from a crate called `fd-find`.
pub(crate) fn behind_sentence(installed: &Installed, newest: &str) -> String {
    let binary = installed
        .binaries
        .first()
        .map_or(installed.name.as_str(), String::as_str);
    // ONE BINARY IS NAMED, SEVERAL ARE NOT. Listing four binaries in a sentence
    // about one upgrade reads as four things to do.
    let subject = if installed.binaries.len() > 1 {
        installed.name.as_str()
    } else {
        binary
    };
    format!(
        "{subject} has a new version: {} → {newest}. \
         Run the following command to compile it: cargo install {}",
        installed.version, installed.name
    )
}

#[cfg(test)]
#[path = "listing/tests.rs"]
mod tests;
