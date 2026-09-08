use super::roster::REQUIRES;
use super::{CORE, Registration, Registry, RegistryError};
use std::collections::BTreeMap;

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

impl Registry {
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
