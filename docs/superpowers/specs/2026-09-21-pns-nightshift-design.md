# Nightshift design

Date: 2026-09-21. Status: the design is ledger task 133 (`docs/remaining-work.md`, line 5343), approved
2026-09-17, restated with every gap it left filled. A gap the agent decided on the operator's behalf is
marked `GAP DECISION` and stands until the operator changes it; all of them are collected again under
`Decisions made in the operator's place`.

Implementation plan: `docs/superpowers/plans/2026-09-21-pns-nightshift-plan.md`.

## Purpose

At bedtime one command hands the night over. It reads the ledger, composes the overnight goal from
every open task that is unblocked, not operator-owned and not in an excluded section, attaches the
standing rules, writes the goal to a file, launches it, and selects the `night` profile until morning.
The morning is `pns recap overnight`, which reads the window the night ran in.

The thing it removes is the twenty minutes of typing that stood between "I am going to bed" and a run
starting: on 2026-09-17 the operator wrote that prompt by hand, and its exclusions, its standing rules
and its task list were all already written down somewhere else. Nightshift retypes none of them.

## Rulings in force

| Ruling | What it binds here |
| --- | --- |
| pns ships no bash (2026-09-20) | Nightshift is a pns verb in Rust, in its own module, never a `libexec` script. |
| The morning is `pns recap`, not morning (2026-09-20) | Nightshift schedules no morning report and writes no morning file. It ends at the handoff. |
| Defaults visible in config (2026-08-31) | Every `[nightshift]` key ships uncommented at its default. |
| Exclusions and rulings are config (task 133) | No exclusion, section name or standing rule is compiled into pns or retyped in a prompt. |
| Profiles: the override is `pns profile`'s (task 139, slice 2) | Nightshift selects `night` through that verb's own code path. It has no switch of its own. |
| Clean code, Rust bindings | Domain logic pure in `pns-domain`, the ledger read and the launch behind ports, files 300 lines ideal and 500 hard cap with tests included. |
| No removal mechanisms | Nothing here deletes a file, a goal or a worktree. |
| Tests only for behavior we wrote | The ledger reader, the composer and the command's exit codes. Not gnhf, not the harness, not the ledger. |

## The verb

```
pns nightshift              compose the goal, launch it, and select the night profile
pns nightshift --dry-run    print the goal and the plan, change nothing
```

Two forms and nothing else. Any other word or flag is the usage and exit 2.

GAP DECISION (no `status` and no `stop`). The obvious third and fourth verbs were cut because both
questions are already answered. What profile is standing and what chose it is `pns profile`; what the
night did is `pns recap overnight`; what the launcher printed is its own log file. Stopping is the
operator interrupting the run they are watching, and the profile ends on its own bound. A verb whose
whole body would be a second reading of another verb's state is a second thing to keep true.

GAP DECISION (`--dry-run` rather than an opt-in launch). The task says one command does the whole
thing, so the bare word does the whole thing. The dry run is the flag, which is the direction that
cannot surprise: an operator who typed the wrong word gets a printed page, never a silent night.

## What it does, in order

1. Read the config. An empty `[nightshift] ledger` is the table being unset, which is refused with a
   sentence naming the key, exit 2.
2. Read the ledger file. Unreadable is exit 1.
3. Parse it into entries and select the ones that survive every filter.
4. Read the standing rules file, when one is named. Unreadable is exit 1.
5. Compose the goal text.
6. `--dry-run` stops here, prints the goal and one plan line, exit 0.
7. No task survived: print one line saying so, launch nothing, select no profile, exit 0.
8. Write the goal file. Unwritable is exit 1.
9. Launch, when `[nightshift] launch` names an argv. A spawn that fails is exit 1, and the profile is
   NOT selected.
10. Select `[nightshift] profile` until `[nightshift] until`, through `pns profile`'s own code path.
    A write that fails is exit 1.
11. Print the handoff line. Exit 0.

THE PROFILE IS SELECTED LAST, after the launch, and this is the one ordering decision in the command.
A handoff that did not start must not leave the machine quiet: the operator is asleep either way, and
the failure mode of selecting first is a silenced machine with nothing running on it, which is the one
state nobody notices until morning.

THE GUARANTEE STOPS AT A FAILED EXEC. Launching is `Command::spawn` on the argv `launch` names, and a
spawn succeeds the moment the program execs, whatever it does after that. For the shipped config that
program is `bash`, and exec of `/bin/bash` succeeds whether the script it runs finds its worktree,
leaves the tree clean, or gnhf accepts the goal at all: a missing worktree, a dirty tree, or gnhf's own
refusal (`docs/runbooks/local-agents.md`) all report a launched pid and still select `night`, which is
exactly the silenced-machine state this ordering exists to prevent. The launcher's own log is the only
place a handoff that started and then died shows up; the operator's `--dry-run` acceptance run at
bedtime is what this design relies on to catch a launcher misconfigured this way before the first
unattended night, not a check this command makes.

