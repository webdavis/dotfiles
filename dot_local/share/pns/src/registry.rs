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

/// The declarations the composition root registers, named once so a test can
/// run against the real thing. Each entry states its KIND, so a sensor rides
/// in the same list as the channels rather than in a second one the
/// composition root has to remember.
pub const ROSTER: [Registration; 6] = [
    Registration {
        // The home probe's router: an INPUT, so it holds no delivery order to
        // state and sits ahead of the channels, whose order is delivery order.
        // `pns home` reads it; no event can route to it, because a sensor
        // carries no routing for a plan to read.
        name: "router",
        kind: PluginKind::Sensor,
    },
    Registration {
        // Which ROOM the operator is in, read off the state file the daemon
        // publishes. A second INPUT, beside the router and ahead of the
        // channels for the same reason. It borrows `[plugins.hue]`'s bridge
        // and key rather than declaring its own, which is what `REQUIRES`
        // above holds it to.
        name: PRESENCE,
        kind: PluginKind::Sensor,
    },
    Registration {
        // The phone. NAMED FOR THE DESTINATION, not for the service behind it:
        // `[plugins.mobile] type` names which backend carries the card (moshi
        // today), so a second one is a value the operator writes rather than a
        // second plugin name and a second table to move settings into.
        name: "mobile",
        kind: PluginKind::Channel(Routing {
            local: false,
            presence_gated: true,
            durable: false,
            event_dispatched: true,
        }),
    },
    Registration {
        // AHEAD OF THE DURABLE LOG, because this one is presence-sensitive
        // and that one is not. The plan is computed from a reading of where
        // the operator is at dispatch, and hermes can post synchronously
        // against a deadline; delivering the banner after it would show the
        // operator a decision taken about a moment that had passed.
        name: "macos-banner",
        kind: PluginKind::Channel(Routing {
            local: true,
            presence_gated: false,
            durable: false,
            event_dispatched: true,
        }),
    },
    Registration {
        name: "hermes",
        kind: PluginKind::Channel(Routing {
            local: false,
            presence_gated: false,
            durable: true,
            event_dispatched: true,
        }),
    },
    Registration {
        // A local surface the binary drives in its own `pulse` mode. It
        // registers so the config can select it and so a typo in its name is
        // still refused, but no event ever routes to it.
        name: "hue",
        kind: PluginKind::Channel(Routing {
            local: true,
            presence_gated: false,
            durable: false,
            event_dispatched: false,
        }),
    },
];

/// WHICH PLUGIN BORROWS WHICH. A sensor that reads another plugin's
/// credential rather than declaring its own is refused when that other plugin
/// is off, because the alternative is a table the operator switched on that
/// silently never reads anything.
///
/// DATA BESIDE `CORE`, for the same reason: this is selection policy, and the
/// roster states what a plugin IS.
const REQUIRES: [(&str, &str); 1] = [(PRESENCE, "hue")];

/// The room-presence sensor's config name, spelled once. Three modules select
/// on it (the roster, the doctor's own check, and the settings reader), and a
/// literal in each is three spellings to drift.
pub const PRESENCE: &str = "presence";

/// THE CORE: what a machine with no usable config runs. Names rather than a
/// flag on the declaration, because this is a selection policy and the roster
/// states what a plugin IS; a name here that nothing registers simply selects
/// nothing, which is what `the_core_is_two_registered_plugins_and_the_config_
/// still_beats_it` is for. IN REGISTRATION ORDER, so the warning that lists it
/// reads in the order the legs run.
pub const CORE: [&str; 2] = ["mobile", "macos-banner"];

