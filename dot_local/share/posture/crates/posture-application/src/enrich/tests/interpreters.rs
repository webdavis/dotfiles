use super::*;

#[test]
fn bash_interpreter_sh() {
    let mut fixture = Fixture::default();
    fixture.plist.insert("Program", Ok(b"/usr/bin/sh".to_vec()));
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
fn bash_interpreter_bash() {
    let mut fixture = Fixture::default();
    fixture
        .plist
        .insert("Program", Ok(b"/usr/bin/bash".to_vec()));
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs bash (interpreter), payload unverified",
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
fn bash_interpreter_zsh() {
    let mut fixture = Fixture::default();
    fixture
        .plist
        .insert("Program", Ok(b"/usr/bin/zsh".to_vec()));
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs zsh (interpreter), payload unverified",
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
fn bash_interpreter_dash() {
    let mut fixture = Fixture::default();
    fixture
        .plist
        .insert("Program", Ok(b"/usr/bin/dash".to_vec()));
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs dash (interpreter), payload unverified",
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
fn bash_interpreter_ksh() {
    let mut fixture = Fixture::default();
    fixture
        .plist
        .insert("Program", Ok(b"/usr/bin/ksh".to_vec()));
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs ksh (interpreter), payload unverified",
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
fn bash_interpreter_python() {
    let mut fixture = Fixture::default();
    fixture
        .plist
        .insert("Program", Ok(b"/usr/bin/python".to_vec()));
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs python (interpreter), payload unverified",
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
fn bash_interpreter_python2() {
    let mut fixture = Fixture::default();
    fixture
        .plist
        .insert("Program", Ok(b"/usr/bin/python2".to_vec()));
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs python2 (interpreter), payload unverified",
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
fn bash_interpreter_python3() {
    let mut fixture = Fixture::default();
    fixture
        .plist
        .insert("Program", Ok(b"/usr/bin/python3".to_vec()));
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs python3 (interpreter), payload unverified",
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
fn bash_interpreter_perl() {
    let mut fixture = Fixture::default();
    fixture
        .plist
        .insert("Program", Ok(b"/usr/bin/perl".to_vec()));
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs perl (interpreter), payload unverified",
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
fn bash_interpreter_ruby() {
    let mut fixture = Fixture::default();
    fixture
        .plist
        .insert("Program", Ok(b"/usr/bin/ruby".to_vec()));
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs ruby (interpreter), payload unverified",
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
fn bash_interpreter_node() {
    let mut fixture = Fixture::default();
    fixture
        .plist
        .insert("Program", Ok(b"/usr/bin/node".to_vec()));
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs node (interpreter), payload unverified",
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
fn bash_interpreter_osascript() {
    let mut fixture = Fixture::default();
    fixture
        .plist
        .insert("Program", Ok(b"/usr/bin/osascript".to_vec()));
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs osascript (interpreter), payload unverified",
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
fn bash_interpreter_php() {
    let mut fixture = Fixture::default();
    fixture
        .plist
        .insert("Program", Ok(b"/usr/bin/php".to_vec()));
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs php (interpreter), payload unverified",
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
fn bash_interpreter_env() {
    let mut fixture = Fixture::default();
    fixture
        .plist
        .insert("Program", Ok(b"/usr/bin/env".to_vec()));
    check(
        "/fixture.plist",
        &mut fixture,
        b"runs env (interpreter), payload unverified",
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
