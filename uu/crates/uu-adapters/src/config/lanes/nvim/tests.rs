use super::*;
use crate::config::probes::{checked_text, refusal, typed};

#[test]
fn an_nvim_lane_without_a_config_key_is_refused_because_nothing_names_the_init() {
    let why = refusal("[lanes.nvim-plugins]\n");
    assert!(why.contains("config") && why.contains("Neovim"), "{why}");
}

#[test]
fn an_nvim_lane_defaults_its_binary_to_nvim_on_the_running_path() {
    let text =
        checked_text("[lanes.editor]\ntype = \"nvim-plugins\"\nconfig = \"/fixture/config\"\n");
    assert_eq!(
        typed::<NvimPluginsLane>(text, "editor"),
        Some(NvimPluginsLane {
            host: NvimHost {
                nvim: "nvim".into(),
                config: "/fixture/config".into()
            },
            auto_commit: false,
            repo: None
        })
    );
}

#[test]
fn an_nvim_lane_config_that_is_not_absolute_is_refused_by_name() {
    for value in ["\"relative\"", "\" \"", "42"] {
        let why = refusal(&format!("[lanes.nvim-plugins]\nconfig = {value}\n"));
        assert!(why.contains("config") && !why.contains("unknown"), "{why}");
    }
}

#[test]
fn a_plugins_lane_with_auto_commit_on_and_no_repo_is_refused_by_name() {
    let why = refusal("[lanes.nvim-plugins]\nconfig = \"/fixture/config\"\nauto_commit = true\n");
    assert!(why.contains("repo") && why.contains("auto_commit"), "{why}");
}

#[test]
fn auto_commit_that_is_not_a_boolean_is_refused_naming_what_was_written() {
    for value in ["42", "\"yes\"", "[]"] {
        let why = refusal(&format!(
            "[lanes.nvim-plugins]\nconfig = \"/fixture/config\"\nauto_commit = {value}\n"
        ));
        assert!(why.contains("auto_commit") && why.contains(value), "{why}");
    }
}

#[test]
fn a_smoke_test_lane_without_a_cache_key_is_refused_because_nothing_names_the_tree() {
    let why = refusal("[lanes.nvim-smoke-test]\nconfig = \"/fixture/config\"\n");
    assert!(why.contains("cache") && !why.contains("unknown"), "{why}");
}

#[test]
fn a_smoke_test_cache_must_be_an_absolute_directory() {
    for value in ["\"relative\"", "\" \"", "42"] {
        let why = refusal(&format!(
            "[lanes.nvim-smoke-test]\nconfig = \"/fixture/config\"\ncache = {value}\n"
        ));
        assert!(why.contains("cache") && !why.contains("unknown"), "{why}");
    }
}
