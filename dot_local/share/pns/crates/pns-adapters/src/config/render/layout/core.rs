use super::*;
pub(super) const DAEMON: Table = Table {
    name: "daemon",
    prose: DAEMON_PROSE,
    opt_in: false,
    keys: &[Key {
        name: "enabled",
        prose: "",
        sample: Sample::Default("true"),
    }],
};
pub(super) const RECAP: Table = Table {
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
};
pub(super) const FOCUS: Table = Table {
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
};
pub(super) const NAG: Table = Table {
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
};
pub(super) const LIGHTS: Table = Table {
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
};
