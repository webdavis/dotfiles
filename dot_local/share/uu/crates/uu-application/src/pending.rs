use crate::delivery::send_alert;
use crate::ports::{
    AlarmKind, AlertOutcome, AlertTarget, Notice, RunDelivery, RunPresentation, RunState, Streak,
    StreakKind,
};
use crate::run::LaneSettings;
use std::collections::BTreeMap;
use uu_domain::{LaneReport, LaneVerdict, next_pending_streak};

pub(crate) fn track_pending(
    state: &impl RunState,
    delivery: &impl RunDelivery,
    presentation: &impl RunPresentation,
    host: &str,
    lanes: &BTreeMap<String, LaneSettings>,
    reports: &[LaneReport],
) {
    for report in reports {
        let threshold = lanes[&report.name].escalate_after_runs;
        let snapshot = state.streak(&report.name, StreakKind::Pending);
        let previous = match snapshot.value {
            Streak::Absent => 0,
            Streak::Value(value) => value,
            Streak::Unreadable(why) => {
                send_alert(
                    delivery,
                    presentation,
                    AlarmKind::Pending,
                    host,
                    AlertTarget::Lane(&report.name),
                    &format!(
                        "this lane's pending streak at {} could not be trusted ({why}); treating it as already close to escalation rather than silently starting over",
                        snapshot.location
                    ),
                );
                threshold.get() - 1
            }
        };
        let (next, tripped) = next_pending_streak(
            previous,
            report.verdict() == LaneVerdict::Pending,
            threshold,
        );
        // An undelivered one-shot alarm remains eligible on the next pending run.
        let recorded = if tripped
            && matches!(
                send_alert(
                    delivery,
                    presentation,
                    AlarmKind::Pending,
                    host,
                    AlertTarget::Lane(&report.name),
                    &format!(
                        "updates have remained pending for {} consecutive attempt(s); operator action is needed",
                        threshold
                    )
                ),
                AlertOutcome::Failed(_)
            ) {
            threshold.get() - 1
        } else {
            next
        };
        if let Err(failure) = state.write_streak(&report.name, StreakKind::Pending, recorded) {
            presentation.notice(Notice::StreakWriteFailed {
                kind: StreakKind::Pending,
                lane: &report.name,
                failure: &failure,
            });
            send_alert(
                delivery,
                presentation,
                AlarmKind::Pending,
                host,
                AlertTarget::Lane(&report.name),
                &format!(
                    "this lane's pending streak at {} could not be recorded ({}); pending tracking for it is unreliable until this is fixed",
                    failure.location, failure.cause
                ),
            );
        }
    }
}
