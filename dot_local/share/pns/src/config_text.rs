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
            Key {
                name: "enabled",
                prose: "",
                sample: Sample::Default("true"),
            },
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
                         # screen is noise, and the light pulse alone marks the command finishing;\n\
                         # set true to be carded anyway.\n",
                sample: Sample::Default("false"),
            },
            Key {
                name: "submit_deadline_secs",
                prose: "# How long pns waits for moshi to acknowledge a submitted permission\n\
                         # prompt, in seconds. The harness draws the prompt only once the hook\n\
                         # returns, so this is time the question is off your screen. On expiry\n\
                         # the submission is killed and its pending card dies with it, and\n\
                         # nothing is said either way. There is no off switch: zero, a negative,\n\
                         # a fraction and anything past 3600 are refused by name.\n",
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
            Key {
                name: "enabled",
                prose: "",
                sample: Sample::Default("true"),
            },
            Key {
                name: "key",
                prose: "",
                sample: Sample::Example("\"\""),
            },
        ],
    },
    Table {
        name: "plugins.macos-banner",
        prose: "# The macOS banner, which is what a machine you are sitting at says.\n",
        opt_in: false,
        keys: &[Key {
            name: "enabled",
            prose: "",
            sample: Sample::Default("true"),
        }],
    },
    Table {
        name: "plugins.hue",
        prose: "# The light pulse: the named rooms flash green when work finishes and red\n\
                 # when it dies. Needs the bridge's address, a key it issued, and the rooms\n\
                 # spelled the way the bridge spells them.\n",
        opt_in: true,
        keys: &[
            Key {
                name: "enabled",
                prose: "",
                sample: Sample::Default("true"),
            },
            Key {
                name: "bridge",
                prose: "",
                sample: Sample::Example("\"\""),
            },
            Key {
                name: "key",
                prose: "",
                sample: Sample::Example("\"\""),
            },
            Key {
                name: "rooms",
                prose: "",
                sample: Sample::Example("[]"),
            },
            Key {
                name: "quiet_hours",
                prose: "# The hours the room pulse stays dark: local wall clock, the start\n\
                         # inclusive and the end exclusive, and it may wrap midnight. A hand-run\n\
                         # `pns pulse` is exempt, so a bridge and key can be checked in-window.\n\
                         # A bare `pns lights quiet <place>` mutes until this window ends and is\n\
                         # refused when none is set. With a `[lights]` table below, each place's\n\
                         # own `dim_window` decides the night instead and this window is the\n\
                         # mute's schedule alone.\n",
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
            Key {
                name: "enabled",
                prose: "",
                sample: Sample::Default("true"),
            },
            Key {
                name: "type",
                prose: "# Which compiled-in backend answers. \"unifi\" is the only one today, and\n\
                         # a table naming none, or naming one nothing answers, is refused out loud.\n",
                sample: Sample::Default("\"unifi\""),
            },
            Key {
                name: "router_url",
                prose: "",
                sample: Sample::Example("\"\""),
            },
            Key {
                name: "device_hostname",
                prose: "# The device is named by device_mac, device_hostname or device_ipv4, at\n\
                         # least one of them; any one matching a client reads home, and on\n\
                         # disagreement the strongest, in that order, names the match. A phone is\n\
                         # matched by NAME, because iOS rotates its wifi address; device_mac is\n\
                         # for a device whose address stays put.\n",
                sample: Sample::Example("\"\""),
            },
            Key {
                name: "device_mac",
                prose: "",
                sample: Sample::Example("\"\""),
            },
            Key {
                name: "device_ipv4",
                prose: "",
                sample: Sample::Example("\"\""),
            },
            Key {
                name: "api_key",
                prose: "",
                sample: Sample::Example("\"\""),
            },
            Key {
                name: "stale_alert_channel",
                prose: "# The hermes route a stale-identifier alert posts to. UNSET IS THE\n\
                         # WORKING SETTING: the alert posts to the default route. Naming another\n\
                         # needs that hermes route prepared first, with the pns signing secret\n\
                         # and a pns-shaped prompt, or the POST is rejected and the leg is silent.\n",
                sample: Sample::Example("\"priority\""),
            },
        ],
    },
    Table {
        name: "daemon",
        prose: DAEMON_PROSE,
        opt_in: false,
        keys: &[Key {
            name: "enabled",
            prose: "",
            sample: Sample::Default("true"),
        }],
    },
    Table {
        name: "recap",
        prose: RECAP_PROSE,
        opt_in: false,
        keys: &[
            Key {
                name: "replay_card",
                prose: "# The catch-up card: the misses queued while you were away, put in front\n\
                         # of you on the first event you are present for. Off never cards from\n\
                         # the journal; the misses stay recorded whatever this says, so switching\n\
                         # the card back on has something to deliver.\n",
                sample: Sample::Default("true"),
            },
            Key {
                name: "digest",
                prose: "# The recap of the whole window posted to hermes, rendered and posted\n\
                         # by a second process that nothing waits for. Off records the window\n\
                         # just the same; only the posting stops.\n",
                sample: Sample::Default("true"),
            },
            Key {
                name: "digest_as_thread",
                prose: "# Whether that recap posts to the `pns-recap` route rather than the\n\
                         # default one. The route has to exist in hermes first, prepared with\n\
                         # the pns signing secret and a prompt of bare `{detail}`; a route that\n\
                         # refuses the post is not silent, the recap goes to the default route\n\
                         # instead, carrying one line saying why it landed there.\n",
                sample: Sample::Default("true"),
            },
            Key {
                name: "min_events",
                prose: "# How many events a window needs before it is worth a recap rather than\n\
                         # the catch-up card alone. Every recap's header prints the window's real\n\
                         # count, which is how the number gets settled. One is the floor and\n\
                         # means any activity at all; zero is refused.\n",
                sample: Sample::Default("8"),
            },
            Key {
                name: "summarizer",
                prose: "# The command that turns the window into the night-in-order lines:\n\
                         # ARGV, NEVER A SHELL STRING, handed the timeline on stdin and answering\n\
                         # on stdout. UNSET IS A WORKING SETTING and posts the plain mechanical\n\
                         # list, and so does a summarizer that fails, is missing, says nothing\n\
                         # or runs long, which the list's own heading says. THE THREE OLLAMA\n\
                         # FLAGS ARE NOT OPTIONAL: without them `ollama run` interleaves terminal\n\
                         # control bytes and a preamble into its output, posted verbatim.\n",
                sample: Sample::Example(
                    "[\"ollama\", \"run\", \"qwen3.5:4b\", \"--think=false\", \"--hidethinking\", \"--nowordwrap\"]",
                ),
            },
            Key {
                name: "summarizer_deadline_secs",
                prose: "# How long that command may take before it is killed and the plain list\n\
                         # is posted instead. It is the whole recap's budget rather than each\n\
                         # question's, and AN HOUR IS THE CEILING: a bigger number is refused by\n\
                         # name.\n",
                sample: Sample::Default("240"),
            },
            Key {
                name: "repos",
                prose: "# The repositories whose merged pull requests become the recap's \"what\n\
                         # it does now\" section. UNSET IS THE WORKING SETTING and it is a fence:\n\
                         # with no repo named, no `gh` process is started at all. Named, the recap\n\
                         # runs one read-only `gh pr list` per repo, bounded in count and in time,\n\
                         # over the window alone; it never touches a token, and `gh`'s own login is\n\
                         # what authorizes it. Each line carries the pull request number it came\n\
                         # from, and a line that cannot be traced back to one pns actually fetched\n\
                         # is dropped rather than posted. A `gh` that is missing, refuses or runs\n\
                         # long costs this section and nothing else. `gh` IS FOUND ON PATH, and the\n\
                         # PATH is the one the event that started the recap was handed: a hook\n\
                         # environment without /opt/homebrew/bin reads `gh` as permanently\n\
                         # unavailable, and the section says so on every window until the harness's\n\
                         # own PATH carries it.\n",
                sample: Sample::Example("[\"owner/name\"]"),
            },
            Key {
                name: "review_notes",
                prose: "# The review notes whose findings become the recap's \"caught by review\"\n\
                         # section: ONE directory, named in full, and a file name that may hold\n\
                         # one `*`. A relative path and a `*` in a directory are both refused,\n\
                         # because this pattern is the whole of what pns is allowed to open. Only\n\
                         # files whose own clock falls inside the window are read, so a note you\n\
                         # had already seen before you left is not news. UNSET IS THE WORKING\n\
                         # SETTING and, as with `repos`, unset means the directory is never\n\
                         # opened. Twenty-five notes is what one recap considers, NEWEST FIRST,\n\
                         # and a window holding more says \"at least\" in its own count rather than\n\
                         # printing a total it cannot back; a matched note that will not open is\n\
                         # named as one that could not be read rather than left out.\n",
                sample: Sample::Example("\"/absolute/path/notes-*.md\""),
            },
        ],
    },
    Table {
        name: "focus",
        prose: "# The macOS Focus modes that pns reads as your own instruction not to be\n\
                 # interrupted. While one of them is active, banners, cards and light\n\
                 # pulses are held back and handed over when it ends; approvals never are,\n\
                 # and neither is the durable log. A name matches however you capitalised\n\
                 # it, a mode's raw modeIdentifier works too, and an empty entry is refused\n\
                 # by name. An unreadable Focus store reads as no Focus, never as silence.\n\
                 # NAMING NO MODE IS THE FEATURE OFF, which is the same statement as no\n\
                 # table at all.\n",
        opt_in: true,
        keys: &[Key {
            name: "silence",
            prose: "",
            sample: Sample::Example("[\"Sleep\"]"),
        }],
    },
    Table {
        name: "nag",
        prose: "# The nag: one more card when an approval has been sitting unanswered. IT\n\
                 # IS A STATEMENT AND NEVER A SECOND PROMPT, so the card raised when the\n\
                 # prompt appeared is still the one carrying Allow and Deny. It needs the\n\
                 # daemon running and the PostToolBatch hook entry that tells pns an\n\
                 # approval was dealt with; without that entry the only clearing signal\n\
                 # is the end of the turn. It respects every mute the first card respects,\n\
                 # a `pns quiet`, a Focus, the quiet window, and a nag held back is LOST\n\
                 # rather than queued. Several approvals waiting are one card rather than\n\
                 # several, each approval is nagged at most once, and a card counts every\n\
                 # approval outstanding at that moment, so a fresh one can be named early\n\
                 # and is then done. The signal is the tool batch RESOLVING rather than\n\
                 # your answer, so a tool approved at once that then runs longer than this\n\
                 # is nagged about anyway; if that bites, raise the number. THIRTY SECONDS\n\
                 # IS THE FLOOR AND AN HOUR THE CEILING, anything outside is refused by\n\
                 # name; no table at all, and after_secs of zero, are the same statement.\n",
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
            prose: "# How often the daemon re-arms the lamps, in seconds. It is also the breath\n\
                     # budget: a breathing lamp is faded by the tick itself, seamlessly, across the\n\
                     # whole interval, so this decides how many fades fit between two ticks. The\n\
                     # range is 10 to 30. The floor is one bridge call, so a tick cannot start while\n\
                     # the last one is still dialling; the ceiling is what the daemon derives a\n\
                     # tick's own lifetime from, and an interval past it would be a breath cut off\n\
                     # part way through.\n",
            sample: Sample::Default("12"),
        }],
    },
    Table {
        name: "lights.done",
        prose: "# The five behaviour shapes. Every number below was set on a real lamp;\n\
                 # only the knobs that APPLY to a behaviour exist, so a blink has a\n\
                 # duration and one brightness and a breath has a duration and two ends.\n",
        opt_in: true,
        keys: &[
            Key {
                name: "duration_ms",
                prose: "",
                sample: Sample::Default("4000"),
            },
            Key {
                name: "brightness",
                prose: "",
                sample: Sample::Default("100"),
            },
        ],
    },
    Table {
        name: "lights.failed",
        prose: "",
        opt_in: true,
        keys: &[
            Key {
                name: "duration_ms",
                prose: "",
                sample: Sample::Default("4000"),
            },
            Key {
                name: "brightness",
                prose: "",
                sample: Sample::Default("100"),
            },
        ],
    },
    Table {
        name: "lights.blocked",
        prose: "",
        opt_in: true,
        keys: &[
            Key {
                name: "duration_ms",
                prose: "",
                sample: Sample::Default("2000"),
            },
            Key {
                name: "high",
                prose: "",
                sample: Sample::Default("100"),
            },
            Key {
                name: "low",
                prose: "",
                sample: Sample::Default("30"),
            },
            Key {
                name: "give_up_after_secs",
                prose: "# How long an unanswered wait may hold the lamp before the daemon gives\n\
                         # up on an abandoned session, in seconds. This is a BACKSTOP, not an\n\
                         # expiry: the locked behaviour is blue breathing continuous until you\n\
                         # answer, and the ordinary end is your session's next event, whatever the\n\
                         # hour. 57600 (16 hours) outlasts a long day away and still gives the lamp\n\
                         # back before the next one starts. The range is 60 to 604800 (a week),\n\
                         # since an abandoned wait can span a weekend away.\n",
                sample: Sample::Default("57600"),
            },
        ],
    },
    Table {
        name: "lights.unread",
        prose: "",
        opt_in: true,
        keys: &[
            Key {
                name: "duration_ms",
                prose: "",
                sample: Sample::Default("4000"),
            },
            Key {
                name: "high",
                prose: "",
                sample: Sample::Default("60"),
            },
            Key {
                name: "low",
                prose: "",
                sample: Sample::Default("10"),
            },
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
            Key {
                name: "duration_ms",
                prose: "",
                sample: Sample::Default("6000"),
            },
            Key {
                name: "high",
                prose: "",
                sample: Sample::Default("60"),
            },
            Key {
                name: "low",
                prose: "",
                sample: Sample::Default("10"),
            },
            Key {
                name: "threshold_secs",
                prose: "# How long work must run continuously before the lamp arms itself, in\n\
                         # seconds. Both an agent herdr calls working and a tracked shell command\n\
                         # count.\n",
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
            Key {
                name: "duration_ms",
                prose: "",
                sample: Sample::Default("3000"),
            },
            Key {
                name: "high",
                prose: "",
                sample: Sample::Default("7"),
            },
            Key {
                name: "low",
                prose: "",
                sample: Sample::Default("1"),
            },
        ],
    },
];

