#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QueueMemory {
    pub count: Option<u64>,
    pub growth_streak: u64,
    /// Dead letters observed on the last readable tick. pns RETAINS a
    /// dead-lettered leg for the life of its ledger and has no statement that
    /// removes one, so a bare "any dead letters" test pages on every tick from
    /// the first one until somebody deletes the database. Only an INCREASE is
    /// news, which needs the previous count carried forward.
    pub deadletters: Option<u64>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueCounts {
    pub pending: Option<u64>,
    pub deadletters: Option<u64>,
    pub alarm_generation: Option<u64>,
}
#[derive(Debug, Clone, Copy)]
pub enum QueueKind {
    Legacy,
    Pns,
}
pub fn judge_queue(
    kind: QueueKind,
    counts: QueueCounts,
    prior: QueueMemory,
) -> (QueueMemory, Vec<String>) {
    let store = match kind {
        QueueKind::Legacy => "legacy alert queue",
        QueueKind::Pns => "pns delivery ledger",
    };
    let mut problems = Vec::new();
    if counts.alarm_generation.is_some() {
        problems.push(format!(
            "{store} has an unacknowledged delivery-health alarm"
        ));
    }
    // Only an INCREASE is news. Neither store deletes a dead-lettered leg, so
    // "the count is above zero" stays true forever once it has been true once,
    // and a page on that predicate repeats on every tick until the database is
    // thrown away. The first readable sighting is reported, each further leg is
    // reported as a delta, and a purge lowers the baseline in silence.
    match (prior.deadletters, counts.deadletters) {
        (_, None) => problems.push(format!("{store} dead-letter count is unreadable")),
        (None, Some(count)) if count > 0 => problems.push(format!(
            "{store} holds {count} dead-lettered delivery obligation(s)"
        )),
        (Some(previous), Some(count)) if count > previous => problems.push(format!(
            "{store} dead-lettered {} more delivery obligation(s) since the last tick ({count} retained)",
            count - previous
        )),
        _ => {}
    }
    let Some(count) = counts.pending else {
        problems.push(format!("{store} pending count is unreadable"));
        return (
            QueueMemory {
                deadletters: counts.deadletters,
                ..QueueMemory::default()
            },
            problems,
        );
    };
    let growth_streak = if prior.count.is_some_and(|previous| count > previous) {
        prior.growth_streak.saturating_add(1)
    } else {
        0
    };
    if growth_streak >= 2 {
        problems.push(format!("{store} pending backlog grew across {growth_streak} consecutive ticks ({count} pending)"));
    }
    (
        QueueMemory {
            count: Some(count),
            growth_streak,
            deadletters: counts.deadletters,
        },
        problems,
    )
}
#[cfg(test)]
mod tests;
