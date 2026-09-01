//! `uu schedule render`: the launchd job for the configured day and time.
//!
//! TWO SCHEDULE TRUTHS, and this module serves the standalone one. A machine
//! whose plist is managed elsewhere takes its timing from that plist; this
//! renders one for a machine that has none, so `uu schedule render > ~/Library/
//! LaunchAgents/<label>.plist` is the whole install.

use crate::config::Schedule;

/// The launchd label the rendered job carries.
pub const DEFAULT_LABEL: &str = "com.webdavis.uu";

/// One launchd job, as a property list.
///
/// EVERY INTERPOLATED VALUE IS XML-ESCAPED. A home directory may legitimately
/// hold `&`, and an unescaped one renders a plist launchd refuses to parse at
/// all, which is a job that silently never loads.
pub fn render_plist(label: &str, program: &str, log_path: &str, schedule: Schedule) -> String {
    let label = escape(label);
    let program = escape(program);
    let log_path = escape(log_path);
    let Schedule {
        weekday,
        hour,
        minute,
    } = schedule;
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n  \
           <key>Label</key>\n  <string>{label}</string>\n  \
           <key>ProgramArguments</key>\n  <array>\n    \
             <string>{program}</string>\n    <string>run</string>\n  </array>\n  \
           <key>RunAtLoad</key>\n  <false/>\n  \
           <key>StartCalendarInterval</key>\n  <dict>\n    \
             <key>Weekday</key>\n    <integer>{weekday}</integer>\n    \
             <key>Hour</key>\n    <integer>{hour}</integer>\n    \
             <key>Minute</key>\n    <integer>{minute}</integer>\n  </dict>\n  \
           <key>StandardOutPath</key>\n  <string>{log_path}</string>\n  \
           <key>StandardErrorPath</key>\n  <string>{log_path}</string>\n\
         </dict>\n</plist>\n"
    )
}

/// XML text-node escaping, the five characters that matter inside an element.
/// The ampersand goes FIRST, or every escape this function just wrote would be
/// escaped again.
fn escape(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Schedule;

    fn rendered(schedule: Schedule) -> String {
        render_plist(
            DEFAULT_LABEL,
            "/home/x/.local/libexec/uu/uu",
            "/home/x/.local/log/uu/uu.log",
            schedule,
        )
    }

    #[test]
    fn the_rendered_job_runs_the_binary_with_the_run_subcommand() {
        let plist = rendered(Schedule::default());
        assert!(
            plist.contains("<string>/home/x/.local/libexec/uu/uu</string>"),
            "{plist}"
        );
        assert!(plist.contains("<string>run</string>"), "{plist}");
    }

    #[test]
    fn the_calendar_interval_carries_the_configured_day_and_time() {
        let plist = rendered(Schedule {
            weekday: 3,
            hour: 7,
            minute: 5,
        });
        assert!(
            plist.contains("<key>Weekday</key>\n    <integer>3</integer>"),
            "{plist}"
        );
        assert!(
            plist.contains("<key>Hour</key>\n    <integer>7</integer>"),
            "{plist}"
        );
        assert!(
            plist.contains("<key>Minute</key>\n    <integer>5</integer>"),
            "{plist}"
        );
    }

    #[test]
    fn the_job_does_not_run_at_load_because_a_login_is_not_a_schedule() {
        assert!(rendered(Schedule::default()).contains("<key>RunAtLoad</key>\n  <false/>"));
    }

    #[test]
    fn the_label_and_both_log_paths_are_the_ones_given() {
        let plist = rendered(Schedule::default());
        assert!(
            plist.contains("<key>Label</key>\n  <string>com.webdavis.uu</string>"),
            "{plist}"
        );
        assert_eq!(
            plist
                .matches("<string>/home/x/.local/log/uu/uu.log</string>")
                .count(),
            2,
            "stdout and stderr both go to the log: {plist}"
        );
    }

    #[test]
    fn a_path_holding_xml_syntax_is_escaped_rather_than_breaking_the_plist() {
        let plist = render_plist(
            "a&b",
            "/home/a&b/uu",
            "/home/a<b>/uu.log",
            Schedule::default(),
        );
        assert!(plist.contains("<string>a&amp;b</string>"), "{plist}");
        assert!(
            plist.contains("<string>/home/a&amp;b/uu</string>"),
            "{plist}"
        );
        assert!(plist.contains("/home/a&lt;b&gt;/uu.log"), "{plist}");
        assert!(!plist.contains("/home/a<b>/"), "{plist}");
    }

    #[test]
    fn every_xml_metacharacter_is_escaped() {
        assert_eq!(escape("a&b<c>d\"e'f"), "a&amp;b&lt;c&gt;d&quot;e&apos;f");
        assert_eq!(escape("plain"), "plain");
    }

    #[test]
    fn the_rendered_plist_opens_with_the_declaration_launchd_expects() {
        let plist = rendered(Schedule::default());
        assert!(
            plist.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"),
            "{plist}"
        );
        assert!(
            plist.contains("<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\""),
            "{plist}"
        );
        assert!(plist.ends_with("</plist>\n"), "{plist}");
    }
}
