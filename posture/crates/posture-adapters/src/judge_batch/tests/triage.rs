use super::*;

#[test]
fn only_integrity_pages_collect_triage_and_missing_facts_cannot_drop_a_page() {
    for provides_facts in [false, true] {
        let world = World::new();
        let mut calls = Vec::new();
        let mut vouches = |path: &str| path == "/known";
        let mut inspect = |_: &str| None;
        let mut triage = |row: &ResultsRow, _: &mut dyn std::io::Write| {
            calls.push(row.column("target_path").to_string());
            provides_facts.then(|| OwnedTriage {
                recorded: "recorded-fact".into(),
                ondisk: "disk-fact".into(),
                upgrade: "upgrade-fact".into(),
            })
        };
        let records = [
            row(
                "file_events_recent",
                serde_json::json!({"category":"other", "target_path":"/ignored"}),
            ),
            row(
                "file_events_recent",
                serde_json::json!({"category":"sudoers", "target_path":"/digest"}),
            ),
            row(
                "file_events_recent",
                serde_json::json!({"category":"managed_bin", "target_path":"/known"}),
            ),
            row(
                "file_events_recent",
                serde_json::json!({"category":"managed_bin", "target_path":"/changed"}),
            ),
            row(
                "file_events_recent",
                serde_json::json!({"category":"sshd_config", "target_path":"/etc/ssh/sshd_config"}),
            ),
            row("new_admin_user", serde_json::json!({"username":"mallory"})),
        ]
        .join("\n");
        let mut diagnostics = std::io::sink();
        let page = BatchJudge {
            home: HOME,
            allowlist_path: "/unused",
            allowlist: None,
            spool: &world.spool,
            now: "1970-01-01T00:00:00Z",
            agents: &posture_domain::AgentLabels::default(),
            diagnostics: &mut diagnostics,
            collaborators: Collaborators {
                vouches: &mut vouches,
                inspect: &mut inspect,
                triage: &mut triage,
            },
        }
        .judge(&records)
        .page
        .expect("all three real pages survive");
        assert_eq!(calls, ["/changed"]);
        assert!(page.title.ends_with("· 3"));
        assert_eq!(
            page.body.matches("recorded-fact").count(),
            usize::from(provides_facts)
        );
    }
}
