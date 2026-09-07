use std::fmt;

/// Why a jump could not be completed.
///
/// These are distinguished rather than collapsed into one string because the
/// composition root reports both failed attempts without losing their causes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JumpError {
    /// herdr could not be reached at all: no socket, or no CLI binary.
    Unreachable(String),
    /// A read or write failed after connecting, or the deadline expired.
    Transport(String),
    /// herdr answered with something this plugin cannot read.
    Malformed(String),
    /// herdr answered with an error envelope.
    Server { code: String, message: String },
    /// Both the socket and the CLI fallback failed. Rendered rather than
    /// nested, so the combined message keeps one prefix instead of stacking
    /// the wrapper's on top of the wrapped one's.
    BothPathsFailed { cli: String, socket: String },
}

impl fmt::Display for JumpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreachable(detail) => write!(f, "herdr unreachable: {detail}"),
            Self::Transport(detail) => write!(f, "herdr transport failed: {detail}"),
            Self::Malformed(detail) => write!(f, "unreadable herdr response: {detail}"),
            Self::Server { code, message } => {
                write!(f, "herdr refused the request: {code}: {message}")
            }
            Self::BothPathsFailed { cli, socket } => {
                write!(f, "{cli} (the socket was tried first: {socket})")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_renders_on_one_line() {
        for error in [
            JumpError::Unreachable("no such file".to_string()),
            JumpError::Transport("timed out".to_string()),
            JumpError::Malformed("not JSON".to_string()),
            JumpError::Server {
                code: "not_found".to_string(),
                message: "gone".to_string(),
            },
            JumpError::BothPathsFailed {
                cli: "herdr unreachable: no binary".to_string(),
                socket: "herdr unreachable: no socket".to_string(),
            },
        ] {
            let rendered = error.to_string();
            assert!(!rendered.contains('\n'), "multi-line: {rendered}");
            assert!(!rendered.is_empty());
        }
    }
}
