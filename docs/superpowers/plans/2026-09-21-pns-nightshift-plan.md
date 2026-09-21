# Nightshift Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended)
> or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`)
> syntax for tracking.

**Goal:** One command at bedtime composes the overnight goal from the ledger, writes it, launches it
detached, and selects the `night` profile until morning, retyping no exclusion and no standing rule.

**Architecture:** The ledger reader, the selection and the goal composer are pure functions in
`pns-domain` with the file handed in as text, so the whole grammar is pinned by golden fixtures with no
filesystem. The config layer parses `[nightshift]` and refuses each malformed shape by name. The
command module reads the config, calls the domain, writes one file, starts one detached child through a
port, and hands the profile selection to `pns profile`'s own code path.

**Tech Stack:** Rust 2024, the `pns` workspace, `toml` for config, `libc` for `setsid`. Gates:
`just test-rust` and `just lint-check`.

**Spec:** `docs/superpowers/specs/2026-09-21-pns-nightshift-design.md`

## Global Constraints

- The verb has two forms: bare `pns nightshift` and `pns nightshift --dry-run`. Anything else is the
  usage and exit 2.
- The nine `[nightshift]` keys are `ledger`, `rules_file`, `goal_file`, `exclude_sections`,
  `exclude_tasks`, `profile`, `until`, `launch` and `log`. No tenth, and no `enabled`: the table is off
  by an empty `ledger`.
- The order of operations is compose, write, launch, select the profile, print. THE PROFILE IS
  SELECTED LAST: a handoff that did not start must leave the machine loud.
- The profile override is written by `pns profile`'s own setter. This task writes no store row and
  knows nothing about how an override is stored.
- Nightshift records no event, writes no table and schedules nothing. The morning is
  `pns recap overnight`.
- The live `docs/remaining-work.md` is NEVER read by a test. Every ledger test reads a committed
  fixture under `pns/crates/pns-domain/src/nightshift/fixtures/`.
- `{goal}` is the one placeholder `launch` accepts. Any other `{word}` in an element is a load error.
- Exit codes: 0 for a handoff, a dry run or a night with nothing to do; 1 for a state error; 2 for argv
  the operator typed wrong, an unset `ledger` or a profile the config does not define.
- Every `.rs` file stays at 300 lines ideal and 500 hard cap, unit tests included.
- `dot_config/pns/private_config.toml.tmpl` is GENERATED. Never hand-edit it: run
  `just pns-config-render`, which `just test-rust` byte-compares.
- No em-dashes in any comment, message or document this plan writes.
- Every slice ends with `just test-rust` and `just lint-check` green.

---

## File Structure

New, in `pns/crates/pns-domain/src/`:

| File | Responsibility |
| --- | --- |
| `nightshift.rs` | The module doc and the re-exports. |
| `nightshift/ledger.rs` | The ledger grammar: entries, their sections, their markers, and the selection. |
| `nightshift/compose.rs` | The composed goal's exact text. |
| `nightshift/report.rs` | The lines `pns nightshift` prints. |
| `nightshift/fixtures/ledger.md` | The committed ledger excerpt every reader test reads. |
| `nightshift/fixtures/goal.md` | The composed goal the composer test compares against. |

New, in `pns/crates/pns-adapters/src/`:

| File | Responsibility |
| --- | --- |
| `config/nightshift.rs` | Parsing `[nightshift]` and every refusal. |
| `config/render/layout/nightshift.rs` | The shipped prose and key list. |
| `process/detached.rs` | The one detached spawn, in its own session. |

New, in `pns/crates/pns/src/`:

| File | Responsibility |
| --- | --- |
| `command_nightshift.rs` | The verb: the two forms, the order of operations and the exit codes. |

Modified: `pns-domain/src/lib.rs`; `pns-application/src/ports/process.rs`, `lib.rs`;
`pns-adapters/src/config/{mod,model,load}.rs`, `config/render/layout.rs`, `pns-adapters/src/lib.rs`;
`pns/src/{lib,invocation,subcommand_usage}.rs`; `dot_config/pns/config-values.toml` and the regenerated
`dot_config/pns/private_config.toml.tmpl`.

---

## The slices

Three pull requests. Each is independently mergeable, each ends both gates green, and each leaves the
binary working.

SLICE 1: the ledger reader and the composed goal
plan-items: the ledger grammar, the two markers, the selection, the goal's exact text, the two golden
fixtures.
why-this-order: first, and it is the whole risk of this task. Everything later is plumbing around these
three pure functions, and they are the half that decides what an unattended agent works on all night.
files: `pns/crates/pns-domain/src/nightshift.rs`,
`pns/crates/pns-domain/src/nightshift/{ledger.rs,compose.rs,report.rs}` (each with its `tests.rs`),
`pns/crates/pns-domain/src/nightshift/fixtures/{ledger.md,goal.md}`,
`pns/crates/pns-domain/src/lib.rs`, `pns/crates/pns-domain/src/recap/window.rs` (one visibility word)
callers-to-update-in-the-same-slice: none. Nothing calls the module yet.
behaviour-to-pin: a `##` and a `###` heading each set the section; a numbered and an unnumbered open
entry parse; a closed entry is skipped; a body carrying `**blocked:**` or `**user:**` marks its entry;
an entry's body survives blank lines and sub-bullets; each of the five filters drops what it should;
file order survives; the goal matches its fixture, with and without the two optional paragraphs; the
two dates are tonight and the morning, across a month end.
risk: low in the code and high in the consequence. Every behaviour is pure and fixture-pinned, which is
the whole guard.
size: medium

SLICE 2: the `[nightshift]` config table
plan-items: the nine keys, the placeholder check, the `HH:MM` check, the shipped values, the rendered
template.
why-this-order: after slice 1, whose `Excluded` the two exclusion keys fill. Before slice 3, which reads
the parsed table.
files: `pns/crates/pns-adapters/src/config/nightshift.rs` (and its `tests.rs`),
`pns/crates/pns-adapters/src/config/{mod,model,load}.rs`,
`pns/crates/pns-adapters/src/config/render/layout/nightshift.rs`,
`pns/crates/pns-adapters/src/config/render/layout.rs`, `dot_config/pns/config-values.toml`,
`dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: none. Nothing reads the new fields yet.
behaviour-to-pin: the nine keys parse; an unknown key is refused by name; an unknown placeholder, a
malformed `until` and a `profile` no `[profiles.<name>]` defines are each refused by their own
message; the template regenerates byte-for-byte.
risk: low. The config gains a table and the binary's behaviour does not change.
size: small

SLICE 3: the verb, the detached launch and the handoff
plan-items: the spawner port and its one adapter, the command's order of operations, the exit codes,
the printed lines, the profile selection.
why-this-order: last. It is the only slice that starts a process or changes a setting, and both halves
it composes exist by then.
files: `pns/crates/pns-application/src/ports/process.rs`,
`pns/crates/pns-adapters/src/process/detached.rs`, `pns/crates/pns-adapters/src/lib.rs`,
`pns/crates/pns/src/command_nightshift.rs` (and its `tests.rs`),
`pns/crates/pns/src/{invocation.rs,subcommand_usage.rs,lib.rs}`
callers-to-update-in-the-same-slice: none outside pns. `pns nightshift` is a new word.
behaviour-to-pin: an unset `ledger` exits 2; a bad flag exits 2; an unreadable ledger, an unwritable
goal and a launch that did not start each exit 1 with their own sentence; a failed launch leaves the
profile unselected; `--dry-run` prints the goal and starts nothing; a night with nothing to do exits 0
having launched nothing; the `{goal}` element reaches the spawner as the goal file's path.
risk: medium, and the highest in the ladder. It starts an unattended process and it silences a machine.
The guard is the ordering test, which is the one that proves a failed launch leaves the machine loud.
size: medium

---

## Slice 1 tasks

### Task 1: the ledger grammar

**Files:**
- Create: `pns/crates/pns-domain/src/nightshift.rs`
- Create: `pns/crates/pns-domain/src/nightshift/ledger.rs`
- Create: `pns/crates/pns-domain/src/nightshift/ledger/tests.rs`
- Create: `pns/crates/pns-domain/src/nightshift/fixtures/ledger.md`
- Modify: `pns/crates/pns-domain/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pns_domain::nightshift::{Entry, parse_ledger}`.
  `parse_ledger(text: &str) -> Vec<Entry>`.

- [ ] **Step 1: Write the failing test**

Create `pns/crates/pns-domain/src/nightshift/fixtures/ledger.md`, an excerpt cut from
`docs/remaining-work.md` and then trimmed to the shapes the grammar has to read. IT IS COMMITTED AND
NEVER REGENERATED: the live ledger changes every day.

```markdown
# Remaining work

## Daily operations tools

- [ ] 132. `pns resume`, where the operator was. The page reads the state pns already keeps.
  A second paragraph of the same entry, indented, which is not a new entry.

- [ ] 133. Nightshift, the one-word overnight handoff, paired with gnhf. At bedtime one command
  composes the overnight goal from the ledger.

  - **blocked:** profiles slice 2, which owns the override this selects the night profile through.

- [x] 125. morning, retired by the recap. Done 2026-09-20.

- [ ] 139. pns profiles. Operator ruling 2026-09-17.

## Waiting on the operator

- [ ] Operator acceptance: reload the multiplexer's configuration and restart the harnesses.

  - **user:** the reload itself, which no agent may run.

## SP8, macOS agent workflow manager

- [ ] 141. The workflow manager's first surface.
```

Create `pns/crates/pns-domain/src/nightshift/ledger/tests.rs`:

```rust
use super::*;

