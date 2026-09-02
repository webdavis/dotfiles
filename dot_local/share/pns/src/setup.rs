//! The first-run wizard's policy: the answers a walk collects, and the one
//! config text they compose into.
//!
//! PURE, so the whole wizard is table-tested without a terminal. The walk
//! itself is an edge in the composition root that does nothing but ask, read a
//! line, and hand what came back to `compose_config`; every rule about what
//! ends up in the file is here.

use std::path::{Path, PathBuf};

/// What the walk came back with. EVERY CREDENTIAL IS A PLAIN STRING AND EMPTY
/// MEANS DECLINED, so answering "no" to a feature and answering "yes" and then
/// pasting nothing compose the same file: an empty value parses as absent and
/// would deliver nothing while reading as configured, which is the state this
/// wizard exists to keep off a fresh machine.
///
/// THE DEFAULT IS THE WHOLE WALK DECLINED, which is the shipped posture: the
/// banner and the phone card, and nothing else armed.
#[derive(Debug, Default, PartialEq)]
pub struct Answers {
    /// The moshi webhook secret the phone card is submitted with. Skipped
    /// leaves mobile on and uncarded until a pairing supplies one.
    pub mobile_token: String,
    pub hermes_key: String,
    pub hue_bridge: String,
    pub hue_key: String,
    pub hue_rooms: Vec<String>,
    /// Which compiled-in backend answers the home probe.
    pub router_type: String,
    pub router_url: String,
    pub router_api_key: String,
    pub router_device_hostname: String,
    /// The Focus mode names that mean "not now". Empty is the feature off,
    /// which is what the parser reads an absent table as.
    pub focus_modes: Vec<String>,
    pub nag: bool,
}

impl Answers {
    /// This walk's answers, as the values table `config_text::render` walks.
    ///
    /// ONLY WHAT WAS ARMED IS HERE. A table this method never inserts is one
    /// `render` writes at its layout default, commented for an opt-in table
    /// and live at the CORE default for `mobile`, `macos-banner`, `daemon`
    /// and `recap`, none of which this wizard even asks about.
    pub fn values(&self) -> toml::Table {
        let mut plugins = toml::Table::new();
        if !self.mobile_token.is_empty() {
            let mut mobile = toml::Table::new();
            mobile.insert(
                "token".to_string(),
                toml::Value::String(self.mobile_token.clone()),
            );
            plugins.insert("mobile".to_string(), toml::Value::Table(mobile));
        }
        if !self.hermes_key.is_empty() {
            let mut hermes = toml::Table::new();
            hermes.insert(
                "key".to_string(),
                toml::Value::String(self.hermes_key.clone()),
            );
            plugins.insert("hermes".to_string(), toml::Value::Table(hermes));
        }
        if hue_is_armed(self) {
            let mut hue = toml::Table::new();
            hue.insert(
                "bridge".to_string(),
                toml::Value::String(self.hue_bridge.clone()),
            );
            hue.insert("key".to_string(), toml::Value::String(self.hue_key.clone()));
            hue.insert(
                "rooms".to_string(),
                toml::Value::Array(
                    self.hue_rooms
                        .iter()
                        .cloned()
                        .map(toml::Value::String)
                        .collect(),
                ),
            );
            plugins.insert("hue".to_string(), toml::Value::Table(hue));
        }
        if router_is_armed(self) {
            let mut router = toml::Table::new();
            router.insert(
                "type".to_string(),
                toml::Value::String(self.router_type.clone()),
            );
            router.insert(
                "router_url".to_string(),
                toml::Value::String(self.router_url.clone()),
            );
            router.insert(
                "api_key".to_string(),
                toml::Value::String(self.router_api_key.clone()),
            );
            router.insert(
                "device_hostname".to_string(),
                toml::Value::String(self.router_device_hostname.clone()),
            );
            plugins.insert("router".to_string(), toml::Value::Table(router));
        }

        let mut values = toml::Table::new();
        if !plugins.is_empty() {
            values.insert("plugins".to_string(), toml::Value::Table(plugins));
        }
        if !self.focus_modes.is_empty() {
            let mut focus = toml::Table::new();
            focus.insert(
                "silence".to_string(),
                toml::Value::Array(
                    self.focus_modes
                        .iter()
                        .cloned()
                        .map(toml::Value::String)
                        .collect(),
                ),
            );
            values.insert("focus".to_string(), toml::Value::Table(focus));
        }
        if self.nag {
            values.insert("nag".to_string(), toml::Value::Table(toml::Table::new()));
        }
        values
    }
}

