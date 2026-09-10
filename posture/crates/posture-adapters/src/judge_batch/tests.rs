//! Read off `results-alerter/route.sh` and `render-page.sh`, the two stages
//! this replaces.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

const HOME: &str = "/Users/someone";

fn spool_path() -> std::path::PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "posture-judge-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    root.join("digest.ndjson")
}

struct World {
    spool: DigestAppendFile,
    path: std::path::PathBuf,
    vouches_everything: bool,
}

impl World {
    fn new() -> Self {
        let path = spool_path();
        Self {
            spool: DigestAppendFile::new(path.clone()),
            path,
            vouches_everything: true,
        }
    }

    fn judge(&self, records: &str, allowlist: Option<&AllowlistText>) -> JudgedBatch {
        let vouches = self.vouches_everything;
        let mut vouch = move |_: &str| vouches;
        let mut inspect = |_: &str| None;
        let mut triage = |_: &ResultsRow| None;
        BatchJudge {
            home: HOME,
            allowlist_path: "/tmp/allowlist",
            allowlist,
            spool: &self.spool,
            now: "2026-09-09T12:00:00Z",
            collaborators: Collaborators {
                vouches: &mut vouch,
                inspect: &mut inspect,
                triage: &mut triage,
            },
        }
        .judge(records)
    }

    fn spooled(&self) -> Vec<posture_protocol::DigestRecord> {
        std::fs::read_to_string(&self.path)
            .map(|text| posture_protocol::decode_spool(&text))
            .unwrap_or_default()
    }
}

fn row(name: &str, columns: serde_json::Value) -> String {
    serde_json::json!({"name": name, "action": "added", "counter": 4, "columns": columns})
        .to_string()
}

#[test]
fn a_critical_finding_becomes_a_page_and_nothing_is_spooled_for_it() {
    // A PAGE IS NOT ALSO A DIGEST LINE. The operator has already been woken;
    // repeating it in tomorrow's summary is the same news twice.
    let world = World::new();
    let batch = world.judge(
        &row("new_admin_user", serde_json::json!({"username": "mallory"})),
        None,
    );
    let page = batch.page.expect("a critical finding pages");
    assert!(page.title.contains("CRITICAL"), "{}", page.title);
    assert!(page.body.contains("mallory"), "{}", page.body);
    assert!(world.spooled().is_empty(), "a paged finding is not spooled");
}

#[test]
fn a_batch_with_nothing_worth_waking_anyone_for_raises_no_page_at_all() {
    // NO FINDINGS IS NO PAGE, not a page saying zero.
    let world = World::new();
    assert!(world.judge("", None).page.is_none(), "an empty batch");
    assert!(
        world.judge("not json\n", None).page.is_none(),
        "a batch of rubbish"
    );
}

#[test]
fn a_finding_that_did_not_earn_a_page_is_spooled_for_the_daily_digest() {
    let world = World::new();
    // `agent_authfile_changed` is the gate's one unconditional digest arm, so
    // it pins the spool path without also depending on a severity rule.
    let batch = world.judge(
        &row(
            "agent_authfile_changed",
            serde_json::json!({"path": "/Users/someone/.hermes/.env"}),
        ),
        None,
    );
    assert!(batch.page.is_none(), "an authfile change does not page");
    let spooled = world.spooled();
    assert_eq!(spooled.len(), 1);
    assert_eq!(
        spooled[0].detector.as_deref(),
        Some("agent_authfile_changed")
    );
    assert_eq!(
        spooled[0].identity.as_deref(),
        Some("/Users/someone/.hermes/.env")
    );
    // THE ROW'S OWN ACTION, not a column of the same name. Read from the
    // columns it spooled an empty verb for every finding, which a real run
    // against the binary is what caught.
    assert_eq!(spooled[0].action.as_deref(), Some("added"));
    assert_eq!(
        spooled[0].summary.as_deref(),
        Some("agent_authfile_changed /Users/someone/.hermes/.env")
    );
    assert_eq!(
        spooled[0].timestamp.as_deref(),
        Some("2026-09-09T12:00:00Z")
    );
}