const LEDGER: &str = include_str!("../fixtures/ledger.md");

#[test]
fn every_open_entry_is_read_and_a_closed_one_is_not() {
    let entries = parse_ledger(LEDGER);
    let numbers: Vec<Option<u32>> = entries.iter().map(|entry| entry.number).collect();
    assert_eq!(
        numbers,
        vec![Some(132), Some(133), Some(139), None, Some(141)],
        "the closed 125 is not an entry, and the unnumbered acceptance is"
    );
}

#[test]
fn an_entry_carries_the_heading_above_it_and_its_own_line() {
    let entries = parse_ledger(LEDGER);
    assert_eq!(entries[1].number, Some(133));
    assert_eq!(entries[1].section, "Daily operations tools");
    assert_eq!(
        entries[1].title,
        "Nightshift, the one-word overnight handoff, paired with gnhf.",
        "the title is the first sentence, so one entry is one line in the goal"
    );
    assert_eq!(
        LEDGER.lines().nth(entries[1].line - 1).unwrap(),
        "- [ ] 133. Nightshift, the one-word overnight handoff, paired with gnhf. At bedtime one command",
        "the line is one-based and lands on the task line itself"
    );
    assert_eq!(entries[4].section, "SP8, macOS agent workflow manager");
}

#[test]
fn an_unnumbered_entry_keeps_its_whole_first_sentence() {
    let entries = parse_ledger(LEDGER);
    assert_eq!(entries[3].number, None);
    assert_eq!(
        entries[3].title,
        "Operator acceptance: reload the multiplexer's configuration and restart the harnesses."
    );
}

#[test]
fn the_two_markers_are_read_out_of_a_body_and_nothing_else_is() {
    let entries = parse_ledger(LEDGER);
    assert_eq!(
        entries[1].blocked.as_deref(),
        Some("profiles slice 2, which owns the override this selects the night profile through."),
        "a sub-bullet marker belongs to the entry above it"
    );
    assert_eq!(entries[1].operator, None);
    assert_eq!(entries[3].operator.as_deref(), Some("the reload itself, which no agent may run."));
    assert_eq!(entries[0].blocked, None, "an unmarked entry is unblocked and unowned");
    assert_eq!(entries[0].operator, None);
}

#[test]
fn a_body_survives_a_blank_line_and_a_heading_ends_it() {
    // 133's marker sits after a blank line, which the test above proves is
    // still its body. A heading is the hard boundary: 141 belongs to SP8 and
    // carries neither of the markers written under an entry two sections up.
    let entries = parse_ledger(LEDGER);
    assert_eq!(entries[4].number, Some(141));
    assert_eq!(entries[4].blocked, None);
    assert_eq!(entries[4].operator, None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain nightshift::`
Expected: FAIL, `file not found for module `nightshift``.

- [ ] **Step 3: Write minimal implementation**

Create `pns/crates/pns-domain/src/nightshift.rs`:

```rust
//! The bedtime handoff: the ledger read, the night's goal composed out of it,
//! and the lines the command prints about what it did.
//!
//! EVERY FUNCTION HERE IS PURE. The file is handed in as text, the date is
//! handed in as a string, and the standing rules are handed in verbatim, so
//! the whole grammar is pinned by committed fixtures and no test reads the
//! live ledger, which changes every day.

mod ledger;
pub use ledger::{Entry, parse_ledger};
```

Create `pns/crates/pns-domain/src/nightshift/ledger.rs`:

```rust
#[cfg(test)]
mod tests;

/// One open task in the ledger: what it is called, where it sits, and
/// whatever its body says about who owns it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Entry {
    /// The `133.` a ledger task line usually opens with. A line that opens
    /// with a sentence instead has none, and keeps its whole text.
    pub number: Option<u32>,
    /// The first sentence of the task line, which is its one line in the goal.
    pub title: String,
    /// The nearest `##` or `###` heading above it, without the hashes.
    pub section: String,
    /// The task line's own line number, one-based, so the goal points at the
    /// line the operator will look at.
    pub line: usize,
    /// What a `**blocked:**` marker in the body said, if there was one.
    pub blocked: Option<String>,
    /// What a `**user:**` marker in the body said, if there was one.
    pub operator: Option<String>,
}

/// An open task line.
const OPEN: &str = "- [ ] ";
/// The marker that takes an entry out of the night.
const BLOCKED: &str = "**blocked:**";
/// The marker that says the entry is the operator's own.
const OPERATOR: &str = "**user:**";

/// Every open entry in the ledger, in file order.
///
/// THE GRAMMAR IS THE LEDGER'S OWN and nothing is added to it: a heading, an
/// open box, a closed box and the indented body under an entry. An entry's
/// body runs until the next line that starts at column zero with text, so a
/// blank line and a sub-bullet both stay inside it and a marker written
/// either way is read.
pub fn parse_ledger(text: &str) -> Vec<Entry> {
    let mut section = String::new();
    let mut entries: Vec<Entry> = Vec::new();
    // Whether the newest entry is still taking body lines.
    let mut open = false;
    for (index, line) in text.lines().enumerate() {
        if let Some(heading) = heading_of(line) {
            section = heading;
            open = false;
            continue;
        }
        let body = line.starts_with(' ') || line.starts_with('\t') || line.trim().is_empty();
        if !body {
            open = false;
            if let Some(rest) = line.strip_prefix(OPEN) {
                entries.push(entry(rest, &section, index + 1));
                open = true;
            }
            continue;
        }
        if open {
            if let Some(entry) = entries.last_mut() {
                mark(entry, line.trim_start().trim_start_matches("- "));
            }
        }
    }
    entries
}

/// The text of a `##` or `###` heading, or None for any other line. Deeper
/// headings are not sections: the ledger does not use them and a section name
/// the operator cannot write in `exclude_sections` is not one.
fn heading_of(line: &str) -> Option<String> {
    let rest = line
        .strip_prefix("### ")
        .or_else(|| line.strip_prefix("## "))?;
    Some(rest.trim().to_string())
}

/// One entry out of the text after its box.
fn entry(rest: &str, section: &str, line: usize) -> Entry {
    let (number, title) = numbered(rest);
    Entry {
        number,
        title: first_sentence(title),
        section: section.to_string(),
        line,
        blocked: None,
        operator: None,
    }
}

/// A leading `133. ` split off as the task's number. A head that is not plain
/// digits is part of the sentence, which is how `71b.` keeps its whole line.
fn numbered(rest: &str) -> (Option<u32>, &str) {
    let Some((head, tail)) = rest.split_once(". ") else {
        return (None, rest);
    };
    match head.parse::<u32>() {
        Ok(number) => (Some(number), tail),
        Err(_) => (None, rest),
    }
}

/// The first sentence, so one entry is one line in the goal whatever its own
/// first paragraph does. A line with no sentence end is the whole line.
fn first_sentence(body: &str) -> String {
    let body = body.trim();
    match body.find(". ") {
        Some(end) => body[..=end].to_string(),
        None => body.to_string(),
    }
}

/// Either marker, read off one body line. Neither is ever printed by pns; it
/// is kept because a refusal that named the reason would be useful later and
/// throwing it away now cannot be undone by a caller.
fn mark(entry: &mut Entry, body: &str) {
    if let Some(why) = body.strip_prefix(BLOCKED) {
        entry.blocked = Some(why.trim().to_string());
    } else if let Some(what) = body.strip_prefix(OPERATOR) {
        entry.operator = Some(what.trim().to_string());
    }
}
```

Add to `pns/crates/pns-domain/src/lib.rs`, in the module list's alphabetical place (after `mute`,
before `profiles`):

```rust
pub mod nightshift;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain nightshift::`
Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```
SKIP_AI_COMMIT=1 git commit -m "feat(pns): read the ledger's open entries and their markers"
```

---

### Task 2: the selection

**Files:**
- Modify: `pns/crates/pns-domain/src/nightshift/ledger.rs`
- Modify: `pns/crates/pns-domain/src/nightshift/ledger/tests.rs`
- Modify: `pns/crates/pns-domain/src/nightshift.rs`

**Interfaces:**
- Consumes: `Entry`.
- Produces: `pns_domain::nightshift::{Excluded, select}`.
  `select<'a>(entries: &'a [Entry], excluded: &Excluded) -> Vec<&'a Entry>`.

- [ ] **Step 1: Write the failing test**

Append to `pns/crates/pns-domain/src/nightshift/ledger/tests.rs`:

```rust
#[test]
fn a_marked_entry_is_not_composed_and_the_rest_keep_file_order() {
    let entries = parse_ledger(LEDGER);
    let composed = select(&entries, &Excluded::default());
    let numbers: Vec<Option<u32>> = composed.iter().map(|entry| entry.number).collect();
    assert_eq!(
        numbers,
        vec![Some(132), Some(139), Some(141)],
        "133 is blocked and the acceptance is the operator's own"
    );
}

#[test]
fn an_excluded_section_takes_every_entry_under_it() {
    let entries = parse_ledger(LEDGER);
    let excluded = Excluded {
        sections: vec!["SP8, macOS agent workflow manager".to_string()],
        tasks: Vec::new(),
    };
    let composed = select(&entries, &excluded);
    assert_eq!(composed.iter().map(|entry| entry.number).collect::<Vec<_>>(), vec![Some(132), Some(139)]);
}

#[test]
fn an_excluded_number_takes_its_own_entry_and_leaves_its_neighbours() {
    let entries = parse_ledger(LEDGER);
    let excluded = Excluded { sections: Vec::new(), tasks: vec![139] };
    let composed = select(&entries, &excluded);
    assert_eq!(composed.iter().map(|entry| entry.number).collect::<Vec<_>>(), vec![Some(132), Some(141)]);
}

#[test]
fn a_section_name_matches_in_full_and_in_case() {
    let entries = parse_ledger(LEDGER);
    let near_miss = Excluded { sections: vec!["sp8".to_string()], tasks: Vec::new() };
    assert_eq!(
        select(&entries, &near_miss).len(),
        3,
        "a partial or differently cased heading excludes nothing, so a typo costs no task silently"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain nightshift::`
