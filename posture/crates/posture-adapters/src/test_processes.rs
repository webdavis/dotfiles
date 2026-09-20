//! A scripted process table, so no adapter test walks the live machine.

use crate::ProcessLookup;
use posture_application::InspectionFailure;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// One recorded walk: the name, the real user, the parent and the directory
/// it was asked for.
pub(crate) type Walk = (String, Option<u32>, Option<u32>, Option<PathBuf>);

pub(crate) struct ScriptedProcesses {
    answers: VecDeque<Result<Vec<i32>, InspectionFailure>>,
    pub(crate) calls: Vec<Walk>,
}

impl ScriptedProcesses {
    pub(crate) fn new(
        answers: impl IntoIterator<Item = Result<Vec<i32>, InspectionFailure>>,
    ) -> Self {
        Self {
            answers: answers.into_iter().collect(),
            calls: Vec::new(),
        }
    }
}

impl ProcessLookup for ScriptedProcesses {
    fn matching(
        &mut self,
        name: &str,
        uid: Option<u32>,
        parent: Option<u32>,
        directory: Option<&Path>,
    ) -> Result<Vec<i32>, InspectionFailure> {
        self.calls.push((
            name.to_owned(),
            uid,
            parent,
            directory.map(Path::to_path_buf),
        ));
        self.answers
            .pop_front()
            .expect("an undeclared extra process walk ran")
    }
}
