pub(super) const HEADER: &str = "# The pns engine's plugin selection, as `pns setup` first wrote it. A\n\
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

pub(super) const DAEMON_PROSE: &str = "# The clock: what runs BETWEEN events, for the two things that are not\n\
     # reactions to one, saying something when nothing happened and keeping a\n\
     # lamp alive while an agent loop is. It holds no state of its own, so a\n\
     # restart loses nothing and a stopped daemon costs those ambient features\n\
     # and never a card. ON UNLESS YOU SAY OTHERWISE, because it delivers\n\
     # nothing by itself; deleting the table is the same statement, and\n\
     # `pns doctor` says which state it is in.\n";

pub(super) const RECAP_PROSE: &str = "# The return recap: what you missed while you were away. THE UNCOMMENTED\n\
     # LINES ARE THE DEFAULTS, written out so they can be seen; each switch\n\
     # gates only its own delivery, and deleting the whole table gets exactly\n\
     # the same behaviour.\n";

pub(super) const LIGHTS_PROSE: &str = "# The lamp map: WHICH LAMP says what. A declaration names a place at one\n\
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
pub(super) const TRAILER: &str = "# ONE MORE MUTE, TYPED RATHER THAN CONFIGURED, and it is LIGHTS ONLY:\n\
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
pub(super) const ROUTING: &str = "# The routing. `dim_window` is local wall clock, the start inclusive and\n\
     # the end exclusive, and it may wrap midnight; `dim_behaviours` names\n\
     # which behaviours run their dim form inside it, and everything else that\n\
     # place carries is SUPPRESSED there. A window with an empty list therefore\n\
     # takes every behaviour away for the night and needs no mode of its own.\n\
     # A place with no window is untouched at every hour; one that states\n\
     # behaviours and no window keeps inheriting its room's window.\n";

/// Written commented, whichever way `[lights]` reads, and only when the
/// caller declared no place of its own: a real declaration is a better
/// example than this one.
pub(super) const EXAMPLE_DECLARATION: &str = "# [lights.room.\"Studio\"]\n\
     # shows = [\"done\", \"failed\"]\n\
     # dim_window = \"22:00-07:00\"\n\
     # dim_behaviours = [\"blocked\", \"unread\", \"loop\"]\n\n";
