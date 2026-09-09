use super::*;

#[test]
fn bash_no_path() {
    let mut fixture = Fixture::default();
    check("", &mut fixture, b"", 0, &[]);
}

#[test]
fn bash_empty_path() {
    let mut fixture = Fixture::default();
    check("", &mut fixture, b"", 0, &[]);
}

#[test]
fn bash_missing_file() {
    let mut fixture = Fixture::default();
    check("/fixture/missing", &mut fixture, b"", 0, &[]);
}

#[test]
fn bash_extra_operand() {
    let mut fixture = Fixture::default();
    check("", &mut fixture, b"", 0, &[]);
}

#[test]
fn bash_regular_mach_o() {
    let mut fixture = Fixture::default();
    check(
        "/fixture/plain binary",
        &mut fixture,
        b"signed: Apple",
        0,
        &[
            "file:/fixture/plain binary",
            "codesign:/fixture/plain binary",
            "xattr:/fixture/plain binary",
        ],
    );
}

#[test]
fn bash_regular_file_failed() {
    let mut fixture = Fixture {
        mach_o: false,
        ..Default::default()
    };
    check(
        "/fixture/plain binary",
        &mut fixture,
        b"owner fixture, mode -rw-r--r--, modified 2026-09-07T00:00:00Z",
        0,
        &["file:/fixture/plain binary", "stat:/fixture/plain binary"],
    );
}

#[test]
fn bash_ordinary_metadata() {
    let mut fixture = Fixture {
        mach_o: false,
        ..Default::default()
    };
    check(
        "/fixture/plain binary",
        &mut fixture,
        b"owner fixture, mode -rw-r--r--, modified 2026-09-07T00:00:00Z",
        0,
        &["file:/fixture/plain binary", "stat:/fixture/plain binary"],
    );
}

#[test]
fn bash_metadata_failure() {
    let mut fixture = Fixture {
        mach_o: false,
        context: None,
        ..Default::default()
    };
    check(
        "/fixture/plain binary",
        &mut fixture,
        b"",
        0,
        &["file:/fixture/plain binary", "stat:/fixture/plain binary"],
    );
}

#[test]
fn bash_directory_metadata() {
    let mut fixture = Fixture::default();
    check(
        "/fixture/directory",
        &mut fixture,
        b"owner fixture, mode -rw-r--r--, modified 2026-09-07T00:00:00Z",
        0,
        &["stat:/fixture/directory"],
    );
}

#[test]
fn bash_hostile_path() {
    let mut fixture = Fixture::default();
    check(
        "/fixture/a\";touch NEVER;#.app",
        &mut fixture,
        b"signed: Apple",
        0,
        &[
            "codesign:/fixture/a\";touch NEVER;#.app",
            "xattr:/fixture/a\";touch NEVER;#.app",
        ],
    );
}