Expected: FAIL, `cannot find type `Excluded``.

- [ ] **Step 3: Write minimal implementation**

Append to `pns/crates/pns-domain/src/nightshift/ledger.rs`:

```rust
/// What the config says is never composed into a goal.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Excluded {
    /// Headings, written exactly as the ledger writes them after the hashes.
    pub sections: Vec<String>,
    /// Task numbers the operator owns and has not marked in the file.
    pub tasks: Vec<u32>,
}

/// The entries that go into tonight's goal, in file order.
///
/// AN UNMARKED ENTRY IS COMPOSED. The two markers do not exist in the ledger
/// today, so the direction is the one whose failure is visible: a marker the
/// operator forgot costs a task an agent works on, where the other direction
/// would drop tasks nobody noticed were gone.
pub fn select<'a>(entries: &'a [Entry], excluded: &Excluded) -> Vec<&'a Entry> {
    entries
        .iter()
        .filter(|entry| composable(entry, excluded))
        .collect()
}

/// Whether one entry survives all four filters. Openness is the fifth and is
/// already settled: a closed box never became an `Entry`.
fn composable(entry: &Entry, excluded: &Excluded) -> bool {
    entry.blocked.is_none()
        && entry.operator.is_none()
        && !excluded.sections.iter().any(|name| name == &entry.section)
        && !entry
            .number
            .is_some_and(|number| excluded.tasks.contains(&number))
}
```

Change the re-export in `pns/crates/pns-domain/src/nightshift.rs`:

```rust
pub use ledger::{Entry, Excluded, parse_ledger, select};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain nightshift::`
Expected: PASS, 9 tests.

- [ ] **Step 5: Commit**

```
SKIP_AI_COMMIT=1 git commit -m "feat(pns): select the ledger entries a night's goal is composed from"
```

---

### Task 3: the composed goal and the printed lines

**Files:**
- Create: `pns/crates/pns-domain/src/nightshift/compose.rs`
- Create: `pns/crates/pns-domain/src/nightshift/compose/tests.rs`
- Create: `pns/crates/pns-domain/src/nightshift/report.rs`
- Create: `pns/crates/pns-domain/src/nightshift/report/tests.rs`
- Create: `pns/crates/pns-domain/src/nightshift/fixtures/goal.md`
- Modify: `pns/crates/pns-domain/src/nightshift.rs`
- Modify: `pns/crates/pns-domain/src/recap/window.rs`

**Interfaces:**
- Consumes: `Entry`, `pns_domain::recap::window::LocalCivilTime`.
- Produces: `pns_domain::nightshift::{Composition, compose, dates, Handoff, headline}`.
  `compose(composition: &Composition) -> String`, `dates(today: LocalCivilTime) -> (String, String)`,
  `headline(handoff: Handoff) -> String`.

`LocalCivilTime::plus_days` is private today and this task makes it `pub`. It is the existing
day-stepping arithmetic, tested in `recap::window`'s own suite; stepping a date rather than
subtracting 86,400 seconds is what keeps the morning's date right on both sides of a zone
transition.

- [ ] **Step 1: Write the failing test**

Create `pns/crates/pns-domain/src/nightshift/fixtures/goal.md`. Its paragraphs are single long lines,
unwrapped, which is what the composer writes:

```markdown
Overnight run, 2026-09-21 to 2026-09-22. I am asleep. Complete as many of the tasks below as possible, unattended, in the order given. The ledger is `~/dotfiles/docs/remaining-work.md`; every task below names its section and its line. Read the full entry before starting it.

OUT OF SCOPE, do not touch: `SP8, macOS agent workflow manager`, `Waiting on the operator`.

Standing rules, all in force tonight:

Never run an apply. No em-dashes anywhere.

TASKS, in ledger order:

- 132. `pns resume`, where the operator was. (`Daily operations tools`, line 5)
- 139. pns profiles. (`Daily operations tools`, line 15)
- Operator acceptance: reload the multiplexer's configuration and restart the harnesses. (`Waiting on the operator`, line 19)

A task that turns out to need me is not skipped: do everything up to that point, write exactly what I owe into its ledger entry, and move on. The morning report is `pns recap overnight`.
```

Create `pns/crates/pns-domain/src/nightshift/compose/tests.rs`:

```rust
use super::*;
use crate::nightshift::parse_ledger;

const LEDGER: &str = include_str!("../fixtures/ledger.md");
const GOAL: &str = include_str!("../fixtures/goal.md");

fn tasks(entries: &[Entry]) -> Vec<&Entry> {
    // Every entry the fixture goal names, taken by number and by its absence,
    // so the composer is tested over the reader's own output.
    entries
        .iter()
        .filter(|entry| matches!(entry.number, Some(132) | Some(139)) || entry.number.is_none())
        .collect()
}

#[test]
fn the_goal_is_the_fixture() {
    let entries = parse_ledger(LEDGER);
    let chosen = tasks(&entries);
    let excluded = [
        "SP8, macOS agent workflow manager".to_string(),
        "Waiting on the operator".to_string(),
    ];
    let goal = compose(&Composition {
        date: "2026-09-21",
        next_date: "2026-09-22",
        ledger: "~/dotfiles/docs/remaining-work.md",
        excluded: &excluded,
        rules: "Never run an apply. No em-dashes anywhere.\n\n\n",
        tasks: &chosen,
    });
    assert_eq!(goal, GOAL, "the rules text is verbatim with its trailing blank lines trimmed");
}

#[test]
fn the_two_optional_paragraphs_are_omitted_rather_than_written_empty() {
    let entries = parse_ledger(LEDGER);
    let chosen = tasks(&entries);
    let goal = compose(&Composition {
        date: "2026-09-21",
        next_date: "2026-09-22",
        ledger: "~/dotfiles/docs/remaining-work.md",
        excluded: &[],
        rules: "",
        tasks: &chosen,
    });
    assert!(!goal.contains("OUT OF SCOPE"), "no section excluded writes no out-of-scope paragraph");
    assert!(!goal.contains("Standing rules"), "no rules file writes no rules block");
    assert!(goal.contains("TASKS, in ledger order:"), "the task list is never optional");
}

#[test]
fn an_unnumbered_task_is_written_without_a_number() {
    let entries = parse_ledger(LEDGER);
    let chosen = tasks(&entries);
    let goal = compose(&Composition {
        date: "2026-09-21",
        next_date: "2026-09-22",
        ledger: "L",
        excluded: &[],
        rules: "",
        tasks: &chosen,
    });
    assert!(
        goal.contains(
            "- Operator acceptance: reload the multiplexer's configuration and restart the \
             harnesses. (`Waiting on the operator`, line 19)"
        ),
        "an entry with no number opens with its title"
    );
}

#[test]
fn the_two_dates_are_tonight_and_the_morning() {
    use crate::recap::window::LocalCivilTime;
    let bedtime = LocalCivilTime { year: 2026, month: 9, day: 21, hour: 23, minute: 40, second: 0 };
    assert_eq!(dates(bedtime), ("2026-09-21".to_string(), "2026-09-22".to_string()));
    let month_end = LocalCivilTime { day: 30, ..bedtime };
    assert_eq!(
        dates(month_end),
        ("2026-09-30".to_string(), "2026-10-01".to_string()),
        "the morning steps the DATE, which is what keeps a month end and a zone change right"
    );
}
```

Create `pns/crates/pns-domain/src/nightshift/report/tests.rs`:

```rust
use super::*;

#[test]
fn the_headline_counts_what_went_in_and_what_stayed_out() {
    assert_eq!(
        headline(Handoff { verb: Verb::HandedOff, tasks: 23, sections: 4 }),
        "nightshift handed off, 23 tasks, 4 sections excluded"
    );
    assert_eq!(
        headline(Handoff { verb: Verb::Composed, tasks: 1, sections: 1 }),
        "nightshift composed, 1 task, 1 section excluded",
        "one of a thing is singular"
    );
    assert_eq!(
        headline(Handoff { verb: Verb::WouldHandOff, tasks: 0, sections: 0 }),
        "nightshift would hand off, 0 tasks, 0 sections excluded"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain nightshift::`
Expected: FAIL, `file not found for module `compose``.

- [ ] **Step 3: Write minimal implementation**

Create `pns/crates/pns-domain/src/nightshift/compose.rs`:

