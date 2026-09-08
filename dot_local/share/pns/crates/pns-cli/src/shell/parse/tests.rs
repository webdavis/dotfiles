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
        vec!["begin", "--pid", "2", "--command", "x", "--exit", "0"],
        vec!["begin", "--pid", "2", "--command", "x", "--pid", "3"],
        vec!["begin", "--pid", "0", "--command", "x"],
        vec!["begin", "--pid", "1", "--command", "x"],
        vec!["begin", "--pid", "2147483648", "--command", "x"],
        vec!["begin", "--pid", "-1", "--command", "x"],
    ] {
        assert!(parse(&words(&args)).is_err(), "{args:?}");
    }
    for value in ["", "-1", "+1", "1.5", "18446744073709551616"] {
        let argv = words(&[
            "end",
            "--pid",
            "2",
            "--command",
            "x",
            "--exit",
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
                "--exit",
                value,
                "--elapsed",
                "30"
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
        "18446744073709551615",
        "--exit",
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
    assert_eq!(elapsed, u64::MAX);
    assert_eq!(exit_code, 255);
}
