use posture_application::{AuditObservation, WatchdogIntegrity};
use posture_domain::{
    AuditBounds, AuditFinding, AuditFingerprint, AuditRefusal, AuditReport, KnownGoodTuple,
    ManifestAuthority, ManifestDigest, audit_file, audit_fingerprint_input,
};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};
mod file;
mod manifest;
use manifest::ManifestLines;

/// The largest pns binary this audit will hash rather than call oversize.
///
/// About twice the size measured on 2026-09-13 (6,966,304 bytes), rounded up to
/// a whole mebibyte. The general audit ceiling defaults to 8 MiB, which pns
/// grows past on an ordinary dependency bump, and an oversize verdict on the
/// engine's own binary pages on every tick until somebody notices. The ceiling
/// is a floor under whatever the operator configured, never a cap on it.
///
/// Two shell sites in the repository that deploys this tool carry the same
/// number by hand and must move with it: `rust_tools.max_artifact_bytes.pns`
/// in `.chezmoidata/rust_tools.yaml`, which the builder reads at render time,
/// and the `max_artifact_bytes` table in
/// `.chezmoiscripts/run_after_41-sudo-osquery-known-good-manifests.sh`. Nothing here
/// reads either one: this is a tool other people install.
const PNS_MAX_BYTES: u64 = 14_680_064;

pub struct WatchdogAudit {
    pub pipeline: PathBuf,
    pub managed_bin: PathBuf,
    pub pns: PathBuf,
    pub authority: [ManifestAuthority; 2],
    pub bounds: AuditBounds,
}
impl WatchdogAudit {
    /// The size ceiling to judge one manifest row against. pns keeps its own,
    /// because the shared one is smaller than the binary it would judge.
    fn bounds_for(&self, path: &Path) -> AuditBounds {
        AuditBounds {
            bytes: if path == self.pns {
                self.bounds.bytes.max(PNS_MAX_BYTES)
            } else {
                self.bounds.bytes
            },
            ..self.bounds
        }
    }
    fn scan(&self, scanned: &mut Scanned) -> Result<(), AuditRefusal> {
        let start = Instant::now();
        for (index, (path, authority)) in [&self.pipeline, &self.managed_bin]
            .into_iter()
            .zip(self.authority)
            .enumerate()
        {
            let governing = index == 0;
            let mut lines = ManifestLines::open(path, authority)?;
            while let Some(line) = lines.next(self.bounds, start)? {
                let tuple = KnownGoodTuple::parse_line(&line).ok_or(AuditRefusal::Malformed)?;
                let bounds = self.bounds_for(Path::new(tuple.path));
                let observed = file::observe(
                    Path::new(tuple.path),
                    bounds,
                    start,
                    matches!(tuple.digest, ManifestDigest::Built(_)),
                )?;
                let kinds = audit_file(tuple, observed.borrowed(), bounds.bytes);
                // The pns verdict is this row's verdict. Only the pipeline
                // manifest may supply it, and only one row of it may: a second
                // row naming the same binary is a manifest that says two things.
                if governing && Path::new(tuple.path) == self.pns {
                    scanned.pns_rows += 1;
                    scanned.pns_clean = kinds.is_empty();
                }
                let findings = kinds
                    .into_iter()
                    .map(|kind| AuditFinding {
                        kind,
                        path: tuple.path,
                    })
                    .collect();
                scanned.report.push_str(
                    &AuditReport {
                        findings,
                        refusal: None,
                    }
                    .text(),
                );
            }
        }
        Ok(())
    }
}
/// What one pass over both manifests observed.
#[derive(Default)]
struct Scanned {
    report: String,
    pns_rows: usize,
    pns_clean: bool,
}
impl WatchdogIntegrity for WatchdogAudit {
    fn pipeline(&mut self) -> AuditObservation {
        let mut scanned = Scanned::default();
        let outcome = self.scan(&mut scanned);
        let mut report = scanned.report;
        if let Err(reason) = outcome {
            report.push_str(
                &AuditReport {
                    findings: vec![],
                    refusal: Some(reason),
                }
                .text(),
            );
        }
        let fingerprint = if report.is_empty() {
            None
        } else {
            AuditFingerprint::parse(&file::hex(&Sha256::digest(
                audit_fingerprint_input(&report).as_bytes(),
            )))
        };
        // A scan that did not finish never vouches for anything: the pns row
        // may sit past the line the refusal stopped on.
        let vouched = outcome.is_ok() && scanned.pns_rows == 1 && scanned.pns_clean;
        AuditObservation {
            completed: outcome.is_ok(),
            report,
            fingerprint,
            pns_problem: (!vouched).then(|| "pns binary integrity could not be verified against its authorized build tuple; do not trust its delivery acknowledgements".into()),
        }
    }
}
#[cfg(test)]
mod tests;
