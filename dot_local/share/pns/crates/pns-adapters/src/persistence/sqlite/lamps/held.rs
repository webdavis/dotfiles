use super::super::{SqliteStore, StoreError};
use pns_domain::lights::phase::HeldEntry;
impl SqliteStore {
    pub fn read_held(&self) -> Result<Vec<HeldEntry>, StoreError> {
        let connection = self.connect()?;
        super::super::import::readable(&connection, "lights-held")?;
        let mut query = connection.prepare("SELECT token FROM held_lamps ORDER BY seq")?;
        let tokens = query
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tokens
            .iter()
            .map(|token| crate::lights_codec::parse_held_token(token))
            .collect())
    }
    pub fn remember_held(&self, held: &[HeldEntry]) -> Result<(), StoreError> {
        // The caller must stop before arming any lamp if this publication fails.
        // Deleting the prior names and recording the new set commit together.
        self.transaction(|transaction| {
            transaction.execute("DELETE FROM held_lamps", [])?;
            let mut insert = transaction.prepare("INSERT INTO held_lamps(token) VALUES (?1)")?;
            for entry in held {
                insert.execute([crate::lights_codec::render_held_token(entry)])?;
            }
            super::super::import::replaced(transaction, "lights-held")?;
            Ok(())
        })
    }
}
