//! `uu schedule render`: the launchd job for the configured day and time.
//!
//! TWO SCHEDULE TRUTHS, and this module serves the standalone one. A machine
//! whose plist is managed elsewhere takes its timing from that plist; this
//! renders one for a machine that has none, so `uu schedule render > ~/Library/
//! LaunchAgents/<label>.plist` is the whole install.

use crate::config::Schedule;
use std::path::{Path, PathBuf};

/// The launchd label the rendered job carries.
pub const DEFAULT_LABEL: &str = "com.webdavis.uu";

/// Where the apply-time build puts the binary launchd runs.
pub fn installed_binary(home: &str) -> PathBuf {
    Path::new(home).join(".local/libexec/uu/uu")
}

/// Where the job's own output goes. The DIRECTORY is the operator's to make;
/// see `render_plist`.
pub fn log_path(home: &str) -> PathBuf {
    Path::new(home).join(".local/log/uu/uu.log")
}

/// What a lane's own child processes are given to search, since launchd hands
/// a job almost no environment at all. It mirrors the tracked plist's list:
/// the operator's own bin directory first, then Homebrew, then the system.
fn search_path(home: &str) -> String {
    format!("{home}/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin")
}

/// One launchd job, as a property list, for a standalone install under `home`.
///
/// THE ENVIRONMENT IS STATED, because launchd gives a job almost none. `uu
/// run` refuses outright without HOME, and a lane's child processes find
/// nothing without PATH, so a plist that omits them renders a job that cannot
/// work. Both mirror the tracked plist this machine loads.
///
/// THE LOG DIRECTORY IS THE OPERATOR'S TO MAKE, and the plist says so in a
/// comment above the paths. launchd creates the log FILE but never its
/// directory, and a job whose output cannot be opened does not start; on this
/// machine the loader script makes it, and a standalone install has no loader.
///
/// EVERY INTERPOLATED VALUE IS XML-ESCAPED. A home directory may legitimately
/// hold `&`, and an unescaped one renders a plist launchd refuses to parse at
/// all, which is a job that silently never loads.
pub fn render_plist(label: &str, home: &str, schedule: Schedule) -> String {
    let label = escape(label);
    let program = escape(&installed_binary(home).display().to_string());
    let log_path = escape(&log_path(home).display().to_string());
    let search_path = escape(&search_path(home));
    let home = escape(home);
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
           <key>EnvironmentVariables</key>\n  <dict>\n    \
             <key>HOME</key>\n    <string>{home}</string>\n    \
             <key>PATH</key>\n    <string>{search_path}</string>\n  </dict>\n  \
           <key>RunAtLoad</key>\n  <false/>\n  \
           <key>StartCalendarInterval</key>\n  <dict>\n    \
             <key>Weekday</key>\n    <integer>{weekday}</integer>\n    \
             <key>Hour</key>\n    <integer>{hour}</integer>\n    \
             <key>Minute</key>\n    <integer>{minute}</integer>\n  </dict>\n  \
           <!-- launchd creates the log file but never its directory, and a \
                job whose output cannot be opened does not start:\n       \
                mkdir -p the directory holding the two paths below before \
                loading this job. -->\n  \
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
        render_plist(DEFAULT_LABEL, "/home/x", schedule)
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
    fn the_rendered_job_carries_the_environment_uu_run_needs() {
        // launchd starts a job with almost no environment: no HOME, so `uu
        // run` refuses before it reads anything, and no PATH, so a lane's own
        // child processes find nothing. The tracked plist beside this one
        // states both, and a rendered job that omits them is a job that never
        // works.
        let plist = rendered(Schedule::default());
        assert!(plist.contains("<key>EnvironmentVariables</key>"), "{plist}");
        assert!(
            plist.contains("<key>HOME</key>\n    <string>/home/x</string>"),
            "{plist}"
        );
        assert!(
            plist.contains("<key>PATH</key>\n    <string>/home/x/.local/bin:"),
            "{plist}"
        );
        assert!(plist.contains(":/usr/bin:/bin:"), "{plist}");
    }

    #[test]
    fn the_rendered_job_says_the_log_directory_has_to_be_made_first() {
        // launchd creates the log FILE and never its directory, and a job
        // whose StandardOutPath cannot be opened does not start. On this
        // machine the loader script makes it; a standalone install has no
        // loader, so the plist itself has to say so.
        let plist = rendered(Schedule::default());
        assert!(plist.contains("mkdir -p"), "{plist}");
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
        let plist = render_plist("a&b", "/home/a<b>&c", Schedule::default());
        assert!(plist.contains("<string>a&amp;b</string>"), "{plist}");
        assert!(
            plist.contains("<string>/home/a&lt;b&gt;&amp;c/.local/libexec/uu/uu</string>"),
            "{plist}"
        );
        assert!(
            plist.contains("<string>/home/a&lt;b&gt;&amp;c/.local/log/uu/uu.log</string>"),
            "{plist}"
        );
        // The environment carries the same home twice more, and an unescaped
        // one there breaks the plist just as thoroughly.
        assert!(
            plist.contains("<key>HOME</key>\n    <string>/home/a&lt;b&gt;&amp;c</string>"),
            "{plist}"
        );
        assert!(!plist.contains("/home/a<b>"), "{plist}");
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
