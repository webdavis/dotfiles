# Remaining work

The open task list for the pns, posture, uu, lights, Neovim and tailnet-pin program. Ordered, one task at
a time, with stopping points that leave the tree in a state worth applying.

Updated as tasks complete. Last updated 2026-09-09.

## Where things stand

`main` carries eighty-six merged pull requests from 2026-09-06 onward and the first full `chezmoi apply`
since then has now run and passed.

`main` is green as of run 34301231057. Both red cases turned out to be wrong assertions rather than
broken code, and both repairs are mutation-verified.

Every pull request tracked by the 2026-09-06 Codex handoff has merged. Two pull requests remain open,
`#24` and `#51`, and both predate this program and are unrelated to it. This file replaces that handoff.

## Red main

- [x] 1. Fix `ledger_awk_spec: reads FIXED-NOTEST as closed as well, so the skip is a prefix match`. Run
  34284143580, merge of PR #462. It looked like an environment difference because it passed here and
  failed there; it is a flake. The case searched the whole output for the two characters `F5`, every
  output line starts with the register's own path, and CI drew the temporary directory
  `nvim.runner/cF5NkO`. The awk program was never wrong. Fixed by matching the id in its own column.
- [x] 2. Fix the e2e case `Two parallel runs deliver a batch exactly once` (`test/e2e`, the osquery
  alerter concurrency test). Run 34270895202, merge of PR #458. `read` returned 142 for SIGALRM: a loaded
  shared runner did not start bash and reach `send_alert` inside the 250 ms bound. All four bounds in
  that case are ceilings on a subject that never signals rather than measurements, so they were widened
  eightfold. A green run still takes about 800 ms.
- [x] 3. Rerun both workflows and confirm `main` is green.

## Apply blockers, merged as PR #463

- [x] 4. Commit, push and merge the apply fixes: the osquery converge configuration check and its tests,
  the deno `--allow-scripts` addition, the gitconfig fsmonitor exclusion for the lazy.nvim checkouts, the
  CLAUDE.md rule about verifying an apply first, the espanso `,,ca` trigger, and the auto-compact window
  setting. Shipped as nine commits with the two test repairs above.

## Untracked files, decide and clear

- [x] 5. Commit `docs/superpowers/specs/2026-07-15-s9-reland-decomposition-design.md`. It is a real
  approved design and every other design spec is tracked in that directory.
- [x] 6. Trash `HANDOFF-CODEX.md`. It is a handoff for a session that has ended and this file replaces
  what it carried. It also sits in the repository root, where no document belongs.
- [x] 7. Trash `nvim.log`. Two Neovim server-start warnings captured by accident.
- [x] 8. Trash `docs/research/moshi-verbose-daemon.log`. A 109 KB daemon capture from the moshi approval
  investigation, which is closed.

## Deployed leftovers

This repository builds no removal mechanisms, so a merged pull request that retires a file leaves the
deployed copy in place until it is trashed by hand. All five below were verified present on 2026-09-08.

Three sit in `~/.config/nvim/lua/custom_api/` with no chezmoi source, left behind by the standalone
Neovim repository that chezmoi replaced. Nothing in the source tree references any of them.

- [x] 9. Trash `~/.config/nvim/lua/custom_api/delegate.lua`. The v3 overhaul design retires it in favour
  of `coder/claudecode.nvim`.
- [x] 10. Trash `~/.config/nvim/lua/custom_api/helpers.lua`. Last touched December 2025, referenced
  nowhere.
- [x] 11. Trash `~/.config/nvim/lua/custom_api/init.lua`. It builds a module table that requires
  `custom_api.delegate`, and nothing requires it in turn.

The other two were retired by merged pull requests and named in their bodies for hand removal.

- [x] 11a. Trash `~/.local/libexec/herdr-jump.sh`. Replaced by the `herdr-workspace-jump` Rust plugin in
  PR #414.

- [x] 11c. DONE 2026-09-09. The apply landed and `uu doctor` now reports the `skills` lane, so the
  blocker cleared; `com.webdavis.update-skills` has been booted out and is gone from `launchctl list`.
  What is left is the removal itself, which is a destructive action the operator confirmed per
  invocation, and did: `~/Library/LaunchAgents/com.webdavis.update-skills.plist`,
  `~/.local/libexec/unattended-upgrades/agent-skills/update-skills.sh` and
  `~/.local/libexec/unattended-upgrades/helpers/log-entries.sh`. Deleting the chezmoi source does not
  delete the deployed copy, which is why these three survive. Take them together: two OTHER unmanaged
  leftovers still source `log-entries.sh`, and task 11e covers them.

- [x] 11e. DONE 2026-09-09. Two more retired unattended-upgrades leftovers, found while clearing 11c on
  2026-09-09. Neither is chezmoi-managed any more (`chezmoi managed` lists only
  `assert-hermes-superpowers-routing.sh` and `live-reconcile.sh` under that tree), and uu's `brew` and
  `claude-plugins` lanes replaced both, yet `com.webdavis.report-plugin-updates` is STILL LOADED and
  firing on its schedule. `com.webdavis.homebrew-weekly-upgrade` has a plist on disk but is not loaded.
  Bootout the first, then trash both plists and
  `~/.local/libexec/unattended-upgrades/{homebrew-weekly-upgrade.sh,claude/report-plugin-updates.sh}`.
  Doing this with 11c is what made `log-entries.sh` safe to remove, since these two were its only
  remaining consumers, verified by grep before anything moved. The agent was booted out first and
  confirmed gone from `launchctl list`; then eight files and three now-empty directories went to the
  trash. `agent-skills/` survives with its two live scripts.