## The composed goal

One file, and its exact shape. Paragraphs are single lines, unwrapped, because the goal is a prompt
rather than a document and a hard wrap inside it is only something a reader can get wrong.

```text
Overnight run, {date} to {next_date}. I am asleep. Complete as many of the tasks below as possible, unattended, in the order given. The ledger is `{ledger}`; every task below names its section and its line. Read the full entry before starting it.

OUT OF SCOPE, do not touch: `{excluded section}`, `{excluded section}`.

Standing rules, all in force tonight:

{rules file, verbatim}

TASKS, in ledger order:

- {number}. {title} (`{section}`, line {line})
- {number}. {title} (`{section}`, line {line})

A task that turns out to need me is not skipped: do everything up to that point, write exactly what I owe into its ledger entry, and move on. The morning report is `pns recap overnight`.
```

Rules the composer holds, each pinned by a test:

- The `OUT OF SCOPE` paragraph is omitted entirely when no section is excluded, rather than written
  with an empty list.
- The `Standing rules` heading and the rules text are omitted together when no rules file is named.
- The rules text is inserted verbatim, with its trailing blank lines trimmed to one. pns never
  reformats it: it is the operator's own prose and the file is the single place it lives.
- Sections are named in the order `exclude_sections` lists them, each in backticks.
- A task with no number is written `- {title} (`{section}`, line {line})`, because a ledger line may
  carry a sentence rather than a number.
- Tasks are in ledger order, which is file order, never sorted or regrouped.

### The first fixture

