use super::*;
pub(super) const PLUGINS_MOBILE: Table = Table {
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
};
pub(super) const PLUGINS_HERMES: Table = Table {
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
};
pub(super) const PLUGINS_MACOS_BANNER: Table = Table {
    name: "plugins.macos-banner",
    prose: "# The macOS banner, which is what a machine you are sitting at says.\n",
    opt_in: false,
    keys: &[Key {
        name: "enabled",
        prose: "",
        sample: Sample::Default("true"),
    }],
};
pub(super) const PLUGINS_HUE: Table = Table {
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
};
