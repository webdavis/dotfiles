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
    // The protocol boundary supplies bounded encoded data; policy does not parse it.
    pub producer_request: Option<String>,
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
    Rejected {
        status: u16,
        detail: String,
    },
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
    // Commit the claimed completion and its retained decision-leg verdict as
    // one operation. A stale claim changes neither record; a pruned decision
    // does not prevent the ledger completion from being retained.
    fn record(
        &self,
        claim: &Self::Claim,
        delivery: &pns_domain::Delivery,
        at: u64,
        backoff: pns_domain::retry::RetryBackoff,
    ) -> Result<(), LedgerFailure>;
    fn claim_retry(
        &self,
        lease: LeaseWindow,
        _limits: pns_domain::retry::RetryLimits,
    ) -> Result<Option<RetryDelivery<Self::Claim>>, LedgerFailure>;
    fn inspect(
        &self,
        identity: &SubmissionIdentity,
    ) -> Result<Option<SubmissionRecord>, LedgerFailure>;
}
