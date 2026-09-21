use super::*;
pub(super) const LIGHTS_DONE: Table = Table {
    name: "lights.done",
    prose: "# The five behaviour shapes. Every number below was set on a real lamp;\n\
                 # only the knobs that APPLY to a behaviour exist, so a blink has a\n\
                 # duration and one brightness and a breath has a duration and two ends.\n\
                 # A `duration` is a fade, bounded \"200ms\" to \"5s\"; a `_percent` key is 1\n\
                 # to 100.\n",
    opt_in: true,
    children: &[],
    keys: &[
        Key {
            name: "duration",
            prose: "",
            sample: Sample::Default("\"4s\""),
        },
        Key {
            name: "brightness_percent",
            prose: "",
            sample: Sample::Default("100"),
        },
    ],
};
pub(super) const LIGHTS_FAILED: Table = Table {
    name: "lights.failed",
    prose: "",
    opt_in: true,
    children: &[],
    keys: &[
        Key {
            name: "duration",
            prose: "",
            sample: Sample::Default("\"4s\""),
        },
        Key {
            name: "brightness_percent",
            prose: "",
            sample: Sample::Default("100"),
        },
    ],
};
pub(super) const LIGHTS_BLOCKED: Table = Table {
    name: "lights.blocked",
    prose: "",
    opt_in: true,
    children: &[],
    keys: &[
        Key {
            name: "duration",
            prose: "",
            sample: Sample::Default("\"2s\""),
        },
        Key {
            name: "high_percent",
            prose: "",
            sample: Sample::Default("100"),
        },
        Key {
            name: "low_percent",
            prose: "",
            sample: Sample::Default("30"),
        },
        Key {
            name: "lease_expiry",
            prose: "# How long an unanswered wait may hold the lamp before the daemon gives\n\
                         # up on an abandoned session. This is a BACKSTOP: the locked behaviour\n\
                         # is the blocked lamp breathing, continuous until you answer, and the\n\
                         # ordinary end is your session's next event, whatever the hour. \"16h\"\n\
                         # outlasts a long day away and still gives the lamp back before the next\n\
                         # one starts. It is bounded \"1m\" to \"168h\" (a week), since an abandoned\n\
                         # wait can span a weekend away.\n",
            sample: Sample::Default("\"16h\""),
        },
    ],
};
pub(super) const LIGHTS_UNSEEN: Table = Table {
    name: "lights.unseen",
    prose: "",
    opt_in: true,
    children: &[],
    keys: &[
        Key {
            name: "duration",
            prose: "",
            sample: Sample::Default("\"4s\""),
        },
        Key {
            name: "high_percent",
            prose: "",
            sample: Sample::Default("60"),
        },
        Key {
            name: "low_percent",
            prose: "",
            sample: Sample::Default("10"),
        },
        Key {
            name: "arm_after",
            prose: "# How old a FINISHED run must be before its lamp arms, bounded \"0s\" to\n\
                         # \"24h\". A run that DIED has no such delay and no knob. \"0s\" arms at\n\
                         # once.\n",
            sample: Sample::Default("\"5m\""),
        },
    ],
};
pub(super) const LIGHTS_CHECKS: Table = Table {
    name: "lights.checks",
    prose: "# The one behaviour that carries its own COLOURS: a GitHub event, purple\n\
                 # for a pass and orange for a failure, as CIE xy `[x, y]` pairs (which is\n\
                 # what the bridge takes; a hex colour would be clamped into its gamut and\n\
                 # desaturated). One brightness for both, as `unseen` has.\n\
                 #\n\
                 # THE PAIR IS CHOSEN FOR A LAMP OF ITS OWN. Give `checks` a lamp whose\n\
                 # `behaviours` names nothing else: the purple sits close enough to the blocked\n\
                 # magenta, and the orange close enough to the unseen daylight, that a lamp\n\
                 # carrying either alongside it cannot be told apart across a room.\n",
    opt_in: true,
    children: &[],
    keys: &[
        Key {
            name: "duration",
            prose: "",
            sample: Sample::Default("\"4s\""),
        },
        Key {
            name: "brightness_percent",
            prose: "",
            sample: Sample::Default("100"),
        },
        Key {
            name: "pass_color",
            prose: "",
            sample: Sample::Default("[0.2725, 0.1283]"),
        },
        Key {
            name: "fail_color",
            prose: "",
            sample: Sample::Default("[0.5562, 0.4084]"),
        },
    ],
};
pub(super) const LIGHTS_LOOP: Table = Table {
    name: "lights.loop",
    prose: "# A live pane lease keeps the summarizer's guessed waits from arming\n\
            # blocked; real hook waits still take priority.\n",
    opt_in: true,
    children: &[],
    keys: &[
        Key {
            name: "duration",
            prose: "",
            sample: Sample::Default("\"4s\""),
        },
        Key {
            name: "high_percent",
            prose: "",
            sample: Sample::Default("80"),
        },
        Key {
            name: "low_percent",
            prose: "",
            sample: Sample::Default("10"),
        },
        Key {
            name: "flare_percent",
            prose: "# The accent at the peak, which is the one shape that has one: the lamp\n\
                         # rises to `high_percent`, flashes to `flare_percent` for\n\
                         # `flare_duration`, then falls back to `low_percent`. The flash must be\n\
                         # brighter than `high_percent` and briefer than `duration`.\n",
            sample: Sample::Default("100"),
        },
        Key {
            name: "flare_duration",
            prose: "",
            sample: Sample::Default("\"200ms\""),
        },
        Key {
            name: "arm_after",
            prose: "# How long work must run continuously before the lamp arms itself, bounded\n\
                         # \"1s\" to \"24h\". Both an agent herdr calls working and a tracked shell\n\
                         # command count.\n",
            sample: Sample::Default("\"5m\""),
        },
        Key {
            name: "lease_expiry",
            prose: "# How long a lease taken by `pns loop begin` survives with nothing\n\
                         # renewing it, bounded \"1m\" to \"24h\". The pane's own hook traffic\n\
                         # renews it.\n",
            sample: Sample::Default("\"65m\""),
        },
    ],
};
pub(super) const LIGHTS_DIM: Table = Table {
    name: "lights.dim",
    prose: "# The DIM FORM: one shape, shared by every behaviour that runs dimmed, at\n\
                 # the faintest levels the hardware has. A dimmed BLINK fires at\n\
                 # `low_percent`, since a blink has no low end to fade to.\n",
    opt_in: true,
    children: &[],
    keys: &[
        Key {
            name: "duration",
            prose: "",
            sample: Sample::Default("\"3s\""),
        },
        Key {
            name: "high_percent",
            prose: "",
            sample: Sample::Default("7"),
        },
        Key {
            name: "low_percent",
            prose: "",
            sample: Sample::Default("1"),
        },
    ],
};
