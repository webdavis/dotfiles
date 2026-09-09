//! herdr's `pane process-info` answer, as the one question this plugin asks of
//! it.
//!
//! THE PARSE IS TOTAL AND FAILS TO `false`. Every way this can go wrong, a
//! herdr that did not run, one that printed nothing, one that printed something
//! else, a pane that no longer exists, means the same thing here: nothing
//! proves Neovim is in that pane. The cost of being wrong that way is a pane
//! move where a split move was wanted, which the operator undoes with the
//! opposite press; the cost of guessing `true` is a chord swallowed by whatever
//! is actually running there.

/// The process name that means the operator is inside an editor whose own
/// splits should move first.
const EDITOR: &str = "nvim";

/// Whether herdr's answer says the editor is in the pane's foreground.
pub fn is_nvim_foreground(process_info_json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(process_info_json)
        .ok()
        .and_then(|answer| {
            answer["result"]["process_info"]["foreground_processes"]
                .as_array()
                .map(|processes| {
                    processes
                        .iter()
                        .any(|process| process["name"].as_str() == Some(EDITOR))
                })
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The editor need not be alone in the pane: a shell that launched it is
    /// still in the same list.
    #[test]
    fn the_editor_is_found_beside_whatever_launched_it() {
        assert!(is_nvim_foreground(
            r#"{"result":{"process_info":{"foreground_processes":[{"name":"bash"},{"name":"nvim"}]}}}"#
        ));
    }

    /// A pane running anything else moves as a pane.
    #[test]
    fn a_pane_without_the_editor_is_not_the_editor() {
        assert!(!is_nvim_foreground(
            r#"{"result":{"process_info":{"foreground_processes":[{"name":"bash"}]}}}"#
        ));
    }

    /// EVERY WAY THIS CAN GO WRONG IS THE SAME ANSWER. A herdr that printed
    /// nothing, one that printed something else, and one whose shape changed
    /// all mean the same thing: nothing proves the editor is there.
    #[test]
    fn every_answer_that_proves_nothing_reads_as_not_the_editor() {
        for answer in [
            "",
            "not json",
            "{}",
            r#"{"result":{}}"#,
            r#"{"result":{"process_info":{"foreground_processes":"nvim"}}}"#,
            r#"{"result":{"process_info":{"foreground_processes":[{"name":42}]}}}"#,
            r#"{"error":{"message":"no such pane"}}"#,
        ] {
            assert!(!is_nvim_foreground(answer), "{answer:?} read as the editor");
        }
    }
}
