use super::*;
pub(super) const PLUGINS_MOBILE: Table = Table {
    name: "plugins.mobile",
    prose: "",
    opt_in: false,
    children: &[PLUGINS_MOBILE_IMAGE_CARDS],
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
            name: "url",
            prose: "# Where the card is pushed. Left out it is moshi\u{27}s own webhook endpoint,\n\
                         # which is what a paired phone answers at; name one to point this\n\
                         # install at a gateway of your own.\n",
            sample: Sample::Example("\"https://api.getmoshi.app/api/webhook\""),
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
/// Which card types carry an image, keyed by card type. An OPEN table, shipped
/// with every card type off.
const PLUGINS_MOBILE_IMAGE_CARDS: Table = Table {
    name: "plugins.mobile.image_cards",
    prose: "# Which card types carry an IMAGE of the whole message, keyed by card type\n\
                 # and off for every one of them until you name it here. A card type is the\n\
                 # state word the event carried: `missed` is the card a return raises,\n\
                 # `failed` a turn that died, `done` one that finished.\n\
                 #\n\
                 # WHAT AN IMAGE BUYS: the card's text is cut at 260 characters, and the\n\
                 # image shows the message whole, wrapped at 60 columns.\n\
                 #\n\
                 # WHAT IT COSTS: a moshi card's `data` carries ONE type, so a card with an\n\
                 # image cannot also carry the deep link that focuses the herdr pane the\n\
                 # event came from. Turning a card type on trades that link away for every\n\
                 # card of that type WHICH HAS A PANE. The return card (`missed`) has no\n\
                 # pane and so no link to lose, which is why it is the example below.\n\
                 #\n\
                 # Uploads are capped at 10 an hour by moshi and the links expire after a\n\
                 # day; a refused upload, a refused card and a message with nothing to draw\n\
                 # all fall back to the ordinary text card with its link.\n\
                 #\n\
                 # THE LINK NEEDS NO CREDENTIAL: anyone who has it can read the whole\n\
                 # message for that day, so arm a card type only where that is acceptable.\n",
    opt_in: true,
    children: &[],
    keys: &[Key {
        name: "missed",
        prose: "",
        sample: Sample::Example("false"),
    }],
};
pub(super) const PLUGINS_LOG: Table = Table {
    name: "plugins.log",
    prose: "# The durable paper trail: every event written where it can be read back\n\
                 # later. ONE TABLE, so two of them cannot be declared: `type` names the\n\
                 # transport that carries it, and the credentials for the other one can sit\n\
                 # here ready so the cutover is that single line.\n",
    opt_in: true,
    children: &[PLUGINS_LOG_KEYS, PLUGINS_LOG_CHANNELS],
    keys: &[
        Key {
            name: "enabled",
            prose: "",
            sample: Sample::Default("true"),
        },
        Key {
            name: "type",
            prose: "# Which compiled-in transport carries the log. \"hermes\" posts one signed\n\
                         # request per event to a hermes route; \"discord\" posts straight to a\n\
                         # channel with pns\u{27}s own bot and no gateway in between. Anything\n\
                         # else, and a table naming none, is refused out loud at load.\n",
            sample: Sample::Default("\"hermes\""),
        },
        Key {
            name: "url",
            prose: "# HERMES ONLY: the gateway endpoint. Left out, each route posts to the\n\
                         # shipped address with that route as its last path segment, so renaming\n\
                         # a route moves the path and not the gateway. NAMED, this one address\n\
                         # carries EVERY route verbatim, which is why it is a whole-install\n\
                         # override rather than the usual way to point at your own gateway.\n",
            sample: Sample::Example("\"http://127.0.0.1:8644/webhooks/pns-events\""),
        },
        Key {
            name: "token",
            prose: "# DISCORD ONLY: the bot token, from the Discord application\u{27}s Bot page.\n\
                         # Every call carries it as `Authorization: Bot <token>`, and a discord\n\
                         # log with none posts nothing and says which key is missing.\n",
            sample: Sample::Example("\"\""),
        },
    ],
};
/// One signing key per route, because one key for all of them means a key
/// leaked from any route can post to every route.
///
/// AN OPEN TABLE, AND THE ROSTER ITSELF. Its keys are the route names the
/// operator's own gateway serves, which pns compiles in no list of, so there
/// is nothing to declare here: whatever the values file writes is what goes
/// out, and a route with no key here posts nothing and says which key is
/// missing rather than signing with somebody else's secret.
const PLUGINS_LOG_KEYS: Table = Table {
    name: "plugins.log.keys",
    prose: "# HERMES ONLY: one signing key per route, keyed by the route name and each\n\
                 # prepared in ~/.hermes/config.yaml under that same name. These keys are\n\
                 # the whole roster: a route named here is a route this machine will post\n\
                 # to, and one with no key posts nothing and says which key is missing.\n",
    opt_in: true,
    children: &[],
    keys: &[],
};
/// The channels the bot posts to, keyed by PROJECT. An OPEN table: every key
/// but `default` is a name the operator chose, so the render writes whatever
/// the values file states rather than a roster of its own.
const PLUGINS_LOG_CHANNELS: Table = Table {
    name: "plugins.log.channels",
    prose: "# DISCORD ONLY: where a post goes, looked up in this order, first hit wins:\n\
                 # the route the event named (the urgent route carries anything critical,\n\
                 # whatever the project), then its repository as `owner/name`, then the\n\
                 # bare project name, then the default route for an event with no project\n\
                 # at all, then `default`. The last two are deliberately different\n\
                 # channels: an unmapped project and no project are two failures, in two\n\
                 # places to look. `default` is REQUIRED and an armed discord log without\n\
                 # it is refused at load. A channel id is the number Discord copies from a\n\
                 # channel\u{27}s Copy Channel ID, and it is a secret like every other id\n\
                 # here.\n",
    opt_in: true,
    children: &[],
    keys: &[Key {
        name: "default",
        prose: "",
        sample: Sample::Example("\"\""),
    }],
};
pub(super) const PLUGINS_BANNER: Table = Table {
    name: "plugins.banner",
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
            name: "type",
            prose: "# Which compiled-in surface raises the banner. \"macos\" is the only one\n\
                         # today, and a table naming one nothing answers is refused out loud.\n",
            sample: Sample::Default("\"macos\""),
        },
        Key {
            name: "terminal_bundle_id",
            prose: "# The terminal a banner click returns to, as a bundle id. Left out, the\n\
                         # one this process was started from is used, which is right on a machine\n\
                         # with one terminal; name it where a hook runs under launchd and\n\
                         # inherits none.\n",
            sample: Sample::Example("\"com.mitchellh.ghostty\""),
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
pub(super) const PLUGINS_LIGHTS: Table = Table {
    name: "plugins.lights",
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
            name: "type",
            prose: "# Which compiled-in bridge answers. \"hue\" is the only one today, and a\n\
                         # table naming one nothing answers is refused out loud.\n",
            sample: Sample::Default("\"hue\""),
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
            name: "certificate",
            prose: "# The one certificate the bridge may present, as `sha256:<64 hex>`.\n\
                         # REQUIRED, and a table with none refuses to pulse rather than\n\
                         # trusting whatever answers the address: the bridge\u{27}s certificate\n\
                         # carries no name any verifier can check, so its own fingerprint is\n\
                         # the whole of what makes this address the device you meant. Get the\n\
                         # value from `pns lights enroll`, which prints this line ready to\n\
                         # paste; it changes only when the bridge hardware does.\n",
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
                         # `pns lights pulse` is exempt, so a bridge and key can be checked\n\
                         # in-window.\n\
                         # A bare `pns lights quiet <place>` mutes until this window ends and is\n\
                         # refused when none is set. With a `[lights]` table below, each place's\n\
                         # own `dim_window` decides the night instead and this window is the\n\
                         # mute's schedule alone.\n",
            sample: Sample::Example("\"22:00-07:00\""),
        },
    ],
};
