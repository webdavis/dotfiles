//! The harness on the other side of a hook.

/// The payload a harness wrote to this process's stdin, WHOLE or not at all.
///
/// `None` COVERS EVERY WAY OF NOT HAVING ONE: nothing arrived, it did not
/// arrive in time, it was not UTF-8, or it ran past the cap. No caller acts
/// differently on them, and the hook path answers 0 and does nothing in every
/// case. A payload that hit the cap is refused rather than truncated, because
/// half a document parses into fields nobody wrote. Statements: S047, S048.
pub trait HarnessPayload {
    fn read(&self) -> Option<String>;
}
