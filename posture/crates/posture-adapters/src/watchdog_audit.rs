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
/// `.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh`. Nothing here
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
    fn scan(&self, report: &mut String) -> Result<(), AuditRefusal> {
        let start = Instant::now();
        for (path, authority) in [&self.pipeline, &self.managed_bin]
            .into_iter()
            .zip(self.authority)
        {
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
                let findings = audit_file(tuple, observed.borrowed(), bounds.bytes)
                    .into_iter()
                    .map(|kind| AuditFinding {
                        kind,
                        path: tuple.path,
                    })
                    .collect();
                report.push_str(
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
    fn pns_matches(&self) -> Result<bool, AuditRefusal> {
        let start = Instant::now();
        let mut lines = ManifestLines::open(&self.pipeline, self.authority[0])?;
        let mut found = None;
        while let Some(line) = lines.next(self.bounds, start)? {
            let tuple = KnownGoodTuple::parse_line(&line).ok_or(AuditRefusal::Malformed)?;
            if Path::new(tuple.path) == self.pns {
                if found.is_some() {
                    return Ok(false);
                }
                found = Some(line);
            }
        }
        let Some(line) = found else {
            return Ok(false);
        };
        let tuple = KnownGoodTuple::parse_line(&line).ok_or(AuditRefusal::Malformed)?;
        let bounds = self.bounds_for(&self.pns);
        let observed = file::observe(
            &self.pns,
            bounds,
            start,
            matches!(tuple.digest, ManifestDigest::Built(_)),
        )?;
        Ok(audit_file(tuple, observed.borrowed(), bounds.bytes).is_empty())
    }
}
impl WatchdogIntegrity for WatchdogAudit {
    fn pipeline(&mut self) -> AuditObservation {
        let mut report = String::new();
        let outcome = self.scan(&mut report);
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
        AuditObservation {
            completed: outcome.is_ok(),
            report,
            fingerprint,
        }
    }
    fn pns_problem(&mut self) -> Option<String> {
        (!matches!(self.pns_matches(), Ok(true))).then(|| "pns binary integrity could not be verified against its authorized build tuple; do not trust its delivery acknowledgements".into())
    }
}
#[cfg(test)]
mod tests;