- [x] 11d. Clear stale `~/.claude/ide/*.lock` files. A lock whose Neovim is gone makes claudecode.nvim
  open a plain HTTP connection to a dead port and warn `Missing or invalid Upgrade header` on every file
  open. Three were found on 2026-09-08, two of them nearly three days old, held by headless Neovim
  processes an agent had leaked. Cleared, and every remaining lock was verified live by its pid. This
  recurs whenever a headless Neovim is killed rather than quit, so it is worth re-checking, not a
  permanent fix.

- [x] 11b. Trash `~/.local/share/herdr/plugins/herdr-last-workspace` and its link. Folded into
  `herdr-workspace-jump` in PR #418. It was still registered in `~/.config/herdr/plugins.json` and still
  running an event hook on every `workspace.focused`, so it needed `herdr plugin unlink` before the
  trash. Deleting the directory alone would have left herdr pointing at a path that no longer exists.

## Before the first stopping point

- [x] 12. Merge PR #458, the auto-commit spec fix and module rename
- [x] 13. Commit and push the failure-reporting spec
- [x] 14. Push `feat/nvim-pns-wiring` (Neovim task 26, pns.nvim), PR, merge
- [x] 15. Push `feat/uu-tooling-e12-e17` (7 commits), PR, merge
- [x] 16. Merge main into PR #448 (herdr), merge
- [x] 17. Pre-apply verification: build pns and posture, headless Neovim start, zero stderr
- [x] 18. Run `chezmoi apply`
- [x] 19. Reload the herdr configuration for the new keybindings

### STOP POINT A

Everything above is additive. posture has not cut over, so the existing osquery pipeline keeps running
untouched. This is the recommended place to stop and apply.

## The monorepo conversion

Operator ruling 2026-09-09, replacing the earlier extraction plan. The tools STAY in this repository for
now, laid out like a monorepo so that lifting one out later is a move rather than a rewrite. They are
products other people install, so nothing in a tool may assume this repository exists.

Extraction into separate repositories is deferred to the tail; see task 68a.

- [x] 20. Convert to the monorepo layout, as ONE change because half-moved paths are the failure mode:
  move `dot_local/share/{pns,uu,posture,lights}` to `{pns,uu,posture,lights}` at the repository root;
  rename the CLI packages `pns-cli` to `pns`, `uu-cli` to `uu`, `posture-cli` to `posture`, so
  `cargo install --git https://github.com/webdavis/dotfiles pns` reads naturally; install the binaries to
  `~/.cargo/bin` and retire the `~/.local/libexec` rule for these four only, the bash scripts keep it.
  Every caller reads ONE declared value rather than a literal path: the pns and uu LaunchAgents, the
  Claude Code hook table in `modify_settings.json`, the Codex hook installer, `dot_bashrc.tmpl`, and the
  herdr and aerospace keybindings. launchd is the exception that needs the absolute path, because it has
  no PATH. Also update `.chezmoiignore`, the four builder scripts, the justfile, `treefmt.toml`,
  `scripts/treefmt/rust-file-size.sh` and the tests that name the old paths. Verified by experiment on
  2026-09-08: `cargo install --git` finds a package in a nested workspace with NO root `Cargo.toml`, so
  no root workspace manifest is needed and none should be added.

  Shipped as three commits on `refactor/monorepo-layout`. Four things the task did not anticipate:
  `lights-cli` was renamed with the other three, because leaving one command crate on the old suffix
  would have been the tree's only inconsistency. The declared value is `.chezmoidata/rust_tools.yaml`,
  and the bashrc reads it as `"$HOME/{{ .rust_tools.install_dir }}/pns"` rather than an absolute render,
  because a shell rc should expand `$HOME` at runtime. pns's two development binaries went behind
  `required-features = ["dev-tools"]`, since `cargo install` installs every binary a package declares and
  the rename would otherwise have put a bare `http-capture` in an installing user's `~/.cargo/bin`. And
  posture's tracked-path allowlist matches `~/.cargo/bin/posture` EXACTLY rather than by prefix, because
  that directory is shared with every other cargo-installed program on the machine. The aerospace keys
  needed no change: they still call `control-hue-lights.sh`, which is task 62's job to retire.

- [ ] 21. THREE OF FOUR CONFIRMED on 2026-09-09, after the apply. `pns doctor` and `uu doctor` both
  answer and exit 0, and `launchctl list` shows both agents loaded. The fourth is the operator's alone:
  an agent's tool shell is not interactive, so bash-preexec never loads and the shell hook never fires,
  which a `sleep 35` proved by leaving no trace in the decision log. `pns doctor` did surface one real
  failure worth carrying: the mobile push is refused by the moshi endpoint while the banner and hermes
  legs both deliver. The daemon is running the current 0.3.16 binary rather than a deleted Cellar, so the
  known stale-daemon fix does not apply. Original text: apply, then confirm every caller still resolves:
  `pns doctor`, `uu doctor`, a `launchctl list` showing both agents loaded, and one real long-running
  command raising its notification through the shell hook. The old binaries under
  `~/.local/libexec/{pns,uu,posture}/` and `~/.local/libexec/lights` are NOT removed by the apply and
  want trashing once this is confirmed; `~/.local/libexec/pns/hooks/` stays, because the Codex hook
  installer still lives there.

## pns closure and the rescued lanes

