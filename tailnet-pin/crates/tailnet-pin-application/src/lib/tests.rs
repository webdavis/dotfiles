use super::*;
use std::cell::RefCell;

/// A hosts file in memory, recording whatever is installed over it.
struct Fake {
    contents: Option<Vec<u8>>,
    installed: RefCell<Option<Vec<u8>>>,
    install_fails: bool,
}

impl Fake {
    fn holding(contents: &[u8]) -> Self {
        Fake {
            contents: Some(contents.to_vec()),
            installed: RefCell::new(None),
            install_fails: false,
        }
    }

    fn unreadable() -> Self {
        Fake {
            contents: None,
            installed: RefCell::new(None),
            install_fails: false,
        }
    }

    fn written(&self) -> String {
        String::from_utf8_lossy(
            self.installed
                .borrow()
                .as_deref()
                .expect("something was installed"),
        )
        .into_owned()
    }
}

impl HostsFile for Fake {
    fn read(&self) -> Option<Vec<u8>> {
        self.contents.clone()
    }

    fn install(&self, contents: &[u8]) -> Result<(), String> {
        if self.install_fails {
            return Err("the rename failed".into());
        }
        *self.installed.borrow_mut() = Some(contents.to_vec());
        Ok(())
    }
}

fn pin() -> Pin {
    Pin::new(b"pin.example.test", b"10.0.0.5", b"pin").expect("a usable pin")
}

const LOCALHOST: &str = "127.0.0.1\tlocalhost\n";
const RECORD: &str = "10.0.0.5\tpin.example.test\tpin\n";

/// The ordinary steady state: one correct terminated record, and nothing is
/// touched.
#[test]
fn a_file_that_already_says_it_is_left_alone() {
    let file = Fake::holding(format!("{LOCALHOST}{RECORD}").as_bytes());
    assert_eq!(reconcile(&file, &pin()), Outcome::Converged);
    assert!(
        file.installed.borrow().is_none(),
        "a converged file was written"
    );
}

/// The ordinary first run.
#[test]
fn a_file_with_no_pin_gains_one_at_the_end() {
    let file = Fake::holding(LOCALHOST.as_bytes());
    assert_eq!(reconcile(&file, &pin()), Outcome::Written);
    assert_eq!(file.written(), format!("{LOCALHOST}{RECORD}"));
}

/// The case the tool exists for: a stale pin resolves confidently to the wrong
/// host, so the old line goes and the right one takes its place.
#[test]
fn a_stale_pin_is_replaced_rather_than_joined() {
    let file = Fake::holding(format!("{LOCALHOST}10.9.9.9\tpin.example.test\tpin\n").as_bytes());
    assert_eq!(reconcile(&file, &pin()), Outcome::Written);
    assert_eq!(file.written(), format!("{LOCALHOST}{RECORD}"));
}

/// A correct line PLUS a duplicate is not converged, and the rebuild leaves
/// exactly one.
#[test]
fn a_duplicate_beside_the_correct_record_is_collapsed_to_one() {
    let file = Fake::holding(format!("{LOCALHOST}{RECORD}{RECORD}").as_bytes());
    assert_eq!(reconcile(&file, &pin()), Outcome::Written);
    assert_eq!(file.written(), format!("{LOCALHOST}{RECORD}"));
}

/// A line claiming only the SHORT name is the pin's too, by decision: the
/// fallback answers for both names, so nothing is left to compete.
#[test]
fn a_line_claiming_only_the_short_name_is_dropped() {
    let file = Fake::holding(format!("{LOCALHOST}192.168.1.9\tpin\n").as_bytes());
    assert_eq!(reconcile(&file, &pin()), Outcome::Written);
    assert_eq!(file.written(), format!("{LOCALHOST}{RECORD}"));
}

/// Everything the pin does not claim is copied through, comments and blanks
/// included, in the order it was written.
#[test]
fn everything_else_is_copied_through_in_order() {
    let source = format!("# a note\n\n{LOCALHOST}10.0.0.9\tnas.home\tnas\n");
    let file = Fake::holding(source.as_bytes());
    assert_eq!(reconcile(&file, &pin()), Outcome::Written);
    assert_eq!(file.written(), format!("{source}{RECORD}"));
}

/// A different host whose name merely contains the pinned one is not the pin's
/// to drop.
#[test]
fn a_host_that_merely_contains_the_pinned_name_survives() {
    let source = format!("{LOCALHOST}10.0.0.7\tpin.example.test.evil\n");
    let file = Fake::holding(source.as_bytes());
    assert_eq!(reconcile(&file, &pin()), Outcome::Written);
    assert_eq!(file.written(), format!("{source}{RECORD}"));
}

