use super::*;
pub(super) const PLUGINS_PRESENCE: Table = Table {
    name: "plugins.presence",
    prose: "# Which room you are in, read off the bridge's per-room motion roll-up so\n\
                 # the lamp that signals is the one beside you. A SENSOR, not a\n\
                 # destination: no event routes to it. It reads the bridge through\n\
                 # [plugins.hue] above, so switching this on with that one off is refused\n\
                 # by name.\n",
    opt_in: true,
    children: &[],
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
    children: &[],
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
pub(super) const PLUGINS_GITHUB: Table = Table {
    name: "plugins.github",
    prose: "# GitHub, polled: which workflow runs, reviews, mentions, releases and\n\
                 # security alerts finished, read off the notifications API once per\n\
                 # interval. A SENSOR rather than a destination, so no event routes to\n\
                 # it; what it finds is submitted through the ordinary producer path and\n\
                 # lands in the channel [plugins.discord.channels] maps the repository\n\
                 # to, or in its `default` catch-all.\n",
    opt_in: true,
    children: &[],
    keys: &[
        Key {
            name: "enabled",
            prose: "",
            sample: Sample::Default("true"),
        },
        Key {
            name: "token",
            prose: "# A CLASSIC personal access token with the `notifications` scope and\n\
                         # nothing else. A fine-grained token cannot call these endpoints at\n\
                         # all: the documentation states they \"only support authentication\n\
                         # using a personal access token (classic)\". `repo` also works and is\n\
                         # write access to every repository you can reach, which a\n\
                         # notification source has no business holding. Not your `gh` login:\n\
                         # pns never reads that, and a daemon whose credentials change when\n\
                         # you re-authorize a CLI is a daemon nobody decided about.\n",
            sample: Sample::Example("\"\""),
        },
        Key {
            name: "poll_secs",
            prose: "# How often the poll runs BEFORE the first answer, bounded 60 to 3600.\n\
                         # From then on the server's own X-Poll-Interval decides, which the\n\
                         # documentation asks for by name, so this is only ever the starting\n\
                         # figure. The floor is 60 because that is what the header says today\n\
                         # and anything under it is a request to be rate-limited; the knob is\n\
                         # for polling SLOWER. Each request sends the stored Last-Modified, so\n\
                         # a quiet minute answers 304 and costs no rate limit at all.\n",
            sample: Sample::Default("60"),
        },
        Key {
            name: "webhook_secret",
            prose: "# The secret the GitHub App's webhook was created with, which is what\n\
                         # arms `pns github receive`: the push transport. Naming one here turns\n\
                         # a delivery into an immediate poll instead of waiting for the next\n\
                         # tick; naming none leaves the receiver exited and the poll is the\n\
                         # source either way. It is a SEPARATE value from the token above and\n\
                         # gets its own vault entry, because the receiver verifies deliveries\n\
                         # with it and reads no notification of its own. The LaunchAgent only\n\
                         # reloads when its plist changes, so arming or rotating this value\n\
                         # needs `launchctl kickstart -k gui/$(id -u)/com.webdavis.pns-github-receiver`.\n",
            sample: Sample::Example("\"\""),
        },
        Key {
            name: "webhook_port",
            prose: "# The loopback port the receiver binds, bounded 1024 to 65535, which\n\
                         # the Cloudflare tunnel's ingress for the webhook hostname points at.\n\
                         # LOOPBACK ONLY: the tunnel is the only way in, and a delivery that\n\
                         # did not arrive through it is refused by its signature anyway.\n",
            sample: Sample::Default("8648"),
        },
    ],
};
