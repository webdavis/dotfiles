//! The composition root: argv in, exit code out.
//!
//! This crate is responsible for decoding a command line into a request,
//! adapting stdin and stdout, constructing the concrete adapters and
//! registering them, invoking one use case, and translating its outcome into
//! operator-facing output and an exit status.
//!
//! It is responsible for no domain policy, state codec, filesystem
//! transaction, HTTP request, hook payload normalization, recap composition,
//! lighting algorithm or delivery implementation. Those are the things a
//! composition root historically accretes, so they are named here as the
//! things this file may not grow.
//!
//! Nothing has moved in yet, and the `pns` binary target still lives in the
//! legacy package at the workspace root: every caller invokes it by that name,
//! so it may not disappear between two pull requests.
