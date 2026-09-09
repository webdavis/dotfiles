use super::*;

#[test]
fn a_desk_only_start_spawns_no_phone_thread_and_the_phone_still_reads_inline() {
    // sol review, ROW 4: no deterministic test exercised a one-sided
    // `Wants`. A production `start` that ignored `phone: false` and
    // spawned the phone chain anyway would pass every other test here.
    use pns_application::{PhoneInputProbe, ProbeStart, Wants};
    let mut scripted: Vec<(String, String)> = DISCOVERY
        .iter()
        .map(|(call, out)| ((*call).to_string(), (*out).to_string()))
        .collect();
    scripted.push((
        "/usr/sbin/ioreg -c IOHIDSystem".to_string(),
        "\"HIDIdleTime\" = 5000000000".to_string(),
    ));
    let probes = SystemProbes::new(
        ExactArgvRunner {
            answers: scripted,
            calls: Mutex::new(Vec::new()),
        },
        "/marker".to_string(),
    );
    probes.start(Wants {
        desk: true,
        phone: false,
    });
    probes.idle_secs(); // joins the desk thread so its calls have landed
    let phone_calls_before_the_read = probes
        .runner
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter(|call| call.starts_with("/usr/bin/pgrep") || call.starts_with("/bin/ps"))
        .count();
    assert_eq!(
        phone_calls_before_the_read, 0,
        "a desk-only start must not touch the phone chain"
    );
    probes.phone_input_atime_secs();
    let phone_calls_after_the_read = probes
        .runner
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter(|call| call.starts_with("/usr/bin/pgrep") || call.starts_with("/bin/ps"))
        .count();
    assert_eq!(
        phone_calls_after_the_read, 3,
        "the later phone read computes inline, exactly as an unstarted read always has"
    );
}

#[test]
fn a_phone_only_start_spawns_no_desk_thread_and_the_desk_still_reads_inline() {
    // The mirror of the test above: a production `start` that ignored
    // `desk: false` and spawned the desk pair anyway during a phone-only
    // read would pass every other test here.
    use pns_application::{IdleProbe, ProbeStart, Wants};
    let mut scripted: Vec<(String, String)> = DISCOVERY
        .iter()
        .map(|(call, out)| ((*call).to_string(), (*out).to_string()))
        .collect();
    scripted.push((
        "/usr/sbin/ioreg -c IOHIDSystem".to_string(),
        "\"HIDIdleTime\" = 5000000000".to_string(),
    ));
    let probes = SystemProbes::new(
        ExactArgvRunner {
            answers: scripted,
            calls: Mutex::new(Vec::new()),
        },
        "/marker".to_string(),
    );
    probes.start(Wants {
        desk: false,
        phone: true,
    });
    probes.phone_input_atime_secs(); // joins the phone thread so its calls have landed
    let desk_calls_before_the_read = probes
        .runner
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter(|call| call.starts_with("/usr/sbin/ioreg"))
        .count();
    assert_eq!(
        desk_calls_before_the_read, 0,
        "a phone-only start must not touch the desk pair"
    );
    probes.idle_secs();
    let desk_calls_after_the_read = probes
        .runner
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter(|call| call.starts_with("/usr/sbin/ioreg"))
        .count();
    assert_eq!(
        desk_calls_after_the_read, 1,
        "the later desk read computes inline, exactly as an unstarted read always has"
    );
}
