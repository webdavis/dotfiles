use super::*;

#[test]
fn bash_plist_unresolved() {
    let mut fixture = Fixture::default();
    check(
        "/fixture.plist",
        &mut fixture,
        b"launchd job, no program resolved (untrusted)",
        10,
        &[
            "plutil:Program:/fixture.plist",
            "plutil:ProgramArguments.0:/fixture.plist",
        ],
    );
}

#[test]
fn bash_plist_program_first() {
    let mut fixture = Fixture::default();
    fixture.plist.insert("Program", Ok(b"/first.app".to_vec()));
    fixture
        .plist
        .insert("ProgramArguments.0", Ok(b"/second.app".to_vec()));
    check(
        "/fixture.plist",
        &mut fixture,
        b"signed: Apple",
        0,
        &[
            "plutil:Program:/fixture.plist",
            "codesign:/first.app",
            "xattr:/first.app",
        ],
    );
}

#[test]
fn bash_plist_fallback() {
    let mut fixture = Fixture::default();
    fixture
        .plist
        .insert("Program", Err(InspectionFailure::Failed));
    fixture
        .plist
        .insert("ProgramArguments.0", Ok(b"/second.app".to_vec()));
    check(
        "/fixture.plist",
        &mut fixture,
        b"signed: Apple",
        0,
        &[
            "plutil:Program:/fixture.plist",
            "plutil:ProgramArguments.0:/fixture.plist",
            "codesign:/second.app",
            "xattr:/second.app",
        ],
    );
}

#[test]
fn bash_script_position_1() {
    let mut fixture = Fixture::default();
    fixture.plist.insert("Program", Ok(b"/bin/sh".to_vec()));
    fixture.plist.insert(
        "ProgramArguments.1",
        Ok(b"/fixture/a quoted script.sh".to_vec()),
    );
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs script a quoted script.sh via sh, payload unverified",
        0,
        &[
            "plutil:Program:/fixture.plist",
            "plutil:ProgramArguments.1:/fixture.plist",
            "xattr:/fixture/a quoted script.sh",
        ],
    );
}

#[test]
fn bash_script_position_2() {
    let mut fixture = Fixture::default();
    fixture.plist.insert("Program", Ok(b"/bin/sh".to_vec()));
    fixture.plist.insert(
        "ProgramArguments.2",
        Ok(b"/fixture/a quoted script.sh".to_vec()),
    );
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs script a quoted script.sh via sh, payload unverified",
        0,
        &[
            "plutil:Program:/fixture.plist",
            "plutil:ProgramArguments.1:/fixture.plist",
            "plutil:ProgramArguments.2:/fixture.plist",
            "xattr:/fixture/a quoted script.sh",
        ],
    );
}

#[test]
fn bash_script_position_3() {
    let mut fixture = Fixture::default();
    fixture.plist.insert("Program", Ok(b"/bin/sh".to_vec()));
    fixture.plist.insert(
        "ProgramArguments.3",
        Ok(b"/fixture/a quoted script.sh".to_vec()),
    );
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs script a quoted script.sh via sh, payload unverified",
        0,
        &[
            "plutil:Program:/fixture.plist",
            "plutil:ProgramArguments.1:/fixture.plist",
            "plutil:ProgramArguments.2:/fixture.plist",
            "plutil:ProgramArguments.3:/fixture.plist",
            "xattr:/fixture/a quoted script.sh",
        ],
    );
}

#[test]
fn bash_script_position_4() {
    let mut fixture = Fixture::default();
    fixture.plist.insert("Program", Ok(b"/bin/sh".to_vec()));
    fixture.plist.insert(
        "ProgramArguments.4",
        Ok(b"/fixture/a quoted script.sh".to_vec()),
    );
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs script a quoted script.sh via sh, payload unverified",
        0,
        &[
            "plutil:Program:/fixture.plist",
            "plutil:ProgramArguments.1:/fixture.plist",
            "plutil:ProgramArguments.2:/fixture.plist",
            "plutil:ProgramArguments.3:/fixture.plist",
            "plutil:ProgramArguments.4:/fixture.plist",
            "xattr:/fixture/a quoted script.sh",
        ],
    );
}

#[test]
fn bash_script_position_5() {
    let mut fixture = Fixture::default();
    fixture.plist.insert("Program", Ok(b"/bin/sh".to_vec()));
    fixture.plist.insert(
        "ProgramArguments.5",
        Ok(b"/fixture/a quoted script.sh".to_vec()),
    );
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs script a quoted script.sh via sh, payload unverified",
        0,
        &[
            "plutil:Program:/fixture.plist",
            "plutil:ProgramArguments.1:/fixture.plist",
            "plutil:ProgramArguments.2:/fixture.plist",
            "plutil:ProgramArguments.3:/fixture.plist",
            "plutil:ProgramArguments.4:/fixture.plist",
            "plutil:ProgramArguments.5:/fixture.plist",
            "xattr:/fixture/a quoted script.sh",
        ],
    );
}

#[test]
fn bash_script_position_6() {
    let mut fixture = Fixture::default();
    fixture.plist.insert("Program", Ok(b"/bin/sh".to_vec()));
    fixture.plist.insert(
        "ProgramArguments.6",
        Ok(b"/fixture/a quoted script.sh".to_vec()),
    );
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs sh (interpreter), payload unverified",
        0,
        &[
            "plutil:Program:/fixture.plist",
            "plutil:ProgramArguments.1:/fixture.plist",
            "plutil:ProgramArguments.2:/fixture.plist",
            "plutil:ProgramArguments.3:/fixture.plist",
            "plutil:ProgramArguments.4:/fixture.plist",
            "plutil:ProgramArguments.5:/fixture.plist",
        ],
    );
}

#[test]
fn bash_script_first_file() {
    let mut fixture = Fixture {
        downloaded: true,
        ..Default::default()
    };
    fixture.plist.insert("Program", Ok(b"/bin/bash".to_vec()));
    fixture
        .plist
        .insert("ProgramArguments.1", Ok(b"relative".to_vec()));
    fixture
        .plist
        .insert("ProgramArguments.2", Ok(b"/fixture/directory".to_vec()));
    fixture.plist.insert(
        "ProgramArguments.3",
        Ok(b"/fixture/a quoted script.sh".to_vec()),
    );
    fixture
        .plist
        .insert("ProgramArguments.4", Ok(b"/fixture/plain binary".to_vec()));
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs script a quoted script.sh via bash, payload unverified, downloaded",
        0,
        &[
            "plutil:Program:/fixture.plist",
            "plutil:ProgramArguments.1:/fixture.plist",
            "plutil:ProgramArguments.2:/fixture.plist",
            "plutil:ProgramArguments.3:/fixture.plist",
            "xattr:/fixture/a quoted script.sh",
        ],
    );
}