```rust
use super::Entry;

#[cfg(test)]
mod tests;

/// Everything the night's goal is written out of, gathered by the caller so
/// this function reads no file and no clock.
pub struct Composition<'a> {
    /// Tonight, `YYYY-MM-DD`.
    pub date: &'a str,
    /// The morning, `YYYY-MM-DD`.
    pub next_date: &'a str,
    /// The ledger's path, as the config writes it.
    pub ledger: &'a str,
    /// The excluded headings, in config order.
    pub excluded: &'a [String],
    /// The standing rules file's text, inserted verbatim.
    pub rules: &'a str,
    /// The entries that survived selection, in ledger order.
    pub tasks: &'a [&'a Entry],
}

/// The night's goal.
///
/// PARAGRAPHS ARE SINGLE LINES. The goal is a prompt rather than a document,
/// and a hard wrap inside it is only something a reader can get wrong.
pub fn compose(composition: &Composition) -> String {
    let mut goal = format!(
        "Overnight run, {} to {}. I am asleep. Complete as many of the tasks below as possible, \
         unattended, in the order given. The ledger is `{}`; every task below names its section \
         and its line. Read the full entry before starting it.\n",
        composition.date, composition.next_date, composition.ledger
    );
    if !composition.excluded.is_empty() {
        let names: Vec<String> = composition
            .excluded
            .iter()
            .map(|name| format!("`{name}`"))
            .collect();
        goal.push_str(&format!(
            "\nOUT OF SCOPE, do not touch: {}.\n",
            names.join(", ")
        ));
    }
    let rules = composition.rules.trim_end();
    if !rules.is_empty() {
        goal.push_str(&format!("\nStanding rules, all in force tonight:\n\n{rules}\n"));
    }
    goal.push_str("\nTASKS, in ledger order:\n\n");
    for task in composition.tasks {
        goal.push_str(&task_line(task));
    }
    goal.push_str(
        "\nA task that turns out to need me is not skipped: do everything up to that point, write \
         exactly what I owe into its ledger entry, and move on. The morning report is \
         `pns recap overnight`.\n",
    );
    goal
}

/// Tonight's date and the morning's, `YYYY-MM-DD` each, out of the local
/// civil moment the composition root read.
///
/// THE MORNING STEPS THE DATE rather than adding 86,400 seconds, which is
/// `recap::window`'s own rule and is why `plus_days` is reused here: adding a
/// day of seconds is an hour wrong twice a year.
pub fn dates(today: crate::recap::window::LocalCivilTime) -> (String, String) {
    (written(today), written(today.plus_days(1)))
}

/// One civil date, zero-padded.
fn written(date: crate::recap::window::LocalCivilTime) -> String {
    format!("{:04}-{:02}-{:02}", date.year, date.month, date.day)
}

/// One task's line: its number when it has one, its title, and where to read
/// the rest of it.
fn task_line(task: &Entry) -> String {
    let opening = match task.number {
        Some(number) => format!("{number}. "),
        None => String::new(),
    };
    format!(
        "- {opening}{} (`{}`, line {})\n",
        task.title, task.section, task.line
    )
}
```

Create `pns/crates/pns-domain/src/nightshift/report.rs`:

```rust
#[cfg(test)]
mod tests;

/// What the run did, which is the only thing the first printed line differs
/// by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// The goal was written and a launcher started.
    HandedOff,
    /// The goal was written and no launcher was configured.
    Composed,
    /// A dry run, which wrote and started nothing.
    WouldHandOff,
}

/// The counts the first line reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handoff {
    pub verb: Verb,
    pub tasks: usize,
    pub sections: usize,
}

/// The first line of what `pns nightshift` prints, without its `pns: ` prefix,
/// which the command's own style adds.
pub fn headline(handoff: Handoff) -> String {
    let verb = match handoff.verb {
        Verb::HandedOff => "handed off",
        Verb::Composed => "composed",
        Verb::WouldHandOff => "would hand off",
    };
    format!(
        "nightshift {verb}, {} {}, {} {} excluded",
        handoff.tasks,
        plural(handoff.tasks, "task"),
        handoff.sections,
        plural(handoff.sections, "section")
    )
}

/// The word, pluralized by its count. One of a thing is singular and every
/// other count, zero included, is plural.
fn plural(count: usize, word: &'static str) -> String {
    if count == 1 {
        word.to_string()
    } else {
        format!("{word}s")
    }
}
```

In `pns/crates/pns-domain/src/recap/window.rs`, make the existing day-stepping helper public. It is
the only change to that file:

```rust
    pub fn plus_days(self, days: i64) -> Self {
```

Change `pns/crates/pns-domain/src/nightshift.rs` to:

```rust
//! The bedtime handoff: the ledger read, the night's goal composed out of it,
//! and the lines the command prints about what it did.
//!
//! EVERY FUNCTION HERE IS PURE. The file is handed in as text, the date is
//! handed in as a string, and the standing rules are handed in verbatim, so
//! the whole grammar is pinned by committed fixtures and no test reads the
//! live ledger, which changes every day.

mod compose;
mod ledger;
mod report;

pub use compose::{Composition, compose, dates};
pub use ledger::{Entry, Excluded, parse_ledger, select};
pub use report::{Handoff, Verb, headline};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain nightshift::`
Expected: PASS, 14 tests.

- [ ] **Step 5: Commit**

```
SKIP_AI_COMMIT=1 git commit -m "feat(pns): compose the night's goal and the handoff's headline"
```

---

## Slice 2 tasks

### Task 4: the `[nightshift]` table

**Files:**
- Create: `pns/crates/pns-adapters/src/config/nightshift.rs`
- Create: `pns/crates/pns-adapters/src/config/nightshift/tests.rs`
- Modify: `pns/crates/pns-adapters/src/config/{mod,model,load}.rs`

**Interfaces:**
- Consumes: `pns_domain::nightshift::Excluded`.
- Produces: `Config::nightshift: Nightshift`, with the nine keys and
  `Nightshift::excluded(&self) -> Excluded`.

- [ ] **Step 1: Write the failing test**

Create `pns/crates/pns-adapters/src/config/nightshift/tests.rs`:

```rust
use super::*;

fn parsed(body: &str) -> Result<Nightshift, ConfigError> {
    let value: toml::Value = body.parse::<toml::Table>().unwrap().into();
    parse_nightshift(value)
}

#[test]
fn every_key_parses_and_the_defaults_are_todays_answer() {
    let table = parsed(
        r#"
ledger = "~/dotfiles/docs/remaining-work.md"
rules_file = "~/dotfiles/docs/gnhf-objective.md"
goal_file = "~/goals/tonight.md"
exclude_sections = ["Waiting on the operator"]
exclude_tasks = [133]
profile = "night"
until = "06:00"
launch = ["bash", "-c", "gnhf < \"$1\"", "_", "{goal}"]
log = "~/goals/tonight.log"
"#,
    )
    .expect("every key is known");
    assert_eq!(table.ledger, "~/dotfiles/docs/remaining-work.md");
    assert_eq!(table.profile, "night");
    assert_eq!(table.until, "06:00");
    assert_eq!(table.launch.len(), 5);
    assert_eq!(
        table.excluded(),
        pns_domain::nightshift::Excluded {
            sections: vec!["Waiting on the operator".to_string()],
            tasks: vec![133],
        }
    );

    let bare = parsed("").expect("an empty table is the defaults");
    assert_eq!(bare.ledger, "", "unset is what turns the handoff off");
    assert_eq!(bare.profile, "night");
    assert_eq!(bare.until, "06:00");
    assert!(bare.launch.is_empty(), "no launcher is the harness-goal mode");
}

#[test]
fn an_unknown_key_is_refused_by_name() {
    let error = parsed("enabled = true").unwrap_err();
    assert!(
        format!("{error}").contains("enabled"),
        "the refusal names the key: {error}"
    );
}

#[test]
fn the_one_placeholder_is_goal_and_any_other_is_refused() {
    let error = parsed(r#"launch = ["bash", "{tasks}"]"#).unwrap_err();
    let message = format!("{error}");
    assert!(message.contains("{tasks}"), "{message}");
    assert!(message.contains("{goal}"), "the refusal names the one that works: {message}");
    parsed(r#"launch = ["bash", "{goal}"]"#).expect("the one placeholder is accepted");
}

#[test]
fn until_is_an_hh_mm_time() {
    let error = parsed(r#"until = "6am""#).unwrap_err();
    assert!(format!("{error}").contains("6am"), "{error}");
    parsed(r#"until = "23:59""#).expect("a valid time parses");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters nightshift`
Expected: FAIL, `file not found for module `nightshift``.

- [ ] **Step 3: Write minimal implementation**

Create `pns/crates/pns-adapters/src/config/nightshift.rs`:

