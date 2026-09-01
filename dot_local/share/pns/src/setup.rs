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

/// The whole config file, composed from one walk's answers.
///
/// EVERY DEFAULT IT RELIES ON IS WRITTEN OUT, because a loaded config is
/// authoritative and an absent `enabled` reads false: a wizard that left the
/// core implicit would hand a fresh machine a file that turns the banner and
/// the card off. A declined feature is present too, as a commented block with
/// empty values, so the file says what it could carry as well as what it does.
pub fn compose_config(answers: &Answers) -> String {
    let mut sections = vec![
        HEADER.to_string(),
        mobile_section(&answers.mobile_token),
        hermes_section(&answers.hermes_key),
        BANNER_SECTION.to_string(),
        hue_section(answers),
        router_section(answers),
        DAEMON_SECTION.to_string(),
        RECAP_SECTION.to_string(),
        focus_section(&answers.focus_modes),
        nag_section(answers.nag),
    ];
    // THE LAMP MAP FOLLOWS HUE and nothing else: it is inert without the
    // transport, so offering it to a machine with no bridge would be a block
    // of example lines that can never do anything.
    if hue_is_armed(answers) {
        sections.push(LIGHTS_STARTER.to_string());
    }
    sections.join("\n")
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

/// Whether the walk armed the home probe. The backend name is not in the test
/// because it has a working default and every other field here does not.
fn router_is_armed(answers: &Answers) -> bool {
    !answers.router_url.is_empty()
        && !answers.router_api_key.is_empty()
        && !answers.router_device_hostname.is_empty()
}

/// One answer as a TOML basic string.
///
/// A PASTED SECRET IS UNTRUSTED TEXT and is escaped rather than refused: raw
/// interpolation composes a file that will not load at best, and at worst one
/// whose value stops where the operator's own quote did.
fn quoted(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for character in value.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            // TOML admits no bare control character inside a basic string.
            control if control < ' ' || control == '\u{7f}' => {
                quoted.push_str(&format!("\\u{:04X}", control as u32));
            }
            plain => quoted.push(plain),
        }
    }
    quoted.push('"');
    quoted
}

/// One answer as a TOML array of basic strings.
fn quoted_list(values: &[String]) -> String {
    let quoted: Vec<String> = values.iter().map(|value| quoted(value)).collect();
    format!("[{}]", quoted.join(", "))
}

/// The phone card. ENABLED EITHER WAY, because it is core beside the banner,
/// and the token is what a pairing supplies.
fn mobile_section(token: &str) -> String {
    let token_line = if token.is_empty() {
        MOBILE_WITHOUT_A_TOKEN.to_string()
    } else {
        format!("token = {}\n", quoted(token))
    };
    format!("{MOBILE_HEAD}{token_line}{MOBILE_TAIL}")
}

/// The durable paper trail.
fn hermes_section(key: &str) -> String {
    if key.is_empty() {
        return format!("{HERMES_PREAMBLE}{DECLINED}{HERMES_DECLINED}");
    }
    format!("[plugins.hermes]\nenabled = true\nkey = {}\n", quoted(key))
}

/// The light pulse.
fn hue_section(answers: &Answers) -> String {
    if !hue_is_armed(answers) {
        return format!("{HUE_PREAMBLE}{DECLINED}{HUE_DECLINED}{HUE_QUIET_HOURS}");
    }
    format!(
        "[plugins.hue]\nenabled = true\nbridge = {}\nkey = {}\nrooms = {}\n{HUE_QUIET_HOURS}",
        quoted(&answers.hue_bridge),
        quoted(&answers.hue_key),
        quoted_list(&answers.hue_rooms)
    )
}

/// The home probe.
fn router_section(answers: &Answers) -> String {
    if !router_is_armed(answers) {
        return format!("{ROUTER_PREAMBLE}{DECLINED}{ROUTER_DECLINED}");
    }
    format!(
        "{ROUTER_PREAMBLE}[plugins.router]\nenabled = true\ntype = {}\nrouter_url = {}\n\
         api_key = {}\n{ROUTER_DEVICE}device_hostname = {}\n",
        quoted(&answers.router_type),
        quoted(&answers.router_url),
        quoted(&answers.router_api_key),
        quoted(&answers.router_device_hostname)
    )
}

