use super::*;

/// Where this install keeps its state and looks for channel executables.
///
/// OPT-IN, and both keys are EXAMPLES rather than defaults, because neither
/// has a value that means what leaving it out means: `state_dir` written is a
/// directory pns will create and use as it stands, and `channels_dir` written
/// at all forces every channel onto its executable.
pub(super) const PATHS: Table = Table {
    name: "paths",
    prose: "# Where this install keeps its own files. Left out, state lives under\n\
            # ~/.local/state/pns and the native channels are used directly. Each is\n\
            # an absolute path or a ~/ path.\n",
    opt_in: true,
    children: &[],
    keys: &[
        Key {
            name: "state_dir",
            prose: "# The ledger, the markers and the spooled jobs. Moving it leaves\n\
                         # whatever is in the old directory where it is.\n",
            sample: Sample::Example("\"~/.local/state/pns\""),
        },
        Key {
            name: "channels_dir",
            prose: "# A directory of channel executables. NAMING IT AT ALL FORCES every\n\
                         # channel through an executable of its own name in here instead of the\n\
                         # compiled-in one, which is what makes it a testing seam rather than a\n\
                         # place to point at the usual location.\n",
            sample: Sample::Example("\"~/.local/libexec/pns/channels\""),
        },
    ],
};
/// What the two routes pns picks for itself are called.
///
/// A CORE TABLE WRITTEN LIVE AT ITS DEFAULTS, because there is no such thing
/// as this machine having no default route: something has to receive an event
/// whose producer named none, and the name is worth reading in the file
/// rather than guessing at.
pub(super) const ROUTES: Table = Table {
    name: "routes",
    prose: "# The two routes pns picks for itself, by the name your gateway serves them\n\
            # under. Every OTHER route is named by the producer that raised the event,\n\
            # and exists for this machine as soon as it has a key below.\n",
    opt_in: false,
    children: &[],
    keys: &[
        Key {
            name: "default",
            prose: "# Where an event whose producer named no route lands, the return recap\n\
                         # included. It is also the last path segment of the gateway URL pns\n\
                         # posts to unless [plugins.log] url names one outright.\n",
            sample: Sample::Default("\"pns-events\""),
        },
        Key {
            name: "urgent",
            prose: "# The route reserved for what needs a human now: a machine-health event\n\
                         # (`pns send --delivery-class health`) whose --state is one somebody has\n\
                         # to answer, and the stale-block escalation, both take it, whatever it is\n\
                         # called.\n",
            sample: Sample::Default("\"priority\""),
        },
    ],
};
pub(super) const GATEWAY: Table = Table {
    name: "gateway",
    prose: GATEWAY_PROSE,
    opt_in: false,
    children: &[],
    keys: &[
        Key {
            name: "enabled",
            prose: "",
            sample: Sample::Default("true"),
        },
        Key {
            name: "service",
            prose: "# The launchd label this machine's clock runs under, which `pns gateway\n\
                         # start|stop|restart|status` starts, stops, restarts and reports on. NO\n\
                         # DEFAULT: pns compiles in no label of its own, since it does not know what\n\
                         # your installation calls its own plist, and every gateway verb refuses\n\
                         # while this is unset.\n",
            sample: Sample::Example("\"\""),
        },
    ],
};
pub(super) const RECAP: Table = Table {
    name: "recap",
    prose: RECAP_PROSE,
    opt_in: false,
    children: &[RECAP_SOURCES],
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
            name: "post_window_recap",
            prose: "# The recap of the whole window posted to hermes, rendered and posted\n\
                         # by a second process that nothing waits for. Off records the window\n\
                         # just the same; only the posting stops.\n",
            sample: Sample::Default("true"),
        },
        Key {
            name: "minimum_events",
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
            name: "summarizer_deadline",
            prose: "# How long that command may take before it is killed and the plain list\n\
                         # is posted instead. It is the whole recap's budget rather than each\n\
                         # question's, and AN HOUR IS THE CEILING: a longer one is refused by\n\
                         # name. It also bounds the turn summarizer that writes each\n\
                         # notification's sentence, which takes at most thirty seconds of it\n\
                         # because a Stop hook is waiting on that one.\n",
            sample: Sample::Default("\"4m\""),
        },
        Key {
            name: "overnight",
            prose: "# The four periods of your own day, local time, as a start and an end.\n\
                         # `pns recap morning` is the most recent instance of that one, in progress\n\
                         # or complete, and a bare `pns recap` is whichever ended most recently.\n\
                         # THE FOUR MUST TILE THE DAY: a gap or an overlap between any two is\n\
                         # refused at load with both windows named.\n",
            sample: Sample::Default("[\"22:00\", \"06:00\"]"),
        },
        Key {
            name: "morning",
            prose: "",
            sample: Sample::Default("[\"06:00\", \"12:00\"]"),
        },
        Key {
            name: "afternoon",
            prose: "",
            sample: Sample::Default("[\"12:00\", \"17:00\"]"),
        },
        Key {
            name: "evening",
            prose: "",
            sample: Sample::Default("[\"17:00\", \"22:00\"]"),
        },
        Key {
            name: "week_starts_on",
            prose: "# Which day `pns recap week` counts from. `monday` or `sunday`, and\n\
                         # nothing else.\n",
            sample: Sample::Default("\"monday\""),
        },
        Key {
            name: "rows_per_section",
            prose: "# How many rows one list section prints before the line that says how\n\
                         # many more there were. `--limit` overrides it for one run.\n",
            sample: Sample::Default("8"),
        },
        Key {
            name: "review_notes_glob",
            prose: "# The review notes whose findings become the recap's \"caught by review\"\n\
                         # section: ONE directory, named in full, and a file name that may hold\n\
                         # one `*`. A relative path and a `*` in a directory are both refused,\n\
                         # because this pattern is the whole of what pns is allowed to open. Only\n\
                         # files whose own clock falls inside the window are read, so a note you\n\
                         # had already seen before you left is not news. UNSET IS THE WORKING\n\
                         # SETTING and, as with `repositories`, unset means the directory is never\n\
                         # opened. Twenty-five notes is what one recap considers, NEWEST FIRST,\n\
                         # and a window holding more says \"at least\" in its own count rather than\n\
                         # printing a total it cannot back; a matched note that will not open is\n\
                         # named as one that could not be read rather than left out.\n",
            sample: Sample::Example("\"/absolute/path/notes-*.md\""),
        },
        Key {
            name: "retain",
            prose: "# How long pns keeps one row of the activity store, the durable table of\n\
                         # harness hook events the recap reads. The gateway prunes anything older\n\
                         # on its own tick, so a shorter value takes effect without a restart.\n\
                         # THIRTY DAYS, SPELLED IN HOURS because a duration is <count><ms|s|m|h>\n\
                         # and a day is not one of the units. An hour is the floor and a year the\n\
                         # ceiling; zero is refused by name, because this store has no off switch\n\
                         # and a retention of nothing would empty it on the next tick.\n",
            sample: Sample::Default("\"720h\""),
        },
    ],
};
/// The commands that fill the recap's list sections, one per section.
pub(super) const RECAP_SOURCES: Table = Table {
    name: "recap.sources",
    prose: "# Where each list section of the recap comes from: an ARGV LIST, never a\n\
                 # shell string, run directly with `{since}` and `{until}` replaced by the\n\
                 # window's own bounds as local RFC 3339 timestamps. One row per line of\n\
                 # standard output; pns counts and prints the lines and parses none of\n\
                 # them. UNSET IS THE WORKING SETTING and it is a fence: a section nobody\n\
                 # named starts no process and is absent from the page, from the document\n\
                 # and from `--section`. A command that exits non-zero costs its own\n\
                 # section one line naming the code and nothing else. `pull_requests` and\n\
                 # `applies` are run a second time with no window at all, which is what\n\
                 # fills the `open` section.\n",
    opt_in: true,
    children: &[],
    keys: &[
        Key {
            name: "pull_requests",
            prose: "",
            sample: Sample::Example(
                "[\"gh\", \"pr\", \"list\", \"--search\", \"updated:>={since}\"]",
            ),
        },
        Key {
            name: "commits",
            prose: "# Work with neither an agent session nor a pull request behind it. It\n\
                         # ships unnamed, so a fresh install has no `commits` section at all.\n",
            sample: Sample::Example(
                "[\"git\", \"log\", \"--since={since}\", \"--until={until}\", \"--oneline\"]",
            ),
        },
        Key {
            name: "tasks",
            prose: "# Your own task tool. pns assumes none, so this and `applies` decide\n\
                         # for themselves whether the window means anything to them.\n",
            sample: Sample::Example("[\"dam\", \"ls\", \"due:today\"]"),
        },
        Key {
            name: "applies",
            prose: "",
            sample: Sample::Example("[\"chezmoi\", \"status\"]"),
        },
    ],
};

