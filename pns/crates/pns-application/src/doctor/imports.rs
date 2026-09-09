#[derive(Debug, PartialEq, Eq)]
pub struct ImportFailure {
    pub record: String,
    pub reason: String,
}

pub(super) fn lines(reading: Result<Vec<ImportFailure>, String>) -> Vec<String> {
    match reading {
        Ok(failures) => failures
            .into_iter()
            .map(|failure| {
                format!(
                    "pns doctor: state import {}: {}; legacy file retained.",
                    failure.record, failure.reason,
                )
            })
            .collect(),
        Err(reason) => vec![format!(
            "pns doctor: state import status could not be read ({reason})."
        )],
    }
}