/// The Focus modes that mean "not now". NAMING NO MODE IS THE FEATURE OFF, so
/// a walk that named none writes the example commented rather than an empty
/// list that reads as configured.
fn focus_section(modes: &[String]) -> String {
    if modes.is_empty() {
        return format!("{FOCUS_PREAMBLE}{FOCUS_DECLINED}");
    }
    format!(
        "{FOCUS_PREAMBLE}[focus]\nsilence = {}\n",
        quoted_list(modes)
    )
}

/// The nudge about an approval nobody answered.
fn nag_section(on: bool) -> String {
    let table = if on {
        format!("[nag]\nafter_secs = {NAG_SUGGESTED_SECS}\n")
    } else {
        format!("# [nag]\n# after_secs = {NAG_SUGGESTED_SECS}\n")
    };
    format!("{NAG_PREAMBLE}{table}")
}

/// What the nag is set to when the walk asks for one.
///
/// FIVE MINUTES rather than the floor, because the signal pns gets is the tool
/// batch RESOLVING rather than the operator answering: a tool approved at once
/// that then runs longer than this cards them about their own approval.
const NAG_SUGGESTED_SECS: u64 = 300;

/// The line every declined block carries, saying why it is commented rather
/// than written with empty values.
const DECLINED: &str = r##"# COMMENTED OUT BECAUSE NOTHING ARMED IT: an empty value parses as absent, so
# writing one would read as set up here and deliver nothing.
"##;

const HEADER: &str = r##"# The pns engine's plugin selection, as `pns setup` first wrote it. A plugin
# runs only when its table here says enabled = true, and a key this schema does
# not serve is refused by name at load, which blocks the whole file until it is
# fixed.
#
# THE BANNER AND THE PHONE CARD ARE THE CORE and are written on. Everything
# else is armed with a credential, so a commented-out block below is a feature
# nothing is set up for yet: fill its values in and uncomment it.
"##;

const MOBILE_HEAD: &str = r##"[plugins.mobile]
enabled = true
# Which compiled-in backend carries the card.
type = "moshi"
"##;

const MOBILE_WITHOUT_A_TOKEN: &str = r##"# Pair with moshi and put the webhook secret it issues here: that pairing is
# what completes the phone card.
# token = ""
"##;

const MOBILE_TAIL: &str = r##"# Whether a long command's card still fires while you are watching that pane
# on the phone.
mobile_watch_card = false
# How long pns waits for moshi to acknowledge a submitted permission prompt, in
# seconds. The harness draws the prompt only once the hook returns, so this is
# time the question is off your screen.
submit_deadline_secs = 5
"##;

const HERMES_PREAMBLE: &str = r##"# The durable paper trail: every event posted to a hermes route, signed with
# the key that route verifies.
"##;

const HERMES_DECLINED: &str = r##"# [plugins.hermes]
# enabled = true
# key = ""
"##;

const BANNER_SECTION: &str = r##"# The macOS banner, which is what a machine you are sitting at says.
[plugins.macos-banner]
enabled = true
"##;

const HUE_PREAMBLE: &str = r##"# The light pulse: the named rooms flash green when work finishes and red when
# it dies. Needs the bridge's address, a key it issued, and the rooms spelled
# the way the bridge spells them.
"##;

const HUE_DECLINED: &str = r##"# [plugins.hue]
# enabled = true
# bridge = ""
# key = ""
# rooms = []
"##;

const HUE_QUIET_HOURS: &str = r##"# The hours the room pulse stays dark: local wall clock, the start inclusive
# and the end exclusive, and it may wrap midnight.
# quiet_hours = "22:00-07:00"
"##;

const ROUTER_PREAMBLE: &str = r##"# The home probe: whether the phone is on the home wifi, answered by the
# router's own client list. A SENSOR rather than a destination, so no event
# ever routes to it; `pns home` is how it is read.
"##;

const ROUTER_DEVICE: &str = r##"# The device is named by device_hostname, device_mac or device_ipv4, at least
# one of them, and on disagreement the strongest of those three names the
# match. A phone is matched by NAME, because iOS rotates its wifi address.
"##;

const ROUTER_DECLINED: &str = r##"# [plugins.router]
# enabled = true
# type = "unifi"
# router_url = ""
# api_key = ""
# device_hostname = ""
"##;

const DAEMON_SECTION: &str = r##"# The clock: what runs BETWEEN events, for the two things that are not
# reactions to one, saying something when nothing happened and keeping a lamp
# alive while an agent loop is. It holds no state of its own, so a restart
# loses nothing and a stopped daemon costs those ambient features and never a
# card. ON UNLESS YOU SAY OTHERWISE, because it delivers nothing by itself.
[daemon]
enabled = true
"##;

