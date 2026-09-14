use super::*;
use std::io::Write;

/// The alerter appends to the live spool while a run folds a claim back onto
/// it. A fold that reads the spool and rewrites it loses every line that
/// landed between its read and its write; a fold that appends cannot.
///
/// The folds are driven directly so that only the fold races the appender.
/// The claim rename has its own, older window (an append through a file
/// handle opened before the rename lands in the claim) and is not this test.
#[test]
fn a_line_appended_while_a_claim_is_folded_back_is_not_lost() {
    const APPENDS: usize = 1000;
    let fixture = Fixture::new();
    fixture.write_spool(&format!("{}\n", line("seed", "zero")));
    let appender = {
        let store = fixture.store.clone();
        std::thread::spawn(move || {
            for n in 0..APPENDS {
                let mut file = fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(&store)
                    .unwrap();
                let record = format!("{}\n", line("live", &format!("live-{n}")));
                file.write_all(record.as_bytes()).unwrap();
            }
        })
    };

    // Fold one fresh claim after another for as long as the appender runs,
    // so every append races a fold.
    let mut folded = 0u64;
    while !appender.is_finished() {
        let claim = fixture.spool(folded).claim_path();
        fs::write(
            &claim,
            format!("{}\n", line("claimed", &format!("claim-{folded}"))),
        )
        .unwrap();
        DigestSpoolFile::fold(&claim, &fixture.store);
        folded += 1;
    }
    appender.join().unwrap();

    let contents = fixture.spool_contents();
    let count = |marker: &str| {
        contents
            .lines()
            .filter(|entry| entry.contains(marker))
            .count()
    };
    assert!(folded > 0);
    assert_eq!(
        count("live-"),
        APPENDS,
        "{} of {APPENDS} appended lines were lost across {folded} folds",
        APPENDS - count("live-")
    );
    assert_eq!(count("claim-") as u64, folded);
    assert_eq!(count("zero"), 1);
    assert!(fixture.strays().is_empty());
}
