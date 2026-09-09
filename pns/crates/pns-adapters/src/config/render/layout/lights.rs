use super::*;
pub(super) const LIGHTS_DONE: Table = Table {
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
};
pub(super) const LIGHTS_FAILED: Table = Table {
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
};
pub(super) const LIGHTS_BLOCKED: Table = Table {
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
                         # expiry: the locked behaviour is the blocked lamp breathing, continuous\n\
                         # until you answer, and the ordinary end is your session's next event,\n\
                         # whatever the hour. 57600 (16 hours) outlasts a long day away and still\n\
                         # gives the lamp back before the next one starts. The range is 60 to\n\
                         # 604800 (a week), since an abandoned wait can span a weekend away.\n",
            sample: Sample::Default("57600"),
        },
    ],
};
pub(super) const LIGHTS_UNREAD: Table = Table {
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
};
pub(super) const LIGHTS_LOOP: Table = Table {
    name: "lights.loop",
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
            sample: Sample::Default("80"),
        },
        Key {
            name: "low",
            prose: "",
            sample: Sample::Default("10"),
        },
        Key {
            name: "flare",
            prose: "# The accent at the peak, which is the one shape that has one: the lamp\n\
                         # rises to `high`, flashes to `flare` for `flare_ms`, then falls back\n\
                         # to `low`. The flash must be brighter than `high` and briefer than\n\
                         # `duration_ms`.\n",
            sample: Sample::Default("100"),
        },
        Key {
            name: "flare_ms",
            prose: "",
            sample: Sample::Default("200"),
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
};
pub(super) const LIGHTS_DIM: Table = Table {
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
};
