use super::*;
use crate::config::Schedule;

fn rendered(schedule: Schedule) -> String {
    render_plist(DEFAULT_LABEL, "/home/x", schedule)
}

#[test]
fn the_rendered_job_runs_the_binary_with_the_run_subcommand() {
    let plist = rendered(Schedule::default());
    assert!(
        plist.contains("<string>/home/x/.cargo/bin/uu</string>"),
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
        plist
            .contains("<key>PATH</key>\n    <string>/home/x/.local/share/fnm/aliases/default/bin:"),
        "{plist}"
    );
    assert!(plist.contains(":/usr/bin:/bin:"), "{plist}");
}

#[test]
fn the_rendered_job_names_the_log_owned_by_uu() {
    let plist = rendered(Schedule::default());
    assert!(plist.contains("/home/x/.local/log/uu/uu.log"), "{plist}");
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
        plist.matches("<string>/dev/null</string>").count(),
        2,
        "launchd output is not duplicated in the run log: {plist}"
    );
    assert!(
        plist.contains("/home/x/.local/log/uu/uu.log"),
        "uu owns the run log: {plist}"
    );
}

#[test]
fn a_path_holding_xml_syntax_is_escaped_rather_than_breaking_the_plist() {
    let plist = render_plist("a&b", "/home/a<b>&c", Schedule::default());
    assert!(plist.contains("<string>a&amp;b</string>"), "{plist}");
    assert!(
        plist.contains("<string>/home/a&lt;b&gt;&amp;c/.cargo/bin/uu</string>"),
        "{plist}"
    );
    assert!(
        plist.contains("/home/a&lt;b&gt;&amp;c/.local/log/uu/uu.log"),
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

mod runtime;
