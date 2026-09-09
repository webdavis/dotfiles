//! Bodies captured by running the jq renderer this replaces, asserted byte for
//! byte.
//!
//! These are the acceptance examples the port is measured against. They were
//! produced by feeding the recorded input to the shipped `render_page` and
//! keeping what it printed, so a difference here is a difference in behavior
//! rather than a difference of opinion about the source.

use super::{body, critical};
use crate::{Signing, Triage};

#[test]
fn an_untrusted_verdict_renders_as_the_shell_rendered_it() {
    let mut finding = critical("persistence_launchd");
    finding.enrichment_path = "/tmp/p";
    finding.columns.label = Some("com.x");
    finding.columns.program = Some("/bin/x");
    finding.signing = Some(Signing {
        untrusted: true,
        text: "unsigned",
    });
    assert_eq!(
        body(finding),
        "**New startup item**\n\
         - **What:** `com.x`\n\
         - **Program:** `/bin/x`\n\
         - **Signing:** ⚠️ **unsigned**\n\
         - Did you set this up? If not, it **auto-runs at every login** - likely malware.\n\
         - **Inspect:** `cat -- '/tmp/p'`"
    );
}

#[test]
fn a_hostile_verdict_renders_as_the_shell_rendered_it() {
    let mut finding = critical("persistence_launchd");
    finding.enrichment_path = "/tmp/p";
    finding.columns.label = Some("a");
    finding.columns.program = Some("b");
    finding.signing = Some(Signing {
        untrusted: false,
        text: "we`ird *bold*\nsecond",
    });
    assert_eq!(
        body(finding),
        "**New startup item**\n\
         - **What:** `a`\n\
         - **Program:** `b`\n\
         - **Signing:** weird bold second\n\
         - Did you set this up? If not, it **auto-runs at every login** - likely malware.\n\
         - **Inspect:** `cat -- '/tmp/p'`"
    );
}

#[test]
fn a_sudoers_change_renders_as_the_shell_rendered_it() {
    let mut finding = critical("file_events_recent");
    finding.enrichment_path = "/e";
    finding.columns.target_path = Some("/etc/sudoers");
    finding.columns.category = Some("sudoers");
    finding.columns.action = Some("UP\nDA\tTED");
    assert_eq!(
        body(finding),
        "**sudoers changed**\n\
         - **File:** `/etc/sudoers`\n\
         - **Action:** UP DA TED\n\
         - Did you change this? If not, someone altered who can log in or run as **root**.\n\
         - **Review:** `sudo cat -- '/e'`"
    );
}

#[test]
fn a_secret_file_renders_as_the_shell_rendered_it() {
    let mut finding = critical("agent_secretfile_changed");
    finding.enrichment_path = "/x";
    finding.columns.path = Some("/home/me/.secret");
    assert_eq!(
        body(finding),
        "**Agent secret file changed**\n\
         - **File:** `.secret`\n\
         - Did you rotate this? If not, an attacker may have your alerting or remote-access secret - **investigate now**."
    );
}

#[test]
fn a_quote_breaking_path_renders_as_the_shell_rendered_it() {
    let mut finding = critical("suid_bin_unexpected");
    finding.enrichment_path = "/tmp/a b'; rm -rf /";
    finding.columns.path = Some("/tmp/x");
    finding.columns.username = Some("root");
    assert_eq!(
        body(finding),
        "**New setuid root binary**\n\
         - **Path:** `/tmp/x`\n\
         - **Owner:** `root`\n\
         - Did you create this? If not, it lets a program run as **root** - a backdoor.\n\
         - **Inspect:** `codesign -dv -- '/tmp/a b'\\''; rm -rf /'`"
    );
}

#[test]
fn an_unmapped_query_renders_as_the_shell_rendered_it() {
    let mut finding = critical("some_odd_query");
    finding.columns.username = Some("bob");
    assert_eq!(body(finding), "**some odd query**\n- **What:** `bob`");
}

#[test]
fn a_triaged_ssh_change_renders_as_the_shell_rendered_it() {
    let mut finding = critical("file_events_recent");
    finding.enrichment_path = "/e";
    finding.columns.target_path = Some("/a/b");
    finding.columns.category = Some("ssh");
    finding.columns.action = Some("X");
    finding.triage = Some(Triage {
        recorded: "aa",
        ondisk: "bb",
        upgrade: "brew 1.2",
    });
    assert_eq!(
        body(finding),
        "**SSH key file changed**\n\
         - **File:** `/a/b`\n\
         - **Action:** X\n\
         - **Recorded:** `aa` · **On disk:** `bb`\n\
         - **Upgrade record:** `brew 1.2`\n\
         - Did you change this? If not, someone altered who can log in or run as **root**.\n\
         - **Review:** `sudo cat -- '/e'`"
    );
}

#[test]
fn an_over_long_field_and_an_empty_path_render_as_the_shell_rendered_them() {
    let long = "y".repeat(250);
    let mut finding = critical("persistence_launchd");
    finding.columns.label = Some("L");
    finding.columns.program = Some(&long);
    assert_eq!(
        body(finding),
        format!(
            "**New startup item**\n\
             - **What:** `L`\n\
             - **Program:** `{}…(truncated)`\n\
             - Did you set this up? If not, it **auto-runs at every login** - likely malware.\n\
             - **Inspect:** `cat -- ''`",
            "y".repeat(240)
        )
    );
}
