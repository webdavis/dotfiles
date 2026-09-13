#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QueueMemory {
    pub count: Option<u64>,
    pub growth_streak: u64,
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
    match counts.deadletters {
        None => problems.push(format!("{store} dead-letter count is unreadable")),
        Some(0) => {}
        Some(count) => problems.push(format!(
            "{store} holds {count} dead-lettered delivery obligation(s)"
        )),
    }
    let Some(count) = counts.pending else {
        problems.push(format!("{store} pending count is unreadable"));
        return (QueueMemory::default(), problems);
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
        },
        problems,
    )
}
#[cfg(test)]
mod tests;
