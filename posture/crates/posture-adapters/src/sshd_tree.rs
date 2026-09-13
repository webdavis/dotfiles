use posture_application::{SshScanFailure, SshTree};
use posture_domain::{
    SshRecord, SshScan, SshTreeRefusal, SshWalkBudget, analyze_include, scan_ssh_line, ssh_roots,
};
use std::{
    collections::BTreeMap,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};
mod reading;
mod resolution;

pub struct SshConfigTree {
    main: PathBuf,
    dropins: PathBuf,
}
impl SshConfigTree {
    pub fn new(main: PathBuf, dropins: PathBuf) -> Self {
        Self { main, dropins }
    }

    fn walk(&self) -> Walk {
        let mut walk = Walk::default();
        let roots = resolution::roots(&self.main, &self.dropins)
            .and_then(|(main, dropins)| ssh_roots(main, Ok(dropins)));
        let roots = match roots {
            Ok(roots) => roots,
            Err(error) => {
                walk.failures.push(SshScanFailure::File(error));
                return walk;
            }
        };
        for root in roots {
            self.visit(&root, false, &[], &mut walk);
        }
        walk
    }

    fn visit(&self, path: &[u8], mut in_match: bool, ancestors: &[Vec<u8>], walk: &mut Walk) {
        if let Err(error) = walk.budget.enter(path, ancestors) {
            walk.fail(SshScanFailure::File(error));
            return;
        }
        let (record, bytes) = match reading::read(path, &mut walk.budget) {
            Ok(reading) => reading,
            Err(error) => {
                walk.fail(SshScanFailure::File(error));
                return;
            }
        };
        walk.records.insert(path.to_vec(), record);
        let mut chain = ancestors.to_vec();
        chain.push(path.to_vec());
        for line in bytes.split(|byte| *byte == b'\n') {
            match scan_ssh_line(line, &mut in_match) {
                SshScan::Include(arguments) => {
                    let directory = self
                        .main
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .unwrap_or(Path::new("."));
                    let patterns =
                        match analyze_include(directory.as_os_str().as_bytes(), &arguments) {
                            Ok(patterns) => patterns,
                            Err(reason) => {
                                walk.fail(SshScanFailure::Include {
                                    path: path.to_vec(),
                                    reason,
                                });
                                continue;
                            }
                        };
                    for pattern in patterns {
                        match resolution::resolve(&pattern) {
                            Ok(paths) => {
                                for path in paths {
                                    self.visit(&path, in_match, &chain, walk);
                                }
                            }
                            Err(error) => walk.fail(SshScanFailure::File(error)),
                        }
                    }
                }
                SshScan::Violation(judgment) => walk.fail(SshScanFailure::Directive {
                    path: path.to_vec(),
                    judgment,
                }),
                SshScan::None => {}
            }
        }
    }
}
impl SshTree for SshConfigTree {
    fn scan(&self) -> Vec<SshScanFailure> {
        self.walk().failures
    }
    fn observe(&self) -> Result<Vec<SshRecord>, SshScanFailure> {
        let walk = self.walk();
        if let Some(error) = walk
            .failures
            .into_iter()
            .find(|error| !matches!(error, SshScanFailure::Directive { .. }))
        {
            return Err(error);
        }
        if walk.records.is_empty() {
            return Err(SshScanFailure::File(SshTreeRefusal::Empty));
        }
        Ok(walk.records.into_values().collect())
    }
}

#[derive(Default)]
struct Walk {
    records: BTreeMap<Vec<u8>, SshRecord>,
    failures: Vec<SshScanFailure>,
    budget: SshWalkBudget,
}
impl Walk {
    fn fail(&mut self, error: SshScanFailure) {
        if !self.failures.contains(&error) {
            self.failures.push(error);
        }
    }
}

#[cfg(test)]
mod tests;
