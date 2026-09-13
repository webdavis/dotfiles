use super::*;
use posture_domain::SshTreeRefusal;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
    tree: SshConfigTree,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "posture-ssh-tree-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let dropins = root.join("sshd_config.d");
        fs::create_dir(&dropins).unwrap();
        let main = root.join("sshd_config");
        fs::write(&main, "").unwrap();
        Self {
            root,
            tree: SshConfigTree::new(main, dropins),
        }
    }
}

#[test]
fn a_newline_named_dropin_is_refused_instead_of_silently_omitted() {
    for name in ["line\nbreak.conf", "unit\u{1f}separator.conf"] {
        let fixture = Fixture::new();
        fs::write(
            fixture.tree.dropins.join(name),
            "Match all\nPasswordAuthentication yes\n",
        )
        .unwrap();
        assert!(matches!(
            fixture.tree.observe(),
            Err(SshScanFailure::File(SshTreeRefusal::Path(_)))
        ));
    }
}

#[test]
fn match_state_flows_into_includes_and_does_not_flow_back() {
    let fixture = Fixture::new();
    fs::write(
        &fixture.tree.main,
        "Include child.conf\nPasswordAuthentication yes\nMatch all\nInclude inherited.conf\n",
    )
    .unwrap();
    fs::write(
        fixture.root.join("child.conf"),
        "Match all\nChallengeResponseAuthentication yes\n",
    )
    .unwrap();
    fs::write(
        fixture.root.join("inherited.conf"),
        "PasswordAuthentication yes\n",
    )
    .unwrap();
    let failures = fixture.tree.scan();
    assert_eq!(failures.len(), 2, "{failures:?}");
    assert!(
        failures
            .iter()
            .all(|failure| matches!(failure, SshScanFailure::Directive { .. }))
    );
}

#[test]
fn repeated_roots_report_a_violation_once_and_observe_each_unique_file() {
    let fixture = Fixture::new();
    fs::write(&fixture.tree.main, "Include sshd_config.d/*\n").unwrap();
    fs::write(
        fixture.tree.dropins.join("bad.conf"),
        "Match all\nPasswordAuthentication yes\n",
    )
    .unwrap();
    assert_eq!(fixture.tree.scan().len(), 1);
    assert_eq!(fixture.tree.observe().unwrap().len(), 2);
}

#[test]
fn observation_follows_symlink_target_attributes_and_detects_every_changed_dimension() {
    use posture_domain::{SshTreeChange, compare_ssh_trees};
    let fixture = Fixture::new();
    let target = fixture.root.join("target");
    fs::write(&target, "before\n").unwrap();
    fs::write(&fixture.tree.main, "Include alias\n").unwrap();
    symlink(&target, fixture.root.join("alias")).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    let before = fixture.tree.observe().unwrap();
    assert_eq!(before.len(), 2);
    assert_eq!(before[0].attributes.mode, 0o600);
    fs::write(&target, "after\n").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
    let changes = compare_ssh_trees(&before, &fixture.tree.observe().unwrap());
    assert!(matches!(
        changes.as_slice(),
        [
            SshTreeChange::Content(_),
            SshTreeChange::Attributes(_, _, _)
        ]
    ));
}

#[test]
fn missing_include_is_ignored_but_empty_include_caret_cycle_and_depth_are_refused() {
    for content in ["Include\n", "Include [^a].conf\n", "Include sshd_config\n"] {
        let fixture = Fixture::new();
        fs::write(&fixture.tree.main, content).unwrap();
        assert!(fixture.tree.observe().is_err(), "{content}");
        assert!(!fixture.tree.scan().is_empty());
    }
    let fixture = Fixture::new();
    fs::write(&fixture.tree.main, "Include absent\n").unwrap();
    assert_eq!(fixture.tree.observe().unwrap().len(), 1);
    assert!(fixture.tree.scan().is_empty());
}

#[test]
fn observation_refuses_width_and_bytes_before_unbounded_reading() {
    let fixture = Fixture::new();
    fs::write(&fixture.tree.main, vec![b'#'; 262145]).unwrap();
    assert!(matches!(
        fixture.tree.observe(),
        Err(SshScanFailure::File(SshTreeRefusal::Bytes))
    ));
    fs::write(&fixture.tree.main, "Include child\n".repeat(512)).unwrap();
    fs::write(fixture.root.join("child"), "").unwrap();
    assert!(matches!(
        fixture.tree.observe(),
        Err(SshScanFailure::File(SshTreeRefusal::Visits))
    ));
}

#[test]
fn a_resolved_file_replaced_by_a_pipe_is_refused_at_open_without_blocking() {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let fixture = Fixture::new();
    let path = fixture.root.join("payload");
    fs::write(&path, "safe\n").unwrap();
    let pattern = posture_domain::analyze_include(
        fixture.root.as_os_str().as_bytes(),
        &[b"payload".to_vec()],
    )
    .unwrap()
    .remove(0);
    let paths = resolution::resolve(&pattern).unwrap();
    fs::rename(&path, fixture.root.join("saved-payload")).unwrap();
    let cpath = CString::new(path.as_os_str().as_bytes()).unwrap();
    // This FIFO replaces only the file just resolved inside this private fixture.
    assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
    assert!(matches!(
        reading::read(&paths[0], &mut SshWalkBudget::default()),
        Err(SshTreeRefusal::NonRegular(_))
    ));
}

#[test]
fn include_depth_and_failed_root_listing_are_explicit_refusals() {
    let fixture = Fixture::new();
    fs::write(&fixture.tree.main, "Include level0\n").unwrap();
    for level in 0..17 {
        fs::write(
            fixture.root.join(format!("level{level}")),
            format!("Include level{}\n", level + 1),
        )
        .unwrap();
    }
    assert!(matches!(
        fixture.tree.observe(),
        Err(SshScanFailure::File(SshTreeRefusal::Depth))
    ));
    let file = fixture.root.join("not-a-directory");
    fs::write(&file, "").unwrap();
    let tree = SshConfigTree::new(fixture.tree.main.clone(), file);
    assert!(matches!(
        tree.observe(),
        Err(SshScanFailure::File(SshTreeRefusal::Unreadable(_)))
    ));
}
