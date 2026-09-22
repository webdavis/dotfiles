//! One line per installed package the config's `declared` roster does not
//! name, said once for both global-package lanes.
//!
//! NOTHING IS EVER REMOVED. The roster is the report's reference: a package
//! absent from it becomes a line the operator reads, and declaring it or
//! leaving it installed are both answers. So these are NOTED rather than
//! pending: a pending line would escalate week after week over work that was
//! never asked for.

use uu_domain::LaneReport;

/// Note every installed name the roster does not declare, in the order the
/// listing gave them.
pub(crate) fn note_undeclared<'a>(
    report: &mut LaneReport,
    installed: impl IntoIterator<Item = (&'a str, &'a str)>,
    declared: &[String],
) {
    for (name, version) in installed {
        if !declared.iter().any(|stated| stated == name) {
            report.noted(format!("undeclared: {name} {version}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_what_the_roster_does_not_name_leaves_a_line() {
        let mut report = LaneReport::new("globals");
        note_undeclared(
            &mut report,
            [("acpx", "0.18.0"), ("stray", "1.2.3")],
            &["acpx".to_string()],
        );
        assert_eq!(report.lines, vec!["undeclared: stray 1.2.3"]);
        assert_eq!(report.failures(), 0);
    }

    #[test]
    fn a_fully_declared_machine_leaves_nothing_behind() {
        let mut report = LaneReport::new("globals");
        note_undeclared(
            &mut report,
            [("acpx", "0.18.0")],
            &["acpx".to_string(), "unused".to_string()],
        );
        assert!(report.lines.is_empty(), "{:?}", report.lines);
    }
}
