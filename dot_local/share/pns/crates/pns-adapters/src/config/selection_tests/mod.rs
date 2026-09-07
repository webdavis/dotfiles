use crate::config::{parse_config, select_plugins};
use pns_domain::registry::{PluginKind, Registry, RegistryError, Routing, Selection, roster};
fn hermes_routing() -> Routing {
    let PluginKind::Channel(routing) = roster()
        .all()
        .iter()
        .find(|r| r.name == "hermes")
        .unwrap()
        .kind
    else {
        panic!("hermes is a channel")
    };
    routing
}

// --- plugin selection at the composition root ---------------------------

fn selection_names(selection: &Selection) -> Vec<&str> {
    selection.iter().map(|r| r.name).collect()
}

mod fallback;
mod parsed;
