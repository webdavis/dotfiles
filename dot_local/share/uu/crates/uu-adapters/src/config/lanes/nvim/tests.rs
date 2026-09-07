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
            }
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
