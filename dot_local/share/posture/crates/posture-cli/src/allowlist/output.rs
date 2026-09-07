use posture_application::{
    CaptureRefusal, CurationFailure, CurationOutcome, PublicationRefusal, SourceRefusal,
};
use std::io::{self, Write};
use std::path::Path;

pub(super) fn success(outcome: CurationOutcome, output: &mut impl Write) -> io::Result<()> {
    match outcome {
        CurationOutcome::Allowed { label, program } => {
            writeln!(output, "allowed: {label} -> {program}")
        }
        CurationOutcome::Denied(label) => writeln!(output, "denied: {label}"),
        CurationOutcome::NotPresent(label) => writeln!(output, "not present: {label}"),
        CurationOutcome::Listed(bytes) => output.write_all(&bytes),
    }
}
pub(super) fn failure(
    error: CurationFailure,
    deployed: &Path,
    label: &str,
    output: &mut impl Write,
) -> io::Result<()> {
    let target = deployed.display();
    match error {
        CurationFailure::Lock => writeln!(
            output,
            "failed to set up the allowlist write lock ({target}.lock)"
        ),
        CurationFailure::InvalidLabel(label) => {
            writeln!(output, "refused (invalid or system label): {label}")
        }
        CurationFailure::Capture(CaptureRefusal::NoAgent) => writeln!(
            output,
            "refused: {label} has no loaded LaunchAgent to capture an identity from; load it and re-run"
        ),
        CurationFailure::Capture(CaptureRefusal::Hash(path)) => writeln!(
            output,
            "refused: sha256 hash capture failed for {path}; not writing an unpinned tuple"
        ),
        CurationFailure::Source(SourceRefusal::Resolve) => writeln!(
            output,
            "refused: could not resolve the chezmoi source for {target}; the allowlist is chezmoi-managed and must be curated through its source"
        ),
        CurationFailure::Source(SourceRefusal::Read) => {
            writeln!(output, "refused: could not read the allowlist")
        }
        CurationFailure::InvalidLine(source) => writeln!(
            output,
            "refused: the allowlist source holds a line that is not a single JSON tuple; repair {} by hand before curating it",
            source.display()
        ),
        CurationFailure::Publication(error) => publication(error, deployed, output),
    }
}
fn publication(
    error: PublicationRefusal,
    deployed: &Path,
    output: &mut impl Write,
) -> io::Result<()> {
    match error {
        PublicationRefusal::Backup => writeln!(
            output,
            "refused: could not stage a rollback copy of the allowlist source"
        ),
        PublicationRefusal::SourceWrite(source) => writeln!(
            output,
            "refused: could not write the allowlist source at {source}"
        ),
        PublicationRefusal::Apply { rollback_error } => {
            write!(
                output,
                "FAILED: chezmoi apply of {} did not succeed. ",
                deployed.display()
            )?;
            match rollback_error {
                None => write!(output, "The allowlist source has been rolled back. ")?,
                Some(error) => write!(output, "The allowlist source rollback failed: {error}. ")?,
            }
            writeln!(
                output,
                "Deployment may be partial; inspect it before retrying."
            )
        }
        PublicationRefusal::ManifestMissing(path) => writeln!(
            output,
            "FAILED: the allowlist was deployed but the known-good-manifests runner could not be located ({}). The manifest is now STALE: until it is refreshed the deployed allowlist is unbound and every user LaunchAgent will page. Run a full chezmoi apply to refresh it.",
            path.map_or_else(|| "unresolved".into(), |path| path.display().to_string())
        ),
        PublicationRefusal::ManifestRefresh(path) => writeln!(
            output,
            "FAILED: the allowlist was deployed but refreshing the known-good manifests failed ({}). The manifest is now STALE: until it is refreshed the deployed allowlist is unbound and every user LaunchAgent will page. Re-run this command, or run a full chezmoi apply.",
            path.display()
        ),
    }
}
