//! Read off `results-alerter/pipeline-verdict.sh`, the helper this replaces.
//!
//! THE TRUST CHECK AND THE MATCH ARE TESTED APART, because the real manifests
//! are root-owned in `/var/osquery` and a test cannot chown. The refusals a
//! non-root manifest earns are asserted through `vouches`; everything about
//! whether the lines describe the file is asserted through
//! `recorded_as_it_stands`, which is the same code path minus that one gate.

use super::*;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicUsize, Ordering};

fn scratch() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "posture-manifest-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

struct Fixture {
    root: PathBuf,
    target: PathBuf,
}

/// The tuple line that records a file exactly as it stands right now.
///
/// The generator that writes a manifest and the reader that checks one have to
/// agree on the field order and the formatting of the mode, so both come from
/// here rather than from two format strings.
fn tuple_line(target: &str) -> Option<String> {
    let observed = observe(Path::new(target))?;
    Some(format!(
        "{} {} {} {target}",
        observed.digest, observed.mode, observed.uid
    ))
}

impl Fixture {
    fn new(contents: &[u8]) -> Self {
        let root = scratch();
        let target = root.join("tracked");
        std::fs::write(&target, contents).unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o644)).unwrap();
        Self { root, target }
    }

    fn path(&self) -> String {
        self.target.to_string_lossy().into_owned()
    }

    /// The manifest text recording the target exactly as it stands right now.
    fn recorded(&self) -> String {
        format!("{}\n", tuple_line(&self.path()).expect("a tuple line"))
    }

    fn vouched(&self, text: &str) -> bool {
        recorded_as_it_stands(text, &self.path())
    }
}

#[test]
fn a_file_exactly_as_recorded_is_vouched_for() {
    let fixture = Fixture::new(b"the recorded bytes\n");
    assert!(fixture.vouched(&fixture.recorded()));
}

#[test]
fn a_file_whose_bytes_changed_is_not_vouched_for() {
    let fixture = Fixture::new(b"the recorded bytes\n");
    let recorded = fixture.recorded();
    std::fs::write(&fixture.target, b"tampered\n").unwrap();
    assert!(!fixture.vouched(&recorded));
}

#[test]
fn a_file_whose_mode_gained_a_write_bit_is_not_vouched_for() {
    // CONTENT ALONE IS NOT THE FILE. A script whose bytes are untouched but
    // which anyone may now rewrite is a different security posture, and the
    // manifest records the mode for exactly this.
    let fixture = Fixture::new(b"the recorded bytes\n");
    let recorded = fixture.recorded();
    std::fs::set_permissions(&fixture.target, std::fs::Permissions::from_mode(0o666)).unwrap();
    assert!(!fixture.vouched(&recorded));
}

#[test]
fn a_file_that_is_gone_is_not_vouched_for() {
    // A deletion is not something a manifest of known-good CONTENT can vouch
    // for, so it pages.
    let fixture = Fixture::new(b"the recorded bytes\n");
    let recorded = fixture.recorded();
    std::fs::remove_file(&fixture.target).unwrap();
    assert!(!fixture.vouched(&recorded));
}

#[test]
fn a_symlink_standing_where_a_tracked_file_was_is_not_vouched_for() {
    // Following it would vouch for the link's TARGET and say nothing about the
    // swap, which is the whole move an attacker would make.
    let fixture = Fixture::new(b"the recorded bytes\n");
    let recorded = fixture.recorded();
    let elsewhere = fixture.root.join("elsewhere");
    std::fs::write(&elsewhere, b"the recorded bytes\n").unwrap();
    std::fs::set_permissions(&elsewhere, std::fs::Permissions::from_mode(0o644)).unwrap();
    std::fs::remove_file(&fixture.target).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &fixture.target).unwrap();
    assert!(!fixture.vouched(&recorded));
}

#[test]
fn a_manifest_that_does_not_name_this_path_vouches_for_nothing() {
    // A tuple is bound to its exact path; the same digest under another name is
    // a different file.
    let fixture = Fixture::new(b"the recorded bytes\n");
    let other = fixture.root.join("untracked");
    std::fs::write(&other, b"the recorded bytes\n").unwrap();
    std::fs::set_permissions(&other, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(!recorded_as_it_stands(
        &fixture.recorded(),
        &other.to_string_lossy()
    ));
}

#[test]
fn a_line_that_is_not_a_tuple_costs_that_line_and_no_other() {
    let fixture = Fixture::new(b"the recorded bytes\n");
    let text = format!("not a tuple\n\nzz 0644 0 /nope\n{}", fixture.recorded());
    assert!(fixture.vouched(&text));
}

#[test]
fn a_manifest_no_root_owns_is_not_believed_however_right_its_lines_are() {
    // IT DECIDES WHETHER A CHANGE IS A PAGE. A manifest the operator's own
    // account could append to would let anything vouch for itself, so the lines
    // are never read at all until the file's own ownership says they can be.
    // The real ones are root-owned 0644 in /var/osquery; this one is not.
    let fixture = Fixture::new(b"the recorded bytes\n");
    let manifest = fixture.root.join("known-good.sha256");
    std::fs::write(&manifest, fixture.recorded()).unwrap();
    std::fs::set_permissions(&manifest, std::fs::Permissions::from_mode(0o644)).unwrap();
    let manifests = KnownGoodManifests::new(
        manifest.clone(),
        manifest,
        fixture.root.to_string_lossy().into_owned(),
    );
    assert!(
        fixture.vouched(&fixture.recorded()),
        "the control: the lines do describe the file"
    );
    assert!(
        !manifests.vouches(&fixture.path()),
        "and they are still refused, because no root owns the manifest"
    );
}

#[test]
fn a_manifest_that_cannot_be_read_at_all_vouches_for_nothing() {
    // READING AN ABSENT MANIFEST AS PERMISSION is how a real tamper goes quiet.
    let fixture = Fixture::new(b"the recorded bytes\n");
    let manifests = KnownGoodManifests::new(
        fixture.root.join("absent"),
        fixture.root.join("absent"),
        fixture.root.to_string_lossy().into_owned(),
    );
    assert!(!manifests.vouches(&fixture.path()));
}
