use super::super::{SqliteStore, StoreError};
use pns_domain::lights::mute::Muted;
impl SqliteStore {
    pub fn read_muted(&self) -> Result<Vec<Muted>, StoreError> {
        let connection = self.connect()?;
        let mut query = connection.prepare("SELECT line FROM lamp_mutes ORDER BY seq")?;
        let lines = query
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        // Parse retained bytes first so malformed records keep their exact
        // complaint. An unreadable import with no rows still fails closed.
        let muted = if lines.is_empty() {
            Vec::new()
        } else {
            crate::lights_codec::muted_entries(&lines.join("\n"))
                .map_err(StoreError::InvalidState)?
        };
        super::super::import::readable(&connection, "lights-quiet")?;
        Ok(muted)
    }
    pub fn write_muted(&self, muted: &[Muted]) -> Result<(), StoreError> {
        self.transaction(|transaction| {
            transaction.execute("DELETE FROM lamp_mutes", [])?;
            let mut insert = transaction.prepare("INSERT INTO lamp_mutes(line) VALUES (?1)")?;
            for entry in muted {
                insert.execute([format!("{} {}", entry.expiry, entry.place)])?;
            }
            super::super::import::replaced(transaction, "lights-quiet")?;
            Ok(())
        })
    }
}