/// Which plugins run, given what loading the config found. The composition
/// policy in one place, and it turns on ONE question: did the file parse?
///
/// A LOADED config is authoritative, and that holds even when it names a
/// plugin nothing registered. The typo is LOUD, the returned warning, and the
/// selection is the WHOLE ROSTER: the file parsed, so every credential in it
/// is in hand and the composition root has already read hue's table, hermes's
/// key and the recap off it. Narrowing here would let one mistyped table name
/// cost a fully configured machine its durable paper trail and its lights,
/// which is not what a spelling mistake asked for.
///
/// A MISSING config selects the CORE, which is what a machine nobody has
/// configured gets. A config that could not be READ (unreadable, malformed,
/// invalid) is loud and selects the core too: on an always-exit-0 notification
/// path, a config error that silently turned every notification off would be
/// the exact failure the config layer exists to refuse, and the three it
/// leaves out could not have delivered anything anyway, since their
/// credentials are in the file nobody could read.
///
/// SELECTING ONLY THE KNOWN NAMES out of a config with one typo in it is a
/// third answer, narrower than either of these. It is not built: it would
/// have to decide what a half-honoured config means for every reader that
/// already took a value off the same file, and nothing has asked for it yet.
pub fn select_plugins(
    registry: &Registry,
    loaded: Result<crate::config::LoadOutcome, crate::config::ConfigError>,
) -> (Selection, Option<String>) {
    use crate::config::LoadOutcome;
    use RegistryError;

    match loaded {
        Ok(LoadOutcome::Loaded(config)) => match registry.enabled(&config.plugin_switches()) {
            Ok(selection) => (selection, None),
            Err(error) => {
                let detail = match error {
                    RegistryError::UnknownPlugin(name) => format!("unknown plugin `{name}`"),
                    RegistryError::Duplicate(name) => format!("duplicate plugin `{name}`"),
                    RegistryError::Unsatisfied { plugin, needs } => {
                        format!("`{plugin}` is enabled and needs `{needs}`, which is not")
                    }
                };
                (registry.all(), Some(every_plugin_warning(&detail)))
            }
        },
        Ok(LoadOutcome::Missing) => (registry.core(), None),
        Err(error) => (registry.core(), Some(core_warning(error.detail()))),
    }
}

/// The line a config that PARSED prints when it names a plugin nothing
/// registered: what was wrong, and that nothing was turned off because of it.
fn every_plugin_warning(detail: &str) -> String {
    format!("pns: config error ({detail}); running every built-in plugin")
}