/// THE GATE. It runs on the lines the rebuild KEEPS, before the pin's own
/// record joins them, so a pin that maps the loopback address cannot vouch for
/// a file that has nothing else left.
#[test]
fn a_rebuild_that_would_lose_localhost_is_refused() {
    // The only loopback record claims the pin's short name, so the rebuild
    // drops it and nothing is left to resolve localhost through.
    let file = Fake::holding(b"127.0.0.1\tlocalhost\tpin\n");
    assert_eq!(
        reconcile(&file, &pin()),
        Outcome::Refused(Refusal::LostLoopback)
    );
    assert!(
        file.installed.borrow().is_none(),
        "a refused rebuild was installed"
    );
}

/// A comment-only loopback line is not a record, which is the gate the old
/// `grep` failed: it passed, and the machine lost its localhost.
#[test]
fn a_comment_only_loopback_line_does_not_satisfy_the_gate() {
    let file = Fake::holding(b"127.0.0.1  # decoy\n10.0.0.9\tnas\n");
    assert_eq!(
        reconcile(&file, &pin()),
        Outcome::Refused(Refusal::LostLoopback)
    );
}

/// An unreadable file is not an empty one. Treating it as one collapsed the
/// rebuild to the pin record alone, throwing the file away as root.
#[test]
fn an_unreadable_file_is_refused_rather_than_rebuilt_from_nothing() {
    let file = Fake::unreadable();
    assert_eq!(
        reconcile(&file, &pin()),
        Outcome::Refused(Refusal::Unreadable)
    );
    assert!(file.installed.borrow().is_none());
}

/// An empty file has no loopback record, so the gate refuses it rather than
/// installing a hosts file whose only line is the pin.
#[test]
fn an_empty_file_is_refused_by_the_gate() {
    let file = Fake::holding(b"");
    assert_eq!(
        reconcile(&file, &pin()),
        Outcome::Refused(Refusal::LostLoopback)
    );
}

/// The correct record on an UNTERMINATED final line is not converged: the
/// resolver reads it one byte short of what the file holds.
#[test]
fn an_unterminated_correct_record_is_rewritten_terminated() {
    let file = Fake::holding(format!("{LOCALHOST}10.0.0.5\tpin.example.test\tpin").as_bytes());
    assert_eq!(reconcile(&file, &pin()), Outcome::Written);
    assert_eq!(file.written(), format!("{LOCALHOST}{RECORD}"));
}

/// A kept final line gains the terminator, without which the appended record
/// would join onto it.
#[test]
fn a_kept_final_line_gains_the_terminator_it_lacked() {
    let file = Fake::holding(b"127.0.0.1\tlocalhost");
    assert_eq!(reconcile(&file, &pin()), Outcome::Written);
    assert_eq!(file.written(), format!("{LOCALHOST}{RECORD}"));
}

/// And it loses the carriage return the resolver never read there, which is the
/// departure that cost this machine its localhost when it was not made.
#[test]
fn a_kept_final_line_loses_the_carriage_return_the_resolver_never_read() {
    let file = Fake::holding(b"127.0.0.1\tlocalhost\r");
    assert_eq!(reconcile(&file, &pin()), Outcome::Written);
    assert_eq!(file.written(), format!("{LOCALHOST}{RECORD}"));
}

/// Bytes are copied through as bytes. The shell this replaced could not hold a
/// NUL in a variable and silently joined the two halves of such a line.
#[test]
fn a_line_carrying_a_nul_byte_is_copied_through_whole() {
    let source = b"127.0.0.1\tlocalhost\n10.0.0.5\tnas.home\0junk\n";
    let file = Fake::holding(source);
    assert_eq!(reconcile(&file, &pin()), Outcome::Written);
    assert_eq!(
        file.installed.borrow().as_deref().expect("installed"),
        [source.as_slice(), RECORD.as_bytes()].concat()
    );
}

/// An install that fails carries its reason, because the caller reports it and
/// "it failed" without a noun is a hunt.
#[test]
fn an_install_that_fails_is_reported_with_its_reason() {
    let mut file = Fake::holding(LOCALHOST.as_bytes());
    file.install_fails = true;
    assert_eq!(
        reconcile(&file, &pin()),
        Outcome::Refused(Refusal::NotInstalled("the rename failed".into()))
    );
}
