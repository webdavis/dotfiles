use crate::{DeliveryHealth, LedgerFailure};
use pns_domain::Delivery;

pub fn report_delivery_health(
    health: Result<&DeliveryHealth, &LedgerFailure>,
    deliver: impl FnOnce(&str) -> Delivery,
    acknowledge: impl FnOnce(u64) -> Result<(), LedgerFailure>,
) -> Result<(), String> {
    let (generation, message) = match health {
        Ok(health) if health.alarm_generation.is_none() => return Ok(()),
        Ok(health) => (health.alarm_generation, format!(
            "{} undelivered delivery leg(s), {} deadlettered; backlog growth streak {}. The delivery pipeline needs attention.",
            health.pending_legs, health.deadlettered_legs, health.growth_streak)),
        Err(_) => (None, "Delivery ledger unreadable; queued delivery and alarm ownership could not be verified.".into()),
    };
    if !matches!(deliver(&message), Delivery::Delivered(_)) {
        return Err("delivery health banner failed; alarm remains pending".into());
    }
    match generation {
        Some(generation) => acknowledge(generation)
            .map_err(|_| "delivery health alarm acknowledgement failed".into()),
        None => Err("delivery ledger unreadable; local alarm attempted".into()),
    }
}

/// The doctor's lines about the ledger.
///
/// ONE FACT PER LINE, and a healthy machine gets ONE line. This used to be a
/// single sentence carrying five facts in the ledger's own vocabulary: pending
/// legs, deadletters, a growth streak, an alarm generation and a recording gap,
/// none of which name anything an operator has a mental model for. Five facts in
/// one line is also five facts a reader skims past together.
///
/// A "LEG" IS ONE CHANNEL'S COPY of one notification, which is why the word does
/// not appear: the reader's unit is the notification and the channel it did not
/// reach.
///
/// IT NAMES THE DETAIL VIEW ONLY WHEN THERE IS DETAIL. Counts say how MUCH is
/// wrong and nothing about what, so a backlog without a next step leaves the
/// operator holding a number; a pointer offered on a healthy machine is one the
/// reader learns to skip.
pub fn delivery_health_lines(health: Result<DeliveryHealth, String>) -> Vec<String> {
    let Ok(health) = health else {
        return vec![format!(
            "{PREFIX}the delivery record could not be read, so nothing here is known"
        )];
    };

    let mut lines = Vec::new();
    if health.pending_legs > 0 {
        lines.push(format!(
            "{PREFIX}{} still waiting to reach a channel",
            plural(health.pending_legs, "notification")
        ));
    }
    if health.deadlettered_legs > 0 {
        lines.push(format!(
            "{PREFIX}{} given up on after retrying",
            plural(health.deadlettered_legs, "notification")
        ));
    }
    // A STREAK OF ZERO IS THE NORMAL CASE and says nothing worth a line.
    if health.growth_streak > 0 {
        lines.push(format!(
            "{PREFIX}the backlog has grown {} in a row, so it is not draining",
            plural(u64::from(health.growth_streak), "check")
        ));
    }
    if health.alarm_generation.is_some() {
        lines.push(format!(
            "{PREFIX}an alarm about this has not reached you yet"
        ));
    }
    if health.recording_gap {
        lines.push(format!(
            "{PREFIX}the daemon log shows it recently failed to record a delivery, so these \
             counts may be low"
        ));
    }

    if lines.is_empty() {
        // THE HEALTHY SENTENCE IS EXPLICIT. An empty section reads as output
        // that failed rather than as nothing to report.
        return vec![format!(
            "{PREFIX}nothing is queued and nothing was given up on"
        )];
    }
    // THE POINTER HANGS OFF THE COUNTS AND NOTHING ELSE. `pns failures` lists
    // what is queued or given up on, so offering it for a growth streak or an
    // unacknowledged alarm would send the reader to a listing that does not
    // answer either.
    if health.pending_legs > 0 || health.deadlettered_legs > 0 {
        lines.push(format!(
            "{PREFIX}run `pns failures` for what is not arriving"
        ));
    }
    lines
}

/// What every sentence here opens with, so a caller printing one alone still
/// says which command is speaking.
const PREFIX: &str = "pns doctor: ";

/// `1 notification` and `2 notifications`, because `1 notification(s)` is a
/// parenthesis asking the reader to do the agreement themselves.
fn plural(count: u64, noun: &str) -> String {
    if count == 1 {
        format!("{count} {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

#[cfg(test)]
mod tests;