const RECAP_SECTION: &str = r##"# The return recap: what you missed while you were away. THE UNCOMMENTED LINES
# ARE THE DEFAULTS, written out so they can be seen; each switch gates only its
# own delivery.
[recap]
# The catch-up card: the misses queued while you were away, put in front of you
# on the first event you are present for.
replay_card = true
# The recap of the whole window posted to hermes, rendered and posted by a
# second process that nothing waits for.
digest = true
# Whether that recap posts to the `pns-recap` route rather than the default
# one. The route has to exist in hermes first.
digest_as_thread = true
# How many events a window needs before it is worth a recap rather than the
# catch-up card alone. Every recap's header prints the window's real count,
# which is how the number gets settled.
min_events = 8
# How long the summarizer may take before it is killed and the plain list is
# posted instead. It is the whole recap's budget rather than each question's.
summarizer_deadline_secs = 240
# The command that turns the window into the night-in-order lines: ARGV, NEVER
# A SHELL STRING, handed the timeline on stdin and answering on stdout. UNSET
# IS A WORKING SETTING and posts the plain mechanical list. THE THREE OLLAMA
# FLAGS ARE NOT OPTIONAL: without them `ollama run` interleaves terminal
# control bytes and a preamble into its output, and those are posted verbatim.
# summarizer = ["ollama", "run", "qwen3.5:4b", "--think=false", "--hidethinking", "--nowordwrap"]
# The repositories whose merged pull requests become the recap's "what it does
# now" section. UNSET MEANS NO `gh` IS EVER STARTED.
# repos = ["owner/name"]
# The directory of review notes behind the "caught by review" section: ONE
# directory named in full, and a file name that may hold one `*`.
# review_notes = "/absolute/path/notes-*.md"
"##;

const FOCUS_PREAMBLE: &str = r##"# The macOS Focus modes that pns reads as your own instruction not to be
# interrupted. While one of them is active, banners, cards and light pulses are
# held back and handed over when it ends; approvals never are. NAMING NO MODE
# IS THE FEATURE OFF, which is the same statement as no table at all.
"##;

const FOCUS_DECLINED: &str = r##"# [focus]
# silence = ["Sleep"]
"##;

const NAG_PREAMBLE: &str = r##"# The nag: one more card when an approval has been sitting unanswered. IT IS A
# STATEMENT AND NEVER A SECOND PROMPT, so the card raised when the prompt
# appeared is still the one carrying Allow and Deny. It needs the daemon
# running, and several approvals waiting are one card rather than several.
# THIRTY SECONDS IS THE FLOOR AND AN HOUR THE CEILING; no table at all, and
# after_secs of zero, are the same statement.
"##;

const LIGHTS_STARTER: &str = r##"# The lamp map: WHICH LAMP says what. A declaration names a place at one of
# three levels, `[lights.lamp."<name>"]`, `[lights.room."<name>"]` or
# `[lights.zone."<name>"]`, spelled as the bridge spells it, and says which of
# the five behaviours it carries: `done` and `failed` blink, and `blocked`,
# `unread` and `loop` breathe while their condition lasts. The most specific
# declaration naming a lamp wins, and levels never merge.
# WITH NO TABLE AT ALL the pulse is the `rooms` array above and nothing else,
# so this is a starter to fill in rather than something to uncomment as it is.
# [lights]
# refresh_secs = 12
#
# [lights.done]
# duration_ms = 4000
# brightness = 100
#
# [lights.room."Studio"]
# shows = ["done", "failed"]
# dim_window = "22:00-07:00"
# dim_behaviours = ["blocked", "unread", "loop"]
"##;

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
    fn the_lamp_map_starter_is_offered_with_hue_alone_and_is_wholly_commented_out() {
        // AN EMPTY `[lights]` IS A DISTINCT STATE, the operator asking for the
        // lamps and naming none, so the starter is an example to fill in and
        // never a heading standing on its own.
        let armed = compose_config(&every_feature_armed());
        assert!(armed.contains("# [lights]"), "{armed}");
        assert!(!armed.contains("\n[lights"), "{armed}");
        assert!(parsed(&armed).lights.is_none());

        let declined = compose_config(&Answers::default());
        assert!(!declined.contains("[lights"), "{declined}");
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
