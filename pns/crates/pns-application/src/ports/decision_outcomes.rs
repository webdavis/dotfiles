use crate::SubmissionIdentity;
use pns_domain::{Delivery, Record};

pub trait DecisionOutcomes {
    // Begin only for a newly created logical submission. An already retained
    // identity keeps its current outcome; retries call revise instead.
    fn begin(&self, identity: &SubmissionIdentity, record: &Record) -> Result<(), String>;
    // A late retry cannot resurrect a decision already removed by retention.
    fn revise(
        &self,
        identity: &SubmissionIdentity,
        destination: &str,
        delivery: &Delivery,
    ) -> Result<bool, String>;
}
