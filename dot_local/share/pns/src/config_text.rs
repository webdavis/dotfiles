//! The one config serializer. `render` walks `LAYOUT`, a static table of every
//! table and key the schema serves, in file order, and turns a values table
//! into the text `pns setup` writes and (once a values file exists) the
//! shipped template regenerates from.
//!
//! THE LAYOUT IS DATA, NOT CODE: a table lists its heading, whether it is
//! CORE (always written live) or OPT-IN (written commented when the caller
//! never mentions it), and its keys, each carrying its own comment and either
//! a real `Default` (written live when the caller leaves it out) or an
//! `Example` (written commented until the caller's own value fills it in,
//! even inside a table that is otherwise live). NO `kind` FIELD: the value's
//! own TOML type decides how it is written, so the same walk renders a bool,
//! an integer, a string, a string array or a keepassxc secret without the
//! layout naming which one a key must be.
//!
//! `render` is the other half: it CONSUMES `values` as it writes, removing
//! every key and table it recognises, and refuses BY NAME anything left over
//! once the walk is done. A values file cannot smuggle an unknown key past
//! this roster any more than a loaded config can past `config`'s.

/// One table, in file order.
pub struct Table {
    /// The heading it writes, dotted (`"plugins.mobile"`, `"lights.done"`),
    /// or a bare top-level name (`"daemon"`).
    pub name: &'static str,
    /// The comment above the heading. Carries its own `# ` prefixes and
    /// trailing newline, the way the wizard's old section constants did.
    pub prose: &'static str,
    /// CORE tables are always written live; OPT-IN tables are written
    /// commented, heading included, when the caller's values never mention
    /// them at all.
    pub opt_in: bool,
    pub keys: &'static [Key],
}

/// One key inside a table.
pub struct Key {
    pub name: &'static str,
    /// The comment above the key line, or `""` for a key nothing says more
    /// about than its table already has.
    pub prose: &'static str,
    pub sample: Sample,
}

