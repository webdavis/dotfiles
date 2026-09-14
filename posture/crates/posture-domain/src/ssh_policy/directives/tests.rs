use super::*;

const HARDENED: &[u8] = b"passwordauthentication no\nkbdinteractiveauthentication no\nusepam yes\npubkeyauthentication yes\npermitrootlogin no\ngssapiauthentication no\nhostbasedauthentication no\n";

#[test]
fn print_config_emits_every_accepted_directive_exactly_once_and_is_pure() {
    let lines: Vec<_> = ssh_config()
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert_eq!(
        lines,
        [
            "PasswordAuthentication no",
            "KbdInteractiveAuthentication no",
            "UsePAM yes",
            "PubkeyAuthentication yes",
            "PermitRootLogin no",
            "GSSAPIAuthentication no",
            "HostbasedAuthentication no"
        ]
    );
}

#[test]
fn print_path_names_000_ssh_hardening_under_the_seam_and_sorts_before_100_macos_conf() {
    let path = ssh_dropin_path(Path::new("/private/fixture"));
    assert_eq!(path, Path::new("/private/fixture/000-ssh-hardening.conf"));
    assert!(path.file_name().unwrap() < std::ffi::OsStr::new("100-macos.conf"));
}

#[test]
fn output_judgment_distinguishes_missing_wrong_and_first_value_wins() {
    assert!(judge_ssh_output(HARDENED).is_empty());
    let mut output = b"passwordauthentication YES\n".to_vec();
    output.extend_from_slice(HARDENED);
    assert_eq!(
        judge_ssh_output(&output),
        [SshJudgment {
            keyword: "passwordauthentication",
            required: "no",
            actual: Some(b"YES".to_vec())
        }]
    );
    let absent = judge_ssh_output(b"passwordauthentication NO\n");
    assert_eq!(absent.len(), 6);
    assert!(absent.iter().all(|judgment| judgment.actual.is_none()));
}

#[test]
fn an_empty_first_effective_value_is_absent_even_if_a_later_value_exists() {
    let output = b"passwordauthentication\npasswordauthentication no\n";
    let failures = judge_ssh_output(output);
    assert_eq!(failures[0].keyword, "passwordauthentication");
    assert_eq!(failures[0].actual, None);
}

#[test]
fn match_scan_folds_aliases_and_retains_inherited_match_state() {
    let mut in_match = false;
    assert_eq!(
        scan_ssh_line(b"PasswordAuthentication yes", &mut in_match),
        SshScan::None
    );
    assert_eq!(
        scan_ssh_line(b"Ma\"tch\" all", &mut in_match),
        SshScan::None
    );
    for (line, keyword, required) in [
        (
            b"ChallengeresponseAuthentication yes".as_slice(),
            "kbdinteractiveauthentication",
            "no",
        ),
        (
            b"SkeyAuthentication yes",
            "kbdinteractiveauthentication",
            "no",
        ),
        (b"DSAAuthentication no", "pubkeyauthentication", "yes"),
    ] {
        let SshScan::Violation(judgment) = scan_ssh_line(line, &mut in_match) else {
            panic!("alias escaped judgment");
        };
        assert_eq!((judgment.keyword, judgment.required), (keyword, required));
    }
    assert_eq!(
        scan_ssh_line(b"Include a.conf b.conf", &mut in_match),
        SshScan::Include(vec![b"a.conf".to_vec(), b"b.conf".to_vec()])
    );
    assert_eq!(
        scan_ssh_line(b"PasswordAuthentication NO", &mut in_match),
        SshScan::None
    );
    assert!(in_match);
}

#[test]
fn password_recovery_requires_a_known_value_for_both_channels() {
    for (output, want) in [
        (
            b"passwordauthentication yes\nkbdinteractiveauthentication no\n".as_slice(),
            PasswordChannel::Open,
        ),
        (
            b"passwordauthentication no\nkbdinteractiveauthentication YES\n",
            PasswordChannel::Open,
        ),
        (
            b"passwordauthentication no\nkbdinteractiveauthentication no\n",
            PasswordChannel::Blocked,
        ),
        (
            b"passwordauthentication yes\nkbdinteractiveauthentication maybe\n",
            PasswordChannel::Unreadable,
        ),
        (b"passwordauthentication yes\n", PasswordChannel::Unreadable),
        (b"", PasswordChannel::Unreadable),
    ] {
        assert_eq!(password_channel(output), want);
    }
}
