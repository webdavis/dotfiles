# `pns resume`: the "where was I" answer

One subcommand that answers the question an operator asks when they come back to a machine they left
mid-flight: which herdr workspace they were in, which agent pane is waiting on them, which branch and
checkout that pane is on, and the last command the shell timed. Every answer is read from state pns
already keeps plus one herdr listing; nothing new is written, nothing is polled, and no config key
belongs to this command.

It is a MODE in `command_recap.rs`'s sense: it takes no decision from any event, and the only delivery it
can make is one the operator asked for by typing `--notify`.

## The three forms

| Invocation           | What it does                                                                      |
| -------------------- | --------------------------------------------------------------------------------- |
| `pns resume`         | prints the page in the house style (`style::header`, `◆ Title ──` sections, no box) |
| `pns resume --json`  | writes the same answers as one `pns.resume/1` object, one field per printed fact   |
| `pns resume --notify` | submits the same page through the ordinary producer path                          |

One flag at a time. `--json --notify` and any other tail earn the usage text on stderr and exit 2, which
is `pns mute`'s code: this command is hand-run, or run by an automation the operator wrote, and a flag
swallowed silently is a page they believe was sent. `pns resume --help` and `-h` print the usage and exit
0, off the one `SUBCOMMAND_USAGE` table every subcommand's help is answered from.

## Where each answer comes from

| Field       | Source                                                                                                            | When it is not known                        |
| ----------- | ----------------------------------------------------------------------------------------------------------------- | ------------------------------------------- |
| `workspace` | `herdr workspace list`, the `label` of the one workspace flagged `focused` (`pns_adapters::parse_workspaces`)      | "herdr named no focused workspace"          |
| `waiting`   | whether any `sessions` row still carries a `blocked_since` (`SqliteStore::newest_wait`)                           | `false`, and the page says nothing is waiting |
| `title`     | that row's `title`, which the session's first prompt wrote                                                         | "not known"                                 |
| `branch`    | that row's `branch`                                                                                                | "not known"                                 |
| `worktree`  | the listed workspace whose `worktree.checkout_path` directory name matches the branch slug, else the focused workspace's checkout, else the row's `project` | "not known"        |
| `command`   | the newest `ledger_events` row whose `agent` is `shell`, its `detail` (`SqliteStore::newest_shell_command`)         | "no command has been timed yet"             |

The waiting pane is the session with the NEWEST `blocked_since`, where the stale-block escalation takes
the oldest: the escalation pages about the wait that has gone unanswered longest, and this page answers
the one the operator walked away from. Escalated blocks are included, because a page already sent about a
block does not make the block answered.

Both store reads open the database READ ONLY and read every failure as "not known", the rule
`failing_legs` already states: a report must not create, import or migrate a database, and a store it
cannot open must not stop the rest of the page printing. The herdr listing is one `SystemCommandRunner`
spawn, bounded by `PROBE_DEADLINE` and `PROBE_READ_MAX` like every other probe, and an answer that will
not parse is no workspaces at all.

## What `--notify` sends

One `pns_protocol::Request` built in this process and read back through `event_flow::submit_encoded`, the
GitHub poll's own path, so the ledger, the policy and the dispatch are the identical ones every producer
reaches. The engine's existing presence gate is what decides banner-only or banner-and-phone; this
command adds no delivery rule.

- `producer` is `pns`: the state it reports is pns's own.
- `state` is `observation`. Nothing is waiting on this page, so it changes no workflow state and arms no
  reminder.
- `detail` is the page rendered with `Paint::Plain`, so the recorded message is what the terminal would
  have printed with no escape sequence in it. No `branch` is set, because a branch on the envelope
  prefixes the recorded message with it and the page names the branch in its own body.
- `request_id` is `resume-<epoch second>`, so two runs are two events rather than the second being
  answered with the first one's ledger record.

## What is out of scope

The unlock automation. A LaunchAgent or a Shortcut that calls `pns resume --notify` is the whole of it,
and the subcommand is the API it uses.
