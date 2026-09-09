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

pub fn delivery_health_line(health: Result<DeliveryHealth, String>) -> String {
    match health {
        Err(_) => "pns doctor: delivery ledger unreadable; backlog and deadletters unknown".into(),
        Ok(health) => format!(
            "pns doctor: delivery ledger: {} pending leg(s), {} deadlettered, growth streak {}, alarm {}; recording gaps {}",
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
            }
        ),
    }
}

#[cfg(test)]
mod tests;