- [x] 22. Rework `fix/pns-retry-backoff` against the current crate layout, PR, merge
- [x] 23. Review and push `feat/posture-producer-commands` (heartbeat), PR, merge
- [x] 24. Review and push `feat/posture-converge-staging`, PR, merge. Shipped as
  `feat/posture-converge-foundation`: five of its six commits. The sixth deletes the bash converge and
  routes through the native command, which is the cutover task 50 owns, and the native path still calls
  `osqueryctl config-check`, the bug PR #463 fixed. It is parked until task 50 ports that fix.
- [x] 25. pns 18.1a: the `cargo doc` gate with `RUSTDOCFLAGS="-D warnings"`
- [x] 26. pns 8.4: the Codex and Claude hook-table verification record
- [x] 27. pns: backfill decision record 0012 (SQLite two fail directions)
- [x] 28. pns 18.1b: the completion report and line counts

### STOP POINT B

The pns refactor plan is closed and nothing is stranded.

## Delivery failure reporting

Designed in `docs/superpowers/specs/2026-09-08-pns-delivery-failure-reporting-design.md`, built from
`docs/superpowers/plans/2026-09-09-pns-delivery-failure-reporting-plan.md`.

- [x] 29. Plan the build, the pull request breakdown from the spec. Seven pull requests, one per task
  below, in that order. Three facts from the survey changed the plan: the backoff already landed, so task
  30 removes its jitter rather than adding the type; an `http_status` column already exists with a CHECK
  admitting only four codes, so task 31 has to recreate it rather than add one; and the banner's click
  slot is already free, returning the no-op `:` for exactly the pane-less events every delivery failure
  is.
- [x] 30. Permanent versus temporary classification in `pns-domain`. `DeliveryOutcome`, `FailureClass`
  and `RetryLimits::verdict`, with `DeadletterReason::Permanent`. Permanence outranks both counters, so a
  refused request dead-letters on attempt one. The jitter went with it: `retry_at` is now a pure function
  of the clock and the attempt count, `RetryBackoff::random_secs` is gone, and `retry_random_secs` is
  refused by name rather than ignored, because a key that parses and changes nothing reads as configured
  behavior.
- [x] 31. The ledger columns and the persisted failure record. Migration step 8 widens the CHECK on both
  `http_status` columns to any real status and adds `permanent` to `deadletter_reason`, and the terminal
  decision moves out of the hermes channel and behind `pns_domain::retry`'s permanent class, so a 404 now
  stops at its first queued retry. The attempt keeps whatever status it got, so a retryable 503 no longer
  leaves its code nowhere. The three denormalized leg columns the plan listed were not built: the attempt
  row already carries them, `route` was already on the leg, and a per-delivery write measurably slowed
  the suite. The read path the plan put here lands with its caller in task 33 rather than as an uncalled
  method.
- [x] 32. The message, both render forms, the per-destination meaning tables. `pns_domain::failure`
  rather than `pns-protocol`, whose own contract is that it holds no view of the domain model. The
  notification form enforces its 256-character budget itself, in the order the reader can least afford to
  lose, because a route name is as long as the producer made it and the surface would otherwise cut the
  fix line off the end. `Surface::Terminal` is a separate type from the notification surfaces, since the
  terminal repair names the route inside the fix line and can exceed the whole budget by itself. Two
  design gaps closed while building: the terminal `fix` needed a repair per code, which the design gave
  only as one worked example, and the 413 and 422 rows named no concrete subject despite the design's own
  rule that every meaning must.
- [x] 33. `pns failures` and the `pns doctor` routing to it. The read path, the listing capped at twenty
  newest first, `pns failures <id>` through the full renderer, and the doctor's ledger line naming the
  command when there is something to look at. NO PICKER, so the design's interaction model is not built:
  a printed listing plus an id needs no prompt, and a prompt is the only thing that model exists to
  guard. The `failed command` is reconstructed from the routing facts rather than stored, since a second
  copy of the flags could disagree with the routing it describes. The design's rung 5, a non-zero exit
  for a synchronous producer, is NOT here: it changes what every producer sees and belongs in its own
  change, filed against task 34's PR.
- [x] 34. The `pns doctor` route check. THE PROBE IS AN UNSIGNED POST, measured against the live gateway
  on 2026-09-09: it answers 401 for a route that exists and 404 for one that does not, while GET and HEAD
  answer 405 for every path and distinguish nothing. The signature is what authorizes a delivery, so a
  request without one cannot become a page. The ledger's distinct routes are the roster, because a
  producer names a route at call time and nothing else records it. A missing route reports loudly but
  does not move the exit code: the roster is derived from history, so a route retired on the gateway
  would fail the doctor forever with nothing an operator could do to clear it.
- [x] 34a. `posture-adapters` flake, PR #486, and only one of the two was a flake. The lifecycle case was
  a REAL DEFECT the suite happened to stand on: the command runner decided whether it had a terminal to
  hand over from the errno of `tcgetpgrp`, and Darwin answers a SOCKET at descriptor 0 with `EOPNOTSUPP`
  rather than `ENOTTY`, so an inherited socket failed every interactive command outright, which a
  socket-activated launchd job would hit. It now asks whether descriptor 0 is a terminal, which covers a
  pipe, `/dev/null` and a socket at once. The lock case was fixture shape: a single nonblocking `flock`
  asked once, in a binary where any concurrent spawn holds a copy of every open descriptor between its
  fork and its exec. Three more of the same class in pns followed, PRs #487 and #488.
