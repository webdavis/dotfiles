use super::{SqliteStore, StoreError};
use rusqlite::Connection;

// Only repository methods choose a table. Callers cannot name SQL or a state key.
#[derive(Clone, Copy)]
pub(super) enum Ring {
    Decisions,
    Journal,
    Activity,
    Presence,
    PolicyAudit,
}
impl Ring {
    pub(super) fn file(self) -> &'static str {
        match self {
            Self::Decisions => crate::DECISIONS,
            Self::Journal => crate::MISSED_NOTIFICATIONS,
            Self::Activity => crate::ACTIVITY,
            Self::Presence => "presence-decisions",
            Self::PolicyAudit => "policy-settings-audit",
        }
    }
    pub(super) fn table(self) -> &'static str {
        match self {
            Self::Decisions => "decisions",
            Self::Journal => "journal",
            Self::Activity => "activity",
            Self::Presence => "presence",
            Self::PolicyAudit => "policy_audit",
        }
    }
    fn where_pending(self) -> &'static str {
        match self {
            Self::Journal => "WHERE claim IS NULL",
            _ => "",
        }
    }
    pub(super) fn kept(self) -> usize {
        match self {
            Self::Decisions => pns_domain::KEPT,
            Self::Journal => pns_domain::missed::KEPT,
            Self::Activity => crate::ACTIVITY_KEPT,
            Self::Presence => pns_domain::KEPT,
            Self::PolicyAudit => super::super::rings::POLICY_SETTINGS_AUDIT_KEPT,
        }
    }
}
impl SqliteStore {
    pub(super) fn append(&self, ring: Ring, line: &str) -> Result<(), StoreError> {
        self.transaction(|transaction| {
            let mut contents = read_ring(transaction, ring)?.unwrap_or_default();
            if !contents.is_empty() && !contents.ends_with('\n') {
                contents.push('\n');
            }
            contents.push_str(line);
            contents.push('\n');
            let lines: Vec<&str> = contents.lines().collect();
            if lines.len() > ring.kept() {
                contents = format!("{}\n", lines[lines.len() - ring.kept()..].join("\n"));
            }
            // Replacing this bounded pending window keeps raw historical bytes
            // until the same prune boundary as the legacy file writer. Claimed
            // batches are excluded and retain their identities throughout.
            transaction.execute(
                &format!("DELETE FROM {} {}", ring.table(), ring.where_pending()),
                [],
            )?;
            let mut insert =
                transaction.prepare(&format!("INSERT INTO {}(line) VALUES (?1)", ring.table()))?;
            for line in contents.split_inclusive('\n') {
                insert.execute([line])?;
            }
            super::import::replaced(transaction, ring.file())?;
            Ok(())
        })
    }
    pub(super) fn read_ring(&self, ring: Ring) -> Result<Option<String>, StoreError> {
        let connection = self.connect()?;
        super::import::readable(&connection, ring.file())?;
        read_ring(&connection, ring)
    }
}
fn read_ring(connection: &Connection, ring: Ring) -> Result<Option<String>, StoreError> {
    let mut statement = connection.prepare(&format!(
        "SELECT line FROM {} {} ORDER BY seq",
        ring.table(),
        ring.where_pending()
    ))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok((!rows.is_empty()).then(|| rows.concat()))
}
