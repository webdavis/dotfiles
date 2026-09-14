use super::*;

// Captured from the owned Bash functions before this implementation was written.
#[test]
fn a_malformed_trailing_argument_keeps_completed_arguments() {
    assert_line(
        b"PasswordAuthentication yes \"unterminated",
        b"PasswordAuthentication",
        &[b"yes"],
    );
    assert_line(
        b"PasswordAuthentication \"unterminated",
        b"PasswordAuthentication",
        &[],
    );
}

#[test]
fn keyword_quotes_end_the_keyword_and_discard_only_one_empty_token() {
    for line in [
        b"=PasswordAuthentication yes".as_slice(),
        b"\"\"PasswordAuthentication yes",
    ] {
        assert_line(line, b"PasswordAuthentication", &[b"yes"]);
    }
    for line in [
        b"==PasswordAuthentication yes".as_slice(),
        b"= = bad",
        b"=#PasswordAuthentication yes",
        b"\"unterminated",
        b"  # comment",
        b"",
    ] {
        assert_eq!(parse_ssh_line(line), None, "{line:?}");
    }
    assert_line(
        b"\"PasswordAuthentication\"=yes",
        b"PasswordAuthentication",
        &[b"=yes"],
    );
    assert_line(b"Ma\"tch\" Address *", b"Match", &[b"Address", b"*"]);
    assert_line(
        b"Pass\"word\"Authentication yes",
        b"Password",
        &[b"Authentication", b"yes"],
    );
}

#[test]
fn arguments_concatenate_quotes_but_only_an_opening_hash_comments() {
    for line in [
        b"PasswordAuthentication y\"es\"".as_slice(),
        b"PasswordAuthentication \"y\"es",
        b"PasswordAuthentication 'yes'",
        b"PasswordAuthentication \"\"yes",
        b"PasswordAuthentication yes #note",
    ] {
        assert_line(line, b"PasswordAuthentication", &[b"yes"]);
    }
    assert_line(
        b"PasswordAuthentication yes#note",
        b"PasswordAuthentication",
        &[b"yes#note"],
    );
    assert_line(b"Include \"\" # empty", b"Include", &[b""]);
}

#[test]
fn trailing_whitespace_and_argument_separators_keep_their_distinct_sets() {
    for ending in [b' ', b'\t', b'\r', 12] {
        let mut line = b"PasswordAuthentication yes".to_vec();
        line.push(ending);
        assert_line(&line, b"PasswordAuthentication", &[b"yes"]);
    }
    assert_line(
        b"PasswordAuthentication yes\x0b",
        b"PasswordAuthentication",
        &[b"yes\x0b"],
    );
    assert_line(
        b"PasswordAuthentication yes\rx",
        b"PasswordAuthentication",
        &[b"yes\rx"],
    );
    assert_line(b"Match Address\r*", b"Match", &[b"Address\r*"]);
}

#[test]
fn argument_escapes_preserve_the_second_include_unescaping_stage() {
    for (line, args) in [
        (
            b"Include pay\\\tload.conf".as_slice(),
            vec![b"pay\\".as_slice(), b"load.conf"],
        ),
        (b"Include \"pay\tload.conf\"", vec![b"pay\tload.conf"]),
        (b"Include pay\\load.conf", vec![b"pay\\load.conf"]),
        (b"Include pay\\\\load.conf", vec![b"pay\\load.conf"]),
        (b"Include pay\\\\\\load.conf", vec![b"pay\\\\load.conf"]),
        (
            b"Include \"with\\ space/payload.conf\"",
            vec![b"with\\ space/payload.conf"],
        ),
        (b"Include z\\*load?.conf", vec![b"z\\*load?.conf"]),
    ] {
        assert_line(line, b"Include", &args);
    }
}

fn assert_line(line: &[u8], keyword: &[u8], arguments: &[&[u8]]) {
    assert_eq!(
        parse_ssh_line(line),
        Some(SshLine {
            keyword: keyword.to_vec(),
            arguments: arguments.iter().map(|arg| arg.to_vec()).collect(),
        }),
        "{line:?}"
    );
}