- [x] 35. The banner click: `pns click`, and its three configured types
- [x] 35a. Raise the failure BANNER, which no task in this plan built: the design gives the notification
  its own 256-character form and its own `fix` line, and `pns_domain::failure::notification` renders it,
  but nothing called it, so a delivery failure was silent everywhere except `pns failures` and
  `pns doctor`. A leg speaks twice at most, on its first failure and on its dead-letter, because a banner
  per attempt teaches the operator to dismiss the one banner the design exists to raise. It carries the
  half of task 35 that had no producer: the banner's click is now `<absolute pns> click <id>`.
- [x] 35b. The failure PHONE CARD, under the existing presence rules (anything but at the desk, the same
  reading `forward_to_moshi` takes). It carries the rule the banner never needed, that a failure is never
  reported through the destination that failed: a mobile refusal pushes no card about itself, and a
  hermes refusal says Discord is empty rather than pointing at it.
- [x] 35c. A non-zero exit code for a synchronous caller whose page did not land, rung 5 of the design's
  "where a failure surfaces". Shipped OPT-IN, behind `--require-delivery`, because the design's
  unconditional form contradicts accepted decision 0010 (a notification never fails the work it reports
  on) and 126 tests pin that contract: every harness hook, the shell notifier and the daemon call the
  event path while real work is in flight. A caller that asked for the answer is one that can take it.
- [x] 36. The local page for moshi's browser preview

### STOP POINT C

A delivery failure is now loud, specific, and quick to act on, so posture can be trusted to page.

## pns command output

`pns doctor` prints twenty lines, twelve of them opening with the same `pns doctor:` prefix, in one
undifferentiated run. Every fact an operator needs is there and nothing says which lines belong together
or which one is the thing to act on. The operator's ruling on 2026-09-09: sections, color, a way to turn
color off, and the gum look rather than the plainer `hermes doctor` one, because output that reads well
is what makes a tool feel finished.

- [x] 69. The house style module and the doctor's report. `pns/crates/pns/src/style.rs` is the only place
  in pns that emits an escape sequence: gum's palette (the pink this repository already picked for
  `.chezmoitemplates/cli-print-style-lib.sh.tmpl`), a rounded frame, a section heading and a set of
  marks. `pns-domain::doctor::report` carries the report's SHAPE with no opinion about presentation, so
  the same report renders painted for an operator and plain down a pipe without either being a second
  copy. `--no-color` is the flag; `NO_COLOR`, `REPORT_LIB_PLAIN=1` and a destination that is not a
  terminal each turn it off on their own, and the flag beats all of them. Glyphs survive plain mode and
  only the color is dropped, because a mark is the row's meaning rather than its decoration. The doctor's
  twenty lines become titled sections, each with one line saying what its rows are for, and the report
  closes with a numbered list of what to act on. NOTE: the doctor currently refuses any argument at all,
  with a comment saying so; that comment changes with the flag.
