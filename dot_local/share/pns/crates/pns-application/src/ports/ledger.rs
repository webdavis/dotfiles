use pns_domain::{Event, routing::ReportMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionIdentity {
    pub producer: String,
    pub request_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerLeg {
    pub destination: String,
    pub route: String,
    pub mode: ReportMode,
    pub decorative: bool,
}
#[derive(Debug, PartialEq)]
pub struct LedgerSubmission {
    pub identity: SubmissionIdentity,
    pub event: Event,
    pub legs: Vec<LedgerLeg>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaseWindow {
    pub now: u64,
    pub until: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnconfirmedDelivery {
    Failed,
    Unlaunched,
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerCompletion {
    Acknowledged {
        detail: String,
    },
    Retry {
        outcome: UnconfirmedDelivery,
        detail: String,
        retry_at: u64,
    },
}
#[derive(Debug, PartialEq, Eq)]
pub enum LedgerFailure {
    Unavailable(String),
    ConflictingSubmission,
    InvalidLease,
    InvalidPlan,
    LostClaim,
}
#[derive(Debug)]
pub struct ClaimedLeg<C> {
    pub claim: C,
    pub leg: LedgerLeg,
}
#[derive(Debug)]
// Only Created grants dispatch ownership. Existing includes an in-flight duplicate.
pub enum PreparedSubmission<C> {
    Created {
        sequence: u64,
        legs: Vec<ClaimedLeg<C>>,
    },
    Existing(Box<SubmissionRecord>),
}
#[derive(Debug)]
pub struct RetryDelivery<C> {
    pub claim: C,
    pub identity: SubmissionIdentity,
    pub event: Event,
    pub leg: LedgerLeg,
}
#[derive(Debug, PartialEq, Eq)]
// A newly claimed generation starts Unknown. That records an unresolved outcome,
// not a proven failure; retry eligibility still respects the active lease.
pub struct LegAttempt {
    pub destination: String,
    pub generation: u64,
    pub at: u64,
    pub completion: LedgerCompletion,
}
#[derive(Debug, PartialEq)]
pub struct SubmissionRecord {
    pub sequence: u64,
    pub submission: LedgerSubmission,
    pub attempts: Vec<LegAttempt>,
}
pub trait DeliveryLedger {
    type Claim;

    fn prepare(
        &self,
        submission: &LedgerSubmission,
        lease: LeaseWindow,
    ) -> Result<PreparedSubmission<Self::Claim>, LedgerFailure>;
    fn record(
        &self,
        claim: &Self::Claim,
        completion: &LedgerCompletion,
        at: u64,
    ) -> Result<(), LedgerFailure>;
    fn claim_retry(
        &self,
        lease: LeaseWindow,
    ) -> Result<Option<RetryDelivery<Self::Claim>>, LedgerFailure>;
    fn inspect(
        &self,
        identity: &SubmissionIdentity,
    ) -> Result<Option<SubmissionRecord>, LedgerFailure>;
}
