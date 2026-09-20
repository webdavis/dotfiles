use super::*;

#[test]
fn a_desk_only_start_spawns_no_phone_thread_and_the_phone_still_reads_inline() {
    // sol review, ROW 4: no deterministic test exercised a one-sided
    // `Wants`. A production `start` that ignored `phone: false` and
    // walked the phone chain anyway would pass every other test here.
    use pns_application::{PhoneInputProbe, ProbeStart, Wants};
    let table = FakeTable::naming(&[DISCOVERY_TERMINAL]);
    let asked = Arc::clone(&table.asked);
    let probes = phone_probe(&DISCOVERY, Arc::new(table))
        .with_registry(Arc::new(FakeRegistry::answering(5_000_000_000, true)));
    probes.start(Wants {
        desk: true,
        phone: false,
    });
    probes.idle_secs(); // joins the desk thread so its work has landed
    assert!(
        probes.runner.calls.lock().unwrap().is_empty() && asked.lock().unwrap().is_empty(),
        "a desk-only start must not touch the phone chain"
    );
    probes.phone_input_atime_secs();
    assert_eq!(
        probes.runner.calls.lock().unwrap().len(),
        1,
        "the later phone read computes inline, exactly as an unstarted read always has"
    );
    assert_eq!(asked.lock().unwrap().len(), 1);
}

#[test]
fn a_phone_only_start_spawns_no_desk_thread_and_the_desk_still_reads_inline() {
    // The mirror of the test above: a production `start` that ignored
    // `desk: false` and read the desk pair anyway during a phone-only read
    // would pass every other test here.
    use pns_application::{IdleProbe, ProbeStart, Wants};
    let registry = FakeRegistry::answering(5_000_000_000, true);
    let idle_reads = Arc::clone(&registry.idle_reads);
    let probes = phone_probe(
        &DISCOVERY,
        Arc::new(FakeTable::naming(&[DISCOVERY_TERMINAL])),
    )
    .with_registry(Arc::new(registry));
    probes.start(Wants {
        desk: false,
        phone: true,
    });
    probes.phone_input_atime_secs(); // joins the phone thread
    assert_eq!(
        idle_reads.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "a phone-only start must not touch the desk pair"
    );
    probes.idle_secs();
    assert_eq!(
        idle_reads.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the later desk read computes inline, exactly as an unstarted read always has"
    );
}