const HEADER: &str = "# The pns engine's plugin selection, as `pns setup` first wrote it. A\n\
     # plugin runs only when its table here says enabled = true, and a key this\n\
     # schema does not serve is refused by name at load, which blocks the whole\n\
     # file until it is fixed: pns falls back to its built-in roster, every\n\
     # secret in here goes unread, and the refusal on stderr names the key.\n\
     #\n\
     # THE BANNER AND THE PHONE CARD ARE THE CORE and are written on. Three of\n\
     # the plugins below are OPT-INS you arm with a credential first: hue needs\n\
     # a bridge and key, hermes needs a signed route, and the home probe needs\n\
     # a router API key, so switching them on by default would deliver nothing\n\
     # and report three failures. Focus, the nag and the lamp map are separate\n\
     # opt-ins below `[plugins]` and need no credential at all. A commented-out\n\
     # block below is a feature nothing is set up for yet: fill its values in\n\
     # and uncomment it. A plugin names its backend with `type`, and the key is\n\
     # required: nothing guesses which implementation a table meant.\n";

const DAEMON_PROSE: &str = "# The clock: what runs BETWEEN events, for the two things that are not\n\
     # reactions to one, saying something when nothing happened and keeping a\n\
     # lamp alive while an agent loop is. It holds no state of its own, so a\n\
     # restart loses nothing and a stopped daemon costs those ambient features\n\
     # and never a card. ON UNLESS YOU SAY OTHERWISE, because it delivers\n\
     # nothing by itself; deleting the table is the same statement, and\n\
     # `pns doctor` says which state it is in.\n";

