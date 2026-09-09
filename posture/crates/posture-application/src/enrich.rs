use posture_domain::{CodeTrust, Enrichment, classify_signing, is_interpreter};
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectionFailure {
    Unavailable,
    Failed,
    TimedOut,
}

pub trait EnrichmentInspection {
    fn plist_value(&mut self, path: &Path, key: &str) -> Result<Vec<u8>, InspectionFailure>;
    fn signing(&mut self, path: &Path) -> Result<Vec<u8>, InspectionFailure>;
    fn quarantined(&mut self, path: &Path) -> bool;
    fn is_file(&self, path: &Path) -> bool;
    fn is_mach_o(&mut self, path: &Path) -> bool;
    fn metadata(&mut self, path: &Path) -> Option<Vec<u8>>;
}

pub fn enrich(path: &Path, inspection: &mut impl EnrichmentInspection) -> Enrichment {
    let bytes = path.as_os_str().as_bytes();
    if bytes.is_empty() {
        return ordinary(Vec::new());
    }
    if bytes.ends_with(b".plist") {
        return launchd(path, inspection);
    }
    if [
        b".app".as_slice(),
        b".kext",
        b".systemextension",
        b".dext",
        b".appex",
    ]
    .iter()
    .any(|suffix| bytes.ends_with(suffix))
        || inspection.is_mach_o(path)
    {
        return code(path, inspection);
    }
    ordinary(inspection.metadata(path).unwrap_or_default())
}

fn launchd(path: &Path, inspection: &mut impl EnrichmentInspection) -> Enrichment {
    let mut program = inspection.plist_value(path, "Program").unwrap_or_default();
    if program.is_empty() {
        program = inspection
            .plist_value(path, "ProgramArguments.0")
            .unwrap_or_default();
    }
    if program.is_empty() {
        return Enrichment {
            fact: b"launchd job, no program resolved (untrusted)".to_vec(),
            trust: CodeTrust::Untrusted,
        };
    }
    if !is_interpreter(basename(&program)) {
        return code(&PathBuf::from(OsString::from_vec(program)), inspection);
    }
    for index in 1..=5 {
        let argument = inspection
            .plist_value(path, &format!("ProgramArguments.{index}"))
            .unwrap_or_default();
        if !argument.starts_with(b"/") {
            continue;
        }
        let script = PathBuf::from(OsString::from_vec(argument.clone()));
        if !inspection.is_file(&script) {
            continue;
        }
        let mut fact = b"runs script ".to_vec();
        fact.extend_from_slice(basename(&argument));
        fact.extend_from_slice(b" via ");
        fact.extend_from_slice(basename(&program));
        fact.extend_from_slice(b", payload unverified");
        append_quarantine(&mut fact, &script, inspection);
        return ordinary(fact);
    }
    let mut fact = b"runs ".to_vec();
    fact.extend_from_slice(basename(&program));
    fact.extend_from_slice(b" (interpreter), payload unverified");
    ordinary(fact)
}

fn code(path: &Path, inspection: &mut impl EnrichmentInspection) -> Enrichment {
    let reading = inspection.signing(path);
    let mut assessment = classify_signing(reading.as_deref().ok());
    append_quarantine(&mut assessment.fact, path, inspection);
    assessment
}

fn append_quarantine(fact: &mut Vec<u8>, path: &Path, inspection: &mut impl EnrichmentInspection) {
    if inspection.quarantined(path) {
        fact.extend_from_slice(b", downloaded");
    }
}

fn basename(path: &[u8]) -> &[u8] {
    if path.iter().all(|byte| *byte == b'/') {
        return b"/";
    }
    let end = path
        .iter()
        .rposition(|byte| *byte != b'/')
        .map_or(1, |index| index + 1);
    let trimmed = &path[..end];
    trimmed
        .rsplit(|byte| *byte == b'/')
        .next()
        .unwrap_or(trimmed)
}

fn ordinary(fact: Vec<u8>) -> Enrichment {
    Enrichment {
        fact,
        trust: CodeTrust::TrustedOrNotApplicable,
    }
}

#[cfg(test)]
mod tests;
