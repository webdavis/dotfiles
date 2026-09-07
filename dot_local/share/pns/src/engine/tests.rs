//! The decision, pinned: readings.

use self::fixtures::{CountingProbes, decide_with, watching};
use pns_domain::Overrides;

mod fixtures;

#[test]
fn writing_the_record_consults_no_probe_the_decision_had_not_already_read() {
    // THE RECORD MUST NOT BECOME A SECOND READING. The whole feature is
    // worthless, and actively misleading, if any value on the line is
    // re-read after `decide` returned: two readings of where the operator
    // is can disagree, and the explanation would then belong to a moment
    // the decision never saw. AN EXTRA READ IS A FAILURE EVEN WHERE THE
    // VALUE HAPPENS TO MATCH, which is why this compares the counts rather
    // than the line.
    let reads = |also_record: bool| {
        let probes = CountingProbes {
            idle: Some(30),
            marker_mtime: Some(999_400),
            phone_atime: Some(999_912),
            screen_locked: Some(false),
            view: Some(watching("wW:p1")),
            ..CountingProbes::default()
        };
        let decision = decide_with(&probes, &Overrides::default(), "wW:p1");
        if also_record {
            pns_adapters::decision_codec::line(&crate::decision_log::Record {
                event: &crate::args::EventArgs::default(),
                decision: &decision,
                overrides: &Overrides::default(),
                legs: &[],
                nag: false,
                permission_mode: "",
                agent_id: "",
                tool_name: "",
            });
        }
        [
            probes.idle_reads.get(),
            probes.marker_reads.get(),
            probes.phone_reads.get(),
            probes.lock_reads.get(),
            probes.view_reads.get(),
        ]
    };
    assert_eq!(reads(true), reads(false));
    // And every probe really was consulted, so the equality above is an
    // agreement between two live readings rather than between two zeroes.
    assert_eq!(reads(false), [1, 1, 1, 1, 1]);
}
