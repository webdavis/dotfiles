//! The domain registry and the adapter mapping from loaded configuration to selection.

pub use pns_domain::registry::{
    CORE, PRESENCE, PluginKind, ROSTER, Registration, Registry, RegistryError, Routing, Selection,
    roster,
};

pub use pns_adapters::select_plugins;
