use crate::PostOutcome;

/// The status codes that mean the record reached the gateway. WRITTEN ONCE:
/// both the sentence and the verdict read it, so the rule cannot be moved for
/// one and left standing for the other, which would have a doctor call a post
/// good while the printed line called it FAILED.
const DELIVERED_STATUS: std::ops::Range<u16> = 200..300;

/// Whether one answer means the record arrived.
pub fn delivered(outcome: PostOutcome) -> bool {
    matches!(outcome, PostOutcome::Status(code) if DELIVERED_STATUS.contains(&code))
}

/// The line sync mode prints for one outcome, exactly as the bash spells it
/// minus the `pns: ` prefix, which the one print site adds.
pub fn outcome_line(outcome: PostOutcome) -> String {
    match outcome {
        PostOutcome::Status(code) if delivered(outcome) => format!("posted HTTP {code}"),
        PostOutcome::Status(code) => format!("post FAILED HTTP {code}"),
        PostOutcome::NoStatus => "post FAILED (curl reported no HTTP status at all)".to_string(),
        PostOutcome::NoResponse => {
            "post FAILED HTTP 000 (no response; is the hermes gateway up?)".to_string()
        }
    }
}

/// The line sync mode prints when there is no signing key. It names the
/// config key to write, because "not set up" without an address sends the
/// operator hunting.
pub fn skipped_line() -> String {
    "post SKIPPED, no hermes key in the config ([plugins.hermes] key); nothing was sent".to_string()
}

#[cfg(test)]
mod tests;
