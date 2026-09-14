#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshAttributes {
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshRecord {
    pub path: Vec<u8>,
    pub attributes: SshAttributes,
    pub checksum: [u8; 32],
}
#[derive(Debug, PartialEq, Eq)]
pub enum SshTreeChange {
    Disappeared(Vec<u8>),
    Content(Vec<u8>),
    Attributes(Vec<u8>, SshAttributes, SshAttributes),
    Appeared(Vec<u8>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshTreeRefusal {
    Path(Vec<u8>),
    Depth,
    Cycle(Vec<u8>),
    Visits,
    Bytes,
    Unreadable(Vec<u8>),
    NonRegular(Vec<u8>),
    Empty,
}

#[derive(Default)]
pub struct SshWalkBudget {
    visits: usize,
    bytes: usize,
}
impl SshWalkBudget {
    pub fn enter(&mut self, path: &[u8], ancestors: &[Vec<u8>]) -> Result<(), SshTreeRefusal> {
        if path.iter().any(|byte| matches!(byte, b'\n' | 31)) {
            return Err(SshTreeRefusal::Path(path.to_vec()));
        }
        if ancestors.len() > 15 {
            return Err(SshTreeRefusal::Depth);
        }
        if ancestors.iter().any(|ancestor| ancestor == path) {
            return Err(SshTreeRefusal::Cycle(path.to_vec()));
        }
        if self.visits >= 512 {
            return Err(SshTreeRefusal::Visits);
        }
        self.visits += 1;
        Ok(())
    }
    pub fn read(&mut self, bytes: usize) -> Result<(), SshTreeRefusal> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|total| *total <= 262144)
            .ok_or(SshTreeRefusal::Bytes)?;
        Ok(())
    }
}
pub fn ssh_roots(
    main: Option<Vec<u8>>,
    dropins: Result<Vec<Vec<u8>>, SshTreeRefusal>,
) -> Result<Vec<Vec<u8>>, SshTreeRefusal> {
    let mut roots = dropins?;
    roots.extend(main);
    roots.sort();
    roots.dedup();
    Ok(roots)
}
pub fn compare_ssh_trees(before: &[SshRecord], after: &[SshRecord]) -> Vec<SshTreeChange> {
    let mut changes = Vec::new();
    for old in before {
        let Some(new) = after.iter().find(|new| new.path == old.path) else {
            changes.push(SshTreeChange::Disappeared(old.path.clone()));
            continue;
        };
        if old.checksum != new.checksum {
            changes.push(SshTreeChange::Content(old.path.clone()));
        }
        if old.attributes != new.attributes {
            changes.push(SshTreeChange::Attributes(
                old.path.clone(),
                old.attributes.clone(),
                new.attributes.clone(),
            ));
        }
    }
    for new in after {
        if !before.iter().any(|old| old.path == new.path) {
            changes.push(SshTreeChange::Appeared(new.path.clone()));
        }
    }
    changes
}

#[cfg(test)]
mod tests;