```rust
use super::*;

#[cfg(test)]
mod tests;

/// The one placeholder a `launch` element may carry, replaced with the goal
/// file's absolute path.
const GOAL_PLACEHOLDER: &str = "{goal}";

/// `[nightshift]`: what the bedtime handoff reads, writes, starts and selects.
///
/// THE TABLE IS OFF BY BEING UNSET, which is `[recap.summarizer]`'s own rule:
/// an empty `ledger` is a handoff with nothing to read, and the command
/// refuses by naming the key rather than guessing at a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nightshift {
    pub ledger: String,
    pub rules_file: String,
    pub goal_file: String,
    pub exclude_sections: Vec<String>,
    pub exclude_tasks: Vec<u32>,
    pub profile: String,
    pub until: String,
    pub launch: Vec<String>,
    pub log: String,
}

impl Default for Nightshift {
    fn default() -> Self {
        Nightshift {
            ledger: String::new(),
            rules_file: String::new(),
            goal_file: String::new(),
            exclude_sections: Vec::new(),
            exclude_tasks: Vec::new(),
            profile: "night".to_string(),
            until: "06:00".to_string(),
            launch: Vec::new(),
            log: String::new(),
        }
    }
}

impl Nightshift {
    /// The two exclusion keys as the domain's own filter.
    pub fn excluded(&self) -> pns_domain::nightshift::Excluded {
        pns_domain::nightshift::Excluded {
            sections: self.exclude_sections.clone(),
            tasks: self.exclude_tasks.clone(),
        }
    }
}

pub(super) fn parse_nightshift(value: toml::Value) -> Result<Nightshift, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`nightshift` is not a table".to_string()));
    };
    let mut nightshift = Nightshift::default();
    for (key, setting) in table {
        admits_flat("nightshift", &key)?;
        match key.as_str() {
            "ledger" => nightshift.ledger = text(&key, &setting)?,
            "rules_file" => nightshift.rules_file = text(&key, &setting)?,
            "goal_file" => nightshift.goal_file = text(&key, &setting)?,
            "log" => nightshift.log = text(&key, &setting)?,
            "profile" => nightshift.profile = text(&key, &setting)?,
            "until" => nightshift.until = time_of_day(&setting)?,
            "exclude_sections" => nightshift.exclude_sections = strings(&key, &setting)?,
            "exclude_tasks" => nightshift.exclude_tasks = numbers(&key, &setting)?,
            "launch" => nightshift.launch = argv(&setting)?,
            _ => return Err(unknown_key("nightshift", "nightshift", &key)),
        }
    }
    Ok(nightshift)
}

/// One string key.
fn text(key: &str, setting: &toml::Value) -> Result<String, ConfigError> {
    setting
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ConfigError::Invalid(format!("`nightshift.{key}` is not a string")))
}

/// A list of strings.
fn strings(key: &str, setting: &toml::Value) -> Result<Vec<String>, ConfigError> {
    let Some(list) = setting.as_array() else {
        return Err(ConfigError::Invalid(format!(
            "`nightshift.{key}` is not a list"
        )));
    };
    list.iter()
        .map(|element| {
            element
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| ConfigError::Invalid(format!("`nightshift.{key}` holds a value that is not a string")))
        })
        .collect()
}

/// A list of task numbers.
fn numbers(key: &str, setting: &toml::Value) -> Result<Vec<u32>, ConfigError> {
    let Some(list) = setting.as_array() else {
        return Err(ConfigError::Invalid(format!(
            "`nightshift.{key}` is not a list"
        )));
    };
    list.iter()
        .map(|element| {
            element
                .as_integer()
                .and_then(|number| u32::try_from(number).ok())
                .ok_or_else(|| {
                    ConfigError::Invalid(format!(
                        "`nightshift.{key}` holds a value that is not a task number"
                    ))
                })
        })
        .collect()
}

/// The launcher's argv, with its one placeholder checked here rather than at
/// run time: a typo that reached the launcher as a literal would start a night
/// with a goal file nobody wrote.
fn argv(setting: &toml::Value) -> Result<Vec<String>, ConfigError> {
    let elements = strings("launch", setting)?;
    for element in &elements {
        if let Some(unknown) = unknown_placeholder(element) {
            return Err(ConfigError::Invalid(format!(
                "unknown `nightshift.launch` placeholder `{unknown}`; the one placeholder is {GOAL_PLACEHOLDER}"
            )));
        }
    }
    Ok(elements)
}

/// The first `{word}` in an element that is not the one placeholder.
fn unknown_placeholder(element: &str) -> Option<String> {
    let mut rest = element;
    while let Some(start) = rest.find('{') {
        let after = &rest[start..];
        let end = after.find('}')?;
        let found = &after[..=end];
        if found != GOAL_PLACEHOLDER {
            return Some(found.to_string());
        }
        rest = &after[end + 1..];
    }
    None
}

/// `HH:MM`, checked at load so a bedtime run does not fail on a typo written
/// weeks earlier.
fn time_of_day(setting: &toml::Value) -> Result<String, ConfigError> {
    let written = text("until", setting)?;
    let valid = match written.split_once(':') {
        Some((hour, minute)) => {
            hour.len() == 2
                && minute.len() == 2
                && hour.parse::<u32>().is_ok_and(|hour| hour < 24)
                && minute.parse::<u32>().is_ok_and(|minute| minute < 60)
        }
        None => false,
    };
    if valid {
        Ok(written)
    } else {
        Err(ConfigError::Invalid(format!(
            "`nightshift.until` \"{written}\" is not an HH:MM time"
        )))
    }
}
```

In `pns/crates/pns-adapters/src/config/mod.rs`, beside the other table modules:

```rust
mod nightshift;
pub use nightshift::Nightshift;
use nightshift::parse_nightshift;
```

In `pns/crates/pns-adapters/src/config/model.rs`, on `Config` and its `Default`:

```rust
    pub nightshift: Nightshift,
```

```rust
            nightshift: Nightshift::default(),
```

In `pns/crates/pns-adapters/src/config/load.rs`, in the top-level match, after the `"gateway"` arm:

```rust
            "nightshift" => config.nightshift = parse_nightshift(value)?,
```

Add `"nightshift"` to the flat-key roster `admits_flat` reads, with its nine keys, in the same place
every other table's roster sits.

And, after every top-level table has parsed, beside the check that refuses a `[[profiles.rules]]` row
naming an undefined profile, refuse a `[nightshift] profile` that names one. IT IS A CROSS-TABLE
CHECK and belongs where that one already is, for the same reason: a profile name that does not resolve
is a night nobody silenced, found weeks after it was typed.

```rust
    if !config.nightshift.profile.is_empty()
        && !config.profiles.contains_key(&config.nightshift.profile)
    {
        let mut defined: Vec<&str> = config.profiles.keys().map(String::as_str).collect();
        defined.sort_unstable();
        return Err(ConfigError::Invalid(format!(
            "`[nightshift] profile` names `{}`, which no `[profiles.<name>]` table defines; this \
             config defines {}",
            config.nightshift.profile,
            defined.join(", ")
        )));
    }
```

and its test, in the same file as the other cross-table refusals:

```rust
#[test]
fn a_nightshift_profile_no_table_defines_is_a_load_error() {
    let error = parse_config(
        "[profiles.default]
quiet = false
banner = \"all\"
discord = \"all\"
         phone = \"all\"
lights = \"all\"

[nightshift]
profile = \"meeting\"
",
    )
    .unwrap_err();
    let message = format!("{error}");
    assert!(message.contains("meeting"), "{message}");
    assert!(message.contains("default"), "the refusal lists what is defined: {message}");
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters nightshift`
Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```
SKIP_AI_COMMIT=1 git commit -m "feat(pns): parse the nightshift table and its nine keys"
```

---

### Task 5: the shipped table and the rendered template

**Files:**
- Create: `pns/crates/pns-adapters/src/config/render/layout/nightshift.rs`
- Modify: `pns/crates/pns-adapters/src/config/render/layout.rs`
- Modify: `dot_config/pns/config-values.toml`
- Regenerate: `dot_config/pns/private_config.toml.tmpl`

**Interfaces:**
- Consumes: the `Table`, `Key` and `Sample` types of `config/render/layout.rs`.
- Produces: `NIGHTSHIFT`, one more row in `LAYOUT`.

- [ ] **Step 1: Write the failing test**

The gate is the existing byte-equality test in `just test-rust`, which compares
`dot_config/pns/private_config.toml.tmpl` against what `pns-config-render` writes from
`dot_config/pns/config-values.toml`. Make it fail first by adding the values without the layout.

Add to `dot_config/pns/config-values.toml`, after the `[recap.sources]` block:

```toml
# The bedtime handoff. `pns nightshift` composes the night's goal out of the
# ledger below, launches it through gnhf in its own worktree, and selects the
# `night` profile until morning. The standing rules are the objective file
# gnhf already reads, so one file holds them whoever is reading. The excluded
# sections are the ones that are not tonight's work: the workflow manager and
# Forzare are out of the goal, and the other three are not tasks at all.
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

- [ ] **Step 2: Run test to verify it fails**

Run: `just test-rust`
Expected: FAIL, the render walks `config-values.toml` against `LAYOUT` and refuses the table it has no
row for.

- [ ] **Step 3: Write minimal implementation**

Create `pns/crates/pns-adapters/src/config/render/layout/nightshift.rs`:

```rust
use super::*;

