use super::*;

#[test]
fn bash_bundle_app() {
    let mut fixture = Fixture::default();
    check(
        "/fixture.app",
        &mut fixture,
        b"signed: Apple",
        0,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}

#[test]
fn bash_bundle_kext() {
    let mut fixture = Fixture::default();
    check(
        "/fixture.kext",
        &mut fixture,
        b"signed: Apple",
        0,
        &["codesign:/fixture.kext", "xattr:/fixture.kext"],
    );
}

#[test]
fn bash_bundle_systemextension() {
    let mut fixture = Fixture::default();
    check(
        "/fixture.systemextension",
        &mut fixture,
        b"signed: Apple",
        0,
        &[
            "codesign:/fixture.systemextension",
            "xattr:/fixture.systemextension",
        ],
    );
}

#[test]
fn bash_bundle_dext() {
    let mut fixture = Fixture::default();
    check(
        "/fixture.dext",
        &mut fixture,
        b"signed: Apple",
        0,
        &["codesign:/fixture.dext", "xattr:/fixture.dext"],
    );
}

#[test]
fn bash_bundle_appex() {
    let mut fixture = Fixture::default();
    check(
        "/fixture.appex",
        &mut fixture,
        b"signed: Apple",
        0,
        &["codesign:/fixture.appex", "xattr:/fixture.appex"],
    );
}

#[test]
fn bash_signing_failure() {
    let mut fixture = Fixture {
        signature: Err(InspectionFailure::Failed),
        ..Default::default()
    };
    check(
        "/fixture.app",
        &mut fixture,
        b"UNSIGNED",
        10,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}

#[test]
fn bash_signing_not_signed() {
    let mut fixture = Fixture {
        signature: Ok(b"Authority=Apple\nNOT SIGNED".to_vec()),
        ..Default::default()
    };
    check(
        "/fixture.app",
        &mut fixture,
        b"UNSIGNED",
        10,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}

#[test]
fn bash_signing_adhoc() {
    let mut fixture = Fixture {
        signature: Ok(b"Authority=Apple\nSignature=AdHoC".to_vec()),
        ..Default::default()
    };
    check(
        "/fixture.app",
        &mut fixture,
        b"ad-hoc signature (untrusted)",
        10,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}

#[test]
fn bash_signing_not_signed_precedes_adhoc() {
    let mut fixture = Fixture {
        signature: Ok(b"adhoc not signed".to_vec()),
        ..Default::default()
    };
    check(
        "/fixture.app",
        &mut fixture,
        b"UNSIGNED",
        10,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}

#[test]
fn bash_signing_no_authority() {
    let mut fixture = Fixture {
        signature: Ok(b"other output".to_vec()),
        ..Default::default()
    };
    check(
        "/fixture.app",
        &mut fixture,
        b"signed, no authority (untrusted)",
        10,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}

#[test]
fn bash_signing_empty_first_authority() {
    let mut fixture = Fixture {
        signature: Ok(b"Authority=\nAuthority=Apple".to_vec()),
        ..Default::default()
    };
    check(
        "/fixture.app",
        &mut fixture,
        b"signed, no authority (untrusted)",
        10,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}

#[test]
fn bash_signing_software_signing() {
    let mut fixture = Fixture {
        signature: Ok(b"Authority=Software Signing".to_vec()),
        ..Default::default()
    };
    check(
        "/fixture.app",
        &mut fixture,
        b"signed: Apple",
        0,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}

#[test]
fn bash_signing_apple_prefix() {
    let mut fixture = Fixture {
        signature: Ok(b"Authority=Apple Anything".to_vec()),
        ..Default::default()
    };
    check(
        "/fixture.app",
        &mut fixture,
        b"signed: Apple",
        0,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}

#[test]
fn bash_signing_developer_id() {
    let mut fixture = Fixture {
        signature: Ok(b"Authority=Developer ID Application: Example Team (ABC)".to_vec()),
        ..Default::default()
    };
    check(
        "/fixture.app",
        &mut fixture,
        b"signed: Example Team (ABC)",
        0,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}

#[test]
fn bash_signing_other_authority() {
    let mut fixture = Fixture {
        signature: Ok(b"Authority=Other Authority".to_vec()),
        ..Default::default()
    };
    check(
        "/fixture.app",
        &mut fixture,
        b"signed: Other Authority",
        0,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}

#[test]
fn bash_signing_authority_equals() {
    let mut fixture = Fixture {
        signature: Ok(b"Authority=Other=Ignored".to_vec()),
        ..Default::default()
    };
    check(
        "/fixture.app",
        &mut fixture,
        b"signed: Other",
        0,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}

#[test]
fn bash_signing_authority_leading_space() {
    let mut fixture = Fixture {
        signature: Ok(b" Authority=Apple".to_vec()),
        ..Default::default()
    };
    check(
        "/fixture.app",
        &mut fixture,
        b"signed, no authority (untrusted)",
        10,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}

#[test]
fn bash_signing_authority_case() {
    let mut fixture = Fixture {
        signature: Ok(b"authority=Apple".to_vec()),
        ..Default::default()
    };
    check(
        "/fixture.app",
        &mut fixture,
        b"signed, no authority (untrusted)",
        10,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}

#[test]
fn bash_signing_stdout_authority() {
    let mut fixture = Fixture {
        signature: Ok(b"Authority=Apple".to_vec()),
        ..Default::default()
    };
    check(
        "/fixture.app",
        &mut fixture,
        b"signed: Apple",
        0,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}

#[test]
fn bash_downloaded() {
    let mut fixture = Fixture {
        downloaded: true,
        ..Default::default()
    };
    check(
        "/fixture.app",
        &mut fixture,
        b"signed: Apple, downloaded",
        0,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}

#[test]
fn bash_quarantine_failure() {
    let mut fixture = Fixture {
        downloaded: false,
        ..Default::default()
    };
    check(
        "/fixture.app",
        &mut fixture,
        b"signed: Apple",
        0,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}

#[test]
fn bash_quarantine_empty() {
    let mut fixture = Fixture {
        downloaded: false,
        ..Default::default()
    };
    check(
        "/fixture.app",
        &mut fixture,
        b"signed: Apple",
        0,
        &["codesign:/fixture.app", "xattr:/fixture.app"],
    );
}
