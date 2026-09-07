//! The composition root: argv in, exit code out.
//!
//! This crate is responsible for decoding a command line into the use case it
//! names, constructing the concrete adapters, invoking that one use case, and
//! translating its outcome into stderr and an exit status. The exit codes per
//! subcommand are the specification's section 2, unchanged, because each
//! caller depends on them.
//!
//! It is responsible for no domain policy, state codec, filesystem
//! transaction, request construction, payload normalization, output
//! composition, scheduling or delivery. Those are the things a composition
//! root historically accretes, so they are named here as the things this
//! crate may not grow.
//!
//! The `posture` binary target lives here from the first pull request, so
//! every later cutover changes one plist argument and nothing else. Until a
//! subcommand lands, the binary refuses every word with usage and exit 2.