/// What a key falls back to when the caller's values do not mention it.
pub enum Sample {
    /// Written LIVE, at this literal, in a table that is itself live.
    Default(&'static str),
    /// Written COMMENTED at this literal, even inside a live table, because
    /// there is no real default: the working setting is unset.
    Example(&'static str),
}

/// Every table this schema serves, in the order the file writes them.
pub const LAYOUT: &[Table] = &[
    Table {
        name: "plugins.mobile",
        prose: "",
        opt_in: false,
        keys: &[
            Key { name: "enabled", prose: "", sample: Sample::Default("true") },
            Key {
                name: "type",
                prose: "# Which compiled-in backend carries the card. \"moshi\" is the only one\n\
                         # today, and a table naming none, or naming one nothing answers, is\n\
                         # refused out loud rather than read as this one.\n",
                sample: Sample::Default("\"moshi\""),
            },
            Key {
                name: "token",
                prose: "# Pair with moshi and put the webhook secret it issues here: that pairing\n\
                         # is what completes the phone card.\n",
                sample: Sample::Example("\"\""),
            },
            Key {
                name: "mobile_watch_card",
                prose: "# Whether a long command's card still fires while you are watching that\n\
                         # pane on the phone. OFF: a card describing the pane already filling the\n\
                         # screen is noise, and the light pulse alone marks the command finishing.\n",
                sample: Sample::Default("false"),
            },
            Key {
                name: "submit_deadline_secs",
                prose: "# How long pns waits for moshi to acknowledge a submitted permission\n\
                         # prompt, in seconds. The harness draws the prompt only once the hook\n\
                         # returns, so this is time the question is off your screen.\n",
                sample: Sample::Default("5"),
            },
        ],
    },
    Table {
        name: "plugins.hermes",
        prose: "# The durable paper trail: every event posted to a hermes route, signed\n\
                 # with the key that route verifies.\n",
        opt_in: true,
        keys: &[
            Key { name: "enabled", prose: "", sample: Sample::Default("true") },
            Key { name: "key", prose: "", sample: Sample::Example("\"\"") },
        ],
    },
    Table {
        name: "plugins.macos-banner",
        prose: "# The macOS banner, which is what a machine you are sitting at says.\n",
        opt_in: false,
        keys: &[Key { name: "enabled", prose: "", sample: Sample::Default("true") }],
    },
    Table {
        name: "plugins.hue",
        prose: "# The light pulse: the named rooms flash green when work finishes and red\n\
                 # when it dies. Needs the bridge's address, a key it issued, and the rooms\n\
                 # spelled the way the bridge spells them.\n",
        opt_in: true,
        keys: &[
            Key { name: "enabled", prose: "", sample: Sample::Default("true") },
            Key { name: "bridge", prose: "", sample: Sample::Example("\"\"") },
            Key { name: "key", prose: "", sample: Sample::Example("\"\"") },
            Key { name: "rooms", prose: "", sample: Sample::Example("[]") },
            Key {
                name: "quiet_hours",
                prose: "# The hours the room pulse stays dark: local wall clock, the start\n\
                         # inclusive and the end exclusive, and it may wrap midnight.\n",
                sample: Sample::Example("\"22:00-07:00\""),
            },
        ],
    },
    Table {
        name: "plugins.router",
        prose: "# The home probe: whether the phone is on the home wifi, answered by the\n\
                 # router's own client list. A SENSOR rather than a destination, so no\n\
                 # event ever routes to it; `pns home` is how it is read.\n",
        opt_in: true,
        keys: &[
            Key { name: "enabled", prose: "", sample: Sample::Default("true") },
            Key { name: "type", prose: "", sample: Sample::Default("\"unifi\"") },
            Key { name: "router_url", prose: "", sample: Sample::Example("\"\"") },
            Key {
                name: "device_hostname",
                prose: "# The device is named by device_hostname, device_mac or device_ipv4, at\n\
                         # least one of them, and on disagreement the strongest of those three\n\
                         # names the match. A phone is matched by NAME, because iOS rotates its\n\
                         # wifi address.\n",
                sample: Sample::Example("\"\""),
            },
            Key { name: "device_mac", prose: "", sample: Sample::Example("\"\"") },
            Key { name: "device_ipv4", prose: "", sample: Sample::Example("\"\"") },
            Key { name: "api_key", prose: "", sample: Sample::Example("\"\"") },
            Key {
                name: "stale_alert_channel",
                prose: "# The hermes route a stale-identifier alert posts to. UNSET IS THE\n\
                         # WORKING SETTING: the alert posts to the default route.\n",
                sample: Sample::Example("\"priority\""),
            },
        ],
    },
    Table {
        name: "daemon",
        prose: DAEMON_PROSE,
        opt_in: false,
        keys: &[Key { name: "enabled", prose: "", sample: Sample::Default("true") }],
    },
    Table {
        name: "recap",
        prose: RECAP_PROSE,
        opt_in: false,
        keys: &[
            Key {
                name: "replay_card",
                prose: "# The catch-up card: the misses queued while you were away, put in front\n\
                         # of you on the first event you are present for.\n",
                sample: Sample::Default("true"),
            },
            Key {
                name: "digest",
                prose: "# The recap of the whole window posted to hermes, rendered and posted\n\
                         # by a second process that nothing waits for.\n",
                sample: Sample::Default("true"),
            },
            Key {
                name: "digest_as_thread",
                prose: "# Whether that recap posts to the `pns-recap` route rather than the\n\
                         # default one. The route has to exist in hermes first.\n",
                sample: Sample::Default("true"),
            },
            Key {
                name: "min_events",
                prose: "# How many events a window needs before it is worth a recap rather than\n\
                         # the catch-up card alone. Every recap's header prints the window's real\n\
                         # count, which is how the number gets settled.\n",
                sample: Sample::Default("8"),
            },
            Key {
                name: "summarizer",
                prose: "# The command that turns the window into the night-in-order lines:\n\
                         # ARGV, NEVER A SHELL STRING, handed the timeline on stdin and answering\n\
                         # on stdout. UNSET IS A WORKING SETTING and posts the plain mechanical\n\
                         # list.\n",
                sample: Sample::Example(
                    "[\"ollama\", \"run\", \"qwen3.5:4b\", \"--think=false\", \"--hidethinking\", \"--nowordwrap\"]",
                ),
            },
            Key {
                name: "summarizer_deadline_secs",
                prose: "# How long that command may take before it is killed and the plain list\n\
                         # is posted instead. It is the whole recap's budget rather than each\n\
                         # question's.\n",
                sample: Sample::Default("240"),
            },
            Key {
                name: "repos",
                prose: "# The repositories whose merged pull requests become the recap's \"what\n\
                         # it does now\" section. UNSET MEANS NO `gh` IS EVER STARTED.\n",
                sample: Sample::Example("[\"owner/name\"]"),
            },
            Key {
                name: "review_notes",
                prose: "# The directory of review notes behind the \"caught by review\" section:\n\
                         # ONE directory named in full, and a file name that may hold one `*`.\n",
                sample: Sample::Example("\"/absolute/path/notes-*.md\""),
            },
        ],
    },
    Table {
        name: "focus",
        prose: "# The macOS Focus modes that pns reads as your own instruction not to be\n\
                 # interrupted. While one of them is active, banners, cards and light\n\
                 # pulses are held back and handed over when it ends; approvals never are.\n\
                 # NAMING NO MODE IS THE FEATURE OFF, which is the same statement as no\n\
                 # table at all.\n",
        opt_in: true,
        keys: &[Key { name: "silence", prose: "", sample: Sample::Example("[\"Sleep\"]") }],
    },
    Table {
        name: "nag",
        prose: "# The nag: one more card when an approval has been sitting unanswered. IT\n\
                 # IS A STATEMENT AND NEVER A SECOND PROMPT, so the card raised when the\n\
                 # prompt appeared is still the one carrying Allow and Deny. It needs the\n\
                 # daemon running, and several approvals waiting are one card rather than\n\
                 # several. THIRTY SECONDS IS THE FLOOR AND AN HOUR THE CEILING; no table\n\
                 # at all, and after_secs of zero, are the same statement.\n",
        opt_in: true,
        keys: &[Key {
            name: "after_secs",
            prose: "",
            sample: Sample::Default("300"),
        }],
    },
    Table {
        name: "lights",
        prose: LIGHTS_PROSE,
        opt_in: true,
        keys: &[Key {
            name: "refresh_secs",
            prose: "# How often the daemon re-arms the lamps, in seconds. It is also the\n\
                     # breath budget: a breathing lamp is faded by the tick itself for the\n\
                     # whole interval and stops at its peak. The range is 10 to 30.\n",
            sample: Sample::Default("12"),
        }],
    },
    Table {
        name: "lights.done",
        prose: "# The five behaviour shapes. Every number below was set on a real lamp;\n\
                 # only the knobs that APPLY to a behaviour exist.\n",
        opt_in: true,
        keys: &[
            Key { name: "duration_ms", prose: "", sample: Sample::Default("4000") },
            Key { name: "brightness", prose: "", sample: Sample::Default("100") },
        ],
    },
    Table {
        name: "lights.failed",
        prose: "",
        opt_in: true,
        keys: &[
            Key { name: "duration_ms", prose: "", sample: Sample::Default("4000") },
            Key { name: "brightness", prose: "", sample: Sample::Default("100") },
        ],
    },
    Table {
        name: "lights.blocked",
        prose: "",
        opt_in: true,
        keys: &[
            Key { name: "duration_ms", prose: "", sample: Sample::Default("2000") },
            Key { name: "high", prose: "", sample: Sample::Default("100") },
            Key { name: "low", prose: "", sample: Sample::Default("30") },
        ],
    },
    Table {
        name: "lights.unread",
        prose: "",
        opt_in: true,
        keys: &[
            Key { name: "duration_ms", prose: "", sample: Sample::Default("4000") },
            Key { name: "high", prose: "", sample: Sample::Default("60") },
            Key { name: "low", prose: "", sample: Sample::Default("10") },
            Key {
                name: "after_secs",
                prose: "# How old a FINISHED run must be before its lamp arms, in seconds. A\n\
                         # run that DIED has no such delay and no knob. Zero arms at once.\n",
                sample: Sample::Default("300"),
            },
        ],
    },
    Table {
        name: "lights.loop",
        prose: "",
        opt_in: true,
        keys: &[
            Key { name: "duration_ms", prose: "", sample: Sample::Default("4000") },
            Key { name: "high", prose: "", sample: Sample::Default("60") },
            Key { name: "low", prose: "", sample: Sample::Default("10") },
            Key {
                name: "threshold_secs",
                prose: "# How long work must run continuously before the lamp arms itself.\n",
                sample: Sample::Default("300"),
            },
            Key {
                name: "lease_timeout_secs",
                prose: "# How long a lease taken by `pns loop begin` survives with nothing\n\
                         # renewing it, in seconds. The pane's own hook traffic renews it.\n",
                sample: Sample::Default("3900"),
            },
        ],
    },
    Table {
        name: "lights.dim",
        prose: "# The DIM FORM: one shape, shared by every behaviour that runs dimmed, at\n\
                 # the faintest levels the hardware has. A dimmed BLINK fires at `low`,\n\
                 # since a blink has no low end to fade to.\n",
        opt_in: true,
        keys: &[
            Key { name: "duration_ms", prose: "", sample: Sample::Default("3000") },
            Key { name: "high", prose: "", sample: Sample::Default("7") },
            Key { name: "low", prose: "", sample: Sample::Default("1") },
        ],
    },
];

const HEADER: &str = "# The pns engine's plugin selection, as `pns setup` first wrote it. A\n\
     # plugin runs only when its table here says enabled = true, and a key this\n\
     # schema does not serve is refused by name at load, which blocks the whole\n\
     # file until it is fixed.\n\
     #\n\
     # THE BANNER AND THE PHONE CARD ARE THE CORE and are written on.\n\
     # Everything else is armed with a credential, so a commented-out block\n\
     # below is a feature nothing is set up for yet: fill its values in and\n\
     # uncomment it.\n";

const DAEMON_PROSE: &str = "# The clock: what runs BETWEEN events, for the two things that are not\n\
     # reactions to one, saying something when nothing happened and keeping a\n\
     # lamp alive while an agent loop is. It holds no state of its own, so a\n\
     # restart loses nothing and a stopped daemon costs those ambient features\n\
     # and never a card. ON UNLESS YOU SAY OTHERWISE, because it delivers\n\
     # nothing by itself.\n";

const RECAP_PROSE: &str = "# The return recap: what you missed while you were away. THE UNCOMMENTED\n\
     # LINES ARE THE DEFAULTS, written out so they can be seen; each switch\n\
     # gates only its own delivery.\n";

const LIGHTS_PROSE: &str = "# The lamp map: WHICH LAMP says what. A declaration names a place at one\n\
     # of three levels, `[lights.lamp.\"<name>\"]`, `[lights.room.\"<name>\"]` or\n\
     # `[lights.zone.\"<name>\"]`, spelled as the bridge spells it, and says\n\
     # which of the five behaviours it carries: `done` and `failed` blink, and\n\
     # `blocked`, `unread` and `loop` breathe while their condition lasts. The\n\
     # most specific declaration naming a lamp wins, and levels never merge.\n\
     # WITH NO TABLE AT ALL the pulse is the `rooms` array above and nothing\n\
     # else.\n";

/// The closing prose: the ad-hoc mute command, which reads about the config
/// rather than being one. IT IS ALWAYS SHOWN, whether or not `[lights]` is
/// armed, because the command exists whichever way that table reads.
const TRAILER: &str = "\n# ONE MORE MUTE, TYPED RATHER THAN CONFIGURED, and it is LIGHTS ONLY:\n\
     #\n\
     #   pns lights quiet \"3F - Studio\" 2h   quiet that place's lamps for two hours\n\
     #   pns lights quiet \"3F - Studio\"      quiet them until quiet hours end\n\
     #   pns lights quiet \"3F - Studio\" off  loud again\n\
     #   pns lights quiet                    what is quiet right now\n\
     #\n\
     # It silences EVERY behaviour on the target and reaches the lamps of one\n\
     # lamp, room or zone and nothing else. THE NAMES IT TAKES ARE EVERY LAMP,\n\
     # ROOM AND ZONE, whether a declaration above writes it or the bridge\n\
     # merely holds it, and a name neither knows is refused with the list of\n\
     # the ones that work.\n";

/// One answer as a TOML basic string.
///
/// A PASTED SECRET IS UNTRUSTED TEXT and is escaped rather than refused: raw
/// interpolation composes a file that will not load at best, and at worst one
/// whose value stops where the operator's own quote did.
pub fn quoted(value: &str) -> String {
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
pub fn quoted_list(values: &[String]) -> String {
    let quoted: Vec<String> = values.iter().map(|value| quoted(value)).collect();
    format!("[{}]", quoted.join(", "))
}

/// The whole config text, built off a values table shaped like the config
/// itself: `{ plugins = { mobile = { ... }, ... }, recap = { ... }, ... }`.
///
/// EVERY KEY AND TABLE IN `values` IS CONSUMED as it is written, so anything
/// left over once the walk is done is a name this schema does not serve, and
/// the whole render is refused rather than silently dropping it.
pub fn render(values: &toml::Table) -> Result<String, String> {
    let mut remaining = values.clone();
    let mut out = String::new();
    out.push_str(HEADER);
    out.push('\n');

    let mut plugins = take_table(&mut remaining, "plugins")?;
    render_core(&mut out, find_table("plugins.mobile"), &mut plugins)?;
    render_opt_in(&mut out, find_table("plugins.hermes"), &mut plugins)?;
    render_core(&mut out, find_table("plugins.macos-banner"), &mut plugins)?;
    render_opt_in(&mut out, find_table("plugins.hue"), &mut plugins)?;
    render_opt_in(&mut out, find_table("plugins.router"), &mut plugins)?;
    if let Some(name) = plugins.keys().next() {
        return Err(format!("unknown plugin `{name}`"));
    }

    render_core(&mut out, find_table("daemon"), &mut remaining)?;
    render_core(&mut out, find_table("recap"), &mut remaining)?;
    render_opt_in(&mut out, find_table("focus"), &mut remaining)?;
    render_opt_in(&mut out, find_table("nag"), &mut remaining)?;
    render_lights(&mut out, &mut remaining)?;

    out.push_str(TRAILER);

    if let Some(name) = remaining.keys().next() {
        return Err(format!("unknown top-level key `{name}`"));
    }
    Ok(out)
}

/// `Answers::values()` composes a table with no `daemon`, `recap` or
/// `lights` key at all, which is exactly what a CORE table's own default
/// path is for: absent means "unmodified," never "off."
fn find_table(name: &str) -> &'static Table {
    LAYOUT
        .iter()
        .find(|table| table.name == name)
        .unwrap_or_else(|| panic!("`{name}` is declared nowhere in LAYOUT"))
}

/// Removes `key` from `container` as a table, or an empty one when it is
/// absent, refusing by name when it is present but not a table.
fn take_table(container: &mut toml::Table, key: &str) -> Result<toml::Table, String> {
    match container.remove(key) {
        None => Ok(toml::Table::new()),
        Some(toml::Value::Table(table)) => Ok(table),
        Some(other) => Err(format!(
            "`{key}` has type `{}`, not a table",
            other.type_str()
        )),
    }
}

fn last_segment(dotted: &str) -> &str {
    dotted.rsplit('.').next().unwrap_or(dotted)
}

/// A CORE table: always written live, whether or not `container` mentions it.
fn render_core(out: &mut String, table: &Table, container: &mut toml::Table) -> Result<(), String> {
    let mut settings = take_table(container, last_segment(table.name))?;
    render_block(out, table, &mut settings, true)
}

/// An OPT-IN table: written commented, heading and all, when `container`
/// never mentions it at all.
fn render_opt_in(out: &mut String, table: &Table, container: &mut toml::Table) -> Result<(), String> {
    match container.remove(last_segment(table.name)) {
        None => render_block(out, table, &mut toml::Table::new(), false),
        Some(toml::Value::Table(mut settings)) => render_block(out, table, &mut settings, true),
        Some(other) => Err(format!(
            "`{}` has type `{}`, not a table",
            table.name,
            other.type_str()
        )),
    }
}

/// Writes one table's prose, heading and keys. `present` decides whether the
/// heading and every `Default` key are live or commented; an `Example` key
/// stays commented either way unless `settings` itself carries a value for
/// it, in which case that value always wins and is always written live.
fn render_block(
    out: &mut String,
    table: &Table,
    settings: &mut toml::Table,
    present: bool,
) -> Result<(), String> {
    out.push_str(table.prose);
    write_note(out, take_note(settings)?);
    if present {
        out.push_str(&format!("[{}]\n", table.name));
    } else {
        out.push_str(&format!("# [{}]\n", table.name));
    }
    for key in table.keys {
        out.push_str(key.prose);
        match settings.remove(key.name) {
            Some(value) => {
                let rendered = render_value(&value)
                    .map_err(|error| format!("`{}` key `{}`: {error}", table.name, key.name))?;
                out.push_str(&format!("{} = {rendered}\n", key.name));
            }
            None => {
                let (literal, force_commented) = match key.sample {
                    Sample::Default(literal) => (literal, false),
                    Sample::Example(literal) => (literal, true),
                };
                if present && !force_commented {
                    out.push_str(&format!("{} = {literal}\n", key.name));
                } else {
                    out.push_str(&format!("# {} = {literal}\n", key.name));
                }
            }
        }
    }
    out.push('\n');
    if let Some(name) = settings.keys().next() {
        return Err(format!("unknown `{}` key `{name}`", table.name));
    }
    Ok(())
}

/// The `[lights]` cluster: one presence flag governs seven headings, because
/// `Config.lights` is one `Option` for the whole table, never seven.
fn render_lights(out: &mut String, remaining: &mut toml::Table) -> Result<(), String> {
    let present = remaining.contains_key("lights");
    let mut lights = take_table(remaining, "lights")?;

    render_block(out, find_table("lights"), &mut lights, present)?;
    for cluster in ["done", "failed", "blocked", "unread", "loop", "dim"] {
        let mut settings = take_table(&mut lights, cluster)?;
        render_block(
            out,
            find_table(&format!("lights.{cluster}")),
            &mut settings,
            present,
        )?;
    }
    for level in ["lamp", "room", "zone"] {
        let declarations = take_table(&mut lights, level)?;
        for (name, entry) in declarations {
            let toml::Value::Table(mut settings) = entry else {
                return Err(format!("`lights.{level}.{name}` is not a table"));
            };
            render_target(out, level, &name, &mut settings)?;
        }
    }
    if let Some(name) = lights.keys().next() {
        return Err(format!("unknown `lights` key `{name}`"));
    }
    Ok(())
}

/// One `[lights.lamp."<name>"]`, `[lights.room."<name>"]` or
/// `[lights.zone."<name>"]` declaration.
fn render_target(
    out: &mut String,
    level: &str,
    name: &str,
    settings: &mut toml::Table,
) -> Result<(), String> {
    write_note(out, take_note(settings)?);
    out.push_str(&format!("[lights.{level}.{}]\n", quoted(name)));
    for key in ["shows", "dim_window", "dim_behaviours"] {
        if let Some(value) = settings.remove(key) {
            let rendered = render_value(&value)
                .map_err(|error| format!("`lights.{level}.{name}` key `{key}`: {error}"))?;
            out.push_str(&format!("{key} = {rendered}\n"));
        }
    }
    out.push('\n');
    if let Some(name) = settings.keys().next() {
        return Err(format!("unknown `lights.{level}` key `{name}`"));
    }
    Ok(())
}

/// Removes and returns `note` off `settings`, refusing by name when it is
/// there but not a string.
///
/// A RESERVED KEY, invisible to the roster: it never reaches the output as
/// `note = "..."`, only as the comment `write_note` turns it into, so a
/// parsed config never carries one.
fn take_note(settings: &mut toml::Table) -> Result<Option<String>, String> {
    match settings.remove("note") {
        None => Ok(None),
        Some(toml::Value::String(note)) => Ok(Some(note)),
        Some(other) => Err(format!("`note` has type `{}`, not a string", other.type_str())),
    }
}

/// Writes `note` as one or more `# `-prefixed comment lines, or nothing at
/// all when there is none.
///
/// EVERY LINE GETS ITS OWN `# `, which is what keeps a newline inside the
/// operator's own text from opening a heading or an uncommented key: nothing
/// this function writes can ever start a line without that prefix, however
/// many newlines the note carries.
fn write_note(out: &mut String, note: Option<String>) {
    let Some(note) = note else { return };
    if note.is_empty() {
        out.push_str("#\n");
        return;
    }
    for line in note.split('\n') {
        out.push_str("# ");
        out.push_str(line);
        out.push('\n');
    }
}

/// One value, written the way its own TOML type is spelled: a bool and an
/// integer are literals, a string and a string array are escaped, and a table
/// shaped `{ keepassxc = "<entry>", field = "Password" | "UserName" }` is a
/// chezmoi action rather than a TOML value at all. Nothing else renders.
fn render_value(value: &toml::Value) -> Result<String, String> {
    match value {
        toml::Value::Boolean(flag) => Ok(flag.to_string()),
        toml::Value::Integer(count) => Ok(count.to_string()),
        toml::Value::String(text) => Ok(quoted(text)),
        toml::Value::Array(items) => {
            let mut strings = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    toml::Value::String(text) => strings.push(text.clone()),
                    other => {
                        return Err(format!(
                            "an array element has type `{}`, not a string",
                            other.type_str()
                        ));
                    }
                }
            }
            Ok(quoted_list(&strings))
        }
        toml::Value::Table(table) => secret_action(table),
        other => Err(format!("type `{}` does not render", other.type_str())),
    }
}

