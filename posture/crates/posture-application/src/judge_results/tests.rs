//! Every expectation was read off the alerter this replaces
//! (`executable_results-alerter.sh`, `main`), whose whole design is that the
//! cursor never advances past a row nobody handled.

use super::*;
use std::cell::RefCell;

#[derive(Default)]
struct Log {
    reading: Option<LiveLog>,
    contents: String,
    spans: RefCell<Vec<(u64, u64)>>,
}

impl ResultsLog for Log {
    fn reading(&self) -> Option<LiveLog> {
        self.reading
    }
    fn span(&self, from: u64, length: u64) -> String {
        self.spans.borrow_mut().push((from, length));
        let start = (from as usize).min(self.contents.len());
        let end = (start + length as usize).min(self.contents.len());
        self.contents[start..end].to_owned()
    }
}

#[derive(Default)]
struct Cursor {
    stored: Option<StoredCursor>,
    written: RefCell<Vec<StoredCursor>>,
}

impl CursorStore for Cursor {
    fn read(&self) -> Option<StoredCursor> {
        self.stored
    }
    fn write(&self, cursor: StoredCursor) {
        self.written.borrow_mut().push(cursor);
    }
}

struct Lock(bool);
impl RunLock for Lock {
    fn taken(&self) -> bool {
        self.0
    }
}

#[derive(Default)]
struct Judge {
    page: Option<BatchPage>,
    saw: RefCell<Vec<String>>,
}

impl JudgeFindings for Judge {
    fn judge(&mut self, records: &str) -> JudgedBatch {
        self.saw.borrow_mut().push(records.to_owned());
        JudgedBatch {
            page: self.page.clone(),
        }
    }
}

#[derive(Default)]
struct Sink {
    refuse: bool,
    sent: Vec<Alert>,
}

impl AlertSink for Sink {
    fn submit(&mut self, alert: &Alert) -> Submission {
        self.sent.push(alert.clone());
        if self.refuse {
            Submission::NotAccepted(crate::SubmissionFailure::Failed)
        } else {
            Submission::Accepted
        }
    }
}

fn page() -> Option<BatchPage> {
    Some(BatchPage {
        title: String::from("🔴 **CRITICAL**"),
        body: String::from("a finding"),
    })
}

struct Case {
    lock: Lock,
    log: Log,
    cursor: Cursor,
    judge: Judge,
    sink: Sink,
}

impl Case {
    /// A log of two whole rows, a cursor at its start, and a lock this run holds.
    fn new(contents: &str, stored: Option<StoredCursor>) -> Self {
        Self {
            lock: Lock(true),
            log: Log {
                reading: Some(LiveLog {
                    inode: 7,
                    size: contents.len() as u64,
                }),
                contents: contents.to_owned(),
                spans: RefCell::new(Vec::new()),
            },
            cursor: Cursor {
                stored,
                written: RefCell::new(Vec::new()),
            },
            judge: Judge::default(),
            sink: Sink::default(),
        }
    }

    fn run(&mut self) -> JudgeOutcome {
        JudgeResults {
            lock: &self.lock,
            log: &self.log,
            cursor: &self.cursor,
            judge: &mut self.judge,
            sink: &mut self.sink,
            occurred_at: Some(100),
        }
        .run()
    }

    fn written(&self) -> Vec<StoredCursor> {
        self.cursor.written.borrow().clone()
    }

    fn judged(&self) -> Vec<String> {
        self.judge.saw.borrow().clone()
    }
}

const TWO_ROWS: &str = "{\"a\":1}\n{\"b\":2}\n";

#[test]
fn a_run_that_cannot_take_the_lock_reads_nothing_and_writes_nothing() {
    // A WatchPaths burst fires several invocations. Two runs would judge the
    // same rows, page twice and race each other's checkpoint.
    let mut case = Case::new(
        TWO_ROWS,
        Some(StoredCursor {
            inode: 7,
            offset: 0,
        }),
    );
    case.lock = Lock(false);
    assert_eq!(case.run(), JudgeOutcome::Contended);
    assert!(case.judged().is_empty());
    assert!(case.written().is_empty());
    assert!(case.sink.sent.is_empty());
}

#[test]
fn a_log_that_has_not_grown_is_not_read_and_the_cursor_is_not_rewritten() {
    let mut case = Case::new(
        TWO_ROWS,
        Some(StoredCursor {
            inode: 7,
            offset: 16,
        }),
    );
    assert_eq!(case.run(), JudgeOutcome::Quiet);
    assert!(case.judged().is_empty());
    assert!(case.written().is_empty());
}

#[test]
fn an_absent_log_is_quiet_rather_than_an_error() {
    let mut case = Case::new("", None);
    case.log.reading = None;
    assert_eq!(case.run(), JudgeOutcome::Quiet);
    assert!(case.written().is_empty());
}

#[test]
fn new_rows_are_read_from_the_cursor_and_bounded_by_the_size_already_taken() {
    // ONE READING, TAKEN ONCE. Reading past the size this run measured would
    // consume a row appended mid-run, which the next run would then never see.
    let mut case = Case::new(
        TWO_ROWS,
        Some(StoredCursor {
            inode: 7,
            offset: 8,
        }),
    );
    case.run();
    assert_eq!(*case.log.spans.borrow(), [(8, 8)]);
    assert_eq!(case.judged(), ["{\"b\":2}\n"]);
}