const RECAP_PROSE: &str = "# The return recap: what you missed while you were away. THE UNCOMMENTED\n\
     # LINES ARE THE DEFAULTS, written out so they can be seen; each switch\n\
     # gates only its own delivery, and deleting the whole table gets exactly\n\
     # the same behaviour.\n";

const LIGHTS_PROSE: &str = "# The lamp map: WHICH LAMP says what. A declaration names a place at one\n\
     # of three levels, `[lights.lamp.\"<name>\"]`, `[lights.room.\"<name>\"]` or\n\
     # `[lights.zone.\"<name>\"]`, spelled as the bridge spells it, and says\n\
     # which of the five behaviours it carries: `done` and `failed` blink, and\n\
     # `blocked`, `unread` and `loop` breathe while their condition lasts. The\n\
     # most specific declaration naming a lamp wins, lamp over room over zone,\n\
     # and levels never merge; each question resolves on its own, so a lamp\n\
     # can state its behaviours and still inherit its room's dim window. On\n\
     # one lamp the held states rank blocked, loop, then unread, and a held\n\
     # state preempts a blink on the lamp holding it. `unread` is one word\n\
     # carrying two colours, one for a run that finished and red for one that\n\
     # died; a lamp carries both or neither. An unknown key at any level, and\n\
     # a behaviour word outside the five, are refused by name.\n\
     #\n\
     # `[lights]` IS INERT UNLESS `[plugins.hue] enabled` IS TRUE: hue is the\n\
     # transport and this is the policy. WITH NO TABLE AT ALL the pulse is the\n\
     # `rooms` array above and nothing else; uncommenting `[lights]` with no\n\
     # declaration replaces that pulse with an empty lamp map, so name a place\n\
     # before you do. Switching hue off while a lamp is held leaves that lamp\n\
     # to the wall switch, since putting it out takes a bridge.\n";

/// The closing prose: the ad-hoc mute command, which reads about the config
/// rather than being one. IT IS ALWAYS SHOWN, whether or not `[lights]` is
/// armed, because the command exists whichever way that table reads.
const TRAILER: &str = "# ONE MORE MUTE, TYPED RATHER THAN CONFIGURED, and it is LIGHTS ONLY:\n\
     #\n\
     #   pns lights quiet \"3F - Studio\" 2h   quiet that place's lamps for two hours\n\
     #   pns lights quiet \"3F - Studio\"      quiet them until quiet hours end\n\
     #   pns lights quiet \"3F - Studio\" off  loud again\n\
     #   pns lights quiet                    what is quiet right now\n\
     #\n\
     # It silences EVERY behaviour on the target and reaches the lamps of one\n\
     # lamp, room or zone and nothing else: cards, banners and the durable log\n\
     # carry on, and `pns quiet`, which mutes all of them, is a different\n\
     # command with a different file that neither reads. A bare mute reads\n\
     # `[plugins.hue] quiet_hours` above as the schedule and is refused when\n\
     # none is set; an explicit duration is the same 1s to 24h `pns quiet`\n\
     # takes. A state file nobody can parse mutes EVERY lamp and says so: dark\n\
     # is the fail direction on a lamp path. THE NAMES IT TAKES ARE EVERY\n\
     # LAMP, ROOM AND ZONE, whether a declaration above writes it or the\n\
     # bridge merely holds it, and a name neither knows is refused with the\n\
     # list of the ones that work.\n";

/// The prose above the declarations, and the one commented declaration a
/// fresh machine's operator can copy: the wizard never asks about the lamp
/// map, so the render is the only place they learn the three keys from.
const ROUTING: &str = "# The routing. `dim_window` is local wall clock, the start inclusive and\n\
     # the end exclusive, and it may wrap midnight; `dim_behaviours` names\n\
     # which behaviours run their dim form inside it, and everything else that\n\
     # place carries is SUPPRESSED there. A window with an empty list therefore\n\
     # takes every behaviour away for the night and needs no mode of its own.\n\
     # A place with no window is untouched at every hour; one that states\n\
     # behaviours and no window keeps inheriting its room's window.\n";

/// Written commented, whichever way `[lights]` reads, and only when the
/// caller declared no place of its own: a real declaration is a better
/// example than this one.
const EXAMPLE_DECLARATION: &str = "# [lights.room.\"Studio\"]\n\
     # shows = [\"done\", \"failed\"]\n\
     # dim_window = \"22:00-07:00\"\n\
     # dim_behaviours = [\"blocked\", \"unread\", \"loop\"]\n\n";