pub(super) const FOCUS: Table = Table {
    name: "focus",
    prose: "# The macOS Focus modes that pns reads as your own instruction not to be\n\
                 # interrupted. While one of them is active, banners, cards and light\n\
                 # pulses are held back and handed over when it ends. Approvals and the\n\
                 # durable log are unchanged. [delivery] classes can exempt banners and\n\
                 # phone cards. A Focus name matches however you capitalised\n\
                 # it, a mode's raw modeIdentifier works too, and an empty entry is refused\n\
                 # by name. An unreadable Focus store reads as no Focus, never as silence.\n\
                 # NAMING NO MODE SILENCES NOTHING, which is the same statement as no\n\
                 # table at all; `enabled = false` keeps the list and stops it being read.\n",
    opt_in: true,
    children: &[],
    keys: &[
        Key {
            name: "enabled",
            prose: "",
            sample: Sample::Default("true"),
        },
        Key {
            name: "modes",
            prose: "",
            sample: Sample::Example("[\"Sleep\"]"),
        },
    ],
};
/// The mute's own section. IT HOLDS NO KEYS: the mute itself is typed
/// (`pns mute 30m`) rather than configured, and the one thing there is to
/// configure about it is the calendar below.
pub(super) const QUIET: Table = Table {
    name: "quiet",
    prose: "# The mute, `pns mute <duration>`, and what else may switch it.\n",
    opt_in: true,
    children: &[QUIET_CALENDAR],
    keys: &[],
};
/// The calendar that switches the mute on and off.
///
/// OPT-IN, and the command is an EXAMPLE rather than a default, because there
/// is no command every machine has: this is a feature nothing does until the
/// operator names the one that reads their own calendar.
pub(super) const QUIET_CALENDAR: Table = Table {
    name: "quiet.calendar",
    prose: "# Quiet that follows your calendar: while an event marked busy is on, the\n\
            # mute is on, and it ends when the event does. WHAT YOU TYPE ALWAYS WINS:\n\
            # a mute you set by hand is never shortened or cleared by this, and quiet\n\
            # you switch off during a meeting stays off for the rest of it.\n\
            # THE CALENDAR IS WHATEVER `command` READS. It is run read-only on the\n\
            # interval below, handed no input, and must answer on stdout with\n\
            # {\"events\": [{\"start\": <epoch>, \"end\": <epoch>, \"busy\": <true|false>}]},\n\
            # every event in the next hour or so, busy or not: pns picks the busy ones.\n\
            # A command that fails, answers something else or runs long changes\n\
            # nothing at all, and a mute already set stands until its own expiry.\n",
    opt_in: true,
    children: &[],
    keys: &[
        Key {
            name: "enabled",
            prose: "",
            sample: Sample::Default("false"),
        },
        Key {
            name: "command",
            prose: "# ARGV, NEVER A SHELL STRING. It is your own command: pns names no\n\
                         # calendar and holds no credential of one.\n",
            sample: Sample::Example("[\"calendar-busy-window\"]"),
        },
        Key {
            name: "poll_interval",
            prose: "# How often it is asked, bounded \"30s\" to \"30m\".\n",
            sample: Sample::Default("\"2m\""),
        },
        Key {
            name: "deadline",
            prose: "# How long one run may take before it is killed and the poll leaves\n\
                         # everything as it was, bounded \"1s\" to \"30s\".\n",
            sample: Sample::Default("\"20s\""),
        },
    ],
};
/// The TOML literal `[remind] delay` ships at, and the value a walk that
/// armed the reminder writes out: one literal, so the wizard cannot arm the
/// reminder at a delay the shipped file never names.
pub(in crate::config) const REMIND_DELAY: &str = "\"5m\"";
pub(super) const REMIND: Table = Table {
    name: "remind",
    prose: "# The reminder: one more card when an approval has been sitting unanswered.\n\
                 # IT IS A STATEMENT AND NEVER A SECOND PROMPT, so the card raised when the\n\
                 # prompt appeared is still the one carrying Allow and Deny. It needs the\n\
                 # daemon running and the PostToolBatch hook entry that tells pns an\n\
                 # approval was dealt with; without that entry the only clearing signal\n\
                 # is the end of the turn. It respects every mute the first card respects,\n\
                 # a `pns mute`, a Focus, the quiet window, and a reminder held back is\n\
                 # LOST rather than queued. Several approvals waiting are one card rather\n\
                 # than several, each approval is reminded about at most once, and a card\n\
                 # counts every approval outstanding at that moment, so a fresh one can be\n\
                 # named early and is then done. The signal is the tool batch RESOLVING\n\
                 # rather than your answer, so a tool approved at once that then runs\n\
                 # longer than this is reminded about anyway; if that bites, raise the\n\
                 # number. THIRTY SECONDS IS THE FLOOR AND AN HOUR THE CEILING, anything\n\
                 # outside is refused by name, \"0s\" included: leaving the key out is the\n\
                 # one way to say the reminder is off.\n",
    opt_in: true,
    children: &[],
    keys: &[Key {
        name: "delay",
        prose: "",
        sample: Sample::Default(REMIND_DELAY),
    }],
};
/// What one producer asked for, keyed by the name that producer sends.
///
/// WRITTEN AS THE PLACEHOLDER IT IS. The heading carries `<name>` rather than
/// any producer this machine happens to run, because pns compiles in no roster
/// of producers and naming one here would read as the only one that works.
pub(super) const PRODUCER: Table = Table {
    name: "producer.<name>",
    prose: "# What one producer asked for, one table per producer, keyed by the name it\n\
                 # sends (`--producer`, or PNS_PRODUCER). Replace <name> with that name.\n\
                 # THE REMINDER IS SWITCHED ON BY THE CALL, NEVER BY THE NAME: a harness\n\
                 # that sends an answered signal passes `--remind` on its own approval hook,\n\
                 # and that flag beats whatever this table says. This is here for a producer\n\
                 # you cannot pass a flag to. It needs `[remind] delay` above; with no delay\n\
                 # set, `remind = true` is still the reminder off.\n",
    opt_in: true,
    children: &[],
    keys: &[Key {
        name: "remind",
        prose: "",
        sample: Sample::Example("true"),
    }],
};
pub(super) const STALE: Table = Table {
    name: "stale",
    prose: "# The OTHER end of the same wait: how long a session stays blocked before\n\
                 # ONE page about it goes out, to the route reserved for things that need\n\
                 # a human. It fires once per block and then says nothing until that block\n\
                 # resolves, and only when you could act on it: nothing is sent while you\n\
                 # are away from both the desk and the phone, or while the screen has been\n\
                 # locked for the whole window, because a page nobody can answer is how\n\
                 # the route reserved for the ones you must answer stops being read. A\n\
                 # screen locked for PART of the window still pages, which is the case\n\
                 # this exists for: you were here, you stepped away, and a session is\n\
                 # stuck. It needs the daemon running. A MINUTE IS THE FLOOR AND A DAY THE\n\
                 # CEILING, anything outside is refused by name, \"0s\" included: the window\n\
                 # is not the switch, `enabled` is.\n",
    opt_in: false,
    children: &[],
    keys: &[
        Key {
            name: "enabled",
            prose: "",
            sample: Sample::Default("true"),
        },
        Key {
            name: "escalate_after",
            prose: "",
            sample: Sample::Default("\"1h\""),
        },
        Key {
            name: "route",
            prose: "# Where that page goes. Unset sends it to `[routes] urgent`, which is\n\
                         # what every other page pns raises for itself takes.\n",
            sample: Sample::Example("\"priority\""),
        },
    ],
};
/// How long a writer waits for the state database's write lock.
///
/// A CORE TABLE WRITTEN LIVE AT ITS DEFAULT, because there is no such thing as
/// this machine having no bound: the number is always in force and is worth
/// reading in the file rather than guessing at.
pub(super) const STORAGE: Table = Table {
    name: "storage",
    prose: "# The state database. One number: how long a write waits for the lock\n\
            # another writer holds before the write is refused. It bounds a WEDGED\n\
            # writer and measures nothing else: every transaction pns makes is a\n\
            # handful of short statements with no network and no sleep in it, and a\n\
            # writer that dies drops its lock with its process, so a wait that\n\
            # expires is a machine in trouble rather than a busy one. A hook you are\n\
            # waiting on pays this bound per lock it takes, which is why the ceiling\n\
            # is a minute; ten milliseconds is the floor, anything outside is refused\n\
            # by name, and \"0s\" refuses a contended write the instant it is\n\
            # contended instead of waiting at all.\n",
    opt_in: false,
    children: &[],
    keys: &[Key {
        name: "busy_deadline",
        prose: "",
        sample: Sample::Default("\"5s\""),
    }],
};
pub(super) const FAILURES: Table = Table {
    name: "failures",
    prose: "# The failure page: the same record `pns failures` prints, served over\n\
                 # loopback so moshi's browser preview can read it from your phone. The\n\
                 # listener binds 127.0.0.1 only, because the per-session SSH forward is\n\
                 # the trust boundary and nothing needs to be reachable from anywhere\n\
                 # else. moshi-hook finds it by probing local ports and remembering the\n\
                 # ones that answer, so nothing registers itself; an operator with a\n\
                 # narrowed scan-ports list adds this number to it. Two limits pns cannot\n\
                 # detect and does not pretend to: browser preview needs a moshi Pro\n\
                 # subscription, and the tunnel exists only while a terminal session is\n\
                 # open. page_enabled = false is the fallback for either, and it costs nothing\n\
                 # else: the notification stands alone, Discord still carries the full\n\
                 # form whenever the hermes leg worked, and `pns failures` is unchanged.\n",
    opt_in: false,
    children: &[],
    keys: &[
        Key {
            name: "page_enabled",
            prose: "",
            sample: Sample::Default("true"),
        },
        Key {
            name: "page_port",
            prose: "",
            sample: Sample::Default("8646"),
        },
    ],
};
pub(super) const LIGHTS: Table = Table {
    name: "lights",
    prose: LIGHTS_PROSE,
    opt_in: true,
    children: &[],
    keys: &[
        Key {
            name: "arm_interval",
            prose: "# How often the daemon re-arms the lamps, bounded \"10s\" to \"30s\". It is\n\
                         # also the breath budget: a breathing lamp is faded by the tick itself,\n\
                         # seamlessly, across the whole interval, so this decides how many fades\n\
                         # fit between two ticks. The floor is one bridge call, so a tick cannot\n\
                         # start while the last one is still dialling; the ceiling is what the\n\
                         # daemon derives a tick's own lifetime from, and an interval past it\n\
                         # would be a breath cut off part way through.\n",
            sample: Sample::Default("\"12s\""),
        },
        Key {
            name: "dim_window",
            prose: "# THE HOUSE DIM WINDOW, and the only one in the vocabulary: local wall\n\
                         # clock, the start inclusive and the end exclusive, and it may wrap\n\
                         # midnight. Every place below that states no `dim_window` of its own\n\
                         # runs this one, and a place that states one overrides it for that\n\
                         # place alone. A bare `pns lights mute <place>` mutes until this\n\
                         # window ends and is refused when none is set.\n",
            sample: Sample::Example("\"22:00-07:00\""),
        },
    ],
};
