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