/// One answer as a TOML basic string.
///
/// A PASTED SECRET IS UNTRUSTED TEXT and is escaped rather than refused: raw
/// interpolation composes a file that will not load at best, and at worst one
/// whose value stops where the operator's own quote did.
///
/// `{` AND `}` ARE ESCAPED TOO, even though TOML itself has no complaint about
/// either one bare: this text is what an eventual `.tmpl` file regenerates
/// from, and chezmoi's own template engine reads a live `{{ ... }}` action
/// anywhere in that file, quotes or no quotes. Splitting the pair into two
/// `\uXXXX` escapes keeps a pasted value from ever handing chezmoi one.
pub fn quoted(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for character in value.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '{' | '}' => quoted.push_str(&format!("\\u{:04X}", character as u32)),
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
    // LAYOUT IS WALKED ONCE, IN ITS OWN ORDER, and `opt_in` is what decides
    // `render_core` versus `render_opt_in` for every table: nothing here
    // hand-picks a table by name, so a table added, reordered or flipped in
    // LAYOUT changes what this walk writes without a matching edit here.
    // `lights` IS THE ONE HARDCODED BRANCH, because its seven headings share
    // one presence flag rather than each carrying its own; every
    // `lights.<x>` entry is written by that one call and skipped here.
    for table in LAYOUT {
        if table.name == "lights" {
            render_lights(&mut out, &mut remaining)?;
        } else if table.name.starts_with("lights.") {
            continue;
        } else if table.name.starts_with("plugins.") {
            if table.opt_in {
                render_opt_in(&mut out, table, &mut plugins)?;
            } else {
                render_core(&mut out, table, &mut plugins)?;
            }
        } else if table.opt_in {
            render_opt_in(&mut out, table, &mut remaining)?;
        } else {
            render_core(&mut out, table, &mut remaining)?;
        }
    }
    if let Some(name) = plugins.keys().next() {
        return Err(format!("unknown plugin `{name}`"));
    }

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
fn render_opt_in(
    out: &mut String,
    table: &Table,
    container: &mut toml::Table,
) -> Result<(), String> {
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
///
/// EVERY CLUSTER AND DECLARATION MAP IS PULLED OUT OF `lights` FIRST, before
/// any of it is written: `render_block`'s own leftover check would otherwise
/// see the whole cluster sitting unclaimed under the bare `[lights]` heading,
/// which serves only `refresh_secs`, and refuse it as an unknown key before
/// the walk ever reaches `[lights.done]`.
fn render_lights(out: &mut String, remaining: &mut toml::Table) -> Result<(), String> {
    let present = remaining.contains_key("lights");
    let mut lights = take_table(remaining, "lights")?;

    let mut own_keys = toml::Table::new();
    for key in ["note", "refresh_secs"] {
        if let Some(value) = lights.remove(key) {
            own_keys.insert(key.to_string(), value);
        }
    }
    let mut clusters = Vec::new();
    for cluster in ["done", "failed", "blocked", "unread", "loop", "dim"] {
        clusters.push((cluster, take_table(&mut lights, cluster)?));
    }
    let mut declarations = Vec::new();
    for level in ["lamp", "room", "zone"] {
        declarations.push((level, take_table(&mut lights, level)?));
    }
    if let Some(name) = lights.keys().next() {
        return Err(format!("unknown `lights` key `{name}`"));
    }

    render_block(out, find_table("lights"), &mut own_keys, present)?;
    for (cluster, mut settings) in clusters {
        render_block(
            out,
            find_table(&format!("lights.{cluster}")),
            &mut settings,
            present,
        )?;
    }
    out.push_str(ROUTING);
    if declarations.iter().all(|(_, names)| names.is_empty()) {
        out.push_str(EXAMPLE_DECLARATION);
    }
    for (level, names) in declarations {
        for (name, entry) in names {
            let toml::Value::Table(mut settings) = entry else {
                return Err(format!("`lights.{level}.{name}` is not a table"));
            };
            render_target(out, level, &name, &mut settings)?;
        }
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
        Some(toml::Value::String(note)) => {
            // A NOTE IS A RAW COMMENT, never a quoted string, so `quoted`'s
            // brace-splitting cannot stand between it and chezmoi's template
            // engine: refuse the opening outright rather than write it.
            if note.contains("{{") {
                return Err("`note` cannot open a chezmoi template action".to_string());
            }
            // CRLF IS AN ORDINARY LINE ENDING, normalized before the control
            // check below so a pasted Windows-style note is accepted rather
            // than refused for the CR half of its own newline.
            let note = note.replace("\r\n", "\n");
            // ANY OTHER CONTROL CHARACTER, a lone CR included, rides straight
            // into `write_note`'s `# `-prefixed comment line and makes
            // `parse_config` refuse text this render just claimed worked.
            if note.chars().any(|character| {
                (character.is_control() || character == '\u{7f}') && character != '\n'
            }) {
                return Err("`note` cannot hold a control character".to_string());
            }
            Ok(Some(note))
        }
        Some(other) => Err(format!(
            "`note` has type `{}`, not a string",
            other.type_str()
        )),
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
pub(crate) const SECRET_FIELDS: [&str; 2] = ["Password", "UserName"];

/// A secret marker, `{ keepassxc = "<entry>", field = "Password" | "UserName" }`,
/// as the chezmoi action `{{ (keepassxc "<entry>").<field> | toToml }}`, with
/// NO author quotes: `toToml` supplies its own once chezmoi resolves the
/// value, and those are the quotes that end up in the deployed file.
///
/// A RENDERED SECRET IS NOT TOML UNTIL `toToml` RUNS. Go's `quote` (`%q`)
/// would emit escapes TOML does not define (`\a`, `\v`, `\xNN`) for a secret
/// holding a control byte, breaking the whole deployed file from that line
/// on; `toToml` emits `\uXXXX` for the same bytes and round-trips every one
/// of them. Author quotes around the action would only duplicate what
/// `toToml` already writes, so this render's job is to write the bare
/// action, not to quote it.
///
/// NEITHER STRING IS TRUSTED. `field` is checked against the two chezmoi
/// methods a keepassxc entry actually exposes, and the entry name is refused
/// if it carries a quote, a backslash, `}}` or a control character: the
/// first three would close the Go string or the action early and let the rest
/// of the entry name run as template syntax, and a newline would start a line
/// of its own in the rendered text.
fn secret_action(table: &toml::Table) -> Result<String, String> {
    // NAME THE OFFENDER FIRST: `table.len() != 2` alone only counts members,
    // so a third, unrecognised one hides behind the generic pair-count
    // message instead of being called out by name.
    for key in table.keys() {
        if key != "keepassxc" && key != "field" {
            return Err(format!(
                "a secret table may only hold `keepassxc` and `field`, not `{key}`"
            ));
        }
    }
    if table.len() != 2 {
        return Err("a table value must be a secret: exactly `keepassxc` and `field`".to_string());
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
    if entry.trim().is_empty() {
        // AN EMPTY OR WHITESPACE-ONLY ENTRY IS NOT A NAME, and writing it
        // through defers the failure to an apply-time vault lookup for
        // `keepassxc ""`, which the operator hits far from wherever the
        // values file went wrong.
        return Err("a secret's `keepassxc` entry name cannot be blank".to_string());
    }
    if entry.contains('"')
        || entry.contains('\\')
        || entry.contains("}}")
        || entry.chars().any(char::is_control)
    {
        return Err(format!(
            "the keepassxc entry name `{entry}` cannot stand inside a chezmoi action"
        ));
    }
    // BUILT WITH `push_str` RATHER THAN `format!`, deliberately: the target
    // text is thick with literal `{`, `}` and `"` characters, and escaping all
    // of them inside a format string is exactly the kind of place a stray
    // brace goes unnoticed.
    let mut action = String::with_capacity(entry.len() + field.len() + 28);
    action.push_str("{{ (keepassxc \"");
    action.push_str(entry);
    action.push_str("\").");
    action.push_str(field);
    action.push_str(" | toToml }}");
    Ok(action)
}

#[cfg(test)]
mod tests {
    use super::render;
    use crate::config::parse_config;

    #[test]
    fn render_walks_every_layout_table_and_writes_no_heading_outside_it() {
        // LAYOUT IS THE SOURCE OF TRUTH: every table it declares must show up
        // in the render, commented or live, and every heading the render
        // writes must be one LAYOUT actually declares. A `render` that
        // enumerates tables by hand rather than walking LAYOUT can drift from
        // this without any test noticing.
        let text = render(&toml::Table::new()).expect("an empty walk still renders");
        let layout_names: std::collections::HashSet<&str> =
            super::LAYOUT.iter().map(|table| table.name).collect();

        for table in super::LAYOUT {
            if table.name.starts_with("lights.") {
                continue; // governed by the single [lights] presence flag
            }
            let live = format!("[{}]\n", table.name);
            let commented = format!("# [{}]\n", table.name);
            assert!(
                text.contains(&live) || text.contains(&commented),
                "`{}` from LAYOUT never appears in the render: {text}",
                table.name
            );
        }

        for line in text.lines() {
            let heading = line.strip_prefix("# [").or_else(|| line.strip_prefix('['));
            let Some(heading) = heading.and_then(|rest| rest.strip_suffix(']')) else {
                continue;
            };
            if heading.contains('"') {
                continue; // a lamp/room/zone target declaration, not a LAYOUT table
            }
            assert!(
                layout_names.contains(heading),
                "the render wrote a heading `{heading}` LAYOUT never declares"
            );
        }
    }

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
        let lights = config.lights.expect("lights was armed");
        assert_eq!(*lights, crate::config::Lights::default());
        assert_eq!(
            lights.blocked.give_up_after_secs,
            crate::config::DEFAULT_BLOCKED_GIVE_UP_AFTER_SECS
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

    #[test]
    fn core_and_armed_lights_defaults_are_written_live_never_commented() {
        // TEXT-LEVEL, NOT PARSED: a `Default` mistakenly changed to an
        // `Example` still parses back to the same value, since `parse_config`
        // fills in the identical default either way, so a test that only
        // reads the parsed config cannot tell a live default line from a
        // commented one that happens to match. This test reads the rendered
        // TEXT, scoped to each table's own heading so a shared key name
        // (`duration_ms`, `high`, `low`) cannot borrow another table's line.
        let text = render(&toml::Table::new()).expect("an empty walk still renders");
        for expected in [
            "[plugins.mobile]\nenabled = true\n",
            "[plugins.macos-banner]\nenabled = true\n",
            "[daemon]\nenabled = true\n",
        ] {
            assert!(text.contains(expected), "{expected} should be live: {text}");
        }
        for expected in [
            "\nreplay_card = true\n",
            "\ndigest = true\n",
            "\ndigest_as_thread = true\n",
        ] {
            assert!(text.contains(expected), "{expected} should be live: {text}");
        }
        // AND, WHILE LIGHTS IS ABSENT, none of its own defaults leak out live.
        assert!(
            !text.contains("\nduration_ms ="),
            "a lights default rendered live while lights is absent: {text}"
        );

        let mut values = toml::Table::new();
        values.insert("lights".to_string(), toml::Value::Table(toml::Table::new()));
        let armed = render(&values).expect("an armed-empty lights table renders");
        // HEADINGS PLUS THEIR PROSE-FREE KEYS, contiguous lines with nothing
        // between them.
        for expected in [
            "[lights.done]\nduration_ms = 4000\nbrightness = 100\n",
            "[lights.failed]\nduration_ms = 4000\nbrightness = 100\n",
            "[lights.blocked]\nduration_ms = 2000\nhigh = 100\nlow = 30\n",
            "[lights.unread]\nduration_ms = 4000\nhigh = 60\nlow = 10\n",
            "[lights.loop]\nduration_ms = 6000\nhigh = 60\nlow = 10\n",
            "[lights.dim]\nduration_ms = 3000\nhigh = 7\nlow = 1\n",
        ] {
            assert!(
                armed.contains(expected),
                "{expected} should be live once lights is armed: {armed}"
            );
        }
        // THE KEYS THAT CARRY THEIR OWN COMMENT (`after_secs`,
        // `threshold_secs`, `lease_timeout_secs`) sit behind that prose
        // rather than right after the previous key's line, so they are
        // checked on their own.
        for expected in [
            "\nafter_secs = 300\n",
            "\nthreshold_secs = 300\n",
            "\nlease_timeout_secs = 3900\n",
        ] {
            assert!(
                armed.contains(expected),
                "{expected} should be live once lights is armed: {armed}"
            );
        }
    }

    /// A secret marker for one keepassxc entry and field.
    fn secret(entry: &str, field: &str) -> toml::Value {
        let mut table = toml::Table::new();
        table.insert(
            "keepassxc".to_string(),
            toml::Value::String(entry.to_string()),
        );
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
                "token = {{ (keepassxc \"Moshi :: Webhook Secret\").Password | toToml }}"
            ),
            "{text}"
        );
        // AND A LITERAL RENDERS QUOTED, right beside it: `type` still comes
        // off the layout's own `Default`, escaped the ordinary way.
        assert!(text.contains("type = \"moshi\""), "{text}");

        // A RENDERED SECRET IS NOT TOML: the action carries no author quotes
        // of its own, since `toToml` is what supplies them once chezmoi
        // substitutes the vault value, so the stub's placeholder has to
        // supply a quoted string in its place before the whole file can
        // parse.
        let rendered =
            crate::config::strip_chezmoi_actions(&text, |_, _| "\"from-the-vault\"".to_string())
                .expect("a chezmoi-stub round trip stands in for a well-formed secret action");
        let config =
            parse_config(&rendered).unwrap_or_else(|error| panic!("{error:?}\n{rendered}"));
        assert_eq!(
            config.plugins["mobile"].settings["token"].as_str(),
            Some("from-the-vault")
        );
    }

    #[test]
    fn a_username_secret_marker_renders_the_exact_action_and_round_trips_through_the_stub() {
        // SECRET_FIELDS HOLDS TWO NAMES, and hue's key is the shipped table
        // that actually needs the second one: a `SECRET_FIELDS` narrowed to
        // `Password` alone would pass every other test in this module, since
        // none of them exercises `UserName`.
        let mut hue = toml::Table::new();
        hue.insert(
            "bridge".to_string(),
            toml::Value::String("192.168.1.9".to_string()),
        );
        hue.insert("key".to_string(), secret("Hue Bridge", "UserName"));
        hue.insert(
            "rooms".to_string(),
            toml::Value::Array(vec![toml::Value::String("Studio".to_string())]),
        );
        let mut plugins = toml::Table::new();
        plugins.insert("hue".to_string(), toml::Value::Table(hue));
        let mut values = toml::Table::new();
        values.insert("plugins".to_string(), toml::Value::Table(plugins));

        let text = render(&values).expect("a UserName secret marker renders");
        assert!(
            text.contains("key = {{ (keepassxc \"Hue Bridge\").UserName | toToml }}"),
            "{text}"
        );
        let rendered =
            crate::config::strip_chezmoi_actions(&text, |_, _| "\"from-the-vault\"".to_string())
                .expect("a chezmoi-stub round trip stands in for a well-formed secret action");
        let config =
            parse_config(&rendered).unwrap_or_else(|error| panic!("{error:?}\n{rendered}"));
        assert_eq!(
            config.plugins["hue"].settings["key"].as_str(),
            Some("from-the-vault")
        );
    }

    #[test]
    fn a_secret_holding_a_quote_and_a_backslash_round_trips_through_the_totoml_stub() {
        // WHAT `toToml` ACTUALLY EMITS for the byte sequence `a"b\c`, per the
        // sol-1 probe table: `"a\"b\\c"`. The stub stands in for chezmoi
        // having already run `| toToml` on the vault value, so it is handed
        // that exact TOML text rather than the raw secret.
        let mut mobile = toml::Table::new();
        mobile.insert(
            "token".to_string(),
            secret("Quote Backslash Secret", "Password"),
        );
        let mut plugins = toml::Table::new();
        plugins.insert("mobile".to_string(), toml::Value::Table(mobile));
        let mut values = toml::Table::new();
        values.insert("plugins".to_string(), toml::Value::Table(plugins));

        let text = render(&values).expect("a secret marker renders");
        let rendered =
            crate::config::strip_chezmoi_actions(&text, |_, _| "\"a\\\"b\\\\c\"".to_string())
                .expect("a chezmoi-stub round trip stands in for a well-formed secret action");
        let config =
            parse_config(&rendered).unwrap_or_else(|error| panic!("{error:?}\n{rendered}"));
        assert_eq!(
            config.plugins["mobile"].settings["token"].as_str(),
            Some("a\"b\\c")
        );
    }

    #[test]
    fn a_plain_secret_round_trips_through_the_totoml_stub_too() {
        let mut mobile = toml::Table::new();
        mobile.insert("token".to_string(), secret("Plain Secret", "Password"));
        let mut plugins = toml::Table::new();
        plugins.insert("mobile".to_string(), toml::Value::Table(mobile));
        let mut values = toml::Table::new();
        values.insert("plugins".to_string(), toml::Value::Table(plugins));

        let text = render(&values).expect("a secret marker renders");
        let rendered = crate::config::strip_chezmoi_actions(&text, |_, _| "\"plain\"".to_string())
            .expect("a chezmoi-stub round trip stands in for a well-formed secret action");
        let config =
            parse_config(&rendered).unwrap_or_else(|error| panic!("{error:?}\n{rendered}"));
        assert_eq!(
            config.plugins["mobile"].settings["token"].as_str(),
            Some("plain")
        );
    }

    #[test]
    fn a_secret_tables_unknown_member_is_named_rather_than_only_counted() {
        // `table.len() != 2` ALONE only counts members, so `{ keepassxc,
        // field, typo }` reports the pair-count rule and never says which
        // key does not belong: naming the offender needs its own check.
        let mut table = toml::Table::new();
        table.insert(
            "keepassxc".to_string(),
            toml::Value::String("entry".to_string()),
        );
        table.insert(
            "field".to_string(),
            toml::Value::String("Password".to_string()),
        );
        table.insert("typo".to_string(), toml::Value::String("oops".to_string()));
        let error =
            super::secret_action(&table).expect_err("an unknown secret member must be refused");
        assert!(error.contains("typo"), "{error}");
    }

    #[test]
    fn a_secrets_field_is_whitelisted_to_the_two_chezmoi_methods() {
        let error = super::secret_action(
            secret("Moshi :: Webhook Secret", "Notes")
                .as_table()
                .unwrap(),
        )
        .expect_err("Notes is not a field keepassxc exposes to chezmoi");
        assert!(error.contains("Notes"), "{error}");
    }

    #[test]
    fn a_hostile_entry_name_is_refused_rather_than_closing_the_chezmoi_action() {
        for hostile in ["a\"b", "a\\b", "a}}b", "a\nb"] {
            let error = super::secret_action(secret(hostile, "Password").as_table().unwrap())
                .expect_err(&format!(
                    "`{hostile}` can break out of the action and must be refused"
                ));
            assert!(error.contains(hostile), "{error}");
        }
    }

    /// THE MUTANT THIS PINS: the blank-entry refusal removed, letting
    /// `keepassxc ""` (or all-whitespace) reach the shipped template and
    /// defer the failure to an apply-time vault lookup nobody is standing in
    /// front of.
    #[test]
    fn a_blank_or_whitespace_only_entry_name_is_refused_rather_than_written() {
        for blank in ["", "   ", "\t"] {
            let error = super::secret_action(secret(blank, "Password").as_table().unwrap())
                .expect_err(&format!("`{blank:?}` names no entry and must be refused"));
            assert!(error.contains("blank"), "{error}");
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
                assert!(
                    line.starts_with('#'),
                    "an injected line escaped its comment: {line}"
                );
            }
        }
        let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
        // AND THE INJECTED TABLE NEVER ARRIVED: a real `[plugins.hue]` armed
        // by the note would be the exact failure this test exists to catch.
        assert!(!config.plugins.contains_key("hue"));
    }

    #[test]
    fn an_unknown_key_is_refused_by_name_wherever_it_appears() {
        // A TOP-LEVEL KEY, A PLUGIN NAME, A KEY INSIDE A TABLE, AND A KEY
        // INSIDE A TARGET DECLARATION: the same leftover check runs after
        // every table this walk writes, so a values file cannot smuggle any
        // of the four past it.
        let mut top_level = toml::Table::new();
        top_level.insert("zzz_not_a_key".to_string(), toml::Value::Boolean(true));
        let error = render(&top_level).expect_err("an unknown top-level key must be refused");
        assert!(error.contains("zzz_not_a_key"), "{error}");

        let mut plugins = toml::Table::new();
        plugins.insert(
            "zzz_not_a_plugin".to_string(),
            toml::Value::Table(toml::Table::new()),
        );
        let mut values = toml::Table::new();
        values.insert("plugins".to_string(), toml::Value::Table(plugins));
        let error = render(&values).expect_err("an unknown plugin name must be refused");
        assert!(error.contains("zzz_not_a_plugin"), "{error}");

        let mut daemon = toml::Table::new();
        daemon.insert("zzz_not_a_key".to_string(), toml::Value::Boolean(true));
        let mut values = toml::Table::new();
        values.insert("daemon".to_string(), toml::Value::Table(daemon));
        let error = render(&values).expect_err("an unknown key inside a table must be refused");
        assert!(error.contains("zzz_not_a_key"), "{error}");

        let mut target = toml::Table::new();
        target.insert("zzz_not_a_key".to_string(), toml::Value::Boolean(true));
        let mut rooms = toml::Table::new();
        rooms.insert("Studio".to_string(), toml::Value::Table(target));
        let mut lights = toml::Table::new();
        lights.insert("room".to_string(), toml::Value::Table(rooms));
        let mut values = toml::Table::new();
        values.insert("lights".to_string(), toml::Value::Table(lights));
        let error = render(&values).expect_err("an unknown key inside a target must be refused");
        assert!(error.contains("zzz_not_a_key"), "{error}");
    }

    #[test]
    fn an_unknown_table_is_refused_by_name() {
        let mut lights = toml::Table::new();
        lights.insert(
            "zzz_not_a_table".to_string(),
            toml::Value::Table(toml::Table::new()),
        );
        let mut values = toml::Table::new();
        values.insert("lights".to_string(), toml::Value::Table(lights));
        let error = render(&values).expect_err("an unknown lights sub-table must be refused");
        assert!(error.contains("zzz_not_a_table"), "{error}");
    }

    #[test]
    fn an_opt_in_table_absent_renders_commented_and_present_renders_live() {
        // ABSENT: the heading, `enabled` and every key are commented, and the
        // table never reaches the parsed config at all.
        let text = render(&toml::Table::new()).expect("an empty walk still renders");
        assert!(
            text.contains("# [plugins.hermes]\n# enabled = true\n"),
            "{text}"
        );
        let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
        assert!(!config.plugins.contains_key("hermes"));

        // PRESENT: the heading and `enabled = true` are LIVE even though the
        // caller never stated `enabled` itself, which is X7's own ruling: an
        // opt-in table's `enabled` is written true the moment the table shows
        // up at all, and the parser is what reads its absence as off.
        let mut hermes = toml::Table::new();
        hermes.insert("key".to_string(), toml::Value::String("secret".to_string()));
        let mut plugins = toml::Table::new();
        plugins.insert("hermes".to_string(), toml::Value::Table(hermes));
        let mut values = toml::Table::new();
        values.insert("plugins".to_string(), toml::Value::Table(plugins));
        let text = render(&values).expect("an armed table renders");
        assert!(
            text.contains("[plugins.hermes]\nenabled = true\n"),
            "{text}"
        );
        let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
        assert!(config.plugins["hermes"].enabled);
    }

    #[test]
    fn every_layout_table_matches_the_config_roster_exactly_in_both_directions() {
        // EVERY LAYOUT TABLE IS ONE THE ROSTER SERVES, and with the SAME key
        // set: the layout is a second statement of `config`'s own vocabulary,
        // and this is what stops the two from drifting apart.
        //
        // `lights` ITSELF IS THE ONE EXCEPTION, and only because `config`
        // reads it as one flat table where this layout writes seven headings:
        // the roster's `done`, `failed`, `blocked`, `unread`, `loop` and `dim`
        // are each a SEPARATE `lights.<name>` entry here, not a `Key` of
        // `lights`, and `lamp`, `room` and `zone` are the hardcoded
        // declaration branch. So `lights`'s effective key set is its own
        // `refresh_secs` plus the leaf name of every `lights.<x>` table this
        // layout declares, plus the three declaration levels.
        for table in super::LAYOUT {
            let (_, roster_keys) = crate::config::TABLE_KEYS
                .iter()
                .find(|(name, _)| *name == table.name)
                .unwrap_or_else(|| panic!("`{}` is not a table the roster serves", table.name));
            let mut layout_keys: Vec<&str> = table.keys.iter().map(|key| key.name).collect();
            if table.name == "lights" {
                layout_keys.extend(["lamp", "room", "zone"]);
                layout_keys.extend(
                    super::LAYOUT
                        .iter()
                        .filter_map(|entry| entry.name.strip_prefix("lights.")),
                );
            }
            layout_keys.sort_unstable();
            layout_keys.dedup();
            let mut roster_keys = roster_keys.to_vec();
            roster_keys.sort_unstable();
            assert_eq!(
                layout_keys, roster_keys,
                "`{}` disagrees between the layout and the roster",
                table.name
            );
        }

        // AND EVERY ROSTER TABLE THE LAYOUT CAN REACH IS WRITTEN BY SOME
        // PATH: `TOP_LEVEL` has no heading of its own to write, and
        // `lights.<level>` is written by the hardcoded target-declaration
        // branch rather than a `Key` list, so both are the two named
        // exceptions rather than gaps.
        for (table, _) in crate::config::TABLE_KEYS.iter().copied() {
            if table == crate::config::TOP_LEVEL || table == crate::config::TARGET_KEYS {
                continue;
            }
            assert!(
                super::LAYOUT.iter().any(|entry| entry.name == table),
                "the roster serves `{table}` and the layout never writes it"
            );
        }
    }

    #[test]
    fn the_hardcoded_target_declaration_branch_writes_every_target_key() {
        // THE HALF THE LAYOUT WALK CANNOT SEE: `lights.<level>` has no `Key`
        // list, so its coverage is proven by exercising the render path
        // directly rather than by a table lookup.
        let mut target = toml::Table::new();
        target.insert(
            "shows".to_string(),
            toml::Value::Array(vec![toml::Value::String("done".to_string())]),
        );
        target.insert(
            "dim_window".to_string(),
            toml::Value::String("22:00-07:00".to_string()),
        );
        target.insert(
            "dim_behaviours".to_string(),
            toml::Value::Array(vec![toml::Value::String("done".to_string())]),
        );
        let mut rooms = toml::Table::new();
        rooms.insert("Studio".to_string(), toml::Value::Table(target));
        let mut lights = toml::Table::new();
        lights.insert("room".to_string(), toml::Value::Table(rooms));
        let mut values = toml::Table::new();
        values.insert("lights".to_string(), toml::Value::Table(lights));

        let text = render(&values).expect("a full target declaration renders");
        let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
        let studio = &config.lights.expect("lights was armed").rooms["Studio"];
        assert_eq!(studio.shows, Some(vec![crate::config::Behaviour::Done]));
        assert_eq!(studio.dim_window.as_deref(), Some("22:00-07:00"));
        assert_eq!(studio.dim_behaviours, vec![crate::config::Behaviour::Done]);
    }

    #[test]
    fn the_target_declaration_key_roster_is_exactly_shows_dim_window_and_dim_behaviours() {
        // THE EXACT KEY SET, not merely "an unknown key is refused": a fourth
        // key added to `render_target`'s own hardcoded list would pass every
        // existing test without ever being asserted as belonging.
        let (_, roster_keys) = crate::config::TABLE_KEYS
            .iter()
            .find(|(name, _)| *name == crate::config::TARGET_KEYS)
            .expect("TARGET_KEYS is declared in the roster");
        let mut roster_keys = roster_keys.to_vec();
        roster_keys.sort_unstable();
        assert_eq!(roster_keys, ["dim_behaviours", "dim_window", "shows"]);
    }

    #[test]
    fn declarations_at_every_level_render_sorted_hostile_names_quoted_with_their_own_notes() {
        // ONE SAFE ROOM IS NOT ENOUGH: lamp and zone rendering, the sort
        // order, a hostile name and a target's own note all need their own
        // exercise, or removing any of them survives every existing test.
        let mut lamps = toml::Table::new();
        lamps.insert(
            "Zeta Lamp".to_string(),
            toml::Value::Table(toml::Table::new()),
        );
        lamps.insert(
            "Alpha Lamp".to_string(),
            toml::Value::Table(toml::Table::new()),
        );

        let mut hostile_target = toml::Table::new();
        hostile_target.insert(
            "note".to_string(),
            toml::Value::String("the desk lamp".to_string()),
        );
        let mut rooms = toml::Table::new();
        rooms.insert(
            "Zeta Room".to_string(),
            toml::Value::Table(toml::Table::new()),
        );
        rooms.insert(
            "Alpha \"Room\"".to_string(),
            toml::Value::Table(hostile_target),
        );

        let mut zones = toml::Table::new();
        zones.insert(
            "Zeta Zone".to_string(),
            toml::Value::Table(toml::Table::new()),
        );
        zones.insert(
            "Alpha Zone".to_string(),
            toml::Value::Table(toml::Table::new()),
        );

        let mut lights = toml::Table::new();
        lights.insert("lamp".to_string(), toml::Value::Table(lamps));
        lights.insert("room".to_string(), toml::Value::Table(rooms));
        lights.insert("zone".to_string(), toml::Value::Table(zones));
        let mut values = toml::Table::new();
        values.insert("lights".to_string(), toml::Value::Table(lights));

        let text = render(&values).expect("declarations at every level render");

        // SORTED, ALPHA BEFORE ZETA, at each of the three levels: `toml::Table`
        // is BTreeMap-ordered, so this catches sorting being disabled.
        for level in ["lamp", "room", "zone"] {
            let alpha = text
                .find(&format!("[lights.{level}.\"Alpha"))
                .unwrap_or_else(|| panic!("no Alpha declaration at `{level}`: {text}"));
            let zeta = text
                .find(&format!("[lights.{level}.\"Zeta"))
                .unwrap_or_else(|| panic!("no Zeta declaration at `{level}`: {text}"));
            assert!(
                alpha < zeta,
                "`{level}` did not sort Alpha before Zeta: {text}"
            );
        }

        // A QUOTE IN THE NAME IS ESCAPED, never raw-interpolated into the
        // heading, which would otherwise close the TOML key string early.
        assert!(
            text.contains("[lights.room.\"Alpha \\\"Room\\\"\"]"),
            "{text}"
        );
        // AND ITS OWN NOTE renders above its own heading, not the room's name
        // it happens to share no other target with.
        assert!(
            text.contains("# the desk lamp\n[lights.room.\"Alpha \\\"Room\\\"\"]"),
            "{text}"
        );

        let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
        let lights = config.lights.expect("lights was armed");
        assert!(lights.lamps.contains_key("Alpha Lamp"));
        assert!(lights.lamps.contains_key("Zeta Lamp"));
        assert!(lights.zones.contains_key("Alpha Zone"));
        assert!(lights.zones.contains_key("Zeta Zone"));
    }

    #[test]
    fn a_hostile_literal_crosses_as_one_inert_string_and_never_as_structure() {
        // THE OTHER INJECTION CASE: a plain value, not a note. A quote could
        // close the string, a newline could open a heading or a key on the
        // next line, and a `#` could start a comment; escaped, all of it is
        // one line inside one basic string and parses back as itself.
        let hostile = "\"\n[evil]\nenabled = true\n# not a comment";
        let mut hermes = toml::Table::new();
        hermes.insert("key".to_string(), toml::Value::String(hostile.to_string()));
        let mut plugins = toml::Table::new();
        plugins.insert("hermes".to_string(), toml::Value::Table(hermes));
        let mut values = toml::Table::new();
        values.insert("plugins".to_string(), toml::Value::Table(plugins));

        let text = render(&values).expect("a hostile literal renders");
        assert!(
            !text.lines().any(|line| line.starts_with("[evil]")),
            "the value opened a heading of its own: {text}"
        );
        let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
        assert_eq!(
            config.plugins["hermes"].settings["key"].as_str(),
            Some(hostile)
        );
    }

    #[test]
    fn a_literal_holding_a_chezmoi_action_opening_crosses_with_its_braces_broken_up() {
        // A LITERAL IS UNTRUSTED TEXT THIS RENDER MUST NEVER HAND CHEZMOI A LIVE
        // ACTION FROM: `quoted` is also what the closing prose relies on to keep
        // an eventual S2-generated `.tmpl` file inert wherever a value sits, so
        // `{{` and `}}` must never survive a quoted string as an adjacent pair.
        let hostile = "before{{ printf \"pwned\" }}after";
        let mut hermes = toml::Table::new();
        hermes.insert("key".to_string(), toml::Value::String(hostile.to_string()));
        let mut plugins = toml::Table::new();
        plugins.insert("hermes".to_string(), toml::Value::Table(hermes));
        let mut values = toml::Table::new();
        values.insert("plugins".to_string(), toml::Value::Table(plugins));

        let text = render(&values).expect("a hostile literal renders");
        assert!(
            !text.contains("{{"),
            "a live action opening survived: {text}"
        );
        assert!(!text.contains("}}"), "a live action close survived: {text}");
        let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
        assert_eq!(
            config.plugins["hermes"].settings["key"].as_str(),
            Some(hostile)
        );
    }

    #[test]
    fn a_note_holding_a_chezmoi_action_opening_is_refused_by_name() {
        // A NOTE IS WRITTEN AS A RAW COMMENT, never a quoted string, so `quoted`'s
        // brace-splitting cannot protect it: chezmoi's template engine reads
        // `{{ ... }}` inside a comment exactly like anywhere else in the file, so
        // the only safe answer is refusing the note outright.
        let mut hermes = toml::Table::new();
        hermes.insert(
            "note".to_string(),
            toml::Value::String("safe {{ printf \"pwned\" }} unsafe".to_string()),
        );
        let mut plugins = toml::Table::new();
        plugins.insert("hermes".to_string(), toml::Value::Table(hermes));
        let mut values = toml::Table::new();
        values.insert("plugins".to_string(), toml::Value::Table(plugins));

        let error = render(&values).expect_err("a note opening a chezmoi action must be refused");
        assert!(error.contains("note"), "{error}");
    }

    #[test]
    fn a_note_holding_a_forbidden_control_character_is_refused_by_name() {
        // `write_note` PREFIXES EACH `\n`-SPLIT LINE WITH `# `, so a NUL or
        // DEL sitting mid-line, or a lone CR that split() never breaks on,
        // rides straight into the comment and makes `parse_config` refuse the
        // text `render` just claimed to succeed on. CRLF is normalized first,
        // since it is an ordinary line ending rather than a hostile control.
        for hostile in [
            "line one\r\nline two",
            "bad\u{0}byte",
            "bad\u{7f}byte",
            "lone\rcarriage",
        ] {
            let mut hermes = toml::Table::new();
            hermes.insert("note".to_string(), toml::Value::String(hostile.to_string()));
            hermes.insert(
                "key".to_string(),
                toml::Value::String("hermes-secret".to_string()),
            );
            let mut plugins = toml::Table::new();
            plugins.insert("hermes".to_string(), toml::Value::Table(hermes));
            let mut values = toml::Table::new();
            values.insert("plugins".to_string(), toml::Value::Table(plugins));

            let result = render(&values);
            if hostile.contains("\r\n") {
                // CRLF is the one shape that must be ACCEPTED, normalized to a
                // plain newline rather than refused as a control character.
                let text = result.expect("CRLF normalizes rather than refusing");
                assert!(text.contains("# line one\n# line two\n"), "{text}");
                let config =
                    parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
                assert!(config.plugins.contains_key("hermes"));
            } else {
                let error = result.expect_err(&format!(
                    "{hostile:?} must be refused rather than rendered into an unparsable comment"
                ));
                assert!(error.contains("note"), "{error}");
            }
        }
    }

    #[test]
    fn the_recap_prose_keeps_the_hook_path_and_note_limit_facts_the_template_carries() {
        // X2's template-prose rule: the shipped template's facts win except
        // where they name the operator's own environment. Dropping the hook
        // PATH explanation on `repos` or the twenty-five note limit on
        // `review_notes` loses a real fact nothing else states.
        let text = render(&toml::Table::new()).expect("an empty walk still renders");
        assert!(text.contains("FOUND ON PATH"), "{text}");
        assert!(text.contains("Twenty-five notes"), "{text}");
    }

    #[test]
    fn the_header_scopes_the_credential_arming_claim_to_the_plugins_it_names() {
        // Focus, the nag and the lamp map are opt-in tables that need no
        // credential at all; only three of the plugins do (hue, hermes,
        // router), so a blanket "everything else is armed with a
        // credential" misstates all three of them.
        let text = render(&toml::Table::new()).expect("an empty walk still renders");
        assert!(
            !text.contains("Everything else is armed with a credential"),
            "{text}"
        );
        assert!(text.contains("need no credential at all"), "{text}");
    }

    #[test]
    fn a_note_above_the_bare_lights_heading_renders_like_any_other_tables() {
        // `lights` IS TAKEN APART BEFORE IT IS WRITTEN, so its own `note` has
        // to be pulled out with `refresh_secs` or the leftover check refuses
        // it as an unknown key, the one table a values file could not comment.
        let mut lights = toml::Table::new();
        lights.insert(
            "note".to_string(),
            toml::Value::String("the lamps this machine drives".to_string()),
        );
        let mut values = toml::Table::new();
        values.insert("lights".to_string(), toml::Value::Table(lights));

        let text = render(&values).expect("a noted lights table renders");
        assert!(
            text.contains("# the lamps this machine drives\n[lights]\n"),
            "{text}"
        );
        let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
        assert_eq!(
            *config.lights.expect("lights was armed"),
            crate::config::Lights::default()
        );
    }

    #[test]
    fn the_routing_prose_is_always_written_and_the_example_only_when_nothing_is_declared() {
        // A FRESH MACHINE LEARNS THE THREE TARGET KEYS FROM THIS RENDER ALONE:
        // the wizard never asks about the lamp map, so the example is what an
        // operator copies. It is commented whichever way `[lights]` reads, and
        // it steps aside for a real declaration, which is the better example.
        let mut declared_lights = toml::Table::new();
        let mut rooms = toml::Table::new();
        rooms.insert(
            "Kitchen".to_string(),
            toml::Value::Table(toml::Table::new()),
        );
        declared_lights.insert("room".to_string(), toml::Value::Table(rooms));
        let mut declared = toml::Table::new();
        declared.insert("lights".to_string(), toml::Value::Table(declared_lights));

        let mut armed_empty = toml::Table::new();
        armed_empty.insert("lights".to_string(), toml::Value::Table(toml::Table::new()));

        for (values, example_expected) in [
            (toml::Table::new(), true),
            (armed_empty, true),
            (declared, false),
        ] {
            let text = render(&values).expect("every lights shape renders");
            assert!(text.contains("# The routing. `dim_window` is"), "{text}");
            assert_eq!(
                text.contains("# [lights.room.\"Studio\"]\n# shows = "),
                example_expected,
                "{text}"
            );
            // AND THE EXAMPLE IS A LINE THE ROSTER SCAN ACCEPTS, spelled the
            // whitespace-exact way, so the wizard's fence keeps reading it.
            crate::config::documented_keys_the_roster_serves(&text);
            let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
            assert!(
                config
                    .lights
                    .is_none_or(|lights| !lights.rooms.contains_key("Studio"))
            );
        }
    }
}