/// The whole config file, composed from one walk's answers.
///
/// EVERY DEFAULT IT RELIES ON IS WRITTEN OUT, because a loaded config is
/// authoritative and an absent `enabled` reads false: a wizard that left the
/// core implicit would hand a fresh machine a file that turns the banner and
/// the card off. A declined feature is present too, as a commented block with
/// empty values, so the file says what it could carry as well as what it does.
///
/// `crate::config_text::render` NEVER REFUSES A WALK'S OWN ANSWERS: every
/// value this method's `values()` composes is a plain literal off the roster
/// this wizard's own layout serves, so the only way this expect fires is a
/// bug in `values()` itself, not an operator's input.
pub fn compose_config(answers: &Answers) -> String {
    crate::config_text::render(&answers.values()).expect("a wizard's own answers always render")
}

/// Where an existing config is kept when `--force` replaces it: a sibling of
/// the config, stamped with the instant it was moved aside.
///
/// A SIBLING because the config's own directory is the one place this wizard
/// already knows it can write, and STAMPED so a second forced run cannot land
/// on the first one's backup. The stamp is UTC and carries no colons: it is a
/// discriminator in a file name rather than a clock anybody reads, and the
/// caller prints the whole path.
///
/// NO CLOCK, NO NAME, and the caller turns that into a refusal: replacing a
/// config whose copy cannot be named is the one outcome that loses the file.
pub fn backup_path(config: &Path, epoch_secs: u64) -> Option<PathBuf> {
    let stamp = crate::system::utc_timestamp(epoch_secs)?
        .replace(':', "-")
        .replace('Z', "");
    let name = config.file_name()?.to_str()?;
    Some(config.with_file_name(format!("{name}.{stamp}.backup")))
}

/// Whether the walk armed the light pulse. THE ROOMS COUNT AS A CREDENTIAL:
/// with none named the plugin falls back to a compiled-in room list that names
/// nobody else's rooms, so a bridge and key alone are a pulse that reaches no
/// lamp and reports nothing.
fn hue_is_armed(answers: &Answers) -> bool {
    !answers.hue_bridge.is_empty() && !answers.hue_key.is_empty() && !answers.hue_rooms.is_empty()
}

/// Whether the walk armed the home probe. THE BACKEND COUNTS AS A CREDENTIAL:
/// the table's keys are free text to the parser, so a name no compiled-in
/// backend answers composes a probe that loads and then refuses every time it
/// runs, which is the same silent nothing an empty credential writes.
fn router_is_armed(answers: &Answers) -> bool {
    answers.router_type == crate::home::UNIFI_TYPE
        && !answers.router_url.is_empty()
        && !answers.router_api_key.is_empty()
        && !answers.router_device_hostname.is_empty()
}

#[cfg(test)]
mod tests {
    use super::{Answers, backup_path, compose_config};
    use crate::config::{DEFAULT_SUBMIT_DEADLINE_SECS, Recap, parse_config};
    use std::path::{Path, PathBuf};

    /// Every table a walk can decline, spelled as a heading standing at the
    /// head of a line: what the two ends of the walk are checked for.
    const DECLINABLE_TABLES: [&str; 5] = [
        "[plugins.hermes]",
        "[plugins.hue]",
        "[plugins.router]",
        "[focus]",
        "[nag]",
    ];

    /// A walk that armed everything it was offered.
    fn every_feature_armed() -> Answers {
        Answers {
            mobile_token: "moshi-secret".to_string(),
            hermes_key: "hermes-secret".to_string(),
            hue_bridge: "192.168.1.9".to_string(),
            hue_key: "hue-secret".to_string(),
            hue_rooms: vec!["Studio".to_string(), "Kitchen".to_string()],
            router_type: "unifi".to_string(),
            router_url: "https://192.168.1.1".to_string(),
            router_api_key: "router-secret".to_string(),
            router_device_hostname: "phone".to_string(),
            focus_modes: vec!["Sleep".to_string()],
            nag: true,
        }
    }

