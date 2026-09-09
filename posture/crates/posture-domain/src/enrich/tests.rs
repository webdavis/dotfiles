use super::*;

#[test]
fn signing_failure() {
    let actual = classify_signing(None);
    assert_eq!(actual.fact, b"UNSIGNED");
    assert_eq!(actual.exit_code(), 10);
}

#[test]
fn signing_not_signed() {
    let actual = classify_signing(Some(b"Authority=Apple\nNOT SIGNED\n"));
    assert_eq!(actual.fact, b"UNSIGNED");
    assert_eq!(actual.exit_code(), 10);
}

#[test]
fn signing_adhoc() {
    let actual = classify_signing(Some(b"Authority=Apple\nSignature=AdHoC\n"));
    assert_eq!(actual.fact, b"ad-hoc signature (untrusted)");
    assert_eq!(actual.exit_code(), 10);
}

#[test]
fn signing_not_signed_precedes_adhoc() {
    let actual = classify_signing(Some(b"adhoc not signed"));
    assert_eq!(actual.fact, b"UNSIGNED");
    assert_eq!(actual.exit_code(), 10);
}

#[test]
fn signing_no_authority() {
    let actual = classify_signing(Some(b"other output"));
    assert_eq!(actual.fact, b"signed, no authority (untrusted)");
    assert_eq!(actual.exit_code(), 10);
}

#[test]
fn signing_empty_first_authority() {
    let actual = classify_signing(Some(b"Authority=\nAuthority=Apple\n"));
    assert_eq!(actual.fact, b"signed, no authority (untrusted)");
    assert_eq!(actual.exit_code(), 10);
}

#[test]
fn signing_software_signing() {
    let actual = classify_signing(Some(b"Authority=Software Signing\n"));
    assert_eq!(actual.fact, b"signed: Apple");
    assert_eq!(actual.exit_code(), 0);
}

#[test]
fn signing_apple_prefix() {
    let actual = classify_signing(Some(b"Authority=Apple Anything\n"));
    assert_eq!(actual.fact, b"signed: Apple");
    assert_eq!(actual.exit_code(), 0);
}

#[test]
fn signing_developer_id() {
    let actual = classify_signing(Some(
        b"Authority=Developer ID Application: Example Team (ABC)\n",
    ));
    assert_eq!(actual.fact, b"signed: Example Team (ABC)");
    assert_eq!(actual.exit_code(), 0);
}

#[test]
fn signing_other_authority() {
    let actual = classify_signing(Some(b"Authority=Other Authority\n"));
    assert_eq!(actual.fact, b"signed: Other Authority");
    assert_eq!(actual.exit_code(), 0);
}

#[test]
fn signing_authority_equals() {
    let actual = classify_signing(Some(b"Authority=Other=Ignored\n"));
    assert_eq!(actual.fact, b"signed: Other");
    assert_eq!(actual.exit_code(), 0);
}

#[test]
fn signing_authority_leading_space() {
    let actual = classify_signing(Some(b" Authority=Apple\n"));
    assert_eq!(actual.fact, b"signed, no authority (untrusted)");
    assert_eq!(actual.exit_code(), 10);
}

#[test]
fn signing_authority_case() {
    let actual = classify_signing(Some(b"authority=Apple\n"));
    assert_eq!(actual.fact, b"signed, no authority (untrusted)");
    assert_eq!(actual.exit_code(), 10);
}

#[test]
fn signing_stdout_authority() {
    let actual = classify_signing(Some(b"Authority=Apple\n"));
    assert_eq!(actual.fact, b"signed: Apple");
    assert_eq!(actual.exit_code(), 0);
}

#[test]
fn interpreter_sh() {
    assert!(is_interpreter(b"sh"));
}
#[test]
fn interpreter_bash() {
    assert!(is_interpreter(b"bash"));
}
#[test]
fn interpreter_zsh() {
    assert!(is_interpreter(b"zsh"));
}
#[test]
fn interpreter_dash() {
    assert!(is_interpreter(b"dash"));
}
#[test]
fn interpreter_ksh() {
    assert!(is_interpreter(b"ksh"));
}
#[test]
fn interpreter_python() {
    assert!(is_interpreter(b"python"));
}
#[test]
fn interpreter_python2() {
    assert!(is_interpreter(b"python2"));
}
#[test]
fn interpreter_python3() {
    assert!(is_interpreter(b"python3"));
}
#[test]
fn interpreter_perl() {
    assert!(is_interpreter(b"perl"));
}
#[test]
fn interpreter_ruby() {
    assert!(is_interpreter(b"ruby"));
}
#[test]
fn interpreter_node() {
    assert!(is_interpreter(b"node"));
}
#[test]
fn interpreter_osascript() {
    assert!(is_interpreter(b"osascript"));
}
#[test]
fn interpreter_php() {
    assert!(is_interpreter(b"php"));
}
#[test]
fn interpreter_env() {
    assert!(is_interpreter(b"env"));
}
#[test]
fn similar_names_are_not_interpreters() {
    for name in [b"python4".as_slice(), b"BASH", b"bashful", b""] {
        assert!(!is_interpreter(name));
    }
}
