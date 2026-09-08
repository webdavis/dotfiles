use super::*;
use pns_domain::routing::ReportMode;
use rusqlite::{Transaction, params};

pub(super) fn submission(
    transaction: &Transaction<'_>,
    input: &LedgerSubmission,
    lease: LeaseWindow,
) -> Result<PreparedSubmission<DeliveryClaim>, StoreError> {
    if let Some(record) = read::find(transaction, &input.identity)? {
        return Ok(PreparedSubmission::Existing(Box::new(record)));
    }
    let event = &input.event;
    transaction.execute(
        "INSERT INTO ledger_events(producer,request_id,agent,state,project,branch,detail,title,message,preview,pane)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![input.identity.producer,input.identity.request_id,event.agent,event.state,event.project,
            event.branch,event.detail,event.title,event.message,event.preview,event.pane],
    )?;
    let sequence = transaction.last_insert_rowid();
    let mut legs = Vec::new();
    for (position, leg) in input.legs.iter().enumerate() {
        transaction.execute(
            "INSERT INTO ledger_legs(event,position,destination,route,mode,decorative,generation,owner,token,lease_until,due)
             VALUES (?1,?2,?3,?4,?5,?6,1,?7,randomblob(16),?8,?8)",
            params![sequence,position,leg.destination,leg.route,
                i32::from(leg.mode == ReportMode::ReportOutcome),leg.decorative,std::process::id(),lease.until.to_be_bytes()],
        )?;
        let claim = claims::started(transaction, transaction.last_insert_rowid(), lease)?;
        legs.push(ClaimedLeg {
            claim,
            leg: leg.clone(),
        });
    }
    Ok(PreparedSubmission::Created {
        sequence: u64::try_from(sequence)
            .map_err(|_| StoreError::InvalidState("invalid ledger sequence".into()))?,
        legs,
    })
}