#[test]
fn one_page_carries_every_critical_finding_in_the_batch_and_counts_them_all() {
    // ONE PAGE PER BATCH, not one per finding: a full cursor replay is a page,
    // not a per-finding storm.
    let world = World::new();
    let batch = format!(
        "{}\n{}\n",
        row("new_admin_user", serde_json::json!({"username": "mallory"})),
        row("suid_bin_unexpected", serde_json::json!({"path": "/tmp/s"})),
    );
    let page = world.judge(&batch, None).page.expect("two criticals page");
    assert!(page.title.contains("· 2"), "{}", page.title);
}

#[test]
fn an_allowlisted_agent_is_suppressed_and_an_unlisted_one_pages() {
    // The operator seeds known-good agents so their persistence stops paging.
    let world = World::new();
    let listed = AllowlistText::parse(
        &serde_json::json!({
            "label": "com.example.known", "path": "/tmp/known.plist", "program": "/usr/bin/true"
        })
        .to_string(),
        HOME,
    );
    let known = row(
        "persistence_launchd",
        serde_json::json!({
            "label": "com.example.known", "path": "/tmp/known.plist", "program": "/usr/bin/true"
        }),
    );
    let stranger = row(
        "persistence_launchd",
        serde_json::json!({
            "label": "com.example.stranger", "path": "/tmp/x.plist", "program": "/usr/bin/true"
        }),
    );
    assert!(
        world.judge(&known, Some(&listed)).page.is_none(),
        "the allowlisted agent is quiet"
    );
    assert!(
        world.judge(&stranger, Some(&listed)).page.is_some(),
        "the unlisted agent pages"
    );
}

#[test]
fn a_reused_label_over_a_different_plist_is_not_suppressed() {
    // THE TUPLE IS ALL THREE. Suppressing on the label alone would let anything
    // silence itself by naming an agent the operator already trusts.
    let world = World::new();
    let listed = AllowlistText::parse(
        &serde_json::json!({
            "label": "com.example.known", "path": "/tmp/known.plist", "program": "/usr/bin/true"
        })
        .to_string(),
        HOME,
    );
    let impostor = row(
        "persistence_launchd",
        serde_json::json!({
            "label": "com.example.known", "path": "/tmp/ELSEWHERE.plist", "program": "/bin/sh"
        }),
    );
    assert!(
        world.judge(&impostor, Some(&listed)).page.is_some(),
        "the same label over a different plist still pages"
    );
}

#[test]
fn an_allowlist_that_could_not_be_read_pages_rather_than_suppressing() {
    // UNREADABLE IS NOT EMPTY. A list nobody could read has vouched for
    // nothing, and treating it as permission is how a page goes missing.
    let world = World::new();
    let agent = row(
        "persistence_launchd",
        serde_json::json!({
            "label": "com.example.known", "path": "/tmp/known.plist", "program": "/usr/bin/true"
        }),
    );
    assert!(world.judge(&agent, None).page.is_some());
}

#[test]
fn a_listening_port_is_identified_by_all_three_facts_that_describe_it() {
    // The program alone does not say what it exposed, and a digest line naming
    // only `node` is a line nobody can act on.
    let world = World::new();
    world.judge(
        &row(
            "listening_ports_non_loopback",
            serde_json::json!({"name": "node", "address": "0.0.0.0", "port": "8080"}),
        ),
        None,
    );
    assert_eq!(
        world.spooled()[0].identity.as_deref(),
        Some("node 0.0.0.0:8080")
    );
}

#[test]
fn a_finding_with_nothing_to_name_it_carries_the_placeholder_a_reader_knows() {
    // An empty identity reads as a bug in the digest; `?` reads as a finding
    // that genuinely arrived without one.
    let world = World::new();
    world.judge(&row("agent_authfile_changed", serde_json::json!({})), None);
    assert_eq!(world.spooled()[0].identity.as_deref(), Some("?"));
}