/// The keepassxc entry fields that may stand as a secret's `field`. Anything
/// else is refused, because `field` becomes a chezmoi method call verbatim.
const SECRET_FIELDS: [&str; 2] = ["Password", "UserName"];

/// A secret marker, `{ keepassxc = "<entry>", field = "Password" | "UserName" }`,
/// as the chezmoi action `"{{ (keepassxc "<entry>").<field> }}"`, quoted the
/// way the shipped template quotes every secret it carries.
///
/// A RENDERED SECRET IS NOT TOML, which is exactly why the quotes are part of
/// this text rather than added by the ordinary string path: chezmoi replaces
/// the `{{ ... }}` action with the vault value's own bytes, unescaped, inside
/// whatever quotes the template already wrote around it. Only what comes back
/// from THAT substitution is TOML; this render's own job is to write the
/// template text, not the file chezmoi eventually produces.
///
/// NEITHER STRING IS TRUSTED. `field` is checked against the two chezmoi
/// methods a keepassxc entry actually exposes, and the entry name is refused
/// if it carries a quote, a backslash or `}}`: any of the three would close
/// the Go string or the action early and let the rest of the entry name run
/// as template syntax.
fn secret_action(table: &toml::Table) -> Result<String, String> {
    if table.len() != 2 {
        return Err(
            "a table value must be a secret: exactly `keepassxc` and `field`".to_string(),
        );
    }
    let entry = match table.get("keepassxc") {
        Some(toml::Value::String(entry)) => entry,
        _ => return Err("a secret's `keepassxc` must name the entry as a string".to_string()),
    };
    let field = match table.get("field") {
        Some(toml::Value::String(field)) => field,
        _ => return Err("a secret's `field` must be a string".to_string()),
    };
    if !SECRET_FIELDS.contains(&field.as_str()) {
        return Err(format!(
            "a secret's `field` must be one of {SECRET_FIELDS:?}, not `{field}`"
        ));
    }
    if entry.contains('"') || entry.contains('\\') || entry.contains("}}") {
        return Err(format!(
            "the keepassxc entry name `{entry}` cannot stand inside a chezmoi action"
        ));
    }
    // BUILT WITH `push_str` RATHER THAN `format!`, deliberately: the target
    // text is thick with literal `{`, `}` and `"` characters, and escaping all
    // of them inside a format string is exactly the kind of place a stray
    // brace goes unnoticed.
    let mut action = String::with_capacity(entry.len() + field.len() + 20);
    action.push('"');
    action.push_str("{{ (keepassxc \"");
    action.push_str(entry);
    action.push_str("\").");
    action.push_str(field);
    action.push_str(" }}");
    action.push('"');
    Ok(action)
}

