//! The two known-good manifests, and the one question the alerter asks of
//! them: does this manifest vouch for the file at this path, exactly as it
//! stands right now?
//!
//! THE MANIFEST HAS TO BE TRUSTWORTHY BEFORE ITS CONTENTS MEAN ANYTHING. It is
//! the thing that decides whether a file change is a page, so a manifest the
//! operator's own account could rewrite would let anything vouch for itself.
//! `manifest_trustworthy` is the domain's rule and it wants the file's own uid
//! and mode, which is why this reads metadata before it reads bytes.
//!
//! CONTENT, MODE AND OWNER TOGETHER, never content alone. A file whose bytes
//! match while its mode gained a write bit, or whose owner changed, is not the
//! file the manifest recorded. The domain compares all three; this supplies the
//! observed side of that comparison.
//!
//! A MANIFEST THAT CANNOT BE READ VOUCHES FOR NOTHING, which is the direction
//! that pages. Reading an absent manifest as permission is how a real tamper
//! goes quiet.

use posture_domain::{
    KnownGoodTuple, ManifestAuthority, ManifestDigest, ManifestKind, manifest_for,
    manifest_trustworthy,
};
use sha2::{Digest, Sha256};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// The pipeline and managed-bin manifests, and the home they are judged against.
pub struct KnownGoodManifests {
    pipeline: PathBuf,
    managed_bin: PathBuf,
    home: String,
}

impl KnownGoodManifests {
    pub fn new(pipeline: PathBuf, managed_bin: PathBuf, home: String) -> Self {
        Self {
            pipeline,
            managed_bin,
            home,
        }
    }

    /// Whether the manifest that governs `target` vouches for it as it stands.
    pub fn vouches(&self, target: &str) -> bool {
        let manifest = match manifest_for(&self.home, target) {
            ManifestKind::Pipeline => &self.pipeline,
            ManifestKind::ManagedBin => &self.managed_bin,
        };
        let Some(text) = trusted_text(manifest) else {
            return false;
        };
        recorded_as_it_stands(&text, target)
    }
}

/// Whether these manifest lines vouch for the file at `target` as it stands.
///
/// SPLIT FROM THE TRUST CHECK ON PURPOSE, and not only to be testable. They are
/// two different questions with two different failure modes: "may these lines
/// be believed" is about the manifest's own ownership, and "do they describe
/// this file" is about the file. Answering them in one function meant a test
/// could only reach the second by being root.
pub fn recorded_as_it_stands(text: &str, target: &str) -> bool {
    let Some(observed) = observe(Path::new(target)) else {
        // THE FILE IS GONE, and a deletion is not something a manifest of
        // known-good CONTENT can vouch for. It pages.
        return false;
    };
    let observed_tuple = KnownGoodTuple {
        digest: ManifestDigest::Built(&observed.digest),
        mode: &observed.mode,
        uid: &observed.uid,
        path: target,
    };
    text.lines()
        .filter_map(KnownGoodTuple::parse_line)
        .filter(|recorded| recorded.path == target)
        .any(|recorded| recorded.matches(observed_tuple))
}

/// The tuple line that records a file exactly as it stands right now.
///
/// The generator that writes a manifest and the reader that checks one have to
/// agree on the field order and the formatting of the mode, so both come from
/// here rather than from two format strings.
pub fn tuple_line(target: &str) -> Option<String> {
    let observed = observe(Path::new(target))?;
    Some(format!(
        "{} {} {} {target}",
        observed.digest, observed.mode, observed.uid
    ))
}

/// The manifest's bytes, but only once its own ownership says it can be
/// believed.
fn trusted_text(manifest: &Path) -> Option<String> {
    let metadata = std::fs::metadata(manifest).ok()?;
    let uid = metadata.uid().to_string();
    let mode = u16::try_from(metadata.mode() & 0o7777).ok()?;
    // THE AUTHORITY IS ALWAYS `Protected` HERE. `ExplicitOverride` is a thing
    // the manifest GENERATOR is handed, not something a reader may assume of a
    // file it found on disk: it short-circuits the root-owned check, which is
    // the whole reason to consult a manifest at all.
    if !manifest_trustworthy(ManifestAuthority::Protected, Some(&uid), Some(mode)) {
        return None;
    }
    std::fs::read_to_string(manifest).ok()
}

struct Observed {
    digest: String,
    mode: String,
    uid: String,
}

/// The file as it stands: its content digest, its mode and its owner.
fn observe(path: &Path) -> Option<Observed> {
    // METADATA WITHOUT FOLLOWING A SYMLINK. A link the manifest never recorded,
    // pointed at a file it did, would otherwise vouch for the link's target and
    // say nothing about the swap.
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    Some(Observed {
        digest: Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        mode: format!("{:04o}", metadata.mode() & 0o7777),
        uid: metadata.uid().to_string(),
    })
}

#[cfg(test)]
#[path = "known_good_read/tests.rs"]
mod tests;
