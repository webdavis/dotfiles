//! Every expectation was read off the alerter this replaces
//! (`executable_results-alerter.sh`, the snapshot block).

use super::*;
use crate::test_sandbox::Sandbox;

/// A log in a directory that removes itself. Hold the sandbox for as long as
/// the path is used; dropping it early takes the log with it.
fn fixture(contents: &[u8]) -> (Sandbox, PathBuf) {
    let sandbox = Sandbox::new("results");
    let path = sandbox.join("results.log");
    std::fs::write(&path, contents).unwrap();
    (sandbox, path)
}

#[test]
fn an_absent_log_offers_no_reading_rather_than_failing() {
    // osquery has not written one yet on a fresh machine, and a run that
    // refused would page about its own startup.
    let log = ResultsFile::new(std::env::temp_dir().join("posture-results-absent-forever"));
    assert!(log.reading().is_none());
}

#[test]
fn a_reading_carries_the_inode_and_the_size_together() {
    // THE INODE IS HALF OF IT: the log rotates, and an offset alone would keep
    // reading a file that no longer exists at that path.
    let (_sandbox, path) = fixture(b"{\"a\":1}\n");
    let log = ResultsFile::new(path.clone());
    let reading = log.reading().expect("a reading");
    assert_eq!(reading.size, 8);
    assert_ne!(reading.inode, 0);
    // The same file read twice is the same inode.
    assert_eq!(log.reading().unwrap().inode, reading.inode);
}

#[test]
fn a_span_starts_where_it_is_told_and_stops_where_it_is_told() {
    let (_sandbox, path) = fixture(b"{\"a\":1}\n{\"b\":2}\n");
    let log = ResultsFile::new(path);
    assert_eq!(log.span(8, 8), "{\"b\":2}\n");
    assert_eq!(log.span(0, 8), "{\"a\":1}\n");
}

#[test]
fn a_span_never_reads_past_the_length_it_was_given() {
    // ONE READING, TAKEN ONCE. A row appended while this run is judging must
    // land in the NEXT batch, not be consumed early and checkpointed past.
    let (_sandbox, path) = fixture(b"{\"a\":1}\n");
    let log = ResultsFile::new(path.clone());
    let reading = log.reading().expect("a reading");
    std::fs::write(&path, b"{\"a\":1}\n{\"appended\":true}\n").unwrap();
    assert_eq!(log.span(0, reading.size), "{\"a\":1}\n");
}

#[test]
fn a_span_past_the_end_is_empty_rather_than_an_error() {
    let (_sandbox, path) = fixture(b"{\"a\":1}\n");
    let log = ResultsFile::new(path);
    assert_eq!(log.span(64, 8), "");
}

#[test]
fn a_row_carrying_bytes_that_are_not_utf8_still_reaches_the_judge() {
    // LOSSY ON PURPOSE. The judge drops an unparseable row; refusing the whole
    // span would drop every other finding in the batch with it.
    let (_sandbox, path) = fixture(b"{\"a\":\"\xff\"}\n{\"b\":2}\n");
    let log = ResultsFile::new(path);
    let span = log.span(0, 21);
    assert!(span.contains("{\"b\":2}"), "{span:?}");
    assert!(span.ends_with('\n'), "{span:?}");
}

#[test]
fn a_log_that_cannot_be_opened_yields_an_empty_span_rather_than_a_panic() {
    let log = ResultsFile::new(std::env::temp_dir().join("posture-results-absent-forever"));
    assert_eq!(log.span(0, 10), "");
}
