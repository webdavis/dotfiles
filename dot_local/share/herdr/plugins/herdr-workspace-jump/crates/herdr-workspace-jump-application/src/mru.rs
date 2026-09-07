use herdr_workspace_jump_domain::{Bounce, Mru, decide_bounce, next_mru};

use crate::{JumpError, WorkspaceDirectory};

// History is best effort: unreadable state is cold, and recording cannot fail a focus event.
pub trait WorkspaceHistory {
    fn read(&self) -> Mru;
    fn write(&mut self, mru: &Mru);
}

pub fn record(history: &mut impl WorkspaceHistory, new_id: &str) {
    if let Some(next) = next_mru(&history.read(), new_id) {
        history.write(&next);
    }
}

pub fn bounce(
    directory: &mut impl WorkspaceDirectory,
    history: &mut impl WorkspaceHistory,
    mru: &Mru,
) -> Result<Bounce, JumpError> {
    if mru.previous.is_empty() {
        return Ok(Bounce::Nothing);
    }
    let outcome = decide_bounce(mru, &directory.list()?);
    match &outcome {
        Bounce::Focus(workspace_id) => directory.focus(workspace_id)?,
        Bounce::DropStale => history.write(&Mru {
            current: mru.current.clone(),
            previous: String::new(),
        }),
        Bounce::Nothing => {}
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests;
