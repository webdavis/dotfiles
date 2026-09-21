# pns recap design

Date: 2026-09-19. Status: approved in conversation, awaiting the operator's read of this document.

## Purpose

`pns recap` prints what happened on this machine over a window of time: what the agents did, which
pull requests moved, what the task tool says is due, whether an apply is owed, and what is still
waiting on a person. It replaces `morning`, a separate Rust workspace that printed a fixed page once
a day, and it folds in the return card pns already posts to Discord when the operator comes back to
the desk, so there is one recap engine with several callers.

A recap is one input to a larger page, never the page itself. Bob, the Forzare executive assistant,
composes the morning brief it delivers to the operator from three sources: this recap (what happened overnight, taken as
`pns recap overnight --json` or through a `--schema` mask), the task tool's view of the day (`dam`),
and things neither tool knows, such as the weather and the day's news. `pns recap` therefore keeps
its name and its machine-readable document stable for that consumer, and never grows sections for
data it does not own.

The design was settled in a question-and-answer session on 2026-09-19. Every decision below is a
ruling from that session; the reasoning is kept short and the decision is what binds.

## Decisions in force

1. `pns recap` is a pns subcommand. `morning` is deleted: the workspace, its build script, its config
   template, its justfile lines, its `.chezmoiignore` entry and its CLAUDE.md paragraph. The deployed
   binary and config are trashed by hand after the apply that removes the build script.
1. The task tool is never assumed. Tasks, pull requests and applies each come from a command the
   operator names in config. On this machine tasks come from `dam`; someone else points them at
   Todoist or Things 3.
1. Agent activity comes from pns's own durable store, written by the hooks pns already receives from
   Claude Code and Codex. hermes is a delivery destination, not a hook source, and stays out of the
   store until it gets a hook of its own.
1. Every flag has a JSON field of the same name, the rule the pns refactor fixed for the request
   envelope: `--since` is `since`, `--section` is `sections`, `--to` is `to`, `--previous` is
   `previous`, `--recent` is `recent`, `--limit` is `limit`, `--schema` is `schema`, `--verbose` is
   `verbose`.
1. `pns gateway` absorbs `pns daemon`. The four verbs `run`, `retry`, `schedule` and `cancel` move
   under `gateway` beside the `start`, `stop`, `restart` and `status` verbs that landed on
   2026-09-19, and the `[daemon]` table (`enabled`, `service`) becomes `[gateway]`. The launchd label
   does not change.

## Command surface

```
pns recap                                  the window that most recently ended
pns recap overnight|morning|afternoon|evening
pns recap today|yesterday|week|last-week
pns recap open                             what is waiting on a person, no window
pns recap <window> --previous              one instance back
pns recap --since <when> [--until <when>]
pns recap --duration <n>[m|h|d|w]
pns recap --recent <n>                    the last n sessions with any activity, however old
pns recap agent --stdin                    unchanged: forward a composed recap to the durable route
pns recap git                              unchanged: print the Git block for the work-recap layout
```

Modifiers, valid on every window form:

| Flag | Meaning |
| --- | --- |
| `-v`, `--verbose` | show empty sections and the per-session detail listed under Output |
| `--section <name>` | repeatable; limit the output to the named sections |
| `--limit <n>` | at most `n` rows per list for this run, overriding `rows_per_section` |
| `--json`, `--toon` | emit the whole document in that format instead of the styled page |
| `--schema <file>` or `--schema -` | a JSON or TOON field mask; the output is the document in the mask's own format |
| `--to <destination>` | send the recap to one of pns's configured destinations by name |
| `--summarize` | run the summarizer for this page, or regenerate a stored summary |
| `--with-transcripts` | hand the model each session's assistant turns as well as the document |

A window name, `--since`, `--duration` and `--recent` are four ways of naming one span, so any two
together are refused with exit 2 and a sentence naming the one to keep. `--previous` without a window
name is refused the same way. `--recent <n>` is count-bounded rather than time-bounded: the last `n`
sessions that had any activity, however long ago, which is the question no time window answers after
a quiet weekend. Its document `window` is `{"name": "recent", "count": n}` with no bounds, and the
windowed source commands receive no `{since}` or `{until}`, so a command that uses them is skipped
with its section saying so. `--schema` on its own selects document output: a JSON mask returns JSON and a
TOON mask returns TOON. `--json` or `--toon` beside `--schema` overrides the mask's format, so
`--json --schema mask.toon` is a TOON mask with JSON out.

