//! The plugin registry: compiled-in plugins declare themselves here, and the
//! config selects among them.
//!
//! This is what closes routing's KNOWN LIMIT. A channel no longer appears in
//! core policy by NAME; it registers a name plus a routing DECLARATION (local
//! surface, presence-gated, durable log), and the plan is computed over
//! whatever is registered and enabled. Adding a destination is a registration
//! at the composition root, never an edit to policy.
//!
//! A plugin comes in two KINDS. A channel is a destination; a sensor is an
//! input and carries no routing, so it shares the one config table space and
//! the one name check without being reachable by a delivery leg.
//!
//! Fail directions: registering the same name twice is refused (two plugins
//! answering one config table is a wiring bug, not a preference), and a config
//! that enables a name nothing registered is refused naming it, because a
//! typo'd plugin name that silently no-ops is a notification quietly turned
//! off, the same failure the config layer refuses everywhere else.

use std::collections::BTreeMap;

mod roster;

use roster::REQUIRES;
pub use roster::{CORE, PRESENCE, ROSTER};

/// What a plugin declares about WHERE it delivers. The plan is computed from
/// these three properties and nothing else, which is what keeps policy closed
/// to new names while open to new destinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Routing {
    /// Delivers to this machine's own surfaces (a banner, a light).
    pub local: bool,
    /// The presence verdict may suppress it (the phone leg today).
    pub presence_gated: bool,
    /// The durable log: what remote-only selects, and synchronously, because
    /// an undelivered log entry is invisible in a way an undelivered alert
    /// is not.
    pub durable: bool,
    /// Whether an EVENT dispatches it at all. False for a plugin the binary
    /// serves in its own mode rather than as a leg (hue's pulse today): it
    /// registers so the config can select it and a typo in its name is still
    /// caught, but no notification ever routes to it.
    pub event_dispatched: bool,
}

/// What KIND of plugin this is, which decides whether it can be a delivery
/// leg at all.
///
/// A CHANNEL is a destination and carries the routing that says where. A
/// SENSOR is an input and carries no routing, so "a sensor never becomes a
/// leg" is unrepresentable rather than filtered: there is nothing for the
/// plan to read. That is deliberately a different question from
/// [`Routing::event_dispatched`], which asks whether an event routes to an
/// OUTPUT the binary drives in its own mode (hue's pulse). This asks whether
/// the plugin is an output at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginKind {
    /// A delivery destination, with the declaration of where it delivers.
    Channel(Routing),
    /// An input the engine reads. Never a destination.
    Sensor,
}

/// One registered plugin: its config-table name and its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registration {
    pub name: &'static str,
    pub kind: PluginKind,
}

/// Why registration or selection was refused, always naming the offender.
#[derive(Debug, PartialEq)]
pub enum RegistryError {
    /// Two plugins claimed the same name.
    Duplicate(String),
    /// The config names a plugin nothing registered, enabled or not: the
    /// typo is the defect either way.
    UnknownPlugin(String),
    /// A plugin the config switched on that needs another one it did not.
    /// BOTH ARE NAMED, because the fix is in the other table and an operator
    /// reading only the first name would go and edit the one that is right.
    Unsatisfied { plugin: String, needs: String },
}

/// A vetted selection, and the only value a plan can be computed over. The
/// inner list is private and no constructor is public, so a Selection can
/// only come out of [`Registry::enabled`]: fabricated registrations cannot
/// reach routing without passing the duplicate and unknown-name refusals.
#[derive(Debug, PartialEq)]
pub struct Selection(Vec<Registration>);

impl Selection {
    pub fn iter(&self) -> std::slice::Iter<'_, Registration> {
        self.0.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The ordered set of compiled-in plugins. Registration order is delivery
/// order, so the composition root states the order once and the config cannot
/// scramble it.
#[derive(Debug, Default)]
pub struct Registry {
    registrations: Vec<Registration>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a delivery destination. A name already taken is refused.
    pub fn register_channel(
        &mut self,
        name: &'static str,
        routing: Routing,
    ) -> Result<(), RegistryError> {
        self.register_plugin(name, PluginKind::Channel(routing))
    }

    /// Add an input. It gets a name so the config can select it and so a typo
    /// in that name is still refused, and no routing at all, so no path
    /// reaches it with an event.
    pub fn register_sensor(&mut self, name: &'static str) -> Result<(), RegistryError> {
        self.register_plugin(name, PluginKind::Sensor)
    }

