//! Everything concrete, organized by the capability it provides.
//!
//! This crate is responsible for implementing the ports `posture-application`
//! declares, one module per real capability: the results and snapshot logs,
//! the single-record state files published by rename, the two kernel locks,
//! the known-good manifest reader with its trust check, the deployed-state
//! reader that refuses symlinks before hashing, the allowlist and controls
//! codecs, the upgrade-record reader, the bounded process runners for every
//! external tool the roster permits, the pns producer that submits through
//! pns's protocol crate, the last-resort banner for the engine-down case, the
//! privileged install and osqueryctl calls the converge alone may make, the
//! staging tree's symlink walk and private copy, the chezmoi publisher, and
//! the clock.
//!
//! Every spawned child runs under an explicit deadline with process-group
//! termination, through one command runner with a scripted double, so no
//! adapter test runs a real `sudo`, `osqueryctl`, `osqueryi`, `codesign`,
//! `tailscale` or `pns`. Nothing has moved in yet.
