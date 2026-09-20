//! Where the operator was, as one answer.
//!
//! ONE VALUE FOR BOTH READERS, the `pns tap` shape: the printed page and the
//! `--json` object are rendered from this same struct, so a field the page
//! shows and a field the JSON carries cannot drift apart.
//!
//! EVERY FIELD IS NAMED THE WAY THE PAGE NAMES IT, and an unknown is the
//! empty string rather than a guess: herdr that did not answer, a machine
//! with nothing waiting, and a shell that has timed nothing all read as
//! "not known" to whoever gets this.

use crate::envelope::{Rejected, encode};
use crate::identifiers::SchemaId;
use serde::Serialize;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct ResumePage {
    /// The herdr workspace this session is showing.
    pub workspace: String,
    /// Whether any session is waiting on the operator right now.
    pub waiting: bool,
    /// What the waiting session was asked to do.
    pub title: String,
    /// The branch that session is on.
    pub branch: String,
    /// The checkout that branch is in.
    pub worktree: String,
    /// The newest command the shell notifier timed.
    pub command: String,
}

impl ResumePage {
    pub fn encode(&self) -> Result<String, Rejected> {
        #[derive(Serialize)]
        struct Wire<'a> {
            schema: &'static str,
            #[serde(flatten)]
            page: &'a ResumePage,
        }
        encode(
            &Wire {
                schema: "pns.resume/1",
                page: self,
            },
            &SchemaId {
                name: "pns.resume".into(),
                major: 1,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_json_carries_every_field_under_the_name_the_page_uses() {
        let encoded = ResumePage {
            workspace: "dotfiles".into(),
            waiting: true,
            title: "ship the resume page".into(),
            branch: "feat/resume".into(),
            worktree: "/tmp/feat-resume".into(),
            command: "cargo".into(),
        }
        .encode()
        .expect("the page encodes");
        for field in [
            r#""schema":"pns.resume/1""#,
            r#""workspace":"dotfiles""#,
            r#""waiting":true"#,
            r#""title":"ship the resume page""#,
            r#""branch":"feat/resume""#,
            r#""worktree":"/tmp/feat-resume""#,
            r#""command":"cargo""#,
        ] {
            assert!(encoded.contains(field), "{field} missing from {encoded}");
        }
    }

    #[test]
    fn a_page_that_knows_nothing_still_encodes_every_field() {
        let encoded = ResumePage::default().encode().expect("the empty page");
        assert!(encoded.contains(r#""waiting":false"#), "{encoded}");
        assert!(encoded.contains(r#""workspace":"""#), "{encoded}");
    }
}