`--since` and `--until` take a date (`2026-09-19`), a date-time (`2026-09-19T08:00`), or a duration
ago (`2h`, `3d`); `--until` defaults to now. They replace `--since-epoch` and `--until-epoch` on
`pns recap`; the daemon's `schedule --until +<duration>` and `--until-epoch` keep their names,
because those are the clock's flags and nobody types them.

## Windows

The four periods are local time and ship at these defaults in `[recap]`, editable in config:

| Window | Default |
| --- | --- |
| overnight | 22:00 to 06:00 |
| morning | 06:00 to 12:00 |
| afternoon | 12:00 to 17:00 |
| evening | 17:00 to 22:00 |

`today` is midnight to now. `week` is the start of the configured week to now, with
`week_starts_on = "monday"` by default and `"sunday"` the other value. `yesterday` is exactly
`today --previous` and `last-week` is exactly `week --previous`; the names are shortcuts, not separate
rules.

A named window means its most recent instance, in progress or complete: `pns recap morning` at 10:00
is this morning so far, and at 15:00 it is this morning complete. `pns recap overnight` at 08:00 is
last night. `--previous` steps back exactly one instance from whatever the bare name would have
chosen.

Bare `pns recap` picks the window that most recently ended, so sitting down at 08:00 gives overnight
and coming back at 13:30 gives morning.

`open` has no window and takes no `--previous`, `--since`, `--duration` or `--recent`. It prints the
`open` section alone, headed by one line naming the herdr workspace and pane the operator was last
in, with that pane's branch and worktree, read from the sessions store and the herdr CLI. It is the
"where was I" answer: `pns recap open` at the keyboard, `pns recap open --to banner` from an unlock
automation. `--limit` is the flag that says how many of each list to show.

The four periods must tile the day with no gap and no overlap; a config that breaks that is refused at
load with the two windows named.

## Sections and sources

The page has these sections, in this order. `--limit <n>` caps every list at `n` rows for one run;
`rows_per_section` is the configured default.

| Section | Source | Windowed |
| --- | --- | --- |
| `agents` | pns's activity store | yes |
| `pull_requests` | `[recap.sources] pull_requests` command | yes, through `{since}` and `{until}` |
| `commits` | `[recap.sources] commits` command | yes, through `{since}` and `{until}` |
| `tasks` | `[recap.sources] tasks` command | the command decides |
| `applies` | `[recap.sources] applies` command | the command decides |
| `review_notes` | `[recap] review_notes_glob`, carried over from the return card | yes, by file mtime |
| `summary` | the summarizer over the assembled document | yes, it summarizes the window |
| `open` | the activity store plus the `pull_requests` and `applies` commands | no |

A source command is an argv list. Before it runs, `{since}` and `{until}` in any argument are
replaced with the window bounds as RFC 3339 timestamps with the local offset
(`2026-09-19T06:00:00-04:00`). The command's standard output is one row per line; `rows_per_section`
caps the rows and a final line says how many more there were. A section whose command is not
configured is absent from the page, from the document and from `--section`'s accepted names. A
command that fails or times out renders as one line naming the exit code, never as an empty section.

`open` is the section that always prints, even when empty: an empty "Open items" is the news the page
exists to carry, and it is also the whole page under `pns recap open`. It holds sessions whose last state
is `blocked`, `asked` or `waiting` with no later `resolved` for the same session; open pull requests as
the `pull_requests` command reports them when given no window; and applies owed as the `applies` command
reports them. When neither command is configured, `open` holds only the sessions.

Empty windowed sections are omitted unless `-v` is given.

`commits` exists for work that has neither an agent session nor a pull request behind it: commits made
by hand in the window. It follows the source-command contract exactly, one row per line, so a command
such as `git -C <repo> log --all --since={since} --until={until} --format=%h %s` reports one repository
and a wrapper script reports several. It ships unconfigured, so a fresh install has no `commits`
section until the operator names the command.

## The agents section

Records are grouped by project. Under each project, one line per session:

```
dotfiles-modernization  claude  feat/pns-gateway  2h 14m  done  #795
```

That is the title, the harness, the branch, how long the session ran inside the window, its last
state in the window, and the pull request it opened or merged if the `pull_requests` rows name one
on that branch.

Under `-v` each session line gains the session id, the pane, the herdr workspace label and the
model, and is followed by one line per event with its time and state.

The title is chosen in this order: the name the operator gave the session (Claude Code writes it as
`customTitle` in the session transcript), then the harness's own generated title (`aiTitle`), then
the opening words of the first prompt. Codex sends no title and no prompt, so a Codex session shows
project and branch alone. The title is read from the transcript file when a hook fires, so a rename
mid-session shows on the next event.

