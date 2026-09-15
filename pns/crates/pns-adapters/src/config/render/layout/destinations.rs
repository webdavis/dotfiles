use super::*;
pub(super) const PLUGINS_MOBILE: Table = Table {
    name: "plugins.mobile",
    prose: "",
    opt_in: false,
    children: &[],
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
};
pub(super) const PLUGINS_DISCORD: Table = Table {
    name: "plugins.discord",
    prose: "# The durable paper trail, posted straight to Discord by pns's own bot with\n\
                 # no gateway in between. THE ALTERNATIVE TO [plugins.hermes] ABOVE, never a\n\
                 # companion: both enabled at once is refused at load, naming both tables,\n\
                 # because two durable logs post every event twice. The cutover is two lines\n\
                 # in one edit, and the rollback is the same two the other way.\n",
    opt_in: true,
    children: &[PLUGINS_DISCORD_CHANNELS],
    keys: &[
        Key {
            name: "enabled",
            prose: "",
            sample: Sample::Default("true"),
        },
        Key {
            name: "type",
            prose: "# Which compiled-in transport carries the post. \"bot\" is the only one\n\
                         # today, and a table naming none, or naming one nothing answers, is\n\
                         # refused out loud rather than read as this one.\n",
            sample: Sample::Default("\"bot\""),
        },
        Key {
            name: "token",
            prose: "# The bot token, from the Discord application\u{27}s Bot page. Every call\n\
                         # carries it as `Authorization: Bot <token>`, and a table with none\n\
                         # posts nothing and says which key is missing.\n",
            sample: Sample::Example("\"\""),
        },
    ],
};
/// The channels the bot posts to, keyed by PROJECT. An OPEN table: every key
/// but `default` is a name the operator chose, so the render writes whatever
/// the values file states rather than a roster of its own.
const PLUGINS_DISCORD_CHANNELS: Table = Table {
    name: "plugins.discord.channels",
    prose: "# Where a post goes, looked up in this order, first hit wins: the route the\n\
                 # event named (the urgent route carries anything critical, whatever the\n\
                 # project), then its repository as `owner/name`, then the bare project\n\
                 # name, then the default route for an event with no project at all, then\n\
                 # `default`. The last two are deliberately different channels: an\n\
                 # unmapped project and no project are two failures, in two places to\n\
                 # look. `default` is REQUIRED and an armed table without it is refused at\n\
                 # load. A channel id is the number Discord copies from a channel\u{27}s\n\
                 # Copy Channel ID, and it is a secret like every other id here.\n",
    opt_in: true,
    children: &[],
    keys: &[Key {
        name: "default",
        prose: "",
        sample: Sample::Example("\"\""),
    }],
};
pub(super) const PLUGINS_HERMES: Table = Table {
    name: "plugins.hermes",
    prose: "# The durable paper trail: every event posted to a hermes route, signed\n\
                 # with the key that route verifies.\n",
    opt_in: true,
    children: &[PLUGINS_HERMES_KEYS],
    keys: &[Key {
        name: "enabled",
        prose: "",
        sample: Sample::Default("true"),
    }],
};
/// One signing key per route, because one key for all of them means a key
/// leaked from any route can post to every route.
///
/// AN OPEN TABLE, AND THE ROSTER ITSELF. Its keys are the route names the
/// operator's own gateway serves, which pns compiles in no list of, so there
/// is nothing to declare here: whatever the values file writes is what goes
/// out, and a route with no key here posts nothing and says which key is
/// missing rather than signing with somebody else's secret.
const PLUGINS_HERMES_KEYS: Table = Table {
    name: "plugins.hermes.keys",
    prose: "# One signing key per route, keyed by the route name and each prepared in\n\
                 # ~/.hermes/config.yaml under that same name. These keys are the whole\n\
                 # roster: a route named here is a route this machine will post to, and\n\
                 # one with no key posts nothing and says which key is missing.\n",
    opt_in: true,
    children: &[],
    keys: &[],
};
pub(super) const PLUGINS_MACOS_BANNER: Table = Table {
    name: "plugins.macos-banner",
    prose: "# The macOS banner, which is what a machine you are sitting at says.\n",
    opt_in: false,
    children: &[],
    keys: &[
        Key {
            name: "enabled",
            prose: "",
            sample: Sample::Default("true"),
        },
        Key {
            name: "click_type",
            prose: "# What a click on a DELIVERY FAILURE banner opens: \"herdr\" splits a pane\n\
                         # in the running session, \"window\" opens a new terminal window, and\n\
                         # \"command\" runs `click_command` below. Left unset it is inferred: the\n\
                         # pane when herdr is on PATH, a window when it is not. A click on an\n\
                         # ordinary banner still focuses the pane its own event came from.\n",
            sample: Sample::Example("\"herdr\""),
        },
        Key {
            name: "click_command",
            prose: "# The command `click_type = \"command\"` runs, with `{id}` replaced by the\n\
                         # failure's id. A click runs in a bare launchd context with no PATH, so\n\
                         # every program here needs an absolute path, and the string is split on\n\
                         # whitespace rather than handed to a shell.\n",
            sample: Sample::Example("\"\""),
        },
    ],
};
pub(super) const PLUGINS_HUE: Table = Table {
    name: "plugins.hue",
    prose: "# The light pulse: the named rooms flash green when work finishes and red\n\
                 # when it dies. Needs the bridge's address, a key it issued, and the rooms\n\
                 # spelled the way the bridge spells them.\n",
    opt_in: true,
    children: &[],
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
};
