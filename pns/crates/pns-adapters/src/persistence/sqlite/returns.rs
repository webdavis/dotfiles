use super::{SqliteStore, StoreError};
use pns_application::{Claim, ReturnMoment};
use rusqlite::{Connection, OptionalExtension, Transaction};
mod journal;
pub(super) mod schema;

impl SqliteStore {
    pub fn claim_return(
        &self,
        now: Option<u64>,
        take_journal: bool,
    ) -> Result<Option<Claim>, StoreError> {
        let mut ownership = self
            .claim
            .lock()
            .map_err(|_| StoreError::InvalidState("return ownership poisoned".into()))?;
        if ownership.is_some() {
            return Ok(None);
        }
        let Some((claim, owned)) = self.transaction(|transaction| {
            let since = read_edge(transaction)?;
            let Some((claim, owned)) = (if take_journal {
                journal::claim(transaction, since, now)?
            } else {
                Some((
                    Claim {
                        since,
                        waiting: Vec::new(),
                        replay: None,
                    },
                    None,
                ))
            }) else {
                return Ok(None);
            };
            if let Some(edge) = since.max(now) {
                write_edge(transaction, edge)?;
            }
            Ok(Some((claim, owned)))
        })?
        else {
            return Ok(None);
        };
        *ownership = owned;
        Ok(Some(claim))
    }

    pub fn complete_return(&self) -> Result<(), StoreError> {
        let mut ownership = self
            .claim
            .lock()
            .map_err(|_| StoreError::InvalidState("return ownership poisoned".into()))?;
        let Some(owned) = *ownership else {
            return Ok(());
        };
        self.transaction(|transaction| {
            transaction.execute("DELETE FROM journal WHERE claim = ?1", [owned])?;
            transaction.execute("DELETE FROM return_claims WHERE id = ?1", [owned])?;
            Ok(())
        })?;
        *ownership = None;
        Ok(())
    }
    pub fn mark_present(&self, now: u64) -> Result<(), StoreError> {
        self.transaction(|transaction| {
            write_edge(
                transaction,
                read_edge(transaction)?.map_or(now, |held| held.max(now)),
            )
        })
        .inspect_err(|error| self.report("return edge", error))
    }
    pub fn last_present(&self) -> Result<Option<u64>, StoreError> {
        read_edge(&self.connect()?)
    }
}

fn read_edge(connection: &Connection) -> Result<Option<u64>, StoreError> {
    let bytes = connection
        .query_row("SELECT epoch FROM return_edge WHERE id = 1", [], |row| {
            row.get::<_, [u8; 8]>(0)
        })
        .optional()?;
    Ok(bytes.map(u64::from_be_bytes))
}
fn write_edge(transaction: &Transaction<'_>, epoch: u64) -> Result<(), StoreError> {
    transaction.execute("INSERT INTO return_edge(id, epoch) VALUES (1, ?1) ON CONFLICT(id) DO UPDATE SET epoch = excluded.epoch", [epoch.to_be_bytes()])?;
    super::import::replaced(transaction, "last-present")?;
    Ok(())
}
impl ReturnMoment for SqliteStore {
    fn claim(&self, now: Option<u64>, take_journal: bool) -> Option<Claim> {
        match self.claim_return(now, take_journal) {
            Ok(claim) => claim,
            Err(error) => {
                self.report("return claim", &error);
                None
            }
        }
    }
    fn complete(&self) {
        if let Err(error) = self.complete_return() {
            self.report("return completion", &error);
        }
    }
}
