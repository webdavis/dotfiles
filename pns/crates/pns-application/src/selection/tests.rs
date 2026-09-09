use crate::selection::{ConfigOutcome, select_plugins};

#[test]
fn an_unreadable_config_names_the_error_and_exactly_the_core_plugins() {
    let (_, warning) = select_plugins(
        &pns_domain::registry::roster(),
        ConfigOutcome::Unreadable("permission denied".to_string()),
    );
    assert_eq!(
        warning.as_deref(),
        Some(
            "pns: config error (permission denied); running the core plugins (mobile, macos-banner)"
        )
    );
}
