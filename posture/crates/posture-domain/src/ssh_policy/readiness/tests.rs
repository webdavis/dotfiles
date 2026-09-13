use super::*;

#[test]
fn readiness_keeps_bash_decimal_forms_but_refuses_empty_and_noncanonical_integers() {
    for interval in ["0", ".5", "0.5", "1.", "1.25"] {
        assert!(
            SshReadiness::parse("30", interval, "5").is_ok(),
            "{interval}"
        );
    }
    for invalid in ["", "0", "01", "-1", "1.5", "1000000000", "1e1"] {
        assert_eq!(
            SshReadiness::parse(invalid, "1", "5"),
            Err(ReadinessRefusal::Attempts)
        );
        assert_eq!(
            SshReadiness::parse("30", "1", invalid),
            Err(ReadinessRefusal::ProbeTimeout)
        );
    }
    for interval in ["", ".", "00", "01.5", "-1", "1e1", "1..2"] {
        assert_eq!(
            SshReadiness::parse("30", interval, "5"),
            Err(ReadinessRefusal::Interval)
        );
    }
    assert_eq!(
        SshReadiness::parse("1", ".5", "999999999")
            .unwrap()
            .interval,
        Duration::from_millis(500)
    );
}

#[test]
fn ports_keep_order_deduplicate_and_refuse_any_unusable_declaration() {
    assert_eq!(
        ssh_ports(b"port 65535\nport 22\nport 22\nother 4\n"),
        Ok(vec![65535, 22])
    );
    assert_eq!(ssh_ports(b"other 22\n"), Err(ReadinessRefusal::MissingPort));
    for port in ["", "0", "022", "65536", "-22", "22x"] {
        assert_eq!(
            ssh_ports(format!("port {port}\nport 22\n").as_bytes()),
            Err(ReadinessRefusal::Port(port.as_bytes().to_vec()))
        );
    }
}

#[test]
fn a_banner_requires_host_key_record_shape_not_comment_or_arbitrary_output() {
    for output in [
        b"# localhost ssh-rsa AAA".as_slice(),
        b"OK",
        b"host ssh-rsa",
        b"",
    ] {
        assert!(!has_host_key(output));
    }
    assert!(has_host_key(
        b"# chatter\n[127.0.0.1]:22 ssh-ed25519 AAAABBB\n"
    ));
    assert!(has_host_key(b"host new-algorithm material extra"));
}

#[test]
fn verify_deadline_keeps_the_positive_bounded_override_or_defaults() {
    assert_eq!(ssh_verify_deadline("86400"), Duration::from_secs(86400));
    assert_eq!(ssh_verify_deadline("1"), Duration::from_secs(1));
    for value in ["", "0", "01", "86401", "999999", "-1", "1.5"] {
        assert_eq!(ssh_verify_deadline(value), Duration::from_secs(120));
    }
}