#[test]
fn a_delivered_batch_advances_the_cursor_exactly_to_the_last_complete_record() {
    let mut case = Case::new(
        TWO_ROWS,
        Some(StoredCursor {
            inode: 7,
            offset: 0,
        }),
    );
    case.judge.page = page();
    assert_eq!(case.run(), JudgeOutcome::Advanced { paged: true });
    assert_eq!(
        case.written(),
        [StoredCursor {
            inode: 7,
            offset: 16
        }]
    );
}

#[test]
fn a_torn_trailing_line_is_neither_judged_nor_checkpointed_past() {
    // osquery writes the row before its newline. Advancing over the torn line
    // would lose that finding outright.
    let mut case = Case::new(
        "{\"a\":1}\n{\"b\":",
        Some(StoredCursor {
            inode: 7,
            offset: 0,
        }),
    );
    assert_eq!(case.run(), JudgeOutcome::Advanced { paged: false });
    assert_eq!(case.judged(), ["{\"a\":1}\n"]);
    assert_eq!(
        case.written(),
        [StoredCursor {
            inode: 7,
            offset: 8
        }]
    );
}

#[test]
fn a_snapshot_that_is_only_a_torn_line_advances_nothing_at_all() {
    let mut case = Case::new(
        "{\"a\":",
        Some(StoredCursor {
            inode: 7,
            offset: 0,
        }),
    );
    assert_eq!(case.run(), JudgeOutcome::Quiet);
    assert!(case.judged().is_empty());
    assert!(case.written().is_empty());
}

#[test]
fn a_batch_with_no_page_still_advances_because_its_rows_were_already_handled() {
    // A digest row is delivered the moment the judge spools it, and a log-only
    // row is deliberately dropped. Holding the cursor for them would replay the
    // whole batch forever.
    let mut case = Case::new(
        TWO_ROWS,
        Some(StoredCursor {
            inode: 7,
            offset: 0,
        }),
    );
    assert_eq!(case.run(), JudgeOutcome::Advanced { paged: false });
    assert!(case.sink.sent.is_empty());
    assert_eq!(
        case.written(),
        [StoredCursor {
            inode: 7,
            offset: 16
        }]
    );
}

#[test]
fn a_page_that_could_be_neither_delivered_nor_stored_leaves_the_cursor_put() {
    // AT-LEAST-ONCE. Nothing was stored, so re-judging these rows cannot
    // double-deliver, and the alternative loses the finding silently.
    let mut case = Case::new(
        TWO_ROWS,
        Some(StoredCursor {
            inode: 7,
            offset: 0,
        }),
    );
    case.judge.page = page();
    case.sink.refuse = true;
    assert_eq!(case.run(), JudgeOutcome::Retained);
    assert!(case.written().is_empty());
}

#[test]
fn a_pages_occurrence_id_is_the_byte_range_it_covers_so_a_retry_matches() {
    let mut case = Case::new(
        TWO_ROWS,
        Some(StoredCursor {
            inode: 7,
            offset: 8,
        }),
    );
    case.judge.page = page();
    case.run();
    assert_eq!(case.sink.sent[0].occurrence_id.as_deref(), Some("7:8:16"));
    assert_eq!(case.sink.sent[0].event, "alert");
    assert_eq!(case.sink.sent[0].signal, AlertSignal::NeedsAttention);
    assert_eq!(case.sink.sent[0].occurred_at, Some(100));
}

#[test]
fn a_rotated_log_is_read_from_the_top_rather_than_the_old_offset() {
    let mut case = Case::new(
        TWO_ROWS,
        Some(StoredCursor {
            inode: 6,
            offset: 8,
        }),
    );
    case.run();
    assert_eq!(*case.log.spans.borrow(), [(0, 16)]);
    assert_eq!(
        case.written(),
        [StoredCursor {
            inode: 7,
            offset: 16
        }]
    );
}

#[test]
fn a_lost_cursor_replays_the_whole_log_and_pages_that_it_happened() {
    // DELETING THE CURSOR MUST NOT SUPPRESS A BATCH. The replay re-surfaces the
    // findings; this page is the separate signal that the alerter's own state
    // was disturbed.
    let mut case = Case::new(TWO_ROWS, None);
    case.judge.page = page();
    assert_eq!(case.run(), JudgeOutcome::Advanced { paged: true });
    assert_eq!(case.judged(), [TWO_ROWS]);
    assert_eq!(case.sink.sent.len(), 2);
    let reset = &case.sink.sent[0];
    assert_eq!(reset.event, "cursor-reset");
    assert_eq!(reset.occurrence_id.as_deref(), Some("cursor-reset:7:16"));
    assert!(reset.title.contains("cursor reset"));
    assert!(reset.detail.contains("replayed in full"));
}

#[test]
fn the_reset_page_is_raised_before_the_batch_and_never_gates_it() {
    // The warning is best effort. A batch held behind a failed warning would
    // trade the finding for the meta-signal about the finding.
    let mut case = Case::new(TWO_ROWS, None);
    case.judge.page = page();
    case.sink.refuse = true;
    assert_eq!(case.run(), JudgeOutcome::Retained);
    assert_eq!(case.sink.sent.len(), 2);
    assert_eq!(case.sink.sent[0].event, "cursor-reset");
    assert_eq!(case.sink.sent[1].event, "alert");
}

#[test]
fn a_lost_cursor_on_an_empty_log_is_quiet_rather_than_alarming() {
    // A reset with nothing to replay raises nothing: the alarm is about a
    // disturbed cursor over rows that exist.
    let mut case = Case::new("", None);
    case.log.reading = Some(LiveLog { inode: 7, size: 0 });
    assert_eq!(case.run(), JudgeOutcome::Quiet);
    assert!(case.sink.sent.is_empty());
    assert!(case.written().is_empty());
}