pub(super) const NIGHTSHIFT: Table = Table {
    name: "nightshift",
    prose: "# The bedtime handoff. `pns nightshift` reads the ledger below, composes the\n\
            # night's goal from every open task that is unblocked, not yours and not in an\n\
            # excluded section, launches it and selects the `night` profile until morning.\n\
            # The morning is `pns recap overnight`. THE TABLE IS OFF BY BEING UNSET: an\n\
            # empty `ledger` is refused by name rather than guessed at.\n",
    opt_in: false,
    children: &[],
    keys: &[
        Key {
            name: "ledger",
            prose: "# The ledger the night's tasks come out of. Its `##` and `###` headings are\n\
                         # the sections `exclude_sections` names, and an open `- [ ]` line under one\n\
                         # is a task. Unset is the whole handoff switched off.\n",
            sample: Sample::Default("\"\""),
        },
        Key {
            name: "rules_file",
            prose: "# A file whose text is inserted into the goal verbatim, under `Standing\n\
                         # rules`. Point it at the rules you already wrote down; pns never\n\
                         # reformats it and never carries a rule of its own.\n",
            sample: Sample::Default("\"\""),
        },
        Key {
            name: "goal_file",
            prose: "# Where the composed goal is written. Unset writes it under the state\n\
                         # directory, dated. Point it at your harness's goal directory to hand the\n\
                         # night over by file instead of by launcher.\n",
            sample: Sample::Default("\"\""),
        },
        Key {
            name: "exclude_sections",
            prose: "# Headings whose tasks never reach a goal, written exactly as the ledger\n\
                         # writes them after the hashes. A near miss excludes nothing, so a typo\n\
                         # costs a task an agent works on rather than one that disappears.\n",
            sample: Sample::Default("[]"),
        },
        Key {
            name: "exclude_tasks",
            prose: "# Task numbers you own. The other way to say it is a `**user:**` line in\n\
                         # the entry itself, which travels with the ledger; this key is for an\n\
                         # entry you would rather not mark.\n",
            sample: Sample::Default("[]"),
        },
        Key {
            name: "profile",
            prose: "# The profile selected at handoff, which has to be one `[profiles.<name>]`\n\
                         # defines. It is selected through `pns profile`'s own path, so its floor\n\
                         # still holds: a priority page reaches you asleep.\n",
            sample: Sample::Default("\"night\""),
        },
        Key {
            name: "until",
            prose: "# The local time that profile is selected until, as the next occurrence of\n\
                         # this minute. Nothing clears it in the morning; the bound is what ends it.\n",
            sample: Sample::Default("\"06:00\""),
        },
        Key {
            name: "launch",
            prose: "# The argv started detached, in its own session, with `{goal}` replaced by\n\
                         # the goal file's path. Empty starts nothing and writes the goal file\n\
                         # alone, which is how a harness picks the night up instead.\n",
            sample: Sample::Default("[]"),
        },
        Key {
            name: "log",
            prose: "# Where the launched process's output is appended. Unset is the goal file's\n\
                         # own path with `.log` for `.md`.\n",
            sample: Sample::Default("\"\""),
        },
    ],
};
```

In `pns/crates/pns-adapters/src/config/render/layout.rs`, beside the other layout modules:

```rust
mod nightshift;
use nightshift::NIGHTSHIFT;
```

and in `LAYOUT`, after `RECAP` and before `FOCUS`, because the handoff reads the recap's window and
narrows what the mute tables below it deliver:

```rust
    NIGHTSHIFT,
```

Then regenerate:

```bash
just pns-config-render
```

- [ ] **Step 4: Run test to verify it passes**

Run: `just test-rust`
Expected: PASS. `git diff --stat dot_config/pns/private_config.toml.tmpl` shows the new table and
nothing else moved.

- [ ] **Step 5: Commit**

```
SKIP_AI_COMMIT=1 git commit -m "feat(pns): ship the nightshift table at its defaults"
```

---

## Slice 3 tasks

### Task 6: the detached spawner

**Files:**
- Modify: `pns/crates/pns-application/src/ports/process.rs`
- Create: `pns/crates/pns-adapters/src/process/detached.rs`
- Modify: `pns/crates/pns-adapters/src/lib.rs`
- Create: `pns/crates/pns-adapters/src/process/detached/tests.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pns_application::DetachedSpawner` and `pns_adapters::SystemDetachedSpawner`.
  `spawn(&self, program: &str, args: &[&str], log: &Path) -> Result<u32, String>`.

- [ ] **Step 1: Write the failing test**

Create `pns/crates/pns-adapters/src/process/detached/tests.rs`:

```rust
use super::*;

