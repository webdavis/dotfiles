use super::*;

#[test]
fn absent_null_and_false_allow_funnel_are_inactive_but_wrong_shapes_gap() {
    // The adapter projects absent, null and false through the captured // empty rule.
    for _source in ["absent", "null", "false"] {
        assert_eq!(
            classify_funnel(&[&[AllowFunnel::Omitted]]),
            FunnelReading::Inactive
        );
    }
    assert_eq!(
        classify_funnel(&[&[AllowFunnel::Map(&[])]]),
        FunnelReading::Inactive
    );
    assert_eq!(
        classify_funnel(&[&[AllowFunnel::Invalid]]),
        FunnelReading::Gap
    );
    assert_eq!(classify_funnel(&[]), FunnelReading::Gap);
    assert_eq!(classify_funnel(&[&[], &[]]), FunnelReading::Gap);
}
#[test]
fn a_true_entry_at_any_projected_depth_is_active_and_invalid_values_win() {
    let inner = [("b:443", Some(true)), ("a:443", Some(false))];
    let other = [("c:443", Some(true)), ("b:443", Some(true))];
    assert_eq!(
        classify_funnel(&[&[
            AllowFunnel::Omitted,
            AllowFunnel::Map(&inner),
            AllowFunnel::Map(&other)
        ]]),
        FunnelReading::Active(vec!["b:443".into(), "c:443".into()])
    );
    assert_eq!(
        classify_funnel(&[&[AllowFunnel::Map(&inner), AllowFunnel::Invalid]]),
        FunnelReading::Gap
    );
    assert_eq!(
        classify_funnel(&[&[AllowFunnel::Map(&[("x", None)]), AllowFunnel::Map(&inner)]]),
        FunnelReading::Gap
    );
    assert_eq!(
        classify_funnel(&[&[AllowFunnel::Map(&[("x", Some(false))])]]),
        FunnelReading::Inactive
    );
}
#[test]
fn baseline_trust_distinguishes_absence_corruption_and_failed_publication() {
    use FunnelBaseline::*;
    assert_eq!(
        funnel_baseline(true, Some("active"), false),
        Trusted(FunnelState::Active)
    );
    assert_eq!(
        funnel_baseline(true, Some("inactive"), false),
        Trusted(FunnelState::Inactive)
    );
    assert_eq!(funnel_baseline(false, None, false), Absent);
    for value in [None, Some(""), Some("active\n"), Some("ACTIVE")] {
        assert_eq!(funnel_baseline(true, value, false), Corrupt);
    }
    assert_eq!(funnel_baseline(true, Some("active"), true), Absent);
    assert_eq!(funnel_baseline(true, None, true), Corrupt);
}
#[test]
fn funnel_pages_on_open_or_untrusted_active_but_not_steady_active_or_close() {
    let reading = FunnelReading::Active(vec!["example.test:443".into()]);
    for prior in [
        FunnelBaseline::Absent,
        FunnelBaseline::Corrupt,
        FunnelBaseline::Trusted(FunnelState::Inactive),
    ] {
        let plan = plan_funnel(&reading, prior, false);
        assert_eq!(
            plan.alert,
            Some(FunnelAlert::Exposure(include_str!("exposure.txt").into()))
        );
        assert_eq!(plan.next, Some(FunnelState::Active));
        assert!(plan.clear_read_gap);
    }
    let steady = plan_funnel(&reading, FunnelBaseline::Trusted(FunnelState::Active), true);
    assert_eq!(steady.alert, None);
    assert!(steady.clear_read_gap);
    let close = plan_funnel(
        &FunnelReading::Inactive,
        FunnelBaseline::Trusted(FunnelState::Active),
        false,
    );
    assert_eq!(close.alert, None);
    assert_eq!(close.next, Some(FunnelState::Inactive));
}
#[test]
fn failed_reads_keep_the_active_baseline_and_corrupt_idle_state_gets_one_gap() {
    let active = FunnelBaseline::Trusted(FunnelState::Active);
    assert_eq!(
        plan_funnel(&FunnelReading::Gap, active, false),
        FunnelPlan {
            alert: Some(FunnelAlert::ReadGap),
            next: None,
            clear_read_gap: false
        }
    );
    assert_eq!(plan_funnel(&FunnelReading::Gap, active, true).alert, None);
    let recovery = plan_funnel(&FunnelReading::Active(vec![]), active, true);
    assert_eq!(recovery.alert, None);
    assert!(recovery.clear_read_gap);
    assert_eq!(
        plan_funnel(&FunnelReading::Inactive, FunnelBaseline::Corrupt, false),
        FunnelPlan {
            alert: Some(FunnelAlert::CorruptBaseline),
            next: Some(FunnelState::Inactive),
            clear_read_gap: true
        }
    );
    assert_eq!(
        plan_funnel(&FunnelReading::Inactive, FunnelBaseline::Absent, false).alert,
        None
    );
}
#[test]
fn exposed_keys_are_sorted_unique_inert_spans_with_exact_200_character_edges() {
    let body = render_funnel_exposure(&["z`\r\n\t@[x]".into(), "a".into(), "a".into()]);
    assert_eq!(
        body,
        "**Tailscale Funnel is exposing a local service to the PUBLIC internet.**\n- Did you set this up? If not, close it now: **tailscale funnel reset**\n- Exposed to the public internet:\n- `a`\n- `z   @[x]`\n"
    );
    for size in [199, 200, 201] {
        let rendered = render_funnel_exposure(&["é".repeat(size)]);
        let suffix = if size > 200 { "…(truncated)" } else { "" };
        assert!(rendered.ends_with(&format!("- `{}{suffix}`\n", "é".repeat(size.min(200)))));
    }
    assert!(render_funnel_exposure(&[]).ends_with("`(unknown)`\n"));
}
