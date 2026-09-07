mod tests {
    use super::super::*;

    #[test]
    fn three_of_a_ticks_bridge_calls_fit_inside_its_own_interval_with_the_breath_to_spare() {
        // THE PROPERTY, not the arithmetic. The resolve makes three calls before
        // the first fade is issued, and at the transport's own ten seconds they
        // outlive every interval the config permits: a wedged bridge then had
        // tick after tick piling up, each still dialling while the next was
        // spawned. What has to hold is that the three fit with room left for a
        // breath, at both ends of the range the config accepts.
        //
        // EVERY LEGAL INTERVAL AND NOT A SAMPLE OF FOUR. `tick_bridge_deadline`
        // divides by five, so the budget is a STEP FUNCTION of the refresh and
        // a four-point sample walks straight past whichever step is tight.
        let shipped = pns::config::Lights::default();
        let cycles = [
            (
                "the locked blocked shape",
                pns::lights::breath_cycle(&shipped.blocked.breath),
            ),
            (
                "the locked loop motion",
                pns::lights::breathe_then_flare_cycle(&shipped.looping.breathe_then_flare),
            ),
        ];
        for refresh_secs in pns::config::MIN_REFRESH_SECS..=pns::config::MAX_REFRESH_SECS {
            let three = tick_bridge_deadline(refresh_secs).as_millis() * 3;
            let interval = u128::from(refresh_secs) * 1000;
            assert!(
                three < interval,
                "refresh {refresh_secs}s: three calls at {three}ms do not fit"
            );
            let left = u64::try_from(interval - three).expect("a budget in milliseconds");
            for (named, cycle) in &cycles {
                assert!(
                    !pns::lights::breath_fades(left, cycle, pns::lights::Resume::default())
                        .is_empty(),
                    "refresh {refresh_secs}s: the {left}ms left over will not hold one \
                     cycle of {named}"
                );
                // AND RESUMED AT THE WORST A LIVE RECORD CAN LEAVE, which is the
                // case a fresh schedule never reaches: `resume_from` caps a
                // phase at the step of the leg it names, so the latest first
                // fade any tick can inherit is the cycle's longest leg's step.
                // A schedule that comes back EMPTY there is a lamp holding
                // still for a whole interval, which is the one thing a liveness
                // signal must never do.
                let worst = cycle
                    .iter()
                    .map(|leg| pns::lights::step_ms(leg.duration_ms))
                    .max()
                    .expect("a cycle has legs");
                assert!(
                    !pns::lights::breath_fades(
                        left,
                        cycle,
                        pns::lights::Resume {
                            first_due_ms: worst,
                            next_leg: 0,
                        }
                    )
                    .is_empty(),
                    "refresh {refresh_secs}s: {named} resumed a whole {worst}ms step \
                     late has no room in the {left}ms the interval leaves"
                );
            }
        }
    }
}