#[test]
fn a_started_child_writes_to_the_log_and_reports_its_own_id() {
    let directory = std::env::temp_dir().join(format!("pns-nightshift-detached-write-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let log = directory.join("night.log");
    let id = SystemDetachedSpawner
        .spawn("/bin/sh", &["-c", "printf started"], &log)
        .expect("/bin/sh starts");
    assert!(id > 0, "the child's own process id is reported");
    // The child is detached, so nothing waits for it. Poll the log rather
    // than sleeping a fixed time, which is what keeps this test fast and
    // keeps it from flaking on a loaded machine.
    for _ in 0..200 {
        if std::fs::read_to_string(&log).unwrap_or_default() == "started" {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the child's output never reached {}", log.display());
}

#[test]
fn a_program_that_is_not_there_is_a_sentence_rather_than_a_panic() {
    let directory = std::env::temp_dir().join(format!("pns-nightshift-detached-missing-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let error = SystemDetachedSpawner
        .spawn(
            "/nonexistent/launcher",
            &[],
            &directory.join("night.log"),
        )
        .expect_err("a missing program does not start");
    assert!(!error.is_empty(), "the refusal says why");
}

#[test]
fn an_unwritable_log_is_refused_before_anything_starts() {
    let error = SystemDetachedSpawner
        .spawn("/bin/sh", &["-c", "true"], Path::new("/nonexistent/night.log"))
        .expect_err("a log that cannot be opened stops the launch");
    assert!(!error.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters detached`
Expected: FAIL, `file not found for module `detached``.

- [ ] **Step 3: Write minimal implementation**

Append to `pns/crates/pns-application/src/ports/process.rs`:

```rust
use std::path::Path;

/// Starts a program and does not wait for it. The seam a night is handed off
/// through.
///
/// DETACHED IS THE WHOLE POINT. The command is typed in a terminal at bedtime
/// and the terminal is closed minutes later; a child in the caller's process
/// group takes the hangup with it and the night ends before it started.
pub trait DetachedSpawner {
    /// The child's process id, or a sentence saying why it did not start.
    fn spawn(&self, program: &str, args: &[&str], log: &Path) -> Result<u32, String>;
}
```

Export it from `pns/crates/pns-application/src/lib.rs` beside `CommandRunner`:

```rust
pub use ports::process::{CommandRunner, DetachedSpawner};
```

Create `pns/crates/pns-adapters/src/process/detached.rs`:

```rust
use pns_application::DetachedSpawner;
use std::fs::OpenOptions;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

#[cfg(test)]
mod tests;

/// The one implementation: a child in a session of its own, its output
/// appended to one file, nothing waiting for it.
pub struct SystemDetachedSpawner;

impl DetachedSpawner for SystemDetachedSpawner {
    fn spawn(&self, program: &str, args: &[&str], log: &Path) -> Result<u32, String> {
        if let Some(parent) = log.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        // OPENED FIRST, so a log that cannot be written refuses the launch
        // rather than starting a night whose output goes nowhere.
        let out = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .map_err(|error| format!("{}: {error}", log.display()))?;
        let errors = out.try_clone().map_err(|error| error.to_string())?;
        let mut command = Command::new(program);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(errors));
        // SAFETY: `setsid` is async-signal-safe and is the only call made
        // between the fork and the exec. It puts the child in a session of
        // its own, so closing the terminal that started the night does not
        // hang it up.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        command
            .spawn()
            .map(|child| child.id())
            .map_err(|error| format!("{program}: {error}"))
    }
}
```

Declare the module and export the type in `pns/crates/pns-adapters/src/lib.rs`, beside the other
adapters:

```rust
mod process;
pub use process::detached::SystemDetachedSpawner;
```

with `pns/crates/pns-adapters/src/process.rs` holding `pub mod detached;`. `libc` is already a
dependency of this crate; add nothing new.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters detached`
Expected: PASS, 3 tests.

- [ ] **Step 5: Commit**

```
SKIP_AI_COMMIT=1 git commit -m "feat(pns): start a detached child in its own session"
```

---

### Task 7: the verb and its refusals

**Files:**
- Create: `pns/crates/pns/src/command_nightshift.rs`
- Create: `pns/crates/pns/src/command_nightshift/tests.rs`
- Modify: `pns/crates/pns/src/{invocation.rs,subcommand_usage.rs,lib.rs}`

**Interfaces:**
- Consumes: `pns_adapters::Nightshift`, `pns_domain::nightshift::*`, `pns_application::DetachedSpawner`.
- Produces: `nightshift_mode() -> i32`, and the pure `plan(...) -> Result<Plan, Refusal>` the tests
  drive.

- [ ] **Step 1: Write the failing test**

Create `pns/crates/pns/src/command_nightshift/tests.rs`:

```rust
use super::*;

fn table() -> Nightshift {
    Nightshift {
        ledger: "/ledger.md".to_string(),
        ..Nightshift::default()
    }
}

#[test]
fn the_two_forms_are_the_whole_surface() {
    assert_eq!(Form::of(&[]), Some(Form::Run));
    assert_eq!(Form::of(&["--dry-run".to_string()]), Some(Form::DryRun));
    assert_eq!(Form::of(&["--json".to_string()]), None, "an unknown flag is the usage");
    assert_eq!(Form::of(&["start".to_string()]), None, "there is no second verb");
    assert_eq!(
        Form::of(&["--dry-run".to_string(), "--dry-run".to_string()]),
        None
    );
}

#[test]
fn an_unset_ledger_names_the_key_it_wants() {
    let refusal = plan(&Nightshift::default(), "", "", "2026-09-21", "2026-09-22").unwrap_err();
    assert_eq!(refusal.code, 2);
    assert!(refusal.message.contains("[nightshift] ledger"), "{}", refusal.message);
}

#[test]
fn a_night_with_nothing_to_do_is_not_a_failure() {
    let ledger = "## Waiting on the operator\n\n- [ ] 1. Something you owe.\n";
    let table = Nightshift {
        exclude_sections: vec!["Waiting on the operator".to_string()],
        ..table()
    };
    let plan = plan(&table, ledger, "", "2026-09-21", "2026-09-22").expect("nothing to do is a plan");
    assert!(plan.tasks.is_empty());
    assert_eq!(
        plan.headline,
        "nightshift composed, 0 tasks, 1 section excluded",
        "the count is reported even when it is zero"
    );
}

#[test]
fn the_goal_carries_every_task_that_survived() {
    let ledger = "## Daily operations tools\n\n- [ ] 133. Nightshift. The handoff.\n";
    let plan = plan(&table(), ledger, "Never run an apply.", "2026-09-21", "2026-09-22")
        .expect("a ledger with a task is a plan");
    assert_eq!(plan.tasks.len(), 1);
    assert!(plan.goal.contains("- 133. Nightshift. (`Daily operations tools`, line 3)"));
    assert!(plan.goal.contains("Never run an apply."), "the rules are inserted verbatim");
    assert!(plan.goal.contains("Overnight run, 2026-09-21 to 2026-09-22."));
}

#[test]
fn the_goal_path_is_dated_when_the_config_names_none() {
    assert_eq!(
        goal_path(&table(), "/state", "2026-09-21"),
        Path::new("/state/nightshift/2026-09-21-overnight.md")
    );
    let named = Nightshift { goal_file: "/goals/tonight.md".to_string(), ..table() };
    assert_eq!(goal_path(&named, "/state", "2026-09-21"), Path::new("/goals/tonight.md"));
}

#[test]
fn the_log_sits_beside_the_goal_when_the_config_names_none() {
    assert_eq!(
        log_path(&table(), Path::new("/goals/2026-09-21-overnight.md")),
        Path::new("/goals/2026-09-21-overnight.log")
    );
    let named = Nightshift { log: "/logs/night.log".to_string(), ..table() };
    assert_eq!(log_path(&named, Path::new("/goals/x.md")), Path::new("/logs/night.log"));
}

#[test]
fn the_goal_placeholder_reaches_the_launcher_as_the_path() {
    let table = Nightshift {
        launch: vec!["bash".to_string(), "-c".to_string(), "run {goal}".to_string()],
        ..table()
    };
    assert_eq!(
        launch_argv(&table, Path::new("/goals/tonight.md")),
        vec!["bash".to_string(), "-c".to_string(), "run /goals/tonight.md".to_string()]
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns nightshift`
Expected: FAIL, `file not found for module `command_nightshift``.

- [ ] **Step 3: Write minimal implementation**

Create `pns/crates/pns/src/command_nightshift.rs`:

```rust
use pns_adapters::Nightshift;
use pns_domain::nightshift::{
    Composition, Entry, Handoff, Verb, compose, headline, parse_ledger, select,
};
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

/// What `pns nightshift` takes, which is the whole word and one flag.
pub(crate) const NIGHTSHIFT_USAGE: &str = "pns: usage: pns nightshift [--dry-run]";

/// Which of the two things this call asked for.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum Form {
    /// Compose, write, launch and select the profile.
    Run,
    /// Print the goal and change nothing.
    DryRun,
}

impl Form {
    /// THE BARE WORD DOES THE WHOLE HANDOFF, and the dry run is the flag: an
    /// operator who typed the wrong word gets a printed page, never a silent
    /// night.
    fn of(arguments: &[String]) -> Option<Self> {
        match arguments {
            [] => Some(Self::Run),
            [flag] if flag == DRY_RUN_FLAG => Some(Self::DryRun),
            _ => None,
        }
    }
}

/// The flag that asks for the goal rather than the night.
const DRY_RUN_FLAG: &str = "--dry-run";

/// A refusal with the exit code it carries.
pub(crate) struct Refusal {
    pub message: String,
    pub code: i32,
}

/// Everything the run decided before it touched the world.
pub(crate) struct Plan {
    pub goal: String,
    pub headline: String,
    pub tasks: Vec<Entry>,
}

/// The pure half: the config and the two files' text in, the goal out.
pub(crate) fn plan(
    table: &Nightshift,
    ledger: &str,
    rules: &str,
    date: &str,
    next_date: &str,
) -> Result<Plan, Refusal> {
    if table.ledger.is_empty() {
        return Err(Refusal {
            message: "pns nightshift: set `[nightshift] ledger` to your ledger's path; the handoff \
                      has nothing to read"
                .to_string(),
            code: 2,
        });
    }
    let entries = parse_ledger(ledger);
    let chosen = select(&entries, &table.excluded());
    let goal = compose(&Composition {
        date,
        next_date,
        ledger: &table.ledger,
        excluded: &table.exclude_sections,
        rules,
        tasks: &chosen,
    });
    let verb = if table.launch.is_empty() {
        Verb::Composed
    } else {
        Verb::HandedOff
    };
    Ok(Plan {
        goal,
        headline: headline(Handoff {
            verb,
            tasks: chosen.len(),
            sections: table.exclude_sections.len(),
        }),
        tasks: chosen.into_iter().cloned().collect(),
    })
}

/// Where the goal is written: what the config says, or a dated file under the
/// state directory.
pub(crate) fn goal_path(table: &Nightshift, state: &str, date: &str) -> PathBuf {
    if table.goal_file.is_empty() {
        Path::new(state)
            .join("nightshift")
            .join(format!("{date}-overnight.md"))
    } else {
        PathBuf::from(&table.goal_file)
    }
}

/// Where the launcher's output goes: what the config says, or the goal's own
/// path with a `.log` extension.
pub(crate) fn log_path(table: &Nightshift, goal: &Path) -> PathBuf {
    if table.log.is_empty() {
        goal.with_extension("log")
    } else {
        PathBuf::from(&table.log)
    }
}

/// The launcher's argv with `{goal}` replaced by the goal file's path.
pub(crate) fn launch_argv(table: &Nightshift, goal: &Path) -> Vec<String> {
    let path = goal.display().to_string();
    table
        .launch
        .iter()
        .map(|element| element.replace("{goal}", &path))
        .collect()
}
```

Wire the verb in `pns/crates/pns/src/invocation.rs`, beside `resume`, and add `NIGHTSHIFT_USAGE` to
`subcommand_usage.rs` the way every other subcommand's usage is listed:

```rust
    // The bedtime handoff. A MODE for the reason the others are: it takes no
    // decision from any event. What it starts is another program, and the one
    // setting it changes is the profile, through `pns profile`'s own path.
    if first == "nightshift" {
        std::process::exit(crate::command_nightshift::nightshift_mode());
    }
```

Task 8 writes `nightshift_mode`. For this task it is a stub that returns the usage and exit 2, so the
word exists and the pure half is under test:

```rust
/// `pns nightshift`: the bedtime handoff. Task 8 gives it its world.
pub(crate) fn nightshift_mode() -> i32 {
    let Some(_form) = Form::of(&crate::arguments_after_subcommand()) else {
        eprintln!("{NIGHTSHIFT_USAGE}");
        return 2;
    };
    eprintln!("{NIGHTSHIFT_USAGE}");
    2
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns nightshift`
Expected: PASS, 7 tests.

- [ ] **Step 5: Commit**

```
SKIP_AI_COMMIT=1 git commit -m "feat(pns): plan the night's handoff from the ledger and the config"
```

---

### Task 8: the handoff itself

**Files:**
- Modify: `pns/crates/pns/src/command_nightshift.rs`
- Modify: `pns/crates/pns/src/command_nightshift/tests.rs`

**Interfaces:**
- Consumes: `pns_application::DetachedSpawner`, `pns_adapters::SystemDetachedSpawner`,
  `pns_adapters::{LoadOutcome, config_path, load_config, local_civil}`, `crate::{now_secs, state_dir}`,
  and the profile override setter `pns profile <name> --until <HH:MM>` calls.
- Produces: `nightshift_mode()` in full, and `hand_off(...)` which the tests drive with a scripted
  spawner.

- [ ] **Step 1: Write the failing test**

Append to `pns/crates/pns/src/command_nightshift/tests.rs`:

```rust
use pns_application::DetachedSpawner;
use std::cell::RefCell;

/// A spawner that records what it was handed and answers as the test says.
struct ScriptedSpawner {
    answer: Result<u32, String>,
    seen: RefCell<Vec<String>>,
}

impl DetachedSpawner for ScriptedSpawner {
    fn spawn(&self, program: &str, args: &[&str], _log: &Path) -> Result<u32, String> {
        let mut seen = self.seen.borrow_mut();
        seen.push(program.to_string());
        seen.extend(args.iter().map(|argument| argument.to_string()));
        self.answer.clone()
    }
}

fn launching() -> Nightshift {
    Nightshift {
        ledger: "/ledger.md".to_string(),
        launch: vec!["bash".to_string(), "{goal}".to_string()],
        ..Nightshift::default()
    }
}

#[test]
fn a_handoff_writes_the_goal_then_launches_then_selects_the_profile() {
    let directory = std::env::temp_dir().join(format!("pns-nightshift-handoff-write-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let goal = directory.join("tonight.md");
    let spawner = ScriptedSpawner { answer: Ok(4242), seen: RefCell::new(Vec::new()) };
    let mut selected: Option<(String, String)> = None;
    let outcome = hand_off(
        "the goal text\n",
        &launching(),
        &goal,
        &spawner,
        &mut |profile, until| {
            selected = Some((profile.to_string(), until.to_string()));
            Ok(())
        },
    );
    assert!(matches!(outcome, Outcome::Launched(4242)), "the child's id is reported");
    assert_eq!(std::fs::read_to_string(&goal).unwrap(), "the goal text\n");
    assert_eq!(
        *spawner.seen.borrow(),
        vec!["bash".to_string(), goal.display().to_string()],
        "the placeholder reached the launcher as the path"
    );
    assert_eq!(selected, Some(("night".to_string(), "06:00".to_string())));
}

#[test]
fn a_launch_that_did_not_start_leaves_the_machine_loud() {
    let directory = std::env::temp_dir().join(format!("pns-nightshift-handoff-noloud-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let spawner = ScriptedSpawner {
        answer: Err("no such file".to_string()),
        seen: RefCell::new(Vec::new()),
    };
    let mut selected = false;
    let outcome = hand_off(
        "goal\n",
        &launching(),
        &directory.join("tonight.md"),
        &spawner,
        &mut |_, _| {
            selected = true;
            Ok(())
        },
    );
    assert!(matches!(outcome, Outcome::Failed(_)), "a spawn that failed is a state error");
    assert!(
        !selected,
        "THE ORDERING RULE: a handoff that did not start must not silence the machine"
    );
}

#[test]
fn no_launcher_writes_the_goal_and_still_selects_the_profile() {
    let directory = std::env::temp_dir().join(format!("pns-nightshift-handoff-nolauncher-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let goal = directory.join("tonight.md");
    let spawner = ScriptedSpawner { answer: Ok(1), seen: RefCell::new(Vec::new()) };
    let mut selected = false;
    let outcome = hand_off(
        "goal\n",
        &Nightshift { ledger: "/ledger.md".to_string(), ..Nightshift::default() },
        &goal,
        &spawner,
        &mut |_, _| {
            selected = true;
            Ok(())
        },
    );
    assert!(matches!(outcome, Outcome::Composed));
    assert!(spawner.seen.borrow().is_empty(), "an empty launch starts nothing");
    assert!(selected, "the operator is asleep either way, so the night profile is selected");
}

#[test]
fn a_goal_that_cannot_be_written_stops_before_the_launch() {
    let spawner = ScriptedSpawner { answer: Ok(1), seen: RefCell::new(Vec::new()) };
    let mut selected = false;
    let outcome = hand_off(
        "goal\n",
        &launching(),
        Path::new("/nonexistent/tonight.md"),
        &spawner,
        &mut |_, _| {
            selected = true;
            Ok(())
        },
    );
    assert!(matches!(outcome, Outcome::Failed(_)));
    assert!(spawner.seen.borrow().is_empty());
    assert!(!selected);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns nightshift`
Expected: FAIL, `cannot find function `hand_off``.

- [ ] **Step 3: Write minimal implementation**

Append to `pns/crates/pns/src/command_nightshift.rs`:

```rust
use pns_application::DetachedSpawner;

/// What the handoff did, which is what the last printed line says.
#[derive(Debug)]
pub(crate) enum Outcome {
    /// A launcher started, with its process id.
    Launched(u32),
    /// The goal was written and no launcher was configured.
    Composed,
    /// A state error, with the sentence that says what did and did not happen.
    Failed(String),
}

/// Write the goal, start the launcher, select the profile. In that order.
///
/// THE PROFILE IS SELECTED LAST. A handoff that did not start must leave the
/// machine loud: the operator is asleep either way, and a quiet machine with
/// nothing running on it is the one state nobody notices until morning.
pub(crate) fn hand_off(
    goal_text: &str,
    table: &Nightshift,
    goal: &Path,
    spawner: &dyn DetachedSpawner,
    select_profile: &mut dyn FnMut(&str, &str) -> Result<(), String>,
) -> Outcome {
    if let Some(parent) = goal.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            return Outcome::Failed(format!(
                "pns: state error (the goal at {} could not be written: {error}); nothing was \
                 handed off",
                goal.display()
            ));
        }
    }
    if let Err(error) = std::fs::write(goal, goal_text) {
        return Outcome::Failed(format!(
            "pns: state error (the goal at {} could not be written: {error}); nothing was handed off",
            goal.display()
        ));
    }
    let mut launched = None;
    if !table.launch.is_empty() {
        let argv = launch_argv(table, goal);
        let arguments: Vec<&str> = argv[1..].iter().map(String::as_str).collect();
        match spawner.spawn(&argv[0], &arguments, &log_path(table, goal)) {
            Ok(id) => launched = Some(id),
            Err(error) => {
                return Outcome::Failed(format!(
                    "pns: state error ({} did not start: {error}); the goal is at {} and the \
                     profile was not changed",
                    argv[0],
                    goal.display()
                ));
            }
        }
    }
    if let Err(error) = select_profile(&table.profile, &table.until) {
        return Outcome::Failed(format!(
            "pns: state error (the profile override could not be written: {error}); the run \
             started and the machine is still loud"
        ));
    }
    match launched {
        Some(id) => Outcome::Launched(id),
        None => Outcome::Composed,
    }
}
```

Replace the stub `nightshift_mode` with the whole command. It is the only place in this module that
reads a file, a clock or the config, which is what keeps everything above it testable:

```rust
/// `pns nightshift`: the bedtime handoff.
///
/// A MODE, in `resume_mode`'s sense: it takes no decision from any event. The
/// one setting it changes is the profile, and it changes it through the same
/// code path `pns profile <name> --until <HH:MM>` uses, so the priority floor
/// and the store row are that verb's and not this one's.
pub(crate) fn nightshift_mode() -> i32 {
    let Some(form) = Form::of(&crate::arguments_after_subcommand()) else {
        eprintln!("{NIGHTSHIFT_USAGE}");
        return 2;
    };
    let home = std::env::var("HOME").unwrap_or_default();
    let Ok(pns_adapters::LoadOutcome::Loaded(config)) =
        pns_adapters::load_config(&pns_adapters::config_path(&home))
    else {
        eprintln!("pns nightshift: the config could not be read; nothing was handed off");
        return 1;
    };
    let table = config.nightshift.clone();
    let Some((civil, _weekday)) = pns_adapters::local_civil(crate::now_secs()) else {
        eprintln!("pns nightshift: the local clock could not be read; nothing was handed off");
        return 1;
    };
    let (date, next_date) = pns_domain::nightshift::dates(civil);
    let ledger = match read(&table.ledger, &home) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "pns: state error (the ledger at {} could not be read: {error}); nothing was \
                 handed off",
                table.ledger
            );
            return 1;
        }
    };
    let rules = if table.rules_file.is_empty() {
        String::new()
    } else {
        match read(&table.rules_file, &home) {
            Ok(text) => text,
            Err(error) => {
                eprintln!(
                    "pns: state error (the standing rules at {} could not be read: {error}); \
                     nothing was handed off",
                    table.rules_file
                );
                return 1;
            }
        }
    };
    let plan = match plan(&table, &ledger, &rules, &date, &next_date) {
        Ok(plan) => plan,
        Err(refusal) => {
            eprintln!("{}", refusal.message);
            return refusal.code;
        }
    };
    if form == Form::DryRun {
        println!(
            "pns: {}",
            headline(Handoff {
                verb: Verb::WouldHandOff,
                tasks: plan.tasks.len(),
                sections: table.exclude_sections.len(),
            })
        );
        print!("{}", plan.goal);
        return 0;
    }
    if plan.tasks.is_empty() {
        println!(
            "pns nightshift: every open task is blocked, yours or excluded; nothing was handed off"
        );
        return 0;
    }
    let goal = goal_path(&table, &crate::state_dir().display().to_string(), &date);
    let mut select = crate::command_profile::selector();
    match hand_off(
        &plan.goal,
        &table,
        &goal,
        &pns_adapters::SystemDetachedSpawner,
        &mut select,
    ) {
        Outcome::Failed(message) => {
            eprintln!("{message}");
            1
        }
        outcome => {
            println!("pns: {}", plan.headline);
            println!("     goal   {}", goal.display());
            if let Outcome::Launched(id) = outcome {
                println!(
                    "     launch {} (pid {id}), log {}",
                    table.launch[0],
                    log_path(&table, &goal).display()
                );
            }
            // READ BACK rather than rendered from what the run intended, which
            // is `pns mute`'s rule and `pns profile`'s: the line cannot claim
            // a profile that never landed.
            println!("     {}", crate::command_profile::active_line());
            0
        }
    }
}

/// One file's text, with a leading `~/` read against this home the way
/// `[plugins.phone] marker_file` already is. A path written any other way is
/// used as it stands.
fn read(path: &str, home: &str) -> std::io::Result<String> {
    let resolved = match path.strip_prefix("~/") {
        Some(tail) if !home.is_empty() => format!("{home}/{tail}"),
        _ => path.to_string(),
    };
    std::fs::read_to_string(resolved)
}
```

`crate::command_profile::selector()` and `crate::command_profile::active_line()` are the two functions
this task adds to the profiles command module: the first is the override setter `pns profile <name>
--until <HH:MM>` already calls, handed out as a closure over the same store; the second is the one line
`pns profile` prints about the active profile, read back from the store. Neither is new behaviour and
neither is tested again here; profiles slice 2's own tests pin both.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns nightshift`
Expected: PASS, 11 tests.

- [ ] **Step 5: Commit**

```
SKIP_AI_COMMIT=1 git commit -m "feat(pns): hand the night off and select the night profile last"
```

---

## Verification

After the last task:

```bash
just test-rust
just lint-check
```

Both exit 0. Then the operator's own acceptance, which no test stands in for: `pns nightshift
--dry-run` at a bedtime, read against what they would have typed by hand, and only then the bare word.

## What this plan does not build

- No `status` and no `stop` verb. `pns profile` says what is standing and `pns recap overnight` says
  what the night did.
- No schedule. Nightshift is typed, and nothing launchd runs calls it.
- No removal of anything. The goal files accumulate under the state directory and the operator trashes
  what they want gone.
- No second copy of gnhf's own constraints. The worktree, `--current-branch`, `GRAPHIFY_SKIP_HOOK=1`
  and the clean tree live in the `launch` argv and in `docs/runbooks/local-agents.md`.
