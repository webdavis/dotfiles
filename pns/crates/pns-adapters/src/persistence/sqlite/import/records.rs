use super::{
    super::{StoreError, rows::Ring},
    families::Family,
};
use rusqlite::Transaction;
// The raw line includes its original terminator. Doctor reads historical text,
// while the replay reader applies its existing schema and skips unknown rows.
pub(super) fn ring_rows(
    transaction: &Transaction<'_>,
    ring: Ring,
    body: &str,
) -> Result<(), StoreError> {
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    let start = lines.len().saturating_sub(ring.kept());
    let mut insert =
        transaction.prepare(&format!("INSERT INTO {}(line) VALUES (?1)", ring.table()))?;
    if lines.is_empty() {
        insert.execute([""])?;
    }
    for line in &lines[start..] {
        insert.execute([*line])?;
    }
    Ok(())
}
pub(super) fn put(
    transaction: &Transaction<'_>,
    family: Family,
    body: &str,
) -> Result<(), StoreError> {
    match family {
        Family::Ring(ring) => ring_rows(transaction, ring, body)?,
        Family::Scalar(scalar) => {
            transaction.execute(
                &format!("INSERT INTO {}(id, body) VALUES (1, ?1)", scalar.table()),
                [body],
            )?;
        }
        Family::Return => {
            if let Ok(epoch) = body.trim().parse::<u64>() {
                transaction.execute(
                    "INSERT INTO return_edge(id, epoch) VALUES (1, ?1)",
                    [epoch.to_be_bytes()],
                )?;
            }
        }
        Family::Held => {
            let mut insert = transaction.prepare("INSERT INTO held_lamps(token) VALUES (?1)")?;
            for token in body.split_whitespace() {
                insert.execute([token])?;
            }
        }
        Family::Muted => {
            let mut insert = transaction.prepare("INSERT INTO lamp_mutes(line) VALUES (?1)")?;
            // An empty file is a malformed mute, not the ordinary absent set.
            for line in body.strip_suffix('\n').unwrap_or(body).split('\n') {
                insert.execute([line])?;
            }
        }
    }
    Ok(())
}