/// The line a config nobody could read prints: what was wrong, and which two
/// plugins carry the machine until it is fixed.
fn core_warning(detail: &str) -> String {
    format!(
        "pns: config error ({detail}); running the core plugins ({})",
        CORE.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::Selection;
    use super::{PluginKind, Registration, Registry, RegistryError, Routing, build_registry};
    use crate::config::parse_config;

    const REMOTE_GATED: Routing = Routing {
        local: false,
        presence_gated: true,
        durable: false,
        event_dispatched: true,
    };
    const DURABLE: Routing = Routing {
        local: false,
        presence_gated: false,
        durable: true,
        event_dispatched: true,
    };
    const LOCAL: Routing = Routing {
        local: true,
        presence_gated: false,
        durable: false,
        event_dispatched: true,
    };

    fn three_plugin_registry() -> Registry {
        let mut registry = Registry::new();
        registry.register_channel("mobile", REMOTE_GATED).unwrap();
        registry.register_channel("hermes", DURABLE).unwrap();
        registry.register_channel("macos-banner", LOCAL).unwrap();
        registry
    }

    // --- registration -------------------------------------------------------

    #[test]
    fn registration_order_is_kept_because_it_is_delivery_order() {
        let registry = three_plugin_registry();
        assert_eq!(registry.names(), vec!["mobile", "hermes", "macos-banner"]);
    }

    #[test]
    fn a_name_already_taken_is_refused_naming_it() {
        let mut registry = Registry::new();
        registry.register_channel("mobile", REMOTE_GATED).unwrap();
        assert_eq!(
            registry.register_channel("mobile", LOCAL),
            Err(RegistryError::Duplicate("mobile".to_string()))
        );
    }

    // --- plugin kinds -------------------------------------------------------

    #[test]
    fn a_sensor_registers_by_name_so_a_typo_near_it_is_still_refused() {
        // A sensor carries no routing, but it occupies a config table like any
        // channel, so the registry has to know its name or the unknown-name
        // refusal would call the operator's correct spelling a typo.
        let mut registry = Registry::new();
        registry.register_channel("hermes", DURABLE).unwrap();
        registry.register_sensor("router").unwrap();
        assert_eq!(registry.names(), vec!["hermes", "router"]);

        let typo = parse_config("[plugins.rotuer]\nenabled = true\n").unwrap();
        assert_eq!(
            registry.enabled(&typo.plugin_switches()),
            Err(RegistryError::UnknownPlugin("rotuer".to_string()))
        );
    }

    #[test]
    fn one_name_cannot_be_two_kinds_because_they_share_one_config_table_space() {
        // `[plugins.router]` names exactly one plugin. If a sensor could take
        // a channel's name the config would select both and the operator
        // would have no way to say which they meant, so the second claim is
        // refused in either order, naming the offender.
        let mut sensor_first = Registry::new();
        sensor_first.register_sensor("router").unwrap();
        assert_eq!(
            sensor_first.register_channel("router", LOCAL),
            Err(RegistryError::Duplicate("router".to_string()))
        );

        let mut channel_first = Registry::new();
        channel_first.register_channel("router", LOCAL).unwrap();
        assert_eq!(
            channel_first.register_sensor("router"),
            Err(RegistryError::Duplicate("router".to_string()))
        );

        let mut twice = Registry::new();
        twice.register_sensor("router").unwrap();
        assert_eq!(
            twice.register_sensor("router"),
            Err(RegistryError::Duplicate("router".to_string()))
        );

        // The refusal is what keeps the roster from growing a second entry,
        // not a silent overwrite of the first.
        assert_eq!(sensor_first.names(), vec!["router"]);
        assert_eq!(channel_first.names(), vec!["router"]);
        assert_eq!(twice.names(), vec!["router"]);
    }

    // --- the one roster constructor -----------------------------------------

    #[test]
    fn a_registry_is_built_from_a_slice_of_declarations_in_the_order_given() {
        // ONE constructor, taking the declarations as data. Two hand-written
        // loops over the same const diverge the moment one of them grows a
        // kind the other does not know about; a slice in and a registry out
        // makes that unrepresentable, and lets a test hand it a roster of its
        // own without the composition root's four entries in the way.
        let entries = [
            Registration {
                name: "hermes",
                kind: PluginKind::Channel(DURABLE),
            },
            Registration {
                name: "router",
                kind: PluginKind::Sensor,
            },
        ];
        assert_eq!(build_registry(&entries).names(), vec!["hermes", "router"]);
    }

    #[test]
    #[should_panic(expected = "router")]
    fn a_roster_that_claims_one_name_twice_panics_naming_it() {
        // The only reachable duplicate is in a compiled-in const, so it is
        // deterministic: it fires on the first call in every mode and every
        // test run and cannot reach an operator's machine. Logging and
        // carrying on instead drops a plugin silently and forever, on a path
        // whose whole job is to not be silent.
        let entries = [
            Registration {
                name: "router",
                kind: PluginKind::Sensor,
            },
            Registration {
                name: "router",
                kind: PluginKind::Channel(LOCAL),
            },
        ];
        build_registry(&entries);
    }

    #[test]
    fn the_production_roster_carries_the_two_sensors_beside_the_four_channels() {
        // The const has to SAY what each entry is, so the sensor rides in the
        // same declaration as the channels rather than in a second list the
        // composition root has to remember to register. The sensor is first
        // because it holds no delivery order to state, which also means a plan
        // that dropped an entry by POSITION rather than by kind would shift
        // every channel after it.
        let declared: Vec<(&str, bool)> = super::ROSTER
            .iter()
            .map(|entry| (entry.name, entry.kind == PluginKind::Sensor))
            .collect();
        assert_eq!(
            declared,
            vec![
                ("router", true),
                ("presence", true),
                ("mobile", false),
                ("macos-banner", false),
                ("hermes", false),
                ("hue", false),
            ]
        );
        assert_eq!(
            super::roster().names(),
            vec![
                "router",
                "presence",
                "mobile",
                "macos-banner",
                "hermes",
                "hue"
            ]
        );
    }

    // --- selection by config ------------------------------------------------

    #[test]
    fn a_presence_table_switched_on_without_hue_is_refused_naming_both() {
        // Presence reads the bridge through `[plugins.hue]`'s own address and
        // key. Selected without it, the sensor is a table the operator turned
        // on that could never take a reading, and a silent one is worse than
        // the refusal: the fix is in the OTHER table, so both are named.
        let config = parse_config("[plugins.presence]\nenabled = true\ntype = \"hue\"\n").unwrap();
        assert_eq!(
            super::roster().enabled(&config.plugin_switches()),
            Err(RegistryError::Unsatisfied {
                plugin: "presence".to_string(),
                needs: "hue".to_string(),
            })
        );
    }

    #[test]
    fn a_hue_table_switched_off_refuses_presence_just_as_an_absent_one_does() {
        let config = parse_config(
            "[plugins.presence]\nenabled = true\ntype = \"hue\"\n\
             [plugins.hue]\nenabled = false\n",
        )
        .unwrap();
        assert!(matches!(
            super::roster().enabled(&config.plugin_switches()),
            Err(RegistryError::Unsatisfied { .. })
        ));
    }

    #[test]
    fn presence_is_selected_once_hue_carries_the_bridge_it_reads() {
        let config = parse_config(
            "[plugins.presence]\nenabled = true\ntype = \"hue\"\n\
             [plugins.hue]\nenabled = true\n",
        )
        .unwrap();
        let names: Vec<&str> = super::roster()
            .enabled(&config.plugin_switches())
            .expect("hue carries it")
            .iter()
            .map(|entry| entry.name)
            .collect();
        assert_eq!(names, vec!["presence", "hue"]);
    }

    #[test]
    fn the_config_selects_and_registration_order_beats_config_order() {
        // The config lists banner before mobile; the plan order is still the
        // registered one, because delivery order is policy, not preference.
        let config = parse_config(
            "[plugins.macos-banner]\nenabled = true\n[plugins.mobile]\nenabled = true\n",
        )
        .unwrap();
        let enabled = three_plugin_registry()
            .enabled(&config.plugin_switches())
            .unwrap();
        let names: Vec<&str> = enabled.iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["mobile", "macos-banner"]);
    }

    #[test]
    fn a_disabled_or_omitted_plugin_is_simply_not_selected() {
        let config =
            parse_config("[plugins.mobile]\nenabled = true\n[plugins.hermes]\nenabled = false\n")
                .unwrap();
        let enabled = three_plugin_registry()
            .enabled(&config.plugin_switches())
            .unwrap();
        let names: Vec<&str> = enabled.iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["mobile"]);
    }

    #[test]
    fn an_enabled_name_nothing_registered_is_refused_naming_it() {
        // A typo'd plugin name that silently no-ops is a notification quietly
        // turned off; the registry refuses it the way the config layer
        // refuses unknown keys.
        let config = parse_config("[plugins.mosih]\nenabled = true\n").unwrap();
        assert_eq!(
            three_plugin_registry().enabled(&config.plugin_switches()),
            Err(RegistryError::UnknownPlugin("mosih".to_string()))
        );
    }

    #[test]
    fn a_disabled_unknown_name_is_still_refused_because_the_typo_is_the_defect() {
        // `enabled = false` on an unknown table is the same typo one edit
        // away from silently disabling a real plugin; refuse it now, while
        // the operator is looking at the file they just edited.
        let config = parse_config("[plugins.mosih]\nenabled = false\n").unwrap();
        assert_eq!(
            three_plugin_registry().enabled(&config.plugin_switches()),
            Err(RegistryError::UnknownPlugin("mosih".to_string()))
        );
    }

    #[test]
    fn a_config_that_enables_a_sensor_selects_it_alongside_the_channels() {
        // A sensor is selected by an ordinary `[plugins.<name>]` table, so the
        // config layer needs no idea that kinds exist. hermes rides along as
        // the positive control: a selection that dropped everything would
        // fail this too.
        let mut registry = Registry::new();
        registry.register_channel("hermes", DURABLE).unwrap();
        registry.register_sensor("router").unwrap();

        let both =
            parse_config("[plugins.router]\nenabled = true\n[plugins.hermes]\nenabled = true\n")
                .unwrap();
        let selection = registry.enabled(&both.plugin_switches()).unwrap();
        let names: Vec<&str> = selection.iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["hermes", "router"]);

        // And `enabled = false` turns a sensor off like anything else: no kind
        // is quietly always-on.
        let off =
            parse_config("[plugins.router]\nenabled = false\n[plugins.hermes]\nenabled = true\n")
                .unwrap();
        let selection = registry.enabled(&off.plugin_switches()).unwrap();
        let names: Vec<&str> = selection.iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["hermes"]);
    }

    #[test]
    fn all_selects_every_registration_which_is_what_the_census_reports_against() {
        let selection = three_plugin_registry().all();
        let names: Vec<&str> = selection.iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["mobile", "hermes", "macos-banner"]);
    }

    #[test]
    fn an_empty_config_selects_nothing_which_is_a_verdict_not_an_error() {
        let config = parse_config("").unwrap();
        let enabled = three_plugin_registry()
            .enabled(&config.plugin_switches())
            .unwrap();
        assert!(enabled.is_empty());
    }

    // --- plugin selection at the composition root ---------------------------

    fn selection_names(selection: &Selection) -> Vec<&str> {
        selection.iter().map(|r| r.name).collect()
    }

    #[test]
    fn a_machine_with_no_config_runs_the_core_and_nothing_that_needs_arming() {
        // THE FALLBACK IS THE CORE, not the whole roster (operator ruling
        // 2026-08-31). A machine with no config gets the two destinations that
        // are useful the moment the binary lands; hue, hermes and router each
        // need a bridge, a route or an API key stood up before they can do
        // anything at all, so defaulting them on delivers nothing and reports
        // three failures.
        use crate::config::LoadOutcome;
        let (selection, warning) =
            super::select_plugins(&super::roster(), Ok(LoadOutcome::Missing));
        assert_eq!(selection_names(&selection), vec!["mobile", "macos-banner"]);
        assert_eq!(warning, None);
    }

    #[test]
    fn the_core_is_two_registered_plugins_and_the_config_still_beats_it() {
        // THE NAME LIST IS THE DRIFT RISK: a misspelling in `CORE` selects
        // nothing rather than failing to compile, and the machine with no
        // config would go quiet with nothing to look at. So the core is
        // asserted against the REAL roster, both members named.
        assert_eq!(
            selection_names(&super::roster().core()),
            vec!["mobile", "macos-banner"],
            "every core name is a registered plugin"
        );
        // AND IT IS ONLY A FALLBACK. A config that exists says what runs, so
        // writing one that omits a core plugin turns that plugin off; nothing
        // is quietly always-on.
        let config = parse_config("[plugins.hermes]\nenabled = true\n").unwrap();
        let selection = super::roster().enabled(&config.plugin_switches()).unwrap();
        assert_eq!(selection_names(&selection), vec!["hermes"]);
    }

    #[test]
    fn a_loaded_config_is_authoritative() {
        use crate::config::LoadOutcome;
        let config = parse_config("[plugins.hermes]\nenabled = true\n").unwrap();
        let (selection, warning) =
            super::select_plugins(&super::roster(), Ok(LoadOutcome::Loaded(config)));
        assert_eq!(selection_names(&selection), vec!["hermes"]);
        assert_eq!(warning, None);
    }

    #[test]
    fn a_broken_config_is_loud_but_never_turns_notifications_off() {
        // THE CORE, not the whole roster: the three left out keep their
        // credentials in the very file nobody could read, so running them
        // would report three failures about a config error already on stderr.
        use crate::config::ConfigError;
        let (selection, warning) = super::select_plugins(
            &super::roster(),
            Err(ConfigError::Malformed(
                "key with no value at line 1".to_string(),
            )),
        );
        assert_eq!(selection_names(&selection), vec!["mobile", "macos-banner"]);
        let warning = warning.expect("a broken config must be said aloud");
        assert!(warning.contains("key with no value"));
    }

    #[test]
    fn a_config_naming_an_unknown_plugin_is_loud_and_falls_back_to_the_roster() {
        // THE WHOLE ROSTER, and the arm is the reason. This config PARSED, so
        // every credential in it is in hand and the composition root has
        // already read hue's table, hermes's key and the recap off it before
        // selection runs. The core fallback exists for a file nobody could
        // read; applying it here lets one mistyped table name cost a fully
        // configured machine its durable paper trail and its lights, which is
        // a blast radius no ruling asked for.
        use crate::config::LoadOutcome;
        let config =
            parse_config("[plugins.mosih]\nenabled = true\n[plugins.hermes]\nenabled = true\n")
                .unwrap();
        let (selection, warning) =
            super::select_plugins(&super::roster(), Ok(LoadOutcome::Loaded(config)));
        assert_eq!(
            selection_names(&selection),
            super::roster().names(),
            "a config-present machine keeps every plugin; the typo is loud, not fatal"
        );
        let warning = warning.expect("the typo'd name must be said aloud");
        assert!(warning.contains("mosih"));
        assert!(
            warning.contains("running every built-in plugin"),
            "and the line says what still runs: {warning}"
        );
    }

    #[test]
    fn a_hue_table_selects_hue_like_any_other_plugin_and_warns_about_nothing() {
        // It used to be a string exception stripped before the unknown-name
        // refusal. It is a registration now, so configuring the pulse is
        // ordinary and costs the operator no part of their event selection.
        use crate::config::LoadOutcome;
        let config =
            parse_config("[plugins.hermes]\nenabled = true\n[plugins.hue]\nenabled = true\n")
                .unwrap();
        let (selection, warning) =
            super::select_plugins(&super::roster(), Ok(LoadOutcome::Loaded(config)));
        assert_eq!(selection_names(&selection), vec!["hermes", "hue"]);
        assert_eq!(warning, None);
    }

    #[test]
    fn the_old_moshi_table_name_is_refused_and_the_mobile_one_is_served() {
        // THE RENAME HAS NO BACK ROAD, and it needs none: the template is the
        // only pns config anyone has and it regenerates at apply. So
        // `[plugins.moshi]` is not a second spelling of the phone plugin, it
        // is a name nothing registered, and it gets the refusal every typo
        // gets rather than quietly selecting the plugin it used to name.
        use crate::config::LoadOutcome;
        let old = parse_config("[plugins.moshi]\nenabled = true\n").unwrap();
        assert_eq!(
            super::roster().enabled(&old.plugin_switches()),
            Err(RegistryError::UnknownPlugin("moshi".to_string()))
        );
        let (_, warning) = super::select_plugins(&super::roster(), Ok(LoadOutcome::Loaded(old)));
        assert!(
            warning
                .expect("the retired name is said aloud")
                .contains("moshi"),
            "the operator is told which name stopped working"
        );

        let new = parse_config("[plugins.mobile]\nenabled = true\n").unwrap();
        let selection = super::roster().enabled(&new.plugin_switches()).unwrap();
        assert_eq!(selection_names(&selection), vec!["mobile"]);
    }

    #[test]
    fn a_true_typo_is_still_refused() {
        use crate::config::LoadOutcome;
        let config = parse_config("[plugins.mosih]\nenabled = true\n").unwrap();
        let (_, warning) = super::select_plugins(&super::roster(), Ok(LoadOutcome::Loaded(config)));
        assert!(
            warning
                .expect("the typo is still the defect")
                .contains("mosih")
        );
    }
}