    /// ONE NAMESPACE FOR BOTH KINDS, because both are selected by one
    /// `[plugins.<name>]` table: two plugins answering one table is a wiring
    /// bug whatever kinds they are, and the operator would have no spelling
    /// left to tell them apart.
    fn register_plugin(
        &mut self,
        name: &'static str,
        kind: PluginKind,
    ) -> Result<(), RegistryError> {
        if self.registrations.iter().any(|entry| entry.name == name) {
            return Err(RegistryError::Duplicate(name.to_string()));
        }
        self.registrations.push(Registration { name, kind });
        Ok(())
    }

    /// Every registered name, in registration order.
    pub fn names(&self) -> Vec<&'static str> {
        self.registrations.iter().map(|entry| entry.name).collect()
    }

    /// Every registration, whatever the config says. The census the doctor
    /// reports against, which has to name a plugin the config declined or a
    /// short report reads as a complete one.
    pub fn all(&self) -> Selection {
        Selection(self.registrations.clone())
    }

    /// What runs with NO usable config: the core, in registration order.
    ///
    /// NOT THE WHOLE ROSTER (operator ruling 2026-08-31). Three of the five
    /// plugins cannot do anything until a credential is stood up for them (a
    /// hue bridge and key, a hermes route to sign for, a router API key), so a
    /// default that switched them on delivered nothing and reported three
    /// failures on a machine whose operator had asked for none of it.
    ///
    /// THE TWO LEFT ARE NOT CREDENTIAL-FREE, and the split is not the line it
    /// looks like: the banner needs nothing, and the phone needs a `token` in
    /// the very same file. The phone is core BY RULING rather than by that
    /// test. What it buys is that the leg is PLANNED and ARMS the moment a
    /// token appears, and what it costs is one honest failure line on a
    /// machine that has written no config at all, naming the key to write. The
    /// other three would each cost the same line for a destination the
    /// operator has given no sign of wanting.
    pub fn core(&self) -> Selection {
        Selection(
            self.registrations
                .iter()
                .filter(|entry| CORE.contains(&entry.name))
                .copied()
                .collect(),
        )
    }

    /// The registrations the config enables, in REGISTRATION order whatever
    /// order the config listed them in. A config naming an unregistered
    /// plugin is refused; a registered plugin the config omits or disables
    /// is simply not selected.
    pub fn enabled(&self, switches: &BTreeMap<String, bool>) -> Result<Selection, RegistryError> {
        // The CONFIG's names are walked first, and the enabled flag is not
        // consulted: an unregistered name is a typo whether or not it is
        // switched on, and the next edit turns it into a silent no-op.
        for name in switches.keys() {
            if !self.registrations.iter().any(|entry| entry.name == name) {
                return Err(RegistryError::UnknownPlugin(name.clone()));
            }
        }
        let switched_on = |name: &str| switches.get(name).copied().unwrap_or(false);
        // AND A BORROWED CREDENTIAL IS CHECKED, so a sensor that reads another
        // plugin's bridge and key is refused out loud rather than selected
        // into a reading it can never take.
        for (plugin, needs) in REQUIRES {
            if switched_on(plugin) && !switched_on(needs) {
                return Err(RegistryError::Unsatisfied {
                    plugin: plugin.to_string(),
                    needs: needs.to_string(),
                });
            }
        }
        Ok(Selection(
            self.registrations
                .iter()
                .filter(|entry| switched_on(entry.name))
                .copied()
                .collect(),
        ))
    }
}

/// A registry out of a slice of declarations: the ONE constructor, used by
/// the composition root and by every test that wants the production set.
/// Four fixtures used to reproduce this independently, so a declaration could
/// change in the roster and stay green in three of them.
///
/// PRIVATE to this module, which is what confines its input to `ROSTER` and
/// the slices its own tests hand it: an operator's config never reaches it.
///
/// IT PANICS on a refused registration, naming the offender, and that is safe
/// on an always-exit-0 path because the only reachable refusal is a duplicate
/// name in a compiled-in const: deterministic, so it fires on the first call
/// in every mode and every test run and cannot reach an operator's machine.
/// Logging and carrying on, which is what this replaced, drops a delivery leg
/// silently and forever on the path whose job is to not be silent.
fn build_registry(entries: &[Registration]) -> Registry {
    let mut registry = Registry::new();
    for entry in entries {
        registry
            .register_plugin(entry.name, entry.kind)
            .unwrap_or_else(|error| panic!("pns: the compiled-in roster is invalid: {error:?}"));
    }
    registry
}

/// THE ROSTER the composition root registers, and the only statement of
/// delivery order. A destination is added here, never to policy.
pub fn roster() -> Registry {
    build_registry(&ROSTER)
}

#[cfg(test)]
mod tests;
