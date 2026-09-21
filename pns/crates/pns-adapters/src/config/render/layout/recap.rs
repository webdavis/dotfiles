use super::*;

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
                "[\"bash\", \"-c\", \"gh pr list --search \\\"updated:>={since}\\\" --json \
                 number,title,headRefName | jq -r '.[] | \\\"#\\\\(.number) \\\\(.headRefName) \
                 \\\\(.title)\\\"'\"]",
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
