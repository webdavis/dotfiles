use super::*;
pub(super) const PLUGINS_PRESENCE: Table = Table {
    name: "plugins.presence",
    prose: "# Which room you are in, read off the bridge's per-room motion roll-up so\n\
                 # the lamp that signals is the one beside you. A SENSOR, not a\n\
                 # destination: no event routes to it. It reads the bridge through\n\
                 # [plugins.hue] above, so switching this on with that one off is refused\n\
                 # by name.\n",
    opt_in: true,
    keys: &[
        Key {
            name: "enabled",
            prose: "",
            sample: Sample::Default("true"),
        },
        Key {
            name: "type",
            prose: "# Which compiled-in backend answers. \"hue\" is the only one today, and a\n\
                         # table naming none, or naming one nothing answers, is refused out loud.\n",
            sample: Sample::Default("\"hue\""),
        },
        Key {
            name: "rooms",
            prose: "# The rooms a reading may name, spelled the way the bridge spells them. A\n\
                         # room with no motion sensor or MotionAware area never reports, and an\n\
                         # empty list watches nothing at all.\n",
            sample: Sample::Example("[]"),
        },
        Key {
            name: "exclude",
            prose: "# Rooms whose presence is discarded even when they are listed above: a\n\
                         # room you pass through, or one you never want lit.\n",
            sample: Sample::Example("[]"),
        },
        Key {
            name: "desk_room",
            prose: "# The room the desk is in, spelled the way the bridge spells it, listed\n\
                         # in rooms above and not in exclude. While the keyboard still speaks\n\
                         # for where you are, this is the room the lamps narrow to, and motion\n\
                         # in it agrees. Newer motion in ANOTHER room is two live readings that\n\
                         # cannot both be you, and nothing narrows for as long as both stand: a\n\
                         # keystroke ends it here, the motion going quiet ends it there. Unset,\n\
                         # the keyboard names no room and motion answers alone, exactly as it\n\
                         # does once the desk goes stale.\n",
            sample: Sample::Example("\"\""),
        },
        Key {
            name: "desk_stale_after_secs",
            prose: "# How long the desk keeps speaking for where you ARE after the last\n\
                         # keystroke, bounded 1 to 3600. Inside it a warm desk beats motion of\n\
                         # the same age, so a cat crossing the kitchen cannot move the lamps off\n\
                         # a keyboard being typed at; past it a keyboard nobody has touched says\n\
                         # nothing about which room you are standing in, and fresher motion\n\
                         # wins. The hour ceiling is what keeps a mistyped digit from parking\n\
                         # the lamps in desk_room for good.\n",
            sample: Sample::Default("120"),
        },
        Key {
            name: "poll_secs",
            prose: "# How often the bridge is read, in seconds, bounded 2 to 60.\n",
            sample: Sample::Default("5"),
        },
        Key {
            name: "stale_after_secs",
            prose: "# How old that read may be before there is no reading at all. A BRIDGE\n\
                         # THAT STOPPED ANSWERING IS UNKNOWN, never \"present\" and never \"away\":\n\
                         # presence only ever narrows which lamp signals, so not knowing costs\n\
                         # the narrowing and nothing else. It may not be under poll_secs, or\n\
                         # every reading would age out before the next read replaced it.\n",
            sample: Sample::Default("15"),
        },
    ],
};
pub(super) const PLUGINS_ROUTER: Table = Table {
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
};
