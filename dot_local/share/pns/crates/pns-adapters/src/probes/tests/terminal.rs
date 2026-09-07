use super::*;

// --- parse_tty_names, the step that becomes a path ----------------------

#[test]
fn a_padded_terminal_name_is_trimmed_because_the_padding_is_the_format() {
    // Unlike a pid line, where padding is output we did not expect, `ps
    // -o tty=` pads its column by design.
    assert_eq!(
        parse_tty_names("ttys000 \nttys001  \n"),
        ["ttys000", "ttys001"]
    );
}

#[test]
fn a_process_with_no_controlling_terminal_names_none() {
    assert!(parse_tty_names("??       \n").is_empty());
    assert!(parse_tty_names("").is_empty());
    assert_eq!(parse_tty_names("??      \nttys000 \n"), ["ttys000"]);
}

#[test]
fn a_name_that_could_escape_the_device_directory_is_refused_outright() {
    // The name is joined onto /dev, so this is the trust boundary: a
    // reading carrying a slash or a dot-dot must never become a path.
    for hostile in [
        "../../etc/passwd",
        "..",
        "tty/../../root",
        "tty s000",
        "tty;rm",
        "tty.0",
    ] {
        assert!(
            parse_tty_names(&format!("{hostile}\n")).is_empty(),
            "case: {hostile}"
        );
    }
}

#[test]
fn the_freshest_terminal_wins_across_every_session_found() {
    // Two phones attached, one put down an hour ago and one in a hand:
    // the reading is the one being used, so the stale session cannot
    // drag the verdict away from the live one.
    let dir = std::env::temp_dir().join(format!("pns-tty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("fixture dir");
    terminal_with_atime(&dir, "ttys000", PUT_DOWN_ATIME);
    terminal_with_atime(&dir, "ttys001", IN_HAND_ATIME);
    let newest = newest_terminal_atime(&dir.to_string_lossy(), "ttys000 \nttys001 \n");
    let stale = newest_terminal_atime(&dir.to_string_lossy(), "ttys000 \n");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        newest,
        Some(IN_HAND_ATIME),
        "the newer atime is the reading"
    );
    assert_eq!(stale, Some(PUT_DOWN_ATIME), "and alone the older one is");
}

#[test]
fn a_terminal_that_cannot_be_stat_ed_drops_out_without_taking_the_others_with_it() {
    let dir = std::env::temp_dir().join(format!("pns-tty-gone-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("fixture dir");
    terminal_with_atime(&dir, "ttys000", PUT_DOWN_ATIME);
    let mixed = newest_terminal_atime(&dir.to_string_lossy(), "ttysGONE \nttys000 \n");
    let none = newest_terminal_atime(&dir.to_string_lossy(), "ttysGONE \n");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(mixed, Some(PUT_DOWN_ATIME));
    assert_eq!(none, None, "nothing readable is no reading at all");
}
