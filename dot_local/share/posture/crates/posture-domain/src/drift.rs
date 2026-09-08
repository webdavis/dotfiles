#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveAttributes {
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveEntry {
    Absent,
    File(LiveAttributes),
    Directory(LiveAttributes),
    Irregular,
    Unreadable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentComparison {
    Equal,
    Different,
    Unreadable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drift {
    Ok,
    Absent,
    Irregular,
    Unreadable,
    Content,
    Mode,
    Owner,
    Group,
}

pub fn file_drift(entry: LiveEntry, content: ContentComparison) -> Drift {
    let attributes = match entry {
        LiveEntry::Absent => return Drift::Absent,
        LiveEntry::Unreadable => return Drift::Unreadable,
        LiveEntry::File(attributes) => attributes,
        _ => return Drift::Irregular,
    };
    match content {
        ContentComparison::Unreadable => Drift::Unreadable,
        ContentComparison::Different => Drift::Content,
        ContentComparison::Equal => attribute_drift(attributes, 0o644),
    }
}

pub fn directory_drift(entry: LiveEntry) -> Drift {
    match entry {
        LiveEntry::Absent => Drift::Absent,
        LiveEntry::Unreadable => Drift::Unreadable,
        LiveEntry::Directory(attributes) => attribute_drift(attributes, 0o755),
        _ => Drift::Irregular,
    }
}

fn attribute_drift(attributes: LiveAttributes, mode: u32) -> Drift {
    if attributes.mode != mode {
        Drift::Mode
    } else if attributes.uid != 0 {
        Drift::Owner
    } else if attributes.gid != 0 {
        Drift::Group
    } else {
        Drift::Ok
    }
}

pub fn restart_required(verdicts: &[Drift]) -> bool {
    verdicts.iter().any(|verdict| *verdict != Drift::Ok)
}

#[cfg(test)]
mod tests;
