use super::*;

#[test]
fn local_only_plans_the_local_surfaces_alone_whatever_the_phone_verdict_was() {
    // Both phone verdicts, because the flag is what decides this plan. Ask
    // only with the phone wanted and a narrowing that quietly reads the
    // phone verdict as well still answers correctly here.
    assert_eq!(
        channel_plan(
            &three_enabled(),
            pns_domain::DeliveryScope::LocalOnly,
            reaching(true, true)
        ),
        vec![decorative("macos-banner", ReportMode::Silent)]
    );
    assert_eq!(
        channel_plan(
            &three_enabled(),
            pns_domain::DeliveryScope::LocalOnly,
            reaching(true, false)
        ),
        vec![decorative("macos-banner", ReportMode::Silent)]
    );
}

#[test]
fn a_selected_sensor_is_never_a_leg_under_local_only_either() {
    // The flag most likely to admit one: a sensor reads THIS machine, so
    // a "local surface" reading of local-only would hand it the event.
    // Nothing about a sensor is local to the plan, because a sensor holds
    // no routing to be local WITH. The banner is the positive control.
    assert_eq!(
        channel_plan(
            &sensor_and_three_enabled(),
            pns_domain::DeliveryScope::LocalOnly,
            reaching(true, true)
        ),
        vec![decorative("macos-banner", ReportMode::Silent)]
    );
    assert_eq!(
        channel_plan(
            &sensor_and_three_enabled(),
            pns_domain::DeliveryScope::LocalOnly,
            reaching(true, false)
        ),
        vec![decorative("macos-banner", ReportMode::Silent)]
    );
}

#[test]
fn remote_only_plans_the_durable_legs_alone_and_sync_which_keeps_a_lost_entry_visible() {
    // The suppressed-phone form is the one that pins SYNC to the flag
    // alone. Without it a narrowing that also consulted the phone verdict
    // would drop this plan back to the ordinary async pair, and a log
    // entry nobody waited for is the invisible loss sync exists to stop.
    assert_eq!(
        channel_plan(
            &three_enabled(),
            pns_domain::DeliveryScope::RemoteOnly,
            reaching(true, true)
        ),
        vec![logged("hermes", ReportMode::ReportOutcome)]
    );
    assert_eq!(
        channel_plan(
            &three_enabled(),
            pns_domain::DeliveryScope::RemoteOnly,
            reaching(true, false)
        ),
        vec![logged("hermes", ReportMode::ReportOutcome)]
    );
}

#[test]
fn a_selected_sensor_is_never_a_leg_under_remote_only_either() {
    // And not on the LOG path, where a leg is planned sync and a failure
    // is printed: a sensor arriving here would be an exec attempt the
    // operator gets told about by name. hermes is the positive control.
    assert_eq!(
        channel_plan(
            &sensor_and_three_enabled(),
            pns_domain::DeliveryScope::RemoteOnly,
            reaching(true, true)
        ),
        vec![logged("hermes", ReportMode::ReportOutcome)]
    );
    assert_eq!(
        channel_plan(
            &sensor_and_three_enabled(),
            pns_domain::DeliveryScope::RemoteOnly,
            reaching(true, false)
        ),
        vec![logged("hermes", ReportMode::ReportOutcome)]
    );
}

#[test]
fn no_plan_over_the_real_roster_hands_the_phone_or_the_banner_a_reporting_leg() {
    // THE STRUCTURAL SAFETY ARGUMENT, and the only thing pinning it. moshi
    // and macos-banner have sentences of their own now, and a REPORTING
    // leg is the sole path that would put one on an event's stdout:
    // `ReportOutcome` is produced under `--remote-only` alone, which keeps
    // durable plugins only, and neither of those two is durable.
    //
    // IT IS BROKEN BY A LINE THIS SLICE NEVER TOUCHES. One `durable: true`
    // in the roster would start printing a moshi sentence on the log path,
    // so the input here is the REAL roster with everything selected rather
    // than a fixture that could stay agreeable while the roster moved.
    let registry = pns_domain::registry::roster();
    let every_plugin = registry.all();
    for scope in [
        pns_domain::DeliveryScope::Automatic,
        pns_domain::DeliveryScope::LocalOnly,
        pns_domain::DeliveryScope::RemoteOnly,
    ] {
        for banner in [false, true] {
            for card in [false, true] {
                let plan = channel_plan(&every_plugin, scope, reaching(banner, card));
                for planned in plan {
                    assert!(
                        !(matches!(planned.name, "mobile" | "macos-banner")
                            && planned.mode == ReportMode::ReportOutcome),
                        "the plan handed {} a reporting leg with scope={scope:?}, banner={banner}, card={card}: its \
                             sentence would reach an event's stdout",
                        planned.name
                    );
                }
            }
        }
    }
}
