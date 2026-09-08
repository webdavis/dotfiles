use super::*;

#[test]
fn a_watchdog_requires_clock_process_and_both_sides_of_canary_freshness() {
    let epoch = CanaryEpoch::parse("2000");
    assert_eq!(
        osquery_problem(None, false, None, 20).as_deref(),
        Some(
            "the watchdog cannot read the system clock, so it cannot verify osqueryd is producing scheduled results"
        )
    );
    assert_eq!(
        osquery_problem(Some(2000), false, epoch, 20).as_deref(),
        Some("osqueryd is not running")
    );
    assert!(
        osquery_problem(Some(2000), true, None, 20)
            .unwrap()
            .contains("MISSING")
    );
    for now in [1980, 1981, 2000, 2019, 2020] {
        assert_eq!(osquery_problem(Some(now), true, epoch, 20), None);
    }
    assert!(
        osquery_problem(Some(1979), true, epoch, 20)
            .unwrap()
            .contains("21s in the future")
    );
    assert!(
        osquery_problem(Some(2021), true, epoch, 20)
            .unwrap()
            .contains("STALE, 21s old")
    );
}

#[test]
fn only_exact_healthy_route_statuses_suppress_the_watchdog_problem() {
    for code in ["200", "201", "299", "405"] {
        assert_eq!(route_problem(Some(code), "configured-route"), None);
    }
    for code in ["199", "300", "404", "406", "0200", "2000", "0", "500"] {
        assert_eq!(
            route_problem(Some(code), "configured-route"),
            Some(format!(
                "hermes #priority route unhealthy (HTTP {code}) at configured-route"
            ))
        );
    }
    for code in [None, Some("200 secret"), Some("")] {
        assert_eq!(
            route_problem(code, "configured-route").as_deref(),
            Some("hermes #priority route unhealthy (HTTP 000) at configured-route")
        );
    }
}

#[test]
fn state_refusal_is_a_problem_and_one_page_keeps_problem_order_and_sound() {
    assert_eq!(state_problem(true, "configured-state"), None);
    let state = state_problem(false, "configured-state").unwrap();
    assert_eq!(
        state,
        "the watchdog cannot persist its state (configured-state); the crash-loop and backlog-growth alarms are degraded until this is fixed"
    );
    assert_eq!(watchdog_page(&[]), None);
    let single = watchdog_page(std::slice::from_ref(&state)).unwrap();
    assert_eq!(single.title, "🔴 **CRITICAL**");
    assert_eq!(single.sound, "Sosumi");
    let page = watchdog_page(&[state.clone(), "osqueryd is not running".into()]).unwrap();
    assert_eq!(page.title, "🔴 **CRITICAL** (2 issues)");
    assert_eq!(
        page.body,
        format!(
            "**Monitoring is DOWN**\n- {state}\n- osqueryd is not running\n- **Diagnose:** `launchctl list | grep -i osquery`\n- Restart the down component, then re-check."
        )
    );
}
