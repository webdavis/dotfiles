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

/// The doctor's one line about the ledger.
///
/// IT STAYS A SUMMARY AND NAMES THE DETAIL VIEW. Counts say how MUCH is wrong
/// and nothing about what, so a line reporting a backlog without saying where to
/// look leaves the operator holding a number and no next step. It names
/// `pns failures` only when there is something there: a pointer offered on a
/// healthy machine is one the reader learns to skip.
pub fn delivery_health_line(health: Result<DeliveryHealth, String>) -> String {
    match health {
        Err(_) => "pns doctor: delivery ledger unreadable; backlog and deadletters unknown".into(),
        Ok(health) => format!(
            "pns doctor: delivery ledger: {} pending leg(s), {} deadlettered, growth streak {}, alarm {}; recording gaps {}{}",
            health.pending_legs,
            health.deadlettered_legs,
            health.growth_streak,
            if health.alarm_generation.is_some() {
                "pending"
            } else {
                "acknowledged"
            },
            if health.recording_gap {
                "recorded in recent daemon log"
            } else {
                "none in recent daemon log"
            },
            if health.pending_legs > 0 || health.deadlettered_legs > 0 {
                "; run `pns failures` for what is not arriving"
            } else {
                ""
            }
        ),
    }
}

#[cfg(test)]
mod tests;
