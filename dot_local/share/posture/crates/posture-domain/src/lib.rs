//! The pipeline's policy, expressed as total functions of their arguments.
//!
//! This crate is responsible for what posture DECIDES: which result-log rows
//! are findings and how severe they are, the page-or-digest-or-log gate, the
//! allowlist and known-good verdicts, the tamper decision over a deployed
//! state and a manifest answer, the upgrade-record triage, the page and digest
//! vocabulary with their caps, the canary freshness rule, the watchdog's five
//! probes and the manifest audit, the controls file's validations, the
//! poller's classifiers and baseline trust rules, the funnel exposure
//! transitions, the converge drift verdicts and restart evidence, the cursor
//! grammar, and the signing classification.
//!
//! It is responsible for none of what posture TOUCHES. No filesystem, clock,
//! JSON, environment variable, spawned process, privilege, macOS API or
//! command-line output appears here or in anything it depends on. Every
//! reading it judges arrives as a typed value with its own "could not read"
//! state, never as an empty string, so each fail-safe direction is an enum
//! arm rather than an emptiness check.
//!
//! Nothing has moved in yet. Each module lands red first, one pull request per
//! group of statements, and every retired bash test is mapped by name in
//! `docs/test-baseline.tsv`.
