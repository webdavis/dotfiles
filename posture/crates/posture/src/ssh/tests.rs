use super::*;
use std::{cell::Cell, os::unix::ffi::OsStringExt, time::Duration};

#[test]
fn bare_extra_mixed_case_or_unknown_ssh_verbs_cannot_invoke_an_action() {
    let config = Configuration::read(|_| None);
    for words in [
        vec![],
        vec!["INSTALL"],
        vec!["--reload"],
        vec!["Reload"],
        vec!["reload", "extra"],
        vec!["install", "extra"],
        vec!["unknown"],
    ] {
        let args: Vec<_> = words.iter().map(OsString::from).collect();
        let mut out = vec![];
        let mut err = vec![];
        assert_eq!(
            execute(
                &args,
                &config,
                |_, _| panic!("no action authorized"),
                &mut out,
                &mut err
            ),
            2
        );
        assert!(out.is_empty());
        assert!(
            String::from_utf8(err)
                .unwrap()
                .contains("usage: posture ssh")
        );
    }
}
#[test]
fn each_explicit_verb_invokes_only_its_named_action_and_preserves_failure_status() {
    let config = Configuration::read(|_| None);
    for (word, verb) in [
        ("install", Verb::Install),
        ("verify", Verb::Verify),
        ("reload", Verb::Reload),
        ("rollback", Verb::Rollback),
    ] {
        let called = Cell::new(false);
        let mut out = vec![];
        let mut err = vec![];
        let code = execute(
            &[word.into()],
            &config,
            |selected, _| {
                assert_eq!(selected, verb);
                called.set(true);
                17
            },
            &mut out,
            &mut err,
        );
        assert_eq!(code, 17);
        assert!(called.get());
    }
}
#[test]
fn print_commands_and_help_do_not_construct_native_actions() {
    let config = Configuration::read(|key| {
        (key == "SSHD_CONFIG_D").then(|| OsString::from_vec(b"/private/with space-\xff".to_vec()))
    });
    for word in ["print-config", "print-path", "--help", "-h"] {
        let mut out = vec![];
        let mut err = vec![];
        assert_eq!(
            execute(
                &[word.into()],
                &config,
                |_, _| panic!("pure mode"),
                &mut out,
                &mut err
            ),
            0
        );
        assert!(err.is_empty());
        match word {
            "print-config" => assert_eq!(out, posture_domain::ssh_config().as_bytes()),
            "print-path" => assert_eq!(out, b"/private/with space-\xff/000-ssh-hardening.conf\n"),
            _ => assert_eq!(out, USAGE.as_bytes()),
        }
    }
}
#[test]
fn configuration_preserves_empty_readiness_and_privilege_while_empty_paths_use_defaults() {
    let default = Configuration::read(|_| None);
    assert_eq!(default.main, std::path::Path::new("/etc/ssh/sshd_config"));
    assert_eq!(default.sshd, std::path::Path::new("/usr/sbin/sshd"));
    assert_eq!(default.sudo, Some("sudo".into()));
    assert_eq!(default.launchctl, std::path::Path::new("/bin/launchctl"));
    assert_eq!(
        default.keyscan,
        std::path::Path::new("/usr/bin/ssh-keyscan")
    );
    assert_eq!(default.deadline, Duration::from_secs(120));
    assert_eq!(default.grace, Duration::from_secs(2));
    assert_eq!(default.readiness().unwrap().attempts, 30);
    let empty = Configuration::read(|_| Some("".into()));
    assert_eq!(empty.main, default.main);
    assert_eq!(empty.dropins, default.dropins);
    assert!(empty.sudo.is_none());
    assert!(empty.readiness().is_err());
    assert!(!empty.allow_missing);
    let override_config = Configuration::read(|key| match key {
        "SSH_HARDENING_VERIFY_DEADLINE" => Some("3".into()),
        "SSH_HARDENING_ALLOW_MISSING_SSHD" => Some("YeS".into()),
        "SSHD_BIN" => Some("/fixture/sshd".into()),
        _ => None,
    });
    assert_eq!(override_config.deadline, Duration::from_secs(3));
    assert!(override_config.allow_missing);
    assert_eq!(override_config.sshd, std::path::Path::new("/fixture/sshd"));
}
