use super::{
    super::StoreError,
    families::{FAMILIES, Family},
};
use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

pub(in super::super) struct Claims {
    pub(super) journal: Vec<(u32, PathBuf)>,
    pub(super) windows: Vec<PathBuf>,
}
pub(in super::super) fn inspect(state: &Path, now: Option<u64>) -> Result<Claims, StoreError> {
    for family in FAMILIES {
        if let Family::Ring(_) = family {
            let lock = state.join(format!("{}.lock", family.name()));
            if lock.try_exists()?
                && !super::super::super::locks::lock_aged_out(
                    &lock,
                    now.unwrap_or(0),
                    super::super::super::ring::RING_LOCK_STALE_SECS,
                )
            {
                return Err(busy());
            }
        }
    }
    let mut journal: Vec<(Option<SystemTime>, PathBuf, u32)> = Vec::new();
    let mut windows = Vec::new();
    for entry in std::fs::read_dir(state)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if let Some(owner) = name.strip_prefix("last-present.claim.") {
            if !crate::protocols::return_window::window_claim_is_free(owner, now) {
                return Err(busy());
            }
            windows.push(entry.path());
        }
        let Some(owner) = name
            .strip_prefix("missed-notifications.claim.")
            .or_else(|| name.strip_prefix("missed-notifications.held."))
        else {
            continue;
        };
        if !crate::marker_files::owner_is_gone(owner) {
            return Err(busy());
        }
        let pid = owner
            .split('.')
            .next()
            .and_then(|pid| pid.parse::<u32>().ok())
            .ok_or_else(busy)?;
        journal.push((
            entry
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok()),
            entry.path(),
            pid,
        ));
    }
    journal.sort();
    windows.sort();
    Ok(Claims {
        journal: journal
            .into_iter()
            .map(|(_, path, owner)| (owner, path))
            .collect(),
        windows,
    })
}
fn busy() -> StoreError {
    std::io::Error::new(
        std::io::ErrorKind::WouldBlock,
        "a legacy state owner must finish before import",
    )
    .into()
}

pub(super) fn import_journal(
    transaction: &rusqlite::Transaction<'_>,
    claims: &Claims,
) -> Result<Option<&'static str>, StoreError> {
    let mut failed = false;
    for (owner, path) in &claims.journal {
        let Ok(body) = super::read::read(path, Some(crate::RING_READ_MAX)) else {
            failed = true;
            continue;
        };
        transaction.execute("INSERT INTO return_claims(owner) VALUES (?1)", [owner])?;
        let id = transaction.last_insert_rowid();
        // These batches were already held; a new pending append must never prune them.
        for line in body.split_inclusive('\n') {
            transaction.execute(
                "INSERT INTO journal(line, claim) VALUES (?1, ?2)",
                rusqlite::params![line, id],
            )?;
        }
    }
    Ok(failed.then_some("an abandoned legacy journal batch could not be read"))
}
pub(super) fn import_window(
    transaction: &rusqlite::Transaction<'_>,
    claims: &Claims,
) -> Result<Option<&'static str>, StoreError> {
    let mut failed = false;
    for path in &claims.windows {
        let Ok(body) = super::read::read(path, None) else {
            failed = true;
            continue;
        };
        let Ok(epoch) = body.trim().parse::<u64>() else {
            failed = true;
            continue;
        };
        transaction.execute("INSERT INTO return_edge(id, epoch) VALUES (1, ?1) ON CONFLICT(id) DO UPDATE SET epoch = max(epoch, excluded.epoch)", [epoch.to_be_bytes()])?;
    }
    Ok(failed.then_some("an abandoned legacy return window could not be read"))
}
