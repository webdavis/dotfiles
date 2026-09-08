use super::*;
use pns_application::DeliveryHealth;
use rusqlite::{Connection, Transaction};

pub(super) fn raise_alarm(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    let generation: u64 = transaction.query_row(
        "SELECT generation FROM delivery_health WHERE id = 1",
        [],
        |row| row.get(0),
    )?;
    let next = generation
        .checked_add(1)
        .filter(|n| *n <= i64::MAX as u64)
        .ok_or_else(|| StoreError::InvalidState("delivery alarm generation exhausted".into()))?;
    transaction.execute(
        "UPDATE delivery_health SET generation = ?1 WHERE id = 1",
        [next],
    )?;
    Ok(())
}
fn snapshot(connection: &Connection) -> Result<DeliveryHealth, StoreError> {
    let (pending_legs, deadlettered_legs) = connection.query_row(
        "SELECT COUNT(CASE WHEN acknowledged = 0 AND deadlettered_at IS NULL THEN 1 END),
          COUNT(deadlettered_at) FROM ledger_legs",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let (growth_streak, generation, acknowledged): (u8, u64, u64) = connection.query_row(
        "SELECT growth,generation,acknowledged FROM delivery_health WHERE id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    Ok(DeliveryHealth {
        pending_legs,
        deadlettered_legs,
        growth_streak,
        alarm_generation: (generation > acknowledged).then_some(generation),
        recording_gap: false,
    })
}
impl SqliteStore {
    pub fn sample_delivery_health(&self) -> Result<DeliveryHealth, LedgerFailure> {
        self.ledger_result(self.existing_transaction(|transaction| {
            let health = snapshot(transaction)?;
            let previous: Option<u64> = transaction.query_row(
                "SELECT previous_pending FROM delivery_health WHERE id = 1",
                [],
                |row| row.get(0),
            )?;
            let growth = if previous.is_some_and(|count| health.pending_legs > count) {
                (health.growth_streak + 1).min(2)
            } else {
                0
            };
            transaction.execute(
                "UPDATE delivery_health SET previous_pending = ?1, growth = ?2 WHERE id = 1",
                rusqlite::params![health.pending_legs, growth],
            )?;
            if growth >= 2 {
                raise_alarm(transaction)?;
            }
            snapshot(transaction)
        }))
    }
    pub fn acknowledge_delivery_alarm(&self, generation: u64) -> Result<(), LedgerFailure> {
        self.ledger_result(self.existing_transaction(|transaction| {
            transaction.execute(
                "UPDATE delivery_health SET acknowledged = ?1 WHERE id = 1 AND generation = ?1",
                [generation],
            )?;
            Ok(())
        }))
    }
    pub fn delivery_health(&self) -> Result<DeliveryHealth, LedgerFailure> {
        // No create, import or migration; include committed WAL rows.
        (|| {
            let mut connection = self.read_only()?;
            let transaction = connection.transaction()?;
            let mut health = snapshot(&transaction)?;
            health.recording_gap = self.delivery_recording_gap()?;
            Ok(health)
        })()
        .map_err(|_: StoreError| LedgerFailure::Unavailable("delivery ledger unreadable".into()))
    }
}
