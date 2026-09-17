//! Reading what `herdr plugin list --json` says about the plugins already
//! installed, which is how a pinned entry is checked without touching it.
//!
//! THE ENVELOPE IS THE ONLY TRUSTED SIGNAL, not the exit code. herdr answers
//! an error as a JSON envelope and still exits 0, so a document with no
//! `result.plugins` array is an ERROR here rather than an empty machine:
//! reading it as "no plugins installed" would report every pinned plugin as
//! missing and hand the operator a list of installs they do not need.

use std::collections::BTreeMap;

/// Where one installed plugin sits: the revision the install ASKED for, and
/// the commit it resolved to. Both are optional, because a plugin installed
/// from tip records no requested revision and a local plugin records neither.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Installed {
    pub(crate) requested_ref: Option<String>,
    pub(crate) commit: Option<String>,
}

impl Installed {
    /// Whether this installed copy is the revision `pin` names.
    ///
    /// THE REQUESTED REVISION IS THE FIRST ANSWER, because a pin is normally a
    /// tag or a branch and that is the field herdr recorded it in. A commit
    /// pin matches the resolved commit, by prefix so a short revision counts,
    /// and only from seven characters up: a shorter prefix would match half
    /// the commits in the repository.
    pub(crate) fn holds(&self, pin: &str) -> bool {
        if self.requested_ref.as_deref() == Some(pin) {
            return true;
        }
        match self.commit.as_deref() {
            Some(commit) => commit == pin || (pin.len() >= 7 && commit.starts_with(pin)),
            None => false,
        }
    }

    /// How the record names the revision this copy is actually at.
    pub(crate) fn revision(&self) -> String {
        match (self.requested_ref.as_deref(), self.commit.as_deref()) {
            (Some(reference), Some(commit)) => format!("{reference} ({commit})"),
            (Some(reference), None) => reference.to_string(),
            (None, Some(commit)) => commit.to_string(),
            (None, None) => "a revision it does not record".to_string(),
        }
    }
}

/// Every installed plugin by its id.
pub(crate) fn parse_plugin_list(stdout: &str) -> Result<BTreeMap<String, Installed>, String> {
    let document: serde_json::Value =
        serde_json::from_str(stdout).map_err(|why| format!("its answer is not JSON ({why})"))?;
    let plugins = document
        .pointer("/result/plugins")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "its answer carries no `result.plugins` list".to_string())?;
    let mut installed = BTreeMap::new();
    for plugin in plugins {
        let Some(id) = plugin.get("plugin_id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let text = |key: &str| {
            plugin
                .pointer(&format!("/source/{key}"))
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        };
        installed.insert(
            id.to_string(),
            Installed {
                requested_ref: text("requested_ref"),
                commit: text("resolved_commit"),
            },
        );
    }
    Ok(installed)
}

/// What the operator is told about a pinned plugin this lane will not install
/// for them: where it is, where the pin says it should be, and the one command
/// that moves it.
///
/// It names the command for the same reason the cargo lane does: a line that
/// says only "pinned and not at its revision" leaves the reader to reconstruct
/// the source repository and the flags from the roster.
pub(crate) fn pin_sentence(
    binary: &str,
    id: &str,
    repo: &str,
    pin: &str,
    at: Option<&Installed>,
) -> String {
    let standing = match at {
        Some(at) => format!("installed at {}", at.revision()),
        None => "not installed".to_string(),
    };
    format!(
        "plugin {id} is pinned at {pin} and is {standing}. Run the following command to install \
         that revision: {binary} plugin install {repo} --ref {pin} --yes"
    )
}

#[cfg(test)]
#[path = "listing/tests.rs"]
mod tests;
