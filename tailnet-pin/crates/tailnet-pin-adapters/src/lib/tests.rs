use super::*;
use std::fs;
use tailnet_pin_application::HostsFile;

/// A private directory that removes itself, so a failing test leaves nothing
/// behind and two tests never share a path.
struct Scratchpad(PathBuf);

impl Scratchpad {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("tailnet-pin-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("a scratch directory");
        Scratchpad(root)
    }

    fn file(&self, name: &str, contents: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, contents).expect("a scratch file");
        path
    }
}

impl Drop for Scratchpad {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The ordinary case: the path is the file.
#[test]
fn a_plain_path_reads_and_names_itself() {
    let scratch = Scratchpad::new("plain");
    let path = scratch.file("hosts", b"127.0.0.1\tlocalhost\n");
    let file = RealHostsFile::open(&path).expect("a regular file opens");
    assert_eq!(
        file.read().as_deref(),
        Some(b"127.0.0.1\tlocalhost\n".as_slice())
    );
    assert_eq!(file.description(), path.display().to_string());
}

/// A symlink chain is followed, and BOTH paths are named, because a refusal
/// about one and a success about the other read as two different files.
#[test]
fn a_symlinked_path_names_both_ends() {
    let scratch = Scratchpad::new("symlink");
    let real = scratch.file("hosts.real", b"127.0.0.1\tlocalhost\n");
    let link = scratch.0.join("hosts");
    std::os::unix::fs::symlink(&real, &link).expect("a symlink");

    let file = RealHostsFile::open(&link).expect("a symlink to a file opens");
    assert_eq!(
        file.read().as_deref(),
        Some(b"127.0.0.1\tlocalhost\n".as_slice())
    );
    let description = file.description();
    assert!(
        description.contains(&link.display().to_string()),
        "{description}"
    );
    assert!(
        description.contains(&real.display().to_string()),
        "{description}"
    );
}

/// A relative link target resolves against the directory of the link that holds
/// it, not against the working directory of whoever ran this.
#[test]
fn a_relative_link_resolves_beside_the_link() {
    let scratch = Scratchpad::new("relative");
    scratch.file("hosts.real", b"127.0.0.1\tlocalhost\n");
    let link = scratch.0.join("hosts");
    std::os::unix::fs::symlink("hosts.real", &link).expect("a relative symlink");

    let file = RealHostsFile::open(&link).expect("a relative symlink resolves");
    assert_eq!(
        file.read().as_deref(),
        Some(b"127.0.0.1\tlocalhost\n".as_slice())
    );
}

/// A loop refuses instead of spinning.
#[test]
fn a_symlink_loop_is_refused() {
    let scratch = Scratchpad::new("loop");
    let one = scratch.0.join("one");
    let two = scratch.0.join("two");
    std::os::unix::fs::symlink(&two, &one).expect("a symlink");
    std::os::unix::fs::symlink(&one, &two).expect("the other symlink");
    assert_eq!(RealHostsFile::open(&one), Err(PathFault::UnresolvedSymlink));
}

/// A path naming nothing, or naming a directory, is not a hosts file.
#[test]
fn a_path_that_is_not_a_regular_file_is_refused() {
    let scratch = Scratchpad::new("not-a-file");
    assert_eq!(
        RealHostsFile::open(&scratch.0.join("absent")),
        Err(PathFault::NotAFile)
    );
    assert_eq!(RealHostsFile::open(&scratch.0), Err(PathFault::NotAFile));
}

/// The install replaces the file's bytes and leaves nothing beside it.
#[test]
fn an_install_replaces_the_file_and_leaves_no_scratch_behind() {
    let scratch = Scratchpad::new("install");
    let path = scratch.file("hosts", b"old\n");
    let file = RealHostsFile::open(&path).expect("opens");
    file.install(b"new\n").expect("installs");

    assert_eq!(fs::read(&path).expect("read back"), b"new\n");
    let left: Vec<_> = fs::read_dir(&scratch.0)
        .expect("listing")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name())
        .collect();
    assert_eq!(left.len(), 1, "{left:?}");
}

/// THE TARGET'S OWN MODE SURVIVES. A rebuild that carried a private mode onto a
/// world-readable hosts file would leave the machine unable to read it.
#[test]
fn the_targets_mode_is_carried_onto_the_rebuild() {
    let scratch = Scratchpad::new("mode");
    let path = scratch.file("hosts", b"old\n");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("chmod");

    let file = RealHostsFile::open(&path).expect("opens");
    file.install(b"new\n").expect("installs");
    let mode = fs::metadata(&path).expect("metadata").permissions().mode() & 0o777;
    assert_eq!(mode, 0o644, "{mode:o}");
}

/// An install through a symlink replaces the FILE, not the link.
#[test]
fn an_install_through_a_symlink_replaces_the_real_file() {
    let scratch = Scratchpad::new("install-symlink");
    let real = scratch.file("hosts.real", b"old\n");
    let link = scratch.0.join("hosts");
    std::os::unix::fs::symlink(&real, &link).expect("a symlink");

    let file = RealHostsFile::open(&link).expect("opens");
    file.install(b"new\n").expect("installs");

    assert_eq!(fs::read(&real).expect("the real file"), b"new\n");
    assert!(
        fs::symlink_metadata(&link).expect("the link").is_symlink(),
        "the symlink was replaced"
    );
}

/// A file that cannot be read answers `None`, which every caller treats as a
/// refusal rather than as an empty file.
#[test]
fn a_file_that_vanishes_reads_as_nothing_rather_than_as_empty() {
    let scratch = Scratchpad::new("vanished");
    let path = scratch.file("hosts", b"127.0.0.1\tlocalhost\n");
    let file = RealHostsFile::open(&path).expect("opens");
    fs::remove_file(&path).expect("remove");
    assert_eq!(file.read(), None);
}
