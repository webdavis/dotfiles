use super::*;

impl<P: SignedPost, A> EngineRunDelivery<'_, P, A> {
    pub(super) fn alarm_post(
        &self,
        records: &Records,
        url: &str,
        kind: AlarmKind,
        host: &str,
        detail: &str,
    ) -> Result<(), String> {
        let state = match kind {
            AlarmKind::Failed => "failed",
            AlarmKind::Stale => "stale",
            AlarmKind::Pending => "pending",
            AlarmKind::RecordLost => "record-lost",
        };
        let body = record_body(state, host, detail);
        let signature = sign(&records.key, &body)
            .ok_or_else(|| "failure webhook could not be signed".to_string())?;
        let outcome = self
            .post
            .post(url, &body, &signature, None, Some(RECORD_DEADLINE));
        if delivered(outcome) {
            Ok(())
        } else {
            Err(format!(
                "failure webhook at {url}: {}",
                outcome_line(outcome)
            ))
        }
    }
}

#[cfg(test)]
mod tests;