#[cfg(test)]
mod tests {
    use super::render;
    use crate::config::parse_config;

    /// A values table naming every core and opt-in table, all literals: the
    /// shape `Answers::values()` produces once every question is answered.
    fn every_table_armed() -> toml::Table {
        toml::toml! {
            [plugins.mobile]
            token = "moshi-secret"

            [plugins.hermes]
            key = "hermes-secret"

            [plugins.hue]
            bridge = "192.168.1.9"
            key = "hue-secret"
            rooms = ["Studio", "Kitchen"]

            [plugins.router]
            type = "unifi"
            router_url = "https://192.168.1.1"
            api_key = "router-secret"
            device_hostname = "phone"

            [focus]
            silence = ["Sleep"]

            [nag]
        }
    }

    #[test]
    fn every_answered_table_renders_and_parses_back_carrying_its_own_values() {
        let text = render(&every_table_armed()).expect("a fully answered walk renders");
        let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));

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
    fn an_empty_walk_still_renders_the_core_at_its_defaults() {
        let text = render(&toml::Table::new()).expect("an empty walk still renders");
        let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
        assert!(config.plugins["mobile"].enabled);
        assert!(config.plugins["macos-banner"].enabled);
        assert!(config.daemon_enabled);
        assert_eq!(config.recap, crate::config::Recap::default());
        for opt_in in ["hermes", "hue", "router"] {
            assert!(!config.plugins.contains_key(opt_in));
        }
        assert!(config.focus_silence.is_empty());
        assert_eq!(config.nag_after_secs, 0);
        assert!(config.lights.is_none());
    }

    #[test]
    fn an_armed_but_unspecified_lights_table_renders_every_locked_default_uncommented() {
        // ANY LIGHTS KEY AT ALL is the operator asking for the lamps, so
        // every one of the five locked shapes is written live rather than
        // waiting on a value nobody supplied. The assertion is against the
        // code's own `Default`, never against a literal copied out of the
        // layout, so a default that drifts here fails this test rather than
        // shipping quietly.
        let mut values = toml::Table::new();
        values.insert("lights".to_string(), toml::Value::Table(toml::Table::new()));
        let text = render(&values).expect("an armed-empty lights table renders");
        let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
        assert_eq!(
            *config.lights.expect("lights was armed"),
            crate::config::Lights::default()
        );
    }

    #[test]
    fn recap_defaults_are_asserted_against_the_code_rather_than_copied_literals() {
        let text = render(&toml::Table::new()).expect("an empty walk still renders");
        let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
        assert_eq!(config.recap, crate::config::Recap::default());
        assert_eq!(
            config.plugins["mobile"].settings["submit_deadline_secs"].as_integer(),
            Some(crate::config::DEFAULT_SUBMIT_DEADLINE_SECS as i64)
        );
    }

    /// A secret marker for one keepassxc entry and field.
    fn secret(entry: &str, field: &str) -> toml::Value {
        let mut table = toml::Table::new();
        table.insert("keepassxc".to_string(), toml::Value::String(entry.to_string()));
        table.insert("field".to_string(), toml::Value::String(field.to_string()));
        toml::Value::Table(table)
    }

    #[test]
    fn a_secret_marker_renders_as_the_chezmoi_action_and_a_literal_renders_quoted() {
        let mut mobile = toml::Table::new();
        mobile.insert(
            "token".to_string(),
            secret("Moshi :: Webhook Secret", "Password"),
        );
        let mut plugins = toml::Table::new();
        plugins.insert("mobile".to_string(), toml::Value::Table(mobile));
        let mut values = toml::Table::new();
        values.insert("plugins".to_string(), toml::Value::Table(plugins));

        let text = render(&values).expect("a secret marker renders");
        assert!(
            text.contains(
                "token = \"{{ (keepassxc \"Moshi :: Webhook Secret\").Password }}\""
            ),
            "{text}"
        );
        // AND A LITERAL RENDERS QUOTED, right beside it: `type` still comes
        // off the layout's own `Default`, escaped the ordinary way.
        assert!(text.contains("type = \"moshi\""), "{text}");

        // A RENDERED SECRET IS NOT TOML: the action's own `"` sits unescaped
        // inside what would otherwise be a basic string, so it has to be
        // faked into vault output before the whole file can parse.
        let rendered = crate::config::strip_chezmoi_actions(&text, "from-the-vault");
        let config =
            parse_config(&rendered).unwrap_or_else(|error| panic!("{error:?}\n{rendered}"));
        assert_eq!(
            config.plugins["mobile"].settings["token"].as_str(),
            Some("from-the-vault")
        );
    }

    #[test]
    fn a_secrets_field_is_whitelisted_to_the_two_chezmoi_methods() {
        let error = super::secret_action(
            secret("Moshi :: Webhook Secret", "Notes").as_table().unwrap(),
        )
        .expect_err("Notes is not a field keepassxc exposes to chezmoi");
        assert!(error.contains("Notes"), "{error}");
    }

    #[test]
    fn a_hostile_entry_name_is_refused_rather_than_closing_the_chezmoi_action() {
        for hostile in ["a\"b", "a\\b", "a}}b"] {
            let error = super::secret_action(secret(hostile, "Password").as_table().unwrap())
                .expect_err(&format!("`{hostile}` can break out of the action and must be refused"));
            assert!(error.contains(hostile), "{error}");
        }
    }

    #[test]
    fn a_note_renders_above_its_heading_as_a_commented_line() {
        let mut hermes = toml::Table::new();
        hermes.insert(
            "note".to_string(),
            toml::Value::String("armed for the pns-recap route".to_string()),
        );
        hermes.insert(
            "key".to_string(),
            toml::Value::String("hermes-secret".to_string()),
        );
        let mut plugins = toml::Table::new();
        plugins.insert("hermes".to_string(), toml::Value::Table(hermes));
        let mut values = toml::Table::new();
        values.insert("plugins".to_string(), toml::Value::Table(plugins));

        let text = render(&values).expect("a noted table renders");
        assert!(
            text.contains("# armed for the pns-recap route\n[plugins.hermes]"),
            "{text}"
        );
        // AND `note` NEVER REACHES THE PARSED CONFIG: it is a renderer
        // directive, not a key the roster serves, so stripping it before a
        // round-trip comparison is not a workaround, it never round-trips.
        let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
        assert!(!config.plugins["hermes"].settings.contains_key("note"));
    }

    #[test]
    fn a_note_holding_a_newline_stays_commented_on_every_line() {
        // THE INJECTION CASE. A note that could open a live heading or an
        // uncommented key on its second line would let a values file smuggle
        // arbitrary config text past every other refusal in this module.
        let mut hermes = toml::Table::new();
        hermes.insert(
            "note".to_string(),
            toml::Value::String(
                "line one\n[plugins.hue]\nenabled = true\nbridge = \"hostile\"".to_string(),
            ),
        );
        hermes.insert(
            "key".to_string(),
            toml::Value::String("hermes-secret".to_string()),
        );
        let mut plugins = toml::Table::new();
        plugins.insert("hermes".to_string(), toml::Value::Table(hermes));
        let mut values = toml::Table::new();
        values.insert("plugins".to_string(), toml::Value::Table(plugins));

        let text = render(&values).expect("a multi-line note renders");
        for line in text.lines() {
            if line.contains("hostile") {
                assert!(line.starts_with('#'), "an injected line escaped its comment: {line}");
            }
        }
        let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
        // AND THE INJECTED TABLE NEVER ARRIVED: a real `[plugins.hue]` armed
        // by the note would be the exact failure this test exists to catch.
        assert!(!config.plugins.contains_key("hue"));
    }
}