## The activity store

pns keeps a durable SQLite table of hook events, one row per event, in the same database as the
ledger. It replaces the activity ring as the recap's source; the ring stays until the return card
reads from the table, then goes.

| Column | From |
| --- | --- |
| `at` | the hook's arrival, RFC 3339 with offset |
| `agent` | the harness name already in the request |
| `state` | the pns state word already in the request |
| `project`, `branch` | derived from git as today |
| `session` | `session_id` from the payload |
| `session_title` | the transcript's `customTitle`, else `aiTitle`, else the first prompt |
| `pane` | `HERDR_PANE_ID` |
| `workspace` | `HERDR_WORKSPACE_ID` |
| `model` | the `SessionStart` payload when it carries one, updated by switch events, else the transcript's most recent assistant line |
| `title`, `detail` | as the request carries them |

Nothing here spawns a process. The transcript read is one bounded tail read of a file the payload
already names, done once per event, and it may lag a turn because Claude Code writes the transcript
asynchronously. The herdr workspace label is not stored; the page resolves it from the id once at
render time by asking herdr, and leaves it blank when herdr is absent.

Rows are kept for `[recap] retain`, a duration string defaulting to thirty days, written `"720h"`
because pns's duration parser takes `<count><ms|s|m|h>` and has no day unit; the gateway prunes on
its tick, once an hour rather than once a second. Existing ring contents are not migrated; the table
starts empty. The `at` column is epoch seconds, like every other time column in that database, and
the reader formats the local time it prints.

## Output

The default output is the pns house style: the accent colour on the title, `◆ Title ──` section
headings, the faint colour for metadata, no box, width from the terminal clamped to the same range
the other pages use, and plain text under `NO_COLOR`, `--no-color` or a pipe.

`--json` and `--toon` emit one document and nothing else. It always carries every configured section,
empty or not, because omission is a display choice and a consumer wants the whole shape every time.
Verbose detail is always present in the document. The shape:

```json
{
  "schema": 1,
  "window": { "name": "morning", "since": "...", "until": "...", "previous": false },
  "sections": {
    "agents": [ { "project": "...", "sessions": [ { "title": "...", "harness": "...",
                  "branch": "...", "session": "...", "pane": "...", "workspace": "...",
                  "model": "...", "duration_secs": 0, "last_state": "...", "pull_request": null,
                  "events": [ { "at": "...", "state": "...", "title": "...", "detail": "..." } ] } ] } ],
    "pull_requests": { "rows": [ "..." ], "more": 0, "at_least": false },
    "commits": { "rows": [ "..." ], "more": 0, "at_least": false },
    "tasks": { "rows": [ "..." ], "more": 0, "at_least": false },
    "applies": { "rows": [ "..." ], "more": 0, "at_least": false },
    "review_notes": { "rows": [ "..." ], "more": 0, "at_least": false },
    "summary": { "text": "...", "written_at": "...", "source": "claude" },
    "open": { "sessions": [ "..." ], "pull_requests": [ "..." ], "applies": [ "..." ] }
  }
}
```

`schema` is the version of that shape; a consumer checks it and a change bumps it.

`--schema <file|->` takes a field mask: a JSON or TOON document whose keys mirror the output and whose
values are `true` where a field is wanted. Absent keys are omitted from the output. An unknown key in
the mask is refused with exit 2 naming it, so a typo never silently empties a field. The format is
detected from the content, so a file may be either and stdin may be either, and that detected format
is the output format unless `--json` or `--toon` says otherwise.

```
pns recap morning --schema ~/.config/pns/recap-sessions.json   # JSON out
pns recap morning --schema - < mask.toon                        # TOON out
```

`--section` applies to both the page and the document; the document keeps `schema` and `window` and
drops the other sections.

## Delivery

With no `--to`, the recap goes to standard output. With `--to <name>`, it goes to the pns destination
of that name, which is the same roster the delivery plugins register (the Discord route, the phone,
the banner, a hermes route), and nothing is printed except one line saying where it landed, the way
`pns recap agent` reports today. A destination that does not exist is refused with exit 2 naming the
configured ones.

A destination receives the styled page rendered plain, or the document when `--json` or `--toon` is
also given. The Discord route keeps its 2000-character limit and the existing shedding rules: whole
sections go before any line is cut, and `open` is never shed. The summarizer (below) runs for a
delivered recap and for the return card without being asked, and on the terminal only with `--summarize`.