- [ ] 70. Every other pns command that prints more than a sentence adopts the same vocabulary. Its scope
  is decided by reading what each command prints today, not by a list written here in advance. Read on
  2026-09-09, the commands that qualify are `pns failures` (a column table, and `listing()` is served by
  the failure page as well, so it takes a `Paint` and the page passes the plain one), `pns home` (a
  multi-line diagnostic), `pns setup` (the wizard's walk) and `pns lights` (a list of lines). Everything
  else prints one sentence or a usage string.

## Framed headers carry labels, never floating sentences

- [ ] 77. NOTHING IN A FRAME FLOATS (operator ruling 2026-09-09). Task 69 shipped the doctor's frame as a
  command name with a bare sentence under it, `pns doctor` over `every suppression gate is bypassed`, and
  a reader has no way to tell whether that sentence is a description, a status or an error. It reads like
  something went wrong. Every line after the command name takes a LABEL naming its role: `Note` for a
  caveat about the report being read, `About` for what a feature is, `Steps` heading a contents list. The
  label is what supplies the context the reader was otherwise left to guess at. THE FRAME ALSO SHOWS THE
  WHOLE INVOCATION, `pns tap --info` rather than `pns tap`, so the reader can tell which flag produced
  the output in front of them. This changes `pns doctor` (shipped) as well as the tap guide, and every
  command task 70 converts.

## The Back Tap marker

Verified on 2026-09-09, and it is the reason the two tasks below exist. The marker is written by a FORCED
COMMAND on an SSH key: `~/.ssh/authorized_keys` line 9 reads
`command="/usr/bin/touch /Users/stephen/.local/state/pns/phone-attention.marker",restrict` on the key
labelled `Shortcuts on mister`. sshd runs that command instead of whatever the client asks for, so the
iOS Shortcut never names the file and could not: a public key has no room for a path. pns has no writer
for it either, which its own spec records as an open question (`persistence-and-process-lifecycle.md`:
"no writer of that path exists anywhere in `src/`"). So the path is written down TWICE, in two systems,
one of which is not chezmoi-managed, and nothing checks that the two agree. A mismatch is silent: pns
sees a file that never updates, reads the tap as stale, and phone cards simply stop.

- [ ] 71. `pns tap` takes over the write. A new subcommand touches the marker at the SAME configured path
  the presence probe reads, so the path exists once. The forced command becomes
  `command="<cargo bin>/pns tap",restrict` and names no path at all, which is what makes task 72's knob
  safe to turn: the operator edits `authorized_keys` once, here, and never again. `restrict` still holds,
  so the key can run this and nothing else. The cost, stated rather than hidden: `/usr/bin/touch` is
  always present and a built binary is not, so a broken build takes the tap with it. That is why the
  doctor row below is part of this task rather than a follow-up: `pns doctor` gains a row under Pairing
  that reads `~/.ssh/authorized_keys`, finds the forced command, and says whether a key is wired to
  `pns tap`, so a key later deleted or mistyped is REPORTED instead of quietly ending phone cards. Its
  flags, and what each one is for. Bare `pns tap` touches the marker and prints the surface that results,
  because the forced command's stdout travels back over SSH and the Shortcut can show it: a tap that says
  nothing is a tap you cannot tell from a broken one. `--info` explains the feature and reports its live
  configuration: the marker path AND WHICH SOURCE SUPPLIED IT (the shipped default, the config file, or
  the environment), whether the file exists, how old it is, the surface that age implies, and whether a
  key in `~/.ssh/authorized_keys` is wired to `pns tap`. Naming the source is the point of it: an
  operator who set the config value and still sees the default is looking at an override they forgot, and
  no other output on the machine would tell them. `--install` PRINTS the `authorized_keys` line for this
  machine, with the binary path resolved, and says where to paste it. It says that a line already wired
  for this should be replaced rather than added beside. THERE IS NO `--write`, AND PNS NEVER READS OR
  WRITES `~/.ssh/authorized_keys`. This task's own history went back and forth on it, so the reasoning is
  recorded rather than the conclusion alone. Against writing: the flag would gate INTENT, never
  CAPABILITY. The write code sits in the binary on every run, and that binary runs unattended as a
  daemon, from every harness hook, and on every shell prompt. Any bug, config injection or compromised
  dependency that reaches it escalates to granting SSH access to the machine, which is not a notification
  tool's blast radius. What it buys against that is one paste, once per machine, ever. pns is also a tool
  other people `cargo install`, and "this notifier can edit your authorized_keys" is a line that should
  stop an auditor cold. Against reading: `--info` PRINTS what it reads, into a terminal whose contents
  get pasted into chats and issues, and the file is the operator's whole SSH trust list. And therefore no
  `--backup`: it only ever existed to make the write safe, and it carried its own hazard, since a copy of
  a trust file re-grants a key that was later revoked if it is restored unread. WHAT REPLACES THE READ IS
  A BETTER CHECK. `--info` and the doctor row report the MARKER'S OWN FRESHNESS: "last tap 3 hours ago",
  or "never tapped". That verifies the whole chain end to end (phone, Shortcut, ssh, key, forced command,
  file) rather than inspecting one link and inferring the rest, and it needs no access to `~/.ssh` at
  all. A wiring mistake anywhere in that chain shows up the same way: the marker never moves. `--install`
  IS A GUIDE, not a dump. It uses task 69's house style, so setup and `pns doctor` read as one tool: the
  framed title, `◆` numbered step headings with a faint blurb on the rule, `·` rows for the parts of a
  line that need explaining, and a closing rule pointing at `pns tap --info` to check the work. THE FRAME
  CARRIES A NUMBERED CONTENTS LIST, not a sentence and not a count of parts (operator ruling 2026-09-09,
  after "two halves" and then "set up this Mac, then set up your phone" were both rejected as too vague).
  It lists the steps by the same numbers their headings use, each with a short gloss:
  `1. This Mac / the authorized_keys line`, `2. Your phone / the PNS Tap shortcut`,
  `3. Trigger methods / Back Tap, Action Button, others`, NOT "A trigger": the section lists ways to fire
  the Shortcut, so it names the category rather than one instance of it. The reader sees the whole job
  before starting one, finds their place again after stepping away, and learns what a step involves
  without scrolling to it. Three steps, in the order they are performed: step 1 the `authorized_keys`
  line, with `command=`, `restrict` and the key placeholder each explained on their own row; step 2 the
  Shortcut, as labelled fields (Host, User, Auth, Script) rather than prose, with a note that the script
  text is cosmetic since step 1 overrides it; step 3 the triggers, listed with the Settings path beside
  each. Host and user come from the machine, never hardcoded. EVERY WORD PNS PRINTS GOES THROUGH THE
  `humanizer` SKILL BEFORE IT SHIPS (operator ruling 2026-09-09, standing, and it covers every pns
  command rather than this guide alone). Terminal output is prose the operator reads under pressure, and
  the tells that skill catches are the ones that make a tool feel generated. The first pass over this
  guide caught four. A subjectless "Nothing is written for you" tacked on as a negation becomes "pns does
  not edit this file". A run of fragments closing on the manufactured punchline "This key does one thing"
  keeps the fact list and loses the punchline. "The script text is cosmetic" becomes "sshd ignores this
  script text", which is shorter and more accurate. And "Found 3 issues to address:" carries filler ahead
  of a numbered list, so the doctor's closing line becomes "3 issues to fix:". The doctor's seven section
  blurbs passed unchanged. STEP 2 SHRINKS LATER. The operator intends to host a public Shortcut people
  can install directly (2026-09-09), at which point step 2 becomes a link and an "install this" rather
  than a field-by-field build. Write it so that swapping those is an edit to one step, not a rewrite of
  the guide. COVERS THE PHONE SIDE TOO, because the wiring has two halves and an operator holding only
  one of them has nothing working. After the `authorized_keys` line it prints the Shortcut recipe (Run
  Script Over SSH, with the host, the user and which key to select) and the triggers that Shortcut can be
  attached to: Back Tap, the Action Button, a Lock Screen widget, Control Center, Siri. The command text
  typed into the Shortcut is cosmetic, since sshd runs the forced command instead, but it is spelled
  `pns tap` anyway so the Shortcut reads as what it does. THE SETUP PROSE STAYS OFF `--info`: that flag
  is read when something is already wrong, and burying a status report under a wall of instructions is
  how a diagnostic stops being read. `--info` closes with one line pointing at `pns tap --install`. THE
  PRINTED INSTRUCTIONS CARRY THE iOS VERSION THEY WERE VERIFIED AGAINST, as a line the reader sees
  ("Settings paths verified on iOS <version>"). The exact paths to Back Tap and the Action Button move
  between releases, and instructions that do not date themselves are worse than none: a reader on a later
  iOS cannot tell a path that moved from a step they got wrong. Verify them against the operator's own
  iOS at build time rather than writing them from memory here, and record the version in the same change
  that writes the text. `--delete-marker` IS NOT BUILT. Verified against
  `pns/docs/specs/presence-and-visibility.md` on 2026-09-09, which settles it: "Mobile and Away both mean
  the phone card", and "Away always cards while Mobile lets [the viewed pane suppress it]". So deleting
  the marker while away from the desk moves the operator Mobile to AWAY, which cards MORE aggressively
  because Away never suppresses, the opposite of what a flag called clear or delete would promise. At the
  desk it is redundant, since typing already cancels a stray tap under newest-signal-wins. Both cases
  fail, so the flag does not ship. The earlier names weighed for it (`--clear`, `--at-desk`) are moot.
  `--json` emits the same answers machine-readably, so the Shortcut renders them rather than dumping a
  sentence. `--no-color` is NOT one of these flags; it is tool-wide, task 73. DELIBERATELY NOT
  `--set-marker`, a flag that writes the config: `~/.config/pns/config.toml` is a chezmoi-rendered target
  on this machine, so a write there is erased by the next apply and the operator would watch their change
  disappear. `--info` names the file that really holds the value instead. DELIBERATELY NOT
  `--for <duration>`, a tap that expires on its own: the probe reads the marker's mtime and never its
  contents (`symlink_metadata`, so a dangling symlink still answers), so an expiry is a reader redesign
  rather than a flag, and it is scoped separately if it is ever wanted.

- [ ] 74. THE HTTP TAP, an opt-in ALTERNATIVE to the SSH one, never a replacement that arrives on its
  own. Operator ruling 2026-09-09: ship the SSH shape first, offer this as an upgrade the operator
  chooses. pns serves a small endpoint the Shortcut posts to ("Get Contents of URL" rather than "Run
  Script Over SSH") and records the tap itself. THE POINT IS THAT PNS OWNS BOTH ENDS: no
  `authorized_keys` line, no forced command, no second system holding a copy of a path, so the decoupling
  tasks 71 and 72 work around stops existing rather than being managed. The machinery is mostly here
  already: the daemon runs, and `pns failures serve` is a listener. IT ASSUMES NOTHING ABOUT THE
  OPERATOR'S NETWORK (operator ruling 2026-09-09, correcting an earlier draft of this task that said "on
  the tailnet"). pns is a tool other people install and it has no idea what anyone's topology looks like:
  no Tailscale, no VPN, no LAN shape, nothing detected and nothing guessed. The listener is OFF unless
  configured, and its `bind` address is written by the operator with NO DEFAULT, because there is no safe
  one to pick: loopback is safe and unreachable from a phone, and every other address is a guess about
  somebody's network. Authentication is a config secret, the way the hermes webhook already is. ITS ONE
  REAL COST, which is why it is opt-in rather than the default: the SSH tap works with pns's daemon dead,
  because sshd and the command it forces carry it end to end, and an HTTP tap does not. An operator whose
  daemon is wedged still wants their phone to say so. Also a listening port where there was none, and a
  secret that needs a rotation story.

- [ ] 76. TEST THE APPLE SHORTCUTS ROUTE BEFORE BUILDING TASK 74. iOS Shortcuts can run a Shortcut ON A
  MAC over iCloud, and a Mac-side Shortcut's "Run Shell Script" action can call `pns tap`. If that works
  it beats both the SSH tap and the HTTP one: NO LISTENING PORT AT ALL, no key, no `authorized_keys`
  line, no shared secret, nothing for pns to own but the marker it already owns. FROM TRAINING, NOT
  VERIFIED, which is exactly why this is an investigation and not a build: whether cross-device execution
  works on this operator's iOS and macOS versions, whether the Mac must be awake or unlocked, and what
  the latency is. Those three answers decide it. Half an hour of testing on the real devices settles
  whether task 74 is worth building at all.

## Tool-wide output flags

- [x] 73. DONE 2026-09-09, shipped with task 69 rather than after it, because a flag whose scope is wrong
  is a contract, and the narrow form would have been the shipped one for as long as it took to widen.
  `--no-color` is a PNS-WIDE flag, accepted in every position: `pns --no-color doctor` and
  `pns doctor --no-color` mean the same thing, because an operator who has decided about color has
  decided about the whole command rather than about one subcommand's report. Task 69 shipped it as a
  `pns doctor`-only argument, which is the narrower reading and wrong; this widens it. The dispatcher
  takes the flag out of argv wherever it appears, remembers it once, and every command that prints reads
  that one answer with no plumbing of its own. THE EVENT PATH IS EXEMPT and keeps its argv untouched: it
  prints nothing but an exit code, and a position-blind filter would eat a producer's
  `--detail "--no-color"` as a flag, which is the same value-position bug the argv grammar already guards
  against elsewhere.

- [ ] 71a. FIVE THINGS THE TAP DESIGN LEFT OUT, found by re-reading it whole on 2026-09-09. Each is a
  silent failure, which is why they are recorded rather than left to be noticed later. EXIT CODE:
  `pns tap` exits non-zero when the touch fails, so the Shortcut can show a failure. A tap that fails
  silently is worse than no tap, because the operator stops checking. THE STATE DIRECTORY:
  `~/.local/state/pns/` may not exist on a fresh machine and `pns tap` may be the first thing to reach
  for it, so it creates the directory rather than failing on it. REMOTE LOGIN IS STEP 0, and the guide
  had no step 0 at all. The whole feature needs sshd accepting connections (System Settings, General,
  Sharing, Remote Login). Without it every other step is wired correctly and nothing happens, which is
  the worst kind of wrong. A SLEEPING MAC does not answer SSH, so the tap is lost with no error anywhere.
  This is the likeliest real-world failure and nothing mentioned it; `--info` gets a section saying so.
  NO CONFIG REQUIRED: `pns tap` must work with no `~/.config/pns/config.toml` at all, falling back to the
  default marker path, because requiring one would fail on exactly the fresh machine `--install` is
  walking somebody through. Still unspecified and minor: the `--json` schema, and how to undo the setup.

- [ ] 72. `[phone] marker_file` makes the path configurable, defaulting to today's
  `$HOME/.local/state/pns/phone-attention.marker`, with `PNS_PHONE_MARKER_FILE` still winning over it so
  the tests and sandboxes are untouched. NOT `[presence]`: `[plugins.presence]` already exists and is the
  Hue room sensor, and two tables a word apart meaning different things is the confusion this avoids.
  Ordered after 71 deliberately: a knob shipped while the path is still duplicated is a knob that breaks
  the tap when it is turned.

## SSH exposure (not a pns task)

- [ ] 75. BIND SSHD TO THE TAILNET. A DOTFILES TASK, NOT A PNS ONE, and it is filed in its own section
  below rather than beside the tap tasks so it cannot be read as pns work. pns never learns that this
  happened: it does not check for it, mention it, or behave differently either way. The tap is merely why
  the listener exists. Measured on 2026-09-09: `netstat -an | grep LISTEN` shows sshd on `*.22`, IPv4 and
  IPv6, so this Mac answers on every network it touches. Public-key-only is already enforced, so nobody
  gets in without a key, but the machine still announces itself as an SSH server to any network it joins.
  Tailscale is the only network its own devices are on. The change: an `ListenAddress` for the Tailscale
  address in the drop-in at `/etc/ssh/sshd_config.d/000-ssh-hardening.conf`, which
  `dot_local/bin/executable_ssh-hardening.sh` already generates and installs. Port 22 then answers on the
  tailnet alone. The phone is on the tailnet, so the tap keeps working. Verify with `netstat` before and
  after AND confirm a real tap still lands, because a wrong address silently ends both SSH and the tap at
  once, and the script's own `--reload` refuses to claim success without a real banner exchange.

## posture foundation

- [ ] 37. posture 2.4: page, domain digest, protocol codec
- [ ] 38. posture 2.9: `drift.rs` and `converge_policy.rs`
- [ ] 39. posture 2.10: `cursor.rs` and `triage.rs`
- [ ] 40. posture 3.1 remainder: four adapters
- [ ] 41. posture 3.2 remainder: tailscale, process, gateway, `LaunchdState`
- [ ] 42. posture 3.3: the converge read half, staging, privileged

### STOP POINT D

The foundation is complete and nothing has cut over, so there is no runtime risk yet.

## posture cutovers

Every task in this section needs the pns-keyed gateway route to exist first. Adding it is an operator
step, and it gates the whole section.

- [ ] 43. posture 6.1: heartbeat cutover
- [ ] 44. posture 6.2: digest cutover
- [ ] 45. posture 6.3: alert cutover
- [ ] 46. posture 6.4: watchdog cutover
- [ ] 47. posture 6.5: poll cutover
- [ ] 48. posture 6.6: funnel cutover
- [ ] 49. posture 6.7: drainer retirement
- [ ] 50. posture 7.1: converge cutover

### STOP POINT E

Every posture producer is Rust and the old pipeline is off.

## uu

- [ ] 51. uu B1: `rust-toolchain.toml`, needs the stable toolchain certified
- [ ] 52. uu D1, D2, D3: the cargo lane and `RustupLane`
- [x] 53. uu E12, E13, E14: skills hermes and forks
- [x] 54. uu E15a, E15: the skills orchestrator
- [x] 55. uu E16: retire `update-skills.sh` and its LaunchAgent
- [x] 56. uu E17: retire `log-entries.sh`
- [ ] 57. uu E19: the log rotation lane, needs the hourly log writer stopped

### STOP POINT F

uu is complete.

## posture cleanup

- [ ] 58. posture 8.1, 8.2, 8.3: the ssh-hardening port, then trash the deployed
  `~/.local/bin/ssh-hardening.sh` it replaces
- [ ] 59. posture 9.1: osquery leaves the tracked set
- [ ] 60. posture 9.2: the completion report and 187-name mapping table

### STOP POINT G

posture is done and osquery is retired.

## The tail

- [ ] 61. lights: the argument-surface differential, owed since PR 1
- [ ] 62. lights PR 12: the aerospace keys F4 to F10 off the bash script
- [ ] 63. lights: the manifest decision for `~/.local/libexec/lights`
- [ ] 64. lights PR 11a, conditional on the `bulk_read_latency` drill
- [ ] 65. Neovim task 63: the acceptance record, needs the clean-home apply
- [x] 66. tailnet-pin: the Rust crate replacing `reconcile-hosts-pin.sh`. Two limits of the shell went
  with the port. A line carrying a NUL byte is now copied through whole, where `read` dropped the NUL and
  joined the two halves because no shell variable can hold one; and the temporary file is removed by
  `Drop` rather than by six signal traps, so there is no signal list to keep in step with a test.
- [x] 66b. tailnet-pin cutover: the builder at `run_onchange_after_40`, the `run_onchange_after_41` call
  site, and the source script and its bash suite deleted. The caller's SHA256 pin of a source file became
  a SOURCE FINGERPRINT the builder records after installing, because a built binary's bytes do not exist
  at render time; the runner refuses every pin unless that record matches what this apply rendered, which
  is what stops a deferred build from aiming a stale binary at `/etc/hosts` as root.
- [x] 66c. DONE 2026-09-09. Trashed the deployed `~/.local/libexec/tailscale/reconcile-hosts-pin.sh` and
  its now-empty directory, after an apply has installed `~/.cargo/bin/tailnet-pin`. Chezmoi does not
  delete a target whose source entry is gone, and this repository builds no removal mechanisms, so it is
  one operator command.
- [x] 66a. herdr: the clean-code pass on `dot_local/share/herdr/plugins/herdr-smart-nav`. The direction
  became an enum, which closed a pair that could disagree: the word and the chord travelled side by side
  as two strings, so a call passing `"left"` with `ctrl+l` compiled and sent Neovim the wrong way. Every
  public item gained the documentation the house voice asks for, and the parse of herdr's answer states
  why every unreadable shape means the same thing.
- [ ] 68a. Extract each tool into its own public repository with `git subtree split`, once the operator
  has hand-rewritten it and is ready to tag a v1. Deferred from tasks 20 and 21; the monorepo layout
  exists so this is a move. Nothing is published to crates.io while a tool is pre-v1.

## Repository hygiene

- [ ] 67. Delete the dead local branches. About a hundred of them: every `wf_*`, `worktree-agent-*` and
  `agent-*` name, plus the old `backup/*` and throwaway experiment branches. Each group needs the
  operator's approval before it goes.
- [ ] 68. Remove the worktrees those branches left under `~/.herdr/worktrees/`. There are 195 of them.
  Their cargo `target/` directories are already cleaned, so this is about clutter rather than disk.
  Remove with `git worktree remove`, never `rm`, and keep any worktree whose branch still holds commits
  that are not on origin.

## Waiting on the operator

Each of these gates work that cannot start without it.

- [x] Add the pns-keyed gateway route, gated tasks 43 to 50. CLEARED 2026-09-09, and it turned out to
  have been done already. The open question below could not confirm it because the hermes config is
  age-encrypted; decrypting the SOURCE and counting route keys alone shows both `priority` and `pns`, so
  the route is durable rather than a hand edit of the deployed copy that the next apply would erase.
  `bypass_silence_classes` is in the pns config schema, which is the plan's step 0.5.
- [x] Certify the stable Rust toolchain, gated tasks 51 and 52. CLEARED 2026-09-09, and certifying it is
  what found the real problem. The machine's stable was 1.88.0 from June 2025 while CI's is 1.98, so the
  two were never the same toolchain; `pns-adapters` uses `file_lock`, stabilized in 1.89, so PINNING the
  old stable would not have compiled at all. After `rustup update stable` to 1.98.1, `just test-rust`
  exits 0 across all six workspaces, 102 green test binaries, and no crate uses `#![feature]`. The
  default toolchain stays nightly; task 51 is what pins stable per directory.
- [x] Stop the hourly log writer, gated task 57. CLEARED 2026-09-09 by ruling rather than by action: task
  57 deletes the script, the plist and the loader itself, so the operator applies once and the job is
  gone. Stopping it by hand first would have been undone by the next apply, since the loader still exists
  until 57 removes it.
- [ ] The clean-home apply from PR #385, gates task 65
- [ ] The lamp drills, gates task 64
- [ ] Archive `webdavis/neovim-config` and remove `~/.config/nvim/.git`
- [ ] Approve the branch and worktree deletions, gates tasks 67 and 68
- [ ] Run `chezmoi apply` to deploy the uu skills lane, which gates task 11c
- [ ] Trash `~/.local/libexec/tailscale/reconcile-hosts-pin.sh` and its directory once an apply has built
  `~/.cargo/bin/tailnet-pin`, gates task 66c
- [ ] Restart Claude Code so the old `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE` leaves the process environment

## Deferred, not scheduled

Carried over from the Codex handoff so it is not lost when that file goes. None of these start without
the operator saying so.

- SP4 and SP5, the shell migration to xonsh. Waits until the Neovim overhaul is finished and the operator
  says go.
- SP3 part 2, the A to H feature proposal. Not approved; it needs an operator decision and a credential.
- SP-nix, the nix-darwin go or no-go. Deferred, research first.
- SP7, the sweep and backlog pass including Todoist hygiene. After everything above.
- The babysitter execute-bit issue, draft in `~/.claude/pipeline/babysitter-issues/`. Deferred by the
  operator on 2026-09-06.

## Open questions

- RESOLVED 2026-09-09. posture 0.2, the pns priority-route, could not be confirmed because the hermes
  config is age-encrypted. Decrypting the source and counting route keys alone, with no value printed,
  shows `priority` and `pns` side by side, which is what the plan's step 0.3 asks for.
- `webdavis/pns.nvim` is its own repository and was not audited. Task 14 finishes its integration here,
  but unfinished work inside that repository would not have shown up.