    /// The config the text composes to, or the refusal as a panic naming it.
    fn parsed(text: &str) -> crate::config::Config {
        parse_config(text).unwrap_or_else(|error| panic!("it must load: {error:?}\n{text}"))
    }

    #[test]
    fn a_walk_that_armed_nothing_still_writes_the_core() {
        // THE SHIPPED POSTURE. Declining every question is the common first
        // run, and what it has to leave behind is a machine that banners and
        // cards: an absent `enabled` reads FALSE, so a written config that
        // states neither would take the core away from the machine that just
        // asked for a config.
        let text = compose_config(&Answers::default());
        let config = parsed(&text);
        assert!(config.plugins["macos-banner"].enabled);
        assert!(config.plugins["mobile"].enabled);
        assert_eq!(
            config.plugins["mobile"].settings["type"].as_str(),
            Some("moshi")
        );
        for opt_in in ["hermes", "hue", "router"] {
            assert!(
                !config.plugins.contains_key(opt_in),
                "`{opt_in}` was armed by nobody"
            );
        }
        assert!(config.lights.is_none());
        assert!(config.focus_silence.is_empty());
        assert_eq!(config.nag_after_secs, 0);
        // AND A DECLINED TABLE IS COMMENTED OUT rather than written with empty
        // values, which is the same rule stated about the text rather than
        // about what it parses to: `silence = []` and `rooms = []` load to the
        // same nothing an absent table does, and read as a feature set up.
        for declined in DECLINABLE_TABLES {
            assert!(
                !text.contains(&format!("\n{declined}")),
                "`{declined}` stands uncommented in a walk that armed nothing:\n{text}"
            );
        }
    }

    #[test]
    fn the_values_it_writes_unprompted_are_the_ones_the_code_defaults_to() {
        // WRITTEN OUT AT THEIR DEFAULTS, and the assertion is against the
        // code's own default rather than against the same literals the
        // composer holds: a default moved in `config` and left standing here
        // would otherwise ship a wizard writing yesterday's number as though
        // it were today's.
        let config = parsed(&compose_config(&Answers::default()));
        assert_eq!(config.recap, Recap::default());
        assert!(config.daemon_enabled);
        let mobile = &config.plugins["mobile"].settings;
        assert_eq!(mobile["mobile_watch_card"].as_bool(), Some(false));
        assert_eq!(
            mobile["submit_deadline_secs"].as_integer(),
            Some(DEFAULT_SUBMIT_DEADLINE_SECS as i64)
        );
    }

    #[test]
    fn a_skipped_token_is_commented_out_rather_than_written_empty() {
        // MOBILE STAYS ON EITHER WAY: pairing is what completes it, and a
        // `token = ""` would read as configured while carding nothing.
        let text = compose_config(&Answers::default());
        assert!(text.contains("# token = \"\""), "{text}");
        let config = parsed(&text);
        assert!(config.plugins["mobile"].enabled);
        assert!(!config.plugins["mobile"].settings.contains_key("token"));
    }

    #[test]
    fn every_armed_feature_reaches_the_parsed_config_carrying_its_own_answers() {
        let text = compose_config(&every_feature_armed());
        // THE MIRROR OF THE DECLINED CASE: an armed table stands uncommented,
        // or the walk collected an answer and then commented it away.
        for armed in DECLINABLE_TABLES {
            assert!(
                text.contains(&format!("\n{armed}")),
                "`{armed}` was armed and written commented out:\n{text}"
            );
        }
        let config = parsed(&text);
        assert_eq!(
            config.plugins.keys().collect::<Vec<_>>(),
            vec!["hermes", "hue", "macos-banner", "mobile", "router"]
        );
        assert!(config.plugins.values().all(|plugin| plugin.enabled));
        assert_eq!(
            config.plugins["mobile"].settings["token"].as_str(),
            Some("moshi-secret")
        );
        assert_eq!(
            config.plugins["hermes"].settings["key"].as_str(),
            Some("hermes-secret")
        );
        let hue = &config.plugins["hue"].settings;
        assert_eq!(hue["bridge"].as_str(), Some("192.168.1.9"));
        assert_eq!(hue["key"].as_str(), Some("hue-secret"));
        assert_eq!(
            hue["rooms"]
                .as_array()
                .map(|rooms| rooms.iter().filter_map(|room| room.as_str()).collect()),
            Some(vec!["Studio", "Kitchen"])
        );
        let router = &config.plugins["router"].settings;
        assert_eq!(router["type"].as_str(), Some("unifi"));
        assert_eq!(router["router_url"].as_str(), Some("https://192.168.1.1"));
        assert_eq!(router["api_key"].as_str(), Some("router-secret"));
        assert_eq!(router["device_hostname"].as_str(), Some("phone"));
        assert_eq!(config.focus_silence, vec!["Sleep".to_string()]);
        assert_eq!(config.nag_after_secs, 300);
    }

