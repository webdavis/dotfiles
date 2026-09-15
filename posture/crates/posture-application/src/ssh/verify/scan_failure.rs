use crate::SshScanFailure;
use posture_domain::{IncludeRefusal, SshTreeRefusal};

pub(super) fn describe(failure: SshScanFailure) -> String {
    let message = match failure {
        SshScanFailure::Directive { path, judgment } => format!(
            "'{}' sets '{} {}' inside a Match block (want '{}'); a Match-scoped re-enable bypasses the global check",
            String::from_utf8_lossy(&path),
            judgment.keyword,
            String::from_utf8_lossy(judgment.actual.as_deref().unwrap_or_default()),
            judgment.required
        ),
        SshScanFailure::Include { path, reason } => {
            let reason = match reason {
                IncludeRefusal::MissingPath => "has an Include with no path".into(),
                IncludeRefusal::CaretBracket(pattern) => format!(
                    "has an Include pattern ('{}') whose bracket begins with '^'; refusing the ambiguous pattern",
                    String::from_utf8_lossy(&pattern)
                ),
            };
            format!("'{}' {reason}", String::from_utf8_lossy(&path))
        }
        SshScanFailure::File(reason) => match reason {
            SshTreeRefusal::Path(path) => format!(
                "path {:?} contains a newline or unit separator",
                String::from_utf8_lossy(&path)
            ),
            SshTreeRefusal::Depth => "a file sits more than 15 Include levels deep".into(),
            SshTreeRefusal::Cycle(path) => format!(
                "Include cycle returning to '{}'",
                String::from_utf8_lossy(&path)
            ),
            SshTreeRefusal::Visits => "the tree exceeds 512 file visits".into(),
            SshTreeRefusal::Bytes => "the tree exceeds 262144 bytes".into(),
            SshTreeRefusal::Unreadable(path) => {
                format!("cannot read '{}'", String::from_utf8_lossy(&path))
            }
            SshTreeRefusal::NonRegular(path) => format!(
                "'{}' is no longer a regular file",
                String::from_utf8_lossy(&path)
            ),
            SshTreeRefusal::Empty => "the configuration tree is empty".into(),
        },
    };
    format!("match scan: {message}; failing closed rather than scanning part of the tree")
}
