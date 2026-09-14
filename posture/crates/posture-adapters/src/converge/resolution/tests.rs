use super::*;
use crate::converge::tests::fixture::Scratch;
use std::cell::RefCell;
use std::os::unix::fs::PermissionsExt;
fn trusted(_: &Path) -> Option<LiveAttributes> {
    Some(LiveAttributes {
        mode: 0o755,
        uid: 0,
        gid: 0,
    })
}

#[test]
fn search_resolution_checks_the_selected_parent_once_and_retains_that_exact_path() {
    let probes = RefCell::new(Vec::new());
    let parents = RefCell::new(Vec::new());
    let result = resolve_with(
        "osqueryctl",
        None,
        OsStr::new("/absent:/trusted"),
        |p| {
            probes.borrow_mut().push(p.to_path_buf());
            p == Path::new("/trusted/osqueryctl")
        },
        |p| {
            parents.borrow_mut().push(p.to_path_buf());
            trusted(p)
        },
    );
    assert_eq!(result, Ok(Some("/trusted/osqueryctl".into())));
    assert_eq!(
        *probes.borrow(),
        [
            PathBuf::from("/absent/osqueryctl"),
            PathBuf::from("/trusted/osqueryctl")
        ]
    );
    assert_eq!(*parents.borrow(), [PathBuf::from("/trusted")]);
}

#[test]
fn an_explicit_command_does_not_fall_back_to_an_unrelated_search_result() {
    let probes = RefCell::new(Vec::new());
    let result = resolve_with(
        "osqueryctl",
        Some(Path::new("/chosen/ctl")),
        OsStr::new("/different"),
        |p| {
            probes.borrow_mut().push(p.to_path_buf());
            true
        },
        trusted,
    );
    assert_eq!(result, Ok(Some("/chosen/ctl".into())));
    assert_eq!(*probes.borrow(), [PathBuf::from("/chosen/ctl")]);
}

#[test]
fn relative_resolution_and_untrusted_parent_attributes_are_returned_as_refusals() {
    assert_eq!(
        resolve_with(
            "osqueryctl",
            Some(Path::new("relative/ctl")),
            OsStr::new(""),
            |_| true,
            trusted
        ),
        Err(CommandRefusal {
            command: "relative/ctl".into(),
            reason: CommandTrustRefusal::Relative
        })
    );
    for (attributes, reason) in [
        (None, CommandTrustRefusal::Unreadable),
        (
            Some(LiveAttributes {
                mode: 0o755,
                uid: 501,
                gid: 0,
            }),
            CommandTrustRefusal::Owner(501),
        ),
        (
            Some(LiveAttributes {
                mode: 0o775,
                uid: 0,
                gid: 0,
            }),
            CommandTrustRefusal::Writable(0o775),
        ),
    ] {
        assert_eq!(
            resolve_with(
                "osqueryctl",
                Some(Path::new("/chosen/ctl")),
                OsStr::new(""),
                |_| true,
                |_| attributes
            ),
            Err(CommandRefusal {
                command: "/chosen/ctl".into(),
                reason
            })
        );
    }
}

#[test]
fn test_osquery_not_being_installed_at_all_is_a_quiet_no_op() {
    let absent = Scratch::new();
    let mut examined = 0;
    assert_eq!(
        resolve_with(
            "osqueryctl",
            None,
            absent.0.as_os_str(),
            |_| {
                examined += 1;
                false
            },
            |_| panic!("no candidate needs trust inspection")
        ),
        Ok(None)
    );
    assert_eq!(examined, 1);
    assert_eq!(
        resolve_osqueryctl(Some(&absent.0.join("missing")), OsStr::new("")),
        Ok(None)
    );
}

#[test]
fn native_resolution_reads_parent_ownership_and_never_launches_the_candidate() {
    let root = Scratch::new();
    let command = root.0.join("osqueryctl");
    std::fs::write(&command, b"must never execute").unwrap();
    std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o700)).unwrap();
    let uid = std::os::unix::fs::MetadataExt::uid(&std::fs::metadata(&root.0).unwrap());
    assert_ne!(uid, 0, "this unprivileged fixture must not run as root");
    assert_eq!(
        resolve_osqueryctl(Some(&command), OsStr::new("")),
        Err(CommandRefusal {
            command: command.clone(),
            reason: CommandTrustRefusal::Owner(uid)
        })
    );
    assert_eq!(std::fs::read(command).unwrap(), b"must never execute");
}