The 2026-09-17 prompt, written by hand at `~/.claude/goals/2026-09-17-overnight.md`, is the shape this
reproduces, and the golden fixture is cut from it. It is not in this repository (it is a goal file in
the operator's harness directory), so the plan copies the paragraphs it pins into
`pns/crates/pns-domain/src/nightshift/fixtures/` and the live file is never read by a test.

What the hand-written prompt had that the composed goal keeps: the date span and the "I am asleep"
opening, the ledger's path, the out-of-scope list, the standing rules as one block, the task list in
priority order with each task's section and line, and the closing rule about a task that turns out to
need the operator. What it had that the composed goal drops: the five hand-numbered PRIORITY bands,
which were the operator sorting the ledger by hand on the night, its closing OPERATOR-OWNED paragraph,
which is `exclude_tasks` and the `**user:**` marker now, and the morning report instruction,
which is `pns recap overnight` now (ruled 2026-09-20).

GAP DECISION (ledger order replaces the priority bands). The task says "in ledger order", and the
ledger is already ordered by the operator. Reproducing the bands would need a second ordering written
somewhere, which is exactly the retyping this task exists to end.

## Reading the ledger

Pure, in the domain, with the file handed in as text.

```rust
pub struct Entry {
    pub number: Option<u32>,
    pub title: String,
    pub section: String,
    pub line: usize,
    pub blocked: Option<String>,
    pub operator: Option<String>,
}

pub fn parse_ledger(text: &str) -> Vec<Entry>;
pub fn select<'a>(entries: &'a [Entry], excluded: &Excluded) -> Vec<&'a Entry>;
```

The grammar it reads, which is the ledger's own and nothing more:

| Shape | Meaning |
| --- | --- |
| `## Heading` or `### Heading` | The section every entry below it belongs to, until the next heading. |
| `- [ ] 133. Nightshift, ...` | An open entry, numbered. `133` is the number, the rest of the line is the title. |
| `- [ ] Operator acceptance: ...` | An open entry with no number. The whole line after the box is the title. |
| `- [x] ...` | A closed entry. Skipped, and it ends the entry above it. |
| An indented line under an open entry | That entry's body, until the next entry or heading. |
| `**blocked:** <why>` in a body | The entry is blocked. `<why>` is kept for the refusal that never prints it. |
| `**user:** <what>` in a body | The entry is the operator's. |

A title is the task line with the box and the number stripped, trimmed, and cut at the first sentence
end (`. ` or the end of the line), so a task's line in the goal is one line whatever its entry's first
paragraph does. The line number is the task line's own, one-based, which is what makes the goal's
`line 5343` land where the operator looks.

GAP DECISION (the two markers do not exist yet). `docs/remaining-work.md` carries no `**blocked:**` and
no `**user:**` line today, and the reader therefore reads every open entry as unblocked and unowned. The
markers are an additive convention Nightshift introduces: the operator writes one into an entry and that
entry stops being composed into a goal. Until then `exclude_sections` and `exclude_tasks` carry the same
weight, which is why both keys exist and why `Waiting on the operator` ships in the excluded list. The
direction is deliberate: an unmarked entry is composed, so a marker the operator forgot costs a task an
agent works on and not a task that silently disappears.

### Selection

An entry is composed into the goal when every one of these holds:

1. It is open (`- [ ]`).
2. Its `blocked` is `None`.
3. Its `operator` is `None`.
4. Its section is not in `exclude_sections`, compared on the heading text after the hashes, trimmed,
   case-sensitively and in full.
5. Its number, when it has one, is not in `exclude_tasks`.

Order is file order throughout. Nothing is deduplicated: two entries carrying one number is the ledger
saying so, and the goal says it too.

## Config

`[nightshift]`, every key visible at its default, in the shipped template.

```toml
# The bedtime handoff. `pns nightshift` reads the ledger below, composes the
# night's goal from every open task that is unblocked, not yours and not in an
# excluded section, launches it and selects the `night` profile until morning.
# The morning is `pns recap overnight`. THE TABLE IS OFF BY BEING UNSET: an
# empty `ledger` is refused by name rather than guessed at.
[nightshift]
ledger = ""
rules_file = ""
goal_file = ""
exclude_sections = []
exclude_tasks = []
profile = "night"
until = "06:00"
launch = []
log = ""
```

| Key | Default | What it decides |
| --- | --- | --- |
| `ledger` | `""` | The ledger file's path. Unset is the table being off, and running the verb then exits 2 naming this key. |
| `rules_file` | `""` | A file whose text is the standing rules, inserted verbatim. Unset omits the block. |
| `goal_file` | `""` | Where the composed goal is written. Unset is `<state dir>/nightshift/<date>-overnight.md`. |
| `exclude_sections` | `[]` | Section headings whose entries are never composed, named as the goal names them. |
| `exclude_tasks` | `[]` | Task numbers never composed, for an entry the operator owns and has not marked. |
| `profile` | `"night"` | The profile selected at handoff. It must be one `[profiles.<name>]` defines. |
| `until` | `"06:00"` | The local `HH:MM` the profile is selected until, resolved to the next occurrence. |
| `launch` | `[]` | The argv started detached. Empty writes the goal file and starts nothing. |
| `log` | `""` | Where the launched process's output goes. Unset is the goal file's path with `.log` for `.md`. |

`{goal}` is the one placeholder `launch` accepts, replaced with the goal file's absolute path. An
element holding an unknown `{word}` is a load error naming it, the way a recap source's is, so a typo
does not reach the launcher as a literal.

Paths starting `~/` expand against the home directory. `until` is parsed by the same `HH:MM` reader
`pns profile --until` uses, and a malformed one is a load error.

The values this machine ships, in `dot_config/pns/config-values.toml`:

```toml
[nightshift]
ledger = "~/workspaces/Ivy/webdavis/dotfiles/docs/remaining-work.md"
rules_file = "~/workspaces/Ivy/webdavis/dotfiles/docs/gnhf-objective.md"
exclude_sections = [
  "SP8, macOS agent workflow manager",
  "After modernization: Forzare and PR #51",
  "Waiting on the operator",
  "After the ledger: approved ideas awaiting design",
  "Open questions",
]
launch = [
  "bash",
  "-c",
  'cd "$HOME/.herdr/worktrees/dotfiles/nightshift" && GRAPHIFY_SKIP_HOOK=1 gnhf --current-branch < "$1"',
  "_",
  "{goal}",
]
```

GAP DECISION (which sections ship excluded). The task says the exclusions are config and names none.
Two of these five are headings the 2026-09-17 prompt excluded by name, SP8 and
`After the ledger: approved ideas awaiting design`. Forzare is the section the brief names, and the
other two are not work at all. The prompt excluded two further headings that no longer exist under
those names, and the rest of its exclusions that night were subjects rather than sections (no homelab
task, whatever its heading); a subject filter is a second grammar to write and to get wrong, so the
operator adds a heading or a task number instead.

GAP DECISION (the standing rules file is the gnhf objective). `docs/gnhf-objective.md` already carries
this repository's standing constraints and already travels with every worktree, which is the runbook's
own arrangement. Pointing `rules_file` at it means one file holds the rules, whoever reads them.

## Launching

Two modes, chosen by whether `launch` names an argv.

**gnhf's loop.** `launch` names it. The argv is started DETACHED: its own session through `setsid`, its
stdin closed, its stdout and stderr appended to `log`. Nightshift prints the child's process id and
returns; it never waits, never reads the child's output and never reports on the run. The runbook's own
constraints are the operator's to keep in that argv: the worktree to run in, `--current-branch`,
`GRAPHIFY_SKIP_HOOK=1` and a clean tree. pns validates none of them, because they are gnhf's and a
second copy of them here is a second copy to get wrong.

DETACHED IS LOAD-BEARING. The command is typed in a terminal at bedtime and the terminal is closed
minutes later. A child in the caller's process group takes the hangup with it, and the night ends
before it started.

**The harness's own goal.** `launch` is empty. Nightshift writes the goal file and prints its path, and
the operator hands that path to the harness. This is what makes `goal_file` worth setting: pointed at
the harness's goal directory, the file lands where the harness already looks.

GAP DECISION (pns does not run the harness itself). The task says gnhf's loop "or the harness's own
goal, chosen by config". A harness goal is a file the operator or the harness picks up, not a process
pns starts, so the second mode is the file and nothing more. An argv that starts a harness is still
available through `launch` for an operator who wants one.

## The handoff to the night profile

Nightshift calls the same function `pns profile <name> --until <HH:MM>` calls, with the same bounds,
the same store row and the same read-back. It writes no row of its own and knows nothing about how an
override is stored. A `[nightshift] profile` naming no `[profiles.<name>]` table is refused when the
config loads, the same cross-table check that refuses an undefined name in `[[profiles.rules]]`, so a
typo never reaches the launch step at all.

It does not clear the override in the morning and nothing schedules a clear: the `until` bound is what
ends it, which is the same reason `pns profile --until` exists. Under `night` the events the run raises
are held, and the roll-up the profiles design specifies is what the morning's recap reads.

## The morning

Nothing. `pns recap overnight` is the morning report, its window is `[recap] overnight`, and its
sections already cover what the night's sessions did. Nightshift writes no file the morning reads and
holds no state the morning needs.

## Errors and exit codes

| Situation | Message | Exit |
| --- | --- | --- |
| A word or flag that is not `--dry-run` | the usage | 2 |
| `[nightshift] ledger` is empty | ``pns nightshift: set `[nightshift] ledger` to your ledger's path; the handoff has nothing to read`` | 2 |
| A `launch` element holds an unknown placeholder | ``unknown `nightshift.launch` placeholder `{tasks}`; the one placeholder is {goal}`` | load error |
| A malformed `until` | ``nightshift.until "6am" is not an HH:MM time`` | load error |
| `[nightshift] profile` names no profile | ``[nightshift] profile` names `meeting`, which no `[profiles.<name>]` table defines; this config defines default, night, work`` | load error |
| The ledger could not be read | ``pns: state error (the ledger at <path> could not be read: <error>); nothing was handed off`` | 1 |
| The rules file could not be read | ``pns: state error (the standing rules at <path> could not be read: <error>); nothing was handed off`` | 1 |
| The goal file could not be written | ``pns: state error (the goal at <path> could not be written: <error>); nothing was handed off`` | 1 |
| The launcher did not start | ``pns: state error (<program> did not start: <error>); the goal is at <path> and the profile was not changed`` | 1 |
| The override could not be written | ``pns: state error (the profile override could not be written: <error>); the run started and the machine is still loud`` | 1 |
| No task survived the filters | ``pns nightshift: every open task is blocked, yours or excluded; nothing was handed off`` | 0 |

Every state error says what did and did not happen, because the operator reading one is deciding
whether to type the rest by hand before bed.

## What it prints

Handed off, with a launcher:

```
pns: nightshift handed off, 23 tasks, 4 sections excluded
     goal   ~/.local/state/pns/nightshift/2026-09-21-overnight.md
     launch bash (pid 40117), log ~/.local/state/pns/nightshift/2026-09-21-overnight.log
     profile `night` until 06:00
```

Handed off, with no launcher:

```
pns: nightshift composed, 23 tasks, 4 sections excluded
     goal   ~/.claude/goals/2026-09-21-overnight.md
     profile `night` until 06:00
```

`--dry-run` prints the same first line with `would hand off`, then the composed goal in full, and
changes nothing.

The profile line is READ BACK from the store rather than rendered from what the run intended, which is
`pns mute`'s own rule and `pns profile`'s: the line cannot claim a profile that never landed.

## Testing

Every test is pure or reaches a temporary directory, and every one finishes well under a second.

- **The ledger reader**, against golden fixtures in `pns/crates/pns-domain/src/nightshift/fixtures/`,
  cut from `docs/remaining-work.md` excerpts and committed. THE LIVE LEDGER IS NEVER READ BY A TEST:
  it changes every day, and a test that reads it fails on work nobody did. The fixture pins a `##`
  section, a `###` section, a numbered open entry, an unnumbered open entry, a closed entry, a body
  carrying `**blocked:**`, a body carrying `**user:**`, and an entry whose first paragraph runs over
  several lines.
- **Selection**: each of the five filters drops what it should and keeps what it should; file order
  survives; a number listed in `exclude_tasks` goes and its neighbours stay.
- **The composer**, against a golden goal fixture: the full shape, the omitted out-of-scope paragraph,
  the omitted rules block, an unnumbered task's line, and the verbatim rules text.
- **The config**: every key parses, every refusal above fires by message, and an unknown key under
  `[nightshift]` is refused by name.
- **The render**: `dot_config/pns/private_config.toml.tmpl` regenerates byte-for-byte from
  `dot_config/pns/config-values.toml`, which is the existing byte-equality test in `just test-rust`.
- **The command**: each exit code above, with a scripted spawner and a temporary state directory. The
  spawner fake records the argv it was handed, which is what pins the `{goal}` substitution, and a
  failing fake pins that the profile is not selected when the launch did not start.

The operator's acceptance, and the one thing no test stands in for: run `pns nightshift --dry-run` at
a bedtime, read the goal it composed against what they would have typed, and only then run the bare
word.

## Decisions made in the operator's place

Every one of these was the agent's, is marked `GAP DECISION` above, and stands until the operator
changes it.

1. The verb is `pns nightshift` with two forms, `--dry-run` and the bare word, and no `status` or
   `stop` verb.
2. The bare word does the whole handoff; the dry run is the flag.
3. The profile is selected LAST, after a successful launch, so a failed handoff leaves the machine
   loud.
4. Ledger order replaces the hand-written prompt's five priority bands.
5. `**blocked:**` and `**user:**` are an additive ledger convention that does not exist in the file
   today, and an unmarked entry is composed.
6. The five sections that ship in `exclude_sections`, and no subject filter beside them.
7. `rules_file` ships pointed at `docs/gnhf-objective.md`.
8. The harness-goal mode is the goal file alone; pns starts no harness of its own.
9. `[nightshift]` is off by being unset, through an empty `ledger`, rather than through an `enabled`
   key of its own.
10. The launch is detached with its own session, and pns validates none of gnhf's own constraints.
11. Nightshift records no event and writes no table: the morning reads the activity the harness hooks
    recorded anyway.

## Files this design touches

- `pns/crates/pns-domain/src/nightshift.rs` and `nightshift/{ledger.rs,compose.rs,report.rs}` (new),
  plus `nightshift/fixtures/`: the reader, the selection, the goal and the printed lines, all pure.
- `pns/crates/pns-domain/src/recap/window.rs`: one visibility word, so the goal's two dates step a day
  rather than subtracting 86,400 seconds.
- `pns/crates/pns-application/src/ports/process.rs`: the detached spawner port beside `CommandRunner`.
- `pns/crates/pns-adapters/src/process/detached.rs` (new): the one implementation.
- `pns/crates/pns-adapters/src/config/nightshift.rs` (new) and `config/{mod,model,load,schema}.rs`:
  the table and its refusals.
- `pns/crates/pns-adapters/src/config/render/layout/nightshift.rs` (new) and `render/layout.rs`: the
  shipped text.
- `pns/crates/pns/src/command_nightshift.rs` (new), `invocation.rs`, `subcommand_usage.rs`: the verb.
- `dot_config/pns/config-values.toml` and the regenerated `dot_config/pns/private_config.toml.tmpl`.

## Relations to other work

| Task | Relation |
| --- | --- |
| 139, pns profiles | Slice 2 is the dependency: Nightshift calls its override setter. Nothing here lands before it. |
| 125 and slices 53 to 55, the recap | The morning. Nightshift adds nothing to it and reads nothing from it. |
| 126, the calendar input | Unrelated. A `calendar_busy` rule may select `night` on its own; Nightshift selects it by hand either way. |
| gnhf, `docs/runbooks/local-agents.md` | The launcher this ships pointed at, and the source of the standing rules file. |
| SP8, the macOS agent workflow manager | The task says to fold Nightshift there if SP8 turns out to be this. It is not: SP8 is out of the goal, and this is one verb and one config table. |