    #[test]
    fn a_credential_left_blank_declines_its_feature_rather_than_arming_an_empty_one() {
        // AN EMPTY VALUE PARSES AS ABSENT and delivers nothing while reading
        // as configured, which is the silent failure the walk must never
        // write. Every required field of a feature is tried on its own, so a
        // feature armed by one of two credentials cannot slip through.
        for (blank, declined) in [
            (
                (|answers: &mut Answers| answers.hermes_key.clear()) as fn(&mut Answers),
                "hermes",
            ),
            (|answers: &mut Answers| answers.hue_bridge.clear(), "hue"),
            (|answers: &mut Answers| answers.hue_key.clear(), "hue"),
            (|answers: &mut Answers| answers.hue_rooms.clear(), "hue"),
            (|answers: &mut Answers| answers.router_url.clear(), "router"),
            (
                |answers: &mut Answers| answers.router_api_key.clear(),
                "router",
            ),
            (
                |answers: &mut Answers| answers.router_device_hostname.clear(),
                "router",
            ),
        ] {
            let mut answers = every_feature_armed();
            blank(&mut answers);
            let config = parsed(&compose_config(&answers));
            // ONE FEATURE GOES AND THE REST STAY: a blank answer must not cost
            // the walk anything the operator did fill in.
            let armed: Vec<&str> = config.plugins.keys().map(String::as_str).collect();
            assert!(
                !armed.contains(&declined),
                "a blank credential armed `{declined}` anyway"
            );
            assert_eq!(
                armed.len(),
                4,
                "blanking `{declined}` cost more than `{declined}`: {armed:?}"
            );
            for plugin in config.plugins.values() {
                for (key, value) in &plugin.settings {
                    assert_ne!(value.as_str(), Some(""), "`{key}` was written empty");
                }
            }
        }
    }

    #[test]
    fn a_backend_the_home_probe_cannot_answer_declines_the_probe_rather_than_arming_it() {
        // THE SILENT NOTHING THIS WALK EXISTS TO PREVENT. Every key of the
        // router table is free text to the parser, so a backend name nothing
        // implements composes a file that loads, is reported as written, and
        // then refuses at the first probe with a type no compiled-in backend
        // answers. WHAT ACCEPTS IT IS ASKED rather than restated: the day a
        // second backend lands, `router_settings` is what has to agree.
        let armed = parsed(&compose_config(&every_feature_armed()));
        crate::home::router_settings(&armed.plugins["router"].settings)
            .expect("an armed walk writes a table the home probe can answer");

        let unanswerable = Answers {
            router_type: "asus".to_string(),
            ..every_feature_armed()
        };
        let config = parsed(&compose_config(&unanswerable));
        assert!(
            !config.plugins.contains_key("router"),
            "a backend nothing answers was written as an armed probe"
        );
    }

    #[test]
    fn the_lamp_map_starter_is_always_offered_and_is_wholly_commented_out() {
        // ALWAYS PRESENT AND COMMENTED, whether or not hue is armed: this
        // wizard never asks about the lamp map at all, so the starter reads
        // as inert documentation rather than a question the walk answered.
        // AN EMPTY `[lights]` IS A DISTINCT STATE, the operator asking for the
        // lamps and naming none, so the starter is an example to fill in and
        // never a heading standing on its own.
        for text in [
            compose_config(&every_feature_armed()),
            compose_config(&Answers::default()),
        ] {
            assert!(text.contains("# [lights]"), "{text}");
            assert!(!text.contains("\n[lights"), "{text}");
            assert!(parsed(&text).lights.is_none());
        }
    }

