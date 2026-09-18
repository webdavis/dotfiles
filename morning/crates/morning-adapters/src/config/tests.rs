use super::*;

const SHIPPED: &str = r#"
timeout_seconds = 6

[apply_log]
path = "~/.local/state/chezmoi-apply/latest.apply.log"

[ledger]
path = ""

[recap]
directory = "~/recaps"

[pull_requests]
command = ["gh", "pr", "list"]

[tasks]
command = []
"#;

fn shipped() -> Config {
    toml::from_str(SHIPPED).unwrap()
}

#[test]
fn a_tilde_path_resolves_against_the_home_it_is_given() {
    let home = Path::new("/Users/someone");
    assert_eq!(
        shipped().apply_log.unwrap().resolve(home).unwrap(),
        Path::new("/Users/someone/.local/state/chezmoi-apply/latest.apply.log")
    );
}

#[test]
fn an_empty_path_is_a_source_that_is_not_configured() {
    assert_eq!(shipped().ledger.unwrap().resolve(Path::new("/home")), None);
}

#[test]
fn an_empty_command_is_a_source_that_is_not_configured() {
    assert_eq!(shipped().tasks.unwrap().resolve(), None);
}

#[test]
fn the_ledger_carries_its_default_phrases_when_the_config_names_none() {
    let ledger = shipped().ledger.unwrap();
    assert_eq!(ledger.apply_markers, vec!["chezmoi apply".to_string()]);
    assert_eq!(ledger.operator_markers, vec!["operator".to_string()]);
}

#[test]
fn the_page_shows_eight_rows_a_section_unless_the_config_says_otherwise() {
    assert_eq!(Config::default().rows_per_section(), 8);
    let config: Config = toml::from_str("rows_per_section = 3\n").unwrap();
    assert_eq!(config.rows_per_section(), 3);
}

#[test]
fn the_timeout_is_the_configured_one_and_otherwise_eight_seconds() {
    assert_eq!(shipped().timeout(), Duration::from_secs(6));
    assert_eq!(Config::default().timeout(), Duration::from_secs(8));
}

#[test]
fn an_absent_config_file_is_a_config_with_no_source_in_it() {
    let absent = Path::new("/nonexistent/morning/config.toml");
    assert_eq!(Config::load(absent).unwrap(), Config::default());
}

#[test]
fn a_config_that_is_not_toml_names_the_file_and_the_complaint() {
    let path = std::env::temp_dir().join("morning-broken-config.toml");
    std::fs::write(&path, "timeout_seconds = [\n").unwrap();
    let error = Config::load(&path).unwrap_err();
    std::fs::remove_file(&path).ok();
    assert!(error.contains("morning-broken-config.toml"), "{error}");
}
