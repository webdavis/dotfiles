//! A scripted process table, so no adapter test walks the live machine.

use crate::ProcessLookup;
use posture_application::InspectionFailure;
use std::collections::VecDeque;

pub(crate) struct ScriptedProcesses {
    answers: VecDeque<Result<Vec<i32>, InspectionFailure>>,
    pub(crate) calls: Vec<(String, Option<u32>, Option<u32>)>,
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
    ) -> Result<Vec<i32>, InspectionFailure> {
        self.calls.push((name.to_owned(), uid, parent));
        self.answers
            .pop_front()
            .expect("an undeclared extra process walk ran")
    }
}