    #[test]
    fn a_credential_carrying_quotes_and_backslashes_reaches_the_config_as_itself() {
        // A PASTED SECRET IS UNTRUSTED TEXT. Interpolating it raw composes a
        // file that will not parse at best, and at worst one whose value stops
        // where the operator's own quote did.
        let answers = Answers {
            hermes_key: "a\"b\\c".to_string(),
            ..every_feature_armed()
        };
        let config = parsed(&compose_config(&answers));
        assert_eq!(
            config.plugins["hermes"].settings["key"].as_str(),
            Some("a\"b\\c")
        );
    }

    /// Every line SHAPED like a documented key, however it is spelled: the
    /// loose reading the strict scan is held against.
    ///
    /// THE STRICT SCAN IS WHITESPACE-EXACT AND LOWERCASE-ONLY, which is what
    /// makes it silent rather than wrong: a line it does not recognise as a key
    /// is not a line it complains about, it is a line it never sees. This
    /// reader recognises the shape alone, so the two disagreeing is the
    /// wizard documenting something the roster was never asked about.
    fn key_shaped_lines(text: &str) -> Vec<&str> {
        text.lines()
            .filter(|line| {
                let bare = line.strip_prefix("# ").unwrap_or(line);
                let Some((name, _)) = bare.split_once('=') else {
                    return false;
                };
                let name = name.trim();
                !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            })
            .collect()
    }

    #[test]
    fn every_key_it_writes_is_a_key_the_roster_serves_however_the_walk_was_answered() {
        // THE ANTI-DRIFT FENCE. The text is compiled into the binary, so
        // nothing else reads it and a key renamed in the schema would leave
        // the wizard writing a line that refuses the whole file the moment it
        // is uncommented. The scan is the shipped template's own, run over
        // both ends of the walk, and it reads the commented lines too.
        for text in [
            compose_config(&Answers::default()),
            compose_config(&every_feature_armed()),
        ] {
            parsed(&text);
            let found = crate::config::documented_keys_the_roster_serves(&text);
            // EVERY KEY-SHAPED LINE REACHED THE SCAN, which is the half a bare
            // count cannot state. The scan checks what it recognises and says
            // nothing about the rest, so a key misspelled past it (`apiKey`
            // for `api_key`, or `enabled=true` without the spaces the scan
            // splits on) is documented, unserved, and silently unchecked: the
            // operator uncomments it and the whole file stops loading.
            let shaped = key_shaped_lines(&text);
            assert!(
                !shaped.is_empty(),
                "the text documents no key at all: {text}"
            );
            assert_eq!(
                found,
                shaped.len(),
                "a key-shaped line never reached the roster scan; the scan read {found} of these {}:\n{}",
                shaped.len(),
                shaped.join("\n")
            );
        }
    }

    #[test]
    fn a_wizard_render_carries_no_chezmoi_action_because_every_answer_is_a_literal() {
        // `Answers` NEVER PRODUCE A SECRET MARKER: every credential the walk
        // collects is a plain string handed straight to `values()`, so a
        // wizard's own composed text is real TOML from the first line, never
        // a template `render` fills in only after chezmoi runs.
        for text in [
            compose_config(&Answers::default()),
            compose_config(&every_feature_armed()),
        ] {
            assert!(!text.contains("{{"), "{text}");
        }
    }

    #[test]
    fn the_backup_sits_beside_the_config_stamped_with_the_instant_it_was_moved() {
        // A SIBLING, because the directory is the one place the wizard already
        // knows it can write, and a stamp rather than a `.bak` so a second
        // forced run cannot land on the first one's name.
        assert_eq!(
            backup_path(Path::new("/home/x/.config/pns/config.toml"), 1_800_000_000),
            Some(PathBuf::from(
                "/home/x/.config/pns/config.toml.2027-01-15T08-00-00.backup"
            ))
        );
    }

    #[test]
    fn a_clock_that_cannot_be_read_names_no_backup_at_all() {
        // NO NAME IS THE REFUSAL the caller turns into "nothing was written":
        // replacing a config whose copy cannot be named is the one outcome
        // that loses the file.
        assert_eq!(backup_path(Path::new("/x/config.toml"), u64::MAX), None);
    }
}
