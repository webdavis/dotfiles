use super::*;

fn reading(contents: &[u8]) -> Vec<&[u8]> {
    split_lines(contents)
        .iter()
        .map(Line::as_the_resolver_reads_it)
        .collect()
}

/// An empty file has no lines, which is what makes it converge-shaped rather
/// than terminator-less: there is no final line to be missing one.
#[test]
fn an_empty_file_has_no_lines() {
    assert!(split_lines(b"").is_empty());
}

/// A blank line is a line. The rebuild copies it through, so it has to exist.
#[test]
fn a_blank_line_is_a_line() {
    assert_eq!(reading(b"\n\n"), [b"".as_slice(), b""]);
    assert_eq!(reading(b"a\n\nb\n"), [b"a".as_slice(), b"", b"b"]);
}

/// The terminator belongs to the file, not to the line: the last line of a
/// terminated file is terminated, and of an unterminated one is not.
#[test]
fn only_the_final_line_can_lack_a_terminator() {
    let terminated = split_lines(b"a\nb\n");
    assert!(terminated.iter().all(|line| line.terminated));

    let unterminated = split_lines(b"a\nb");
    assert!(unterminated[0].terminated);
    assert!(!unterminated[1].terminated);
}

/// The carriage return is dropped ONLY on the final unterminated line, because
/// that is the only line where the resolver never read it.
#[test]
fn a_carriage_return_survives_everywhere_but_the_final_unterminated_line() {
    assert_eq!(reading(b"a\r\nb\r"), [b"a\r".as_slice(), b"b"]);
    assert_eq!(reading(b"a\r\nb\r\n"), [b"a\r".as_slice(), b"b\r"]);
}
