use crate::{IntegrityVerdict, KnownGood, KnownGoodTuple};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Regular,
    Symlink,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rehash {
    Immediate,
    AfterRenameDelay,
}

pub fn integrity_verdict(
    tracked: bool,
    verb: &str,
    event_hash: &str,
    kind: FileKind,
    current_vouch: impl FnOnce(Rehash) -> bool,
) -> IntegrityVerdict {
    if !tracked {
        return IntegrityVerdict::LogOnly;
    }
    if verb == "DELETED" || kind != FileKind::Regular {
        return IntegrityVerdict::Page;
    }
    let rehash = if event_hash.is_empty() {
        Rehash::AfterRenameDelay
    } else {
        Rehash::Immediate
    };
    if current_vouch(rehash) {
        IntegrityVerdict::LogOnly
    } else {
        IntegrityVerdict::Page
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployedState<'a> {
    Regular(KnownGoodTuple<'a>),
    Unreadable,
    Symlink,
    Other,
}

pub fn deployed_state_known_good(known_good: KnownGood<'_>, state: DeployedState<'_>) -> bool {
    match state {
        DeployedState::Regular(observed) => known_good.vouches(observed),
        DeployedState::Unreadable | DeployedState::Symlink | DeployedState::Other => false,
    }
}

#[cfg(test)]
mod tests;