The return card becomes one more caller of this engine. At the return moment the event path spawns
`pns recap --since <last present> --until now --to <durable route>` when `[recap] post_window_recap` is on and
the window holds at least `minimum_events` events; `replay_card` keeps its meaning. The card's own
sections are the same sections with the same renderer, so the Discord card and the terminal page stop
drifting apart. `[recap] repositories` (slice 45's spelling of `repos`) retires: the `pull_requests` source command
is where a repository list lives now.

## The summarizer

A model writes one paragraph over the assembled document, the `summary` section: what moved, what is
waiting, what the operator should look at first. It is present in every output form, the page, the
document and any delivery, so Bob and every other consumer get the same paragraph. The mechanical
sections stay exact underneath it, and the model never rewrites `open`.

Settled on 2026-09-19 in a second question-and-answer session:

- **Harnesses are first class, and a custom argument vector is the escape hatch.** `[recap.summarizer]
  type` is one of `claude`, `codex`, `ollama`, `hermes` or `custom`. For a known type pns owns the
  invocation (`claude -p`, `codex exec`, `ollama run` with the three flags that keep control bytes out
  of its output, `hermes chat -q -Q` with tools off) and `model` is passed through where the tool takes
  one. `custom` reads `command`, an argument vector executed directly with no shell, the prompt on stdin
  and the answer on stdout, the same shape the source commands use. `type` other than `custom` with a
  non-empty `command`, or `custom` with an empty one, is refused at load. hermes qualifies because
  `hermes chat -q <prompt> -Q` runs one non-interactive query and prints only the final answer plus a
  session line; the build strips that line and pins the toolset flag that keeps it from running tools.
- **Drift is caught three ways.** A golden test pins the exact argument vector pns composes for each
  known type, so pns cannot drift on its own. `pns doctor` gains a summarizer row that runs the
  configured invocation with a two-word prompt under the deadline and reports the answer or the
  failure, which is the one check that talks to the real binary and notices the day a harness changes
  its flags. And a failure at run time is loud (below).
- **Failure is loud and present by default.** A summarizer that is missing, refuses, says nothing or
  runs past `summarizer_deadline` leaves one visible line in the summary's place naming which, on every
  output form, not only under `-v`. The mechanical sections are unaffected.
- **The model reads the document, and transcripts only when asked.** The input is the recap's own JSON
  document (schema 1). `--with-transcripts`, or `[recap.summarizer] transcripts = true`, appends each
  session's own assistant turns from its transcript, newest first, capped by `transcript_bytes_per_session`
  and `transcript_bytes_total`; user prompts and tool results are left out because they carry paths and
  secrets. The excerpt is fed to the model only and never enters the document other consumers read.
  Codex sessions are read from their rollout files the same way, which is why the activity store keeps
  `transcript_path`.
- **When it runs.** Without being asked, only when the recap is delivered (`--to` and the return card).
  `--summarize` asks for it on the terminal. `pregenerate` lists the windows whose summary the gateway
  writes in the background at each window's end, `today` and `week` included if named; the paragraph is
  stored in the activity store beside the events, retained under `retain`, and printed with the time it
  was written. A stored summary older than the window's last event is regenerated rather than shown.
  `--summarize` on a window with a stored summary regenerates it.
- **The prompt is fixed in pns, with an override.** `prompt` (inline) or `prompt_file` (a path), both
  set is refused, replaces the recap-summary instruction only; pns still appends the document, so the
  operator writes the instruction and not the plumbing. The per-turn notification sentence keeps its
  fixed prompt, because pns parses its `STATE|SUMMARY` answer.
- **No model call titles a session.** A session with neither a custom nor a generated title keeps its
  opening prompt, cut to one line. Hooks stay fast.

## Config

```toml
[recap]
overnight = ["22:00", "06:00"]
morning = ["06:00", "12:00"]
afternoon = ["12:00", "17:00"]
evening = ["17:00", "22:00"]
week_starts_on = "monday"
rows_per_section = 8
retain = "30d"
post_window_recap = true
replay_card = true
minimum_events = 8
review_notes_glob = ""
pregenerate = []

[recap.summarizer]
type = "custom"
command = []
model = ""
deadline = "4m"
transcripts = false
transcript_bytes_per_session = 8192
transcript_bytes_total = 65536
prompt = ""
prompt_file = ""

[recap.sources]
pull_requests = ["gh", "pr", "list", "--search", "updated:>={since}", "--json", "number,title,state", "--jq", "..."]
commits = []
tasks = ["dam", "ls", "due:today"]
applies = ["bash", "-c", "..."]
```

Every key ships uncommented at its default in the generated template, the way the rest of the pns
config does. Times are `HH:MM`. `retain` and `[recap.summarizer] deadline` (today's `summarizer_deadline`, moved under the
summarizer's own table) are duration strings through the domain's one parser. Unknown keys under
`[recap]`, `[recap.summarizer]` and `[recap.sources]` are refused by name.

The shipped values file for this machine points `tasks` at `dam` and `applies` at the chezmoi apply
log and the ledger, the same sources morning read.

## Errors and exit codes

Refusals exit 2 and name the fix, as every pns refusal does: an unknown window name (the accepted names
listed), two span flags together, `--previous` without a window, a mask key that names nothing, a
destination that is not configured, a source command that is not an argv list. A source command that
fails at run time does not fail the recap; its section says so. A store that cannot be opened is a
refusal, because a recap with no agents section is not a recap.

## Testing

Every test finishes under one second and reaches nothing outside the process. Window arithmetic is
pure and tested at every boundary, including a window that crosses midnight, the two week starts, and
`--previous` at each edge. The activity store is tested through the same SQLite path the ledger uses.
Source commands are tested with a fake on a private `PATH` that echoes its arguments, which pins the
placeholder substitution. Rendering is snapshot-tested plain and styled. The document is pinned by a
golden fixture per schema version, and the mask by a fixture pair. Delivery is tested with the
existing fake destinations. The return-moment fold is tested by the dispatch tests that pin the card
today, re-pointed at the engine.

## Removing morning

One change removes `morning/`, `.chezmoiscripts/run_onchange_after_55-build-morning.sh.tmpl`,
`dot_config/morning/`, the `morning` line in `.chezmoiignore`, the four `morning` lines of the
justfile's Rust gate, and the CLAUDE.md paragraph that describes it. The repository builds no removal
mechanism, so the deployed `~/.cargo/bin/morning` and `~/.config/morning/` stay until the operator
trashes them, and the pull request body carries the two `trash` commands.

## Ladder slices

These are appended to the pns refactor ladder after slice 49 and run in this order, one pull request
each. Slice 52 may land before 51 if the flag change is contested.

| Slice | Change |
| --- | --- |
| 50 | `pns gateway` absorbs `pns daemon`; `[gateway]` replaces `[daemon]` |
| 51 | `--since` and `--until` take dates, date-times and durations ago on `pns recap` |
| 52 | the activity table, the enriched hook record, `[recap] retain`, pruning on the gateway tick |
| 53 | the recap engine: windows, sections, sources, `--section`, `--json`, `--toon`, `--schema`, `--to`, the return-card fold, `repos` retired |
| 54 | morning removed |
| 55 | the summarizer: `[recap.summarizer]`, the known types and `custom`, the `summary` section, `--summarize`, `--with-transcripts`, `pregenerate` on the gateway tick, the doctor row, the prompt override |

## Out of scope

A hermes hook into pns, so hermes-agent sessions appear in the store. Reading the model on every
event by any means other than the payload or the transcript. A template language for output. Any
change to `pns recap agent --stdin` or `pns recap git`.

## To verify while writing slices 52 and 55

VERIFIED while writing slice 52 (2026-09-20), against this machine's own Claude Code session
directory and a Codex rollout file, read-only. A Claude Code transcript states each title on a line of
its own: `{"type": "custom-title", "customTitle": ...}` for the name the operator gave the session and
`{"type": "ai-title", "aiTitle": ...}` for the harness's own generated one, so the two are separate
records rather than two readings of one field, and the hook payload's `session_title` is a third
value pns keeps reading as before. The store takes the newest `customTitle` of the tail, then the
newest `aiTitle`, then the title the sessions row already holds, which is the harness title or the
first prompt. A Codex rollout file (`~/.codex/sessions`, `session_meta`, `event_msg`, `response_item`,
`turn_context`, `token_usage_record` and `world_state` lines) carries neither title field, which is
why a Codex session shows no title. The model comes off the newest assistant line's `message.model`,
which is present on every one of them.

While writing slice 55: whether hermes reads a piped prompt in quiet mode or only `-q`, the exact
shape of the session line `-Q` appends so it can be stripped, and which toolset value runs a query with
no tools; and `claude -p` and `codex exec`'s current flags for a plain-text answer, pinned by the golden
test and probed by `pns doctor`.