#[test]
fn daemon_search_uses_osqueryi_and_applies_the_same_parent_trust_gate() {
    for owner in [0, 501] {
        let found = resolve_with(
            "osqueryi",
            None,
            OsStr::new("/absent:/trusted daemon"),
            |path| path == Path::new("/trusted daemon/osqueryi"),
            |_| {
                Some(LiveAttributes {
                    mode: 0o755,
                    uid: owner,
                    gid: 0,
                })
            },
        );
        if owner == 0 {
            assert_eq!(found, Ok(Some("/trusted daemon/osqueryi".into())));
        } else {
            assert_eq!(
                found,
                Err(CommandRefusal {
                    command: "/trusted daemon/osqueryi".into(),
                    reason: CommandTrustRefusal::Owner(owner)
                })
            );
        }
    }
}

fn candidate(root: &Scratch, name: &str, mode: u32) -> PathBuf {
    let path = root.0.join(name);
    std::fs::write(&path, b"must never execute").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
    path
}

fn ownership_refusal(command: PathBuf) -> Result<Option<PathBuf>, CommandRefusal> {
    let uid = std::os::unix::fs::MetadataExt::uid(&std::fs::metadata(&command).unwrap());
    assert_ne!(uid, 0, "this unprivileged fixture must not run as root");
    Err(CommandRefusal {
        command,
        reason: CommandTrustRefusal::Owner(uid),
    })
}

#[test]
fn native_explicit_nonexecutable_command_does_not_fall_back() {
    for (name, resolve) in [
        (
            "osqueryctl",
            resolve_osqueryctl as fn(Option<&Path>, &OsStr) -> _,
        ),
        ("osqueryi", resolve_osqueryi),
    ] {
        let shadow = Scratch::new();
        let usable = Scratch::new();
        let requested = candidate(&shadow, name, 0o601);
        candidate(&usable, name, 0o700);
        assert_eq!(resolve(Some(&requested), usable.0.as_os_str()), Ok(None));
    }
}

#[test]
fn native_search_skips_nonexecutable_files_before_checking_parent_trust() {
    for (name, resolve) in [
        (
            "osqueryctl",
            resolve_osqueryctl as fn(Option<&Path>, &OsStr) -> _,
        ),
        ("osqueryi", resolve_osqueryi),
    ] {
        let shadow = Scratch::new();
        let usable = Scratch::new();
        candidate(&shadow, name, 0o601);
        let command = candidate(&usable, name, 0o700);
        let path = std::env::join_paths([&shadow.0, &usable.0]).unwrap();
        assert_eq!(resolve(None, &path), ownership_refusal(command));
    }
}

#[test]
fn native_search_skips_directories_the_user_cannot_search() {
    for (name, resolve) in [
        (
            "osqueryctl",
            resolve_osqueryctl as fn(Option<&Path>, &OsStr) -> _,
        ),
        ("osqueryi", resolve_osqueryi),
    ] {
        let shadow = Scratch::new();
        let usable = Scratch::new();
        candidate(&shadow, name, 0o700);
        let command = candidate(&usable, name, 0o700);
        let path = std::env::join_paths([&shadow.0, &usable.0]).unwrap();
        std::fs::set_permissions(&shadow.0, std::fs::Permissions::from_mode(0o601)).unwrap();
        let result = resolve(None, &path);
        std::fs::set_permissions(&shadow.0, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(result, ownership_refusal(command));
    }
}

#[test]
fn native_resolution_never_selects_a_directory_as_a_command() {
    for (name, resolve) in [
        (
            "osqueryctl",
            resolve_osqueryctl as fn(Option<&Path>, &OsStr) -> _,
        ),
        ("osqueryi", resolve_osqueryi),
    ] {
        let root = Scratch::new();
        let directory = root.0.join(name);
        std::fs::create_dir(&directory).unwrap();
        assert_eq!(resolve(Some(&directory), OsStr::new("")), Ok(None));
        assert_eq!(resolve(None, root.0.as_os_str()), Ok(None));
    }
}
