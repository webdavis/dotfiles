use super::*;

fn words(args: &[&str]) -> Vec<String> {
    args.iter().map(|word| (*word).into()).collect()
}

#[test]
fn shell_arguments_refuse_missing_unknown_repeated_and_malformed_values() {
    for args in [
        vec![],
        vec!["wat"],
        vec!["begin"],
        vec!["begin", "--pid", "2", "--command"],
        vec!["begin", "--pid", "2", "--command", "x", "--exit-code", "0"],
        vec!["begin", "--pid", "2", "--command", "x", "--pid", "3"],
        vec!["begin", "--pid", "0", "--command", "x"],
        vec!["begin", "--pid", "1", "--command", "x"],
        vec!["begin", "--pid", "2147483648", "--command", "x"],
        vec!["begin", "--pid", "-1", "--command", "x"],
    ] {
        assert!(parse(&words(&args)).is_err(), "{args:?}");
    }
    // A BARE NUMBER IS REFUSED like every other duration pns reads.
    for value in [
        "",
        "-1",
        "+1",
        "1.5",
        "30",
        "0",
        "18446744073709551616",
        "30x",
        "721h",
    ] {
        let argv = words(&[
            "end",
            "--pid",
            "2",
            "--command",
            "x",
            "--exit-code",
            "0",
            "--elapsed",
            value,
        ]);
        assert!(parse(&argv).is_err(), "{value:?}");
    }
    for value in ["-1", "+1", "256"] {
        assert!(
            parse(&words(&[
                "end",
                "--pid",
                "2",
                "--command",
                "x",
                "--exit-code",
                value,
                "--elapsed",
                "30s"
            ]))
            .is_err()
        );
    }
}

#[test]
fn the_command_is_a_literal_value_even_when_it_starts_with_a_flag() {
    let args = words(&["begin", "--command", "--help", "--pid", "2"]);
    let Action::Begin { pid, command } = parse(&args).unwrap() else {
        panic!("wrong verb")
    };
    assert_eq!(pid, 2);
    assert_eq!(command, "--help");
    let args = words(&[
        "end",
        "--pid",
        "2",
        "--command",
        "",
        "--elapsed",
        "24h",
        "--exit-code",
        "255",
    ]);
    let Action::End {
        command,
        elapsed,
        exit_code,
        ..
    } = parse(&args).unwrap()
    else {
        panic!("wrong verb")
    };
    assert_eq!(command, "");
    assert_eq!(elapsed, 24 * 60 * 60);
    assert_eq!(exit_code, 255);
}

#[test]
fn the_retired_exit_flag_is_refused_and_names_its_replacement() {
    for args in [
        vec![
            "end",
            "--pid",
            "2",
            "--command",
            "x",
            "--exit",
            "0",
            "--elapsed",
            "30s",
        ],
        vec!["begin", "--pid", "2", "--command", "x", "--exit", "0"],
    ] {
        assert_eq!(
            parse(&words(&args)).err(),
            Some("--exit was replaced by --exit-code"),
            "{args:?}"
        );
    }
}
