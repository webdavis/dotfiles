use super::StoreError;
use rusqlite::{Connection, OptionalExtension, Transaction};

// These private table choices belong to named repositories. No caller supplies
// a key or a codec: old scalar codecs retain their exact refusal directions.
#[derive(Clone, Copy)]
pub(super) enum Scalar {
    Quiet,
    Staleness,
    LightsComplaint,
    QuietComplaint,
    News,
    Streak,
}
impl Scalar {
    pub(super) fn file(self) -> &'static str {
        match self {
            Self::Quiet => "quiet-until",
            Self::Staleness => "home-staleness",
            Self::LightsComplaint => "lights-said",
            Self::QuietComplaint => "lights-quiet-said",
            Self::News => "lights-news",
            Self::Streak => "lights-streak",
        }
    }
    pub(super) fn table(self) -> &'static str {
        match self {
            Self::Quiet => "quiet",
            Self::Staleness => "staleness",
            Self::LightsComplaint => "lights_complaint",
            Self::QuietComplaint => "quiet_complaint",
            Self::News => "lamp_news",
            Self::Streak => "lamp_streak",
        }
    }
    pub(super) fn read(self, connection: &Connection) -> Result<Option<String>, StoreError> {
        super::import::readable(connection, self.file())?;
        self.stored(connection)
    }
    pub(super) fn stored(self, connection: &Connection) -> Result<Option<String>, StoreError> {
        Ok(connection
            .query_row(
                &format!("SELECT body FROM {} WHERE id = 1", self.table()),
                [],
                |row| row.get(0),
            )
            .optional()?)
    }
    pub(super) fn write(
        self,
        transaction: &Transaction<'_>,
        value: Option<&str>,
    ) -> Result<(), StoreError> {
        let table = self.table();
        match value {
            Some(body) => {
                transaction.execute(&format!("INSERT INTO {table}(id, body) VALUES (1, ?1) ON CONFLICT(id) DO UPDATE SET body = excluded.body"), [body])?;
            }
            None => {
                transaction.execute(&format!("DELETE FROM {table} WHERE id = 1"), [])?;
            }
        }
        super::import::replaced(transaction, self.file())?;
        Ok(())
    }
}
