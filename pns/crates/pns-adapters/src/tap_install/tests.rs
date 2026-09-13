use super::*;

#[test]
fn authorization_command_keeps_shell_characters_inside_one_executable_argument() {
    let binary = r#"/opt/a 'quoted' \"bin\ $(printf injected);`printf injected`/pns"#;
    let guide = install_guide(binary, "host", "user").unwrap();
    let option = guide
        .authorized_key_line
        .strip_prefix("command=\"")
        .unwrap()
        .strip_suffix("\",restrict ssh-ed25519 <your-Shortcut-public-key>")
        .unwrap();
    // OpenSSH's opt_dequote unescapes only a backslash immediately before a quote:
    // https://github.com/openssh/openssh-portable/blob/master/misc.c
    let command = option.replace("\\\"", "\"");
    let out = std::process::Command::new("/bin/sh")
        .args(["-c", &format!("printf '%s\\n' {command}")])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!("{binary}\ntap\n")
    );
}

#[test]
fn invalid_install_context_cannot_print_an_authorization_line() {
    for (binary, host, user) in [
        ("relative", "host", "user"),
        ("/pns", "", "user"),
        ("/pns", "host", "user\nextra"),
        ("/pns\n", "host", "user"),
    ] {
        assert_eq!(
            install_guide(binary, host, user).unwrap_err().code,
            "install_context_unavailable"
        );
    }
}
