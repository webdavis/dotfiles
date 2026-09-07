use crate::Workspace;

/// The two most recently focused workspaces; the previous one is the toggle target.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mru {
    pub current: String,
    pub previous: String,
}

/// A missing target is forgotten because focusing an absent identifier can report success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bounce {
    Focus(String),
    DropStale,
    Nothing,
}

pub fn next_mru(mru: &Mru, new_id: &str) -> Option<Mru> {
    if new_id.is_empty() || new_id == mru.current {
        return None;
    }
    let previous = if mru.current.is_empty() {
        mru.previous.clone()
    } else {
        mru.current.clone()
    };
    Some(Mru {
        current: new_id.to_string(),
        previous,
    })
}

pub fn decide_bounce(mru: &Mru, workspaces: &[Workspace]) -> Bounce {
    if mru.previous.is_empty() {
        return Bounce::Nothing;
    }
    if workspaces
        .iter()
        .any(|workspace| workspace.workspace_id == mru.previous)
    {
        Bounce::Focus(mru.previous.clone())
    } else {
        Bounce::DropStale
    }
}

#[cfg(test)]
mod tests;
