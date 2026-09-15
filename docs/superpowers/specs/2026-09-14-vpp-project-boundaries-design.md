# vpp project boundaries

Status: design, written 2026-09-14 by an overnight agent with the operator asleep. NOT approved. No
code was written or changed. Every choice made in the operator's place is listed under Assumptions
with its alternative, and the questions that need an answer are at the end.

Scope: this document answers one ledger item, "Keep vpp application code in its own project, Mac
installation and service configuration in dotfiles, output content in the configured directory (Ivy
for this operator), and homelab deployments in homelab. Reuse existing transcription tasks. vpp must
work without Bob, Forzare or the full homelab." It decides where each kind of file lives and how the
independence claim gets proved. It does NOT design ingestion, transcription, tagging, briefs or
redaction; each of those is its own ledger item with its own design document in this wave.

vpp, on first use, is the voice processing pipeline the operator named on 2026-09-12: a Rust tool that
collects everyday Apple Voice Memos synced to the Mac, preserves the original audio, transcribes it,
and produces agent notes and summaries.

## Why a boundary document at all

Four repositories and one content directory already have a claim on some part of this pipeline, and
three of them are governed by different rules:

1. `webdavis/dotfiles`, this checkout, a chezmoi source directory that also holds four Rust tool
   workspaces and every LaunchAgent on the machine.
1. `webdavis/homelab`, which owns server deployments and already records the vpp feature decisions as
   L-R5 in `docs/plans/PLAN-v12-experiments-backlog.md`.
1. `webdavis/Ivy`, the Obsidian vault, which already defines `agent-processing-pipeline/raw/audio/`,
   `transcripts/` and `analysis/`, excludes audio from git vault-wide, and holds a symlink named
   `minutes` pointing at a third-party tool's output directory outside the repository.
1. A vpp project that does not exist yet.

Without a written boundary the default outcome is predictable: application code lands in dotfiles
because that is where the build scripts already are, content lands next to the code, and the tool
becomes unusable by anyone who does not have this checkout. That is the exact failure the operator
already ruled against for the four existing Rust tools ("a tool never hardcodes its own path",
2026-09-08) and for the custom Neovim plugins ("every custom plugin ships as its own public repo",
2026-09-05).

## Constraints this design inherits

From `CLAUDE.md` and the recorded operator rulings, not open for re-litigation here:

1. **Nothing inside a tool workspace may assume this repository exists.** The four Rust tools are
   products other people install with `cargo install --git`.
1. **No workspace may depend on another** (2026-09-10). No shared crate, no cross-workspace path
   dependency. Two tools needing the same thing each get a copy. Runtime integration, one tool
   spawning another tool's published command line, stays allowed and is how uu already reaches pns.
1. **`~/.local/bin` holds only what the operator types; everything launchd, a hook or a recipe
   invokes lives under `~/.local/libexec`.** The four Rust tools are the exemption and install to
   `~/.cargo/bin`, declared once in `.chezmoidata/rust_tools.yaml`.
1. **THE OPERATOR RUNS APPLIES.** Agents propose. Any dotfiles-side mechanism must be verifiable
   without an apply, by rendering the script and running it.
1. **This repository builds no removal mechanisms** (2026-08-02). A boundary is enforced by where new
   files go, not by a retirement script for files already elsewhere.
1. **Tests cover the behavior of tools we wrote, and nothing else** (2026-08-05). "Is the hook wired
   in", "does the declaration agree with the lock table" and similar declaration-consistency checks
   are deleted on sight. A boundary check only earns a test if gutting our own logic would turn it
   red.
1. **Obsidian is optional** and the vault is one possible output directory, not a dependency.
1. **vpp must work with Bob, Forzare and the homelab all absent**, which is this item's
   done-means.

## What is already decided, and where it is recorded

| Source | What it already settles |
| --- | --- |
| `docs/remaining-work.md`, vpp section | Rust; collect synced Voice Memos; preserve originals; notes and summaries; pns for review alerts; Obsidian optional; Forzare keeps its post-modernization slot |
| homelab `PLAN-v12-experiments-backlog.md`, L-R5 | The same feature list, plus the sentence this item formalizes: "vpp owns application code; dotfiles owns Mac installation/configuration, homelab owns deployments, and the configured output directory owns the user's content" |
| homelab `PLAN-v12.md`, L6 | Open Notebook is a homelab deployment, and "must not create a second automatic Voice Memos capture/transcription workflow" |
| homelab `PLAN-v11.md`, Phase 6 | The existing transcription plan: ElevenLabs Scribe v2 plus whisply on `lash`, local faster-whisper fallback for outages or sensitive audio |
| Ivy `CLAUDE.md` | The `agent-processing-pipeline/` layout, audio excluded from git, and the `minutes` symlink precedent |
| Todoist `6hVpPJC2cjJW3V9M`, `6gjGcHp69phXmXj3` | The Mac workflow task and the broader transcription task. "Reuse existing transcription tasks" means these two, and this design files no new ones |

The ledger and L-R5 are one plan recorded twice. That duplication is fine while vpp has no repository
of its own and is a hazard afterwards, which the Assumptions section addresses.

## What exists on this machine today, measured

Measured on 2026-09-14 on dresden, because three of these facts change what the boundary has to say:

1. **The recordings are already on disk and need no privileged access.**
   `~/Library/Group Containers/group.com.apple.VoiceMemos.shared/` is mode `0700`, owned by the
   operator, with no `restricted` flag, and holds `Recordings/` (43 entries: 28 `.m4a` files, 8
   `.waveform` siblings, 5 subdirectories) alongside `CloudRecordings.db` with live `-shm` and `-wal`
   files. Whether macOS also guards that path with a Transparency, Consent and Control prompt was not
   measured, because this shell inherits the harness's own grants; that measurement belongs to the
   discovery design.
1. **`minutes` is installed and already claims part of this territory.** `~/.local/bin/minutes`,
   declared as a Homebrew cask in `.chezmoidata/system_packages_autoinstall.yaml`, whose `list` and
   `search` subcommands document "meetings and voice memos" with a `--content-type memo` filter, whose
   `vault` subcommand links an Obsidian vault, and whose output directory `~/meetings` is the target
   of the vault's `agent-processing-pipeline/minutes` symlink. Its own tool evaluation is still an
   open ledger item.
1. **`scalebar` is the precedent for exactly this boundary shape.** `webdavis/scalebar` is its own
   repository, cloned under `~/workspaces/Ivy/webdavis/scalebar`, and dotfiles owns only the
   installation and service configuration: `.chezmoidata/scalebar.yaml` declares `source_dir` and
   `install_dir`, `run_onchange_after_53-build-scalebar.sh.tmpl` builds the clone behind two
   deferrals (no clone, no Swift toolchain) so a machine without the clone still applies, and
   `Library/LaunchAgents/com.webdavis.scalebar.plist.tmpl` starts it at login.
1. **A new vpp LaunchAgent would not join the security pipeline's integrity arm.**
   `.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh` selects
   `~/Library/LaunchAgents/com.webdavis.osquery-*.plist` for the pipeline manifest and names exactly
   two built binaries, `pns` and `posture`. So `com.webdavis.vpp.plist` and a `vpp` binary in
   `~/.cargo/bin` are outside the manifested set, and adding vpp pages no CRIT and adds no
   full-apply coupling.
1. **pns needs nothing declared for a new producer.** `pns/crates/pns-protocol/fixtures/request-v1.json`
   carries `producer` as a free string, and `Signal::NeedsAttention` exists at
   `pns/crates/pns-protocol/src/request.rs:56`, so L-R5's `signal.kind: "needs_attention"` is real.
   `dot_config/pns/config-values.toml` declares plugins and gates, never producers.
1. **The vault will not carry the audio.** `~/workspaces/Ivy/.gitignore` excludes `*.m4a`, `*.mp3`,
   `*.wav`, `*.aac` and `*.flac`, and no machine backup exists yet: the ledger records the restic
   script as the operator's learning exercise, not built.
1. **A read-only SQLite open is available.** System `sqlite3` is 3.51.0 and accepts
   `file:<path>?mode=ro&immutable=1`, verified against a throwaway database in the scratchpad.

## The boundary question, stated precisely

Three questions hide inside "keep things in their own homes", and they have different answers:

1. **Where does the code live?** A repository decision, and the one the approaches below compare.
1. **Where does the machine-specific installation and service configuration live?** Settled by the
   existing rules: dotfiles, in the scalebar shape.
1. **Where does the content live, and which copy is canonical?** The interesting one, because the
   vault cannot hold the audio and nothing backs up a copy outside it.

## Approaches to the code home

### A. A fifth cargo workspace inside dotfiles

vpp joins `pns/`, `uu/`, `posture/` and `lights/` at the repository root, with its own `Cargo.toml`,
its own `Cargo.lock`, a bare-name entry in `.chezmoiignore`, and a `run_onchange_after_5*` builder
that compiles out of `.chezmoi.sourceDir`. Installable as
`cargo install --git https://github.com/webdavis/dotfiles vpp`.

Cheapest by a wide margin: every mechanism exists and is proven. It satisfies "its own project" in
this repository's own vocabulary, since the four workspaces are already independent projects that may
not depend on each other.

Against it: vpp's domain is larger than the other four (transcription engines, cloud credentials,
personal recordings, a vault writer), and its content-adjacent code and its plan documents would live
in the repository that configures the machine. Every apply also walks the new workspace, and chezmoi
already pays a measurable price for tree size. The four tools are in dotfiles "for now", with
extraction by `git subtree split` as the exit; starting vpp there means scheduling that extraction on
day one for a tool that has no history to preserve yet.

### B. Its own repository, built from a local clone by dotfiles (the scalebar shape)

`webdavis/vpp` from day one. dotfiles gains `.chezmoidata/vpp.yaml` (source and install paths), one
deferral-guarded builder, one LaunchAgent plist with its loader, and one config template. The clone
lives beside the other project clones under `~/workspaces/Ivy/webdavis/vpp`.

It matches the operator's two most recent rulings on this question (own repository on day one for the
Neovim plugins, tools are products others install), keeps personal-content code out of the machine
configuration repository, and copies a mechanism that is already load-bearing on this machine. The
deferral pattern means a machine without the clone still applies cleanly.

Against it: two repositories to move in step during early development, and a builder that has to
tolerate the clone's absence, which is more code than the unguarded four.

### C. Its own repository, installed by `cargo install --git` from a pinned tag

Same repository as B, but dotfiles never builds a working tree: a `run_onchange` script runs
`cargo install --git https://github.com/webdavis/vpp --locked --tag <pin>`, and the pin is the
declared value.

Least dotfiles code of the three, and it is what an outside user does. But it builds from the network
rather than the operator's own working tree, so local iteration needs a second path anyway, the pin
becomes a thing to bump, and a network outage turns into an apply failure or another deferral. The
machine compiles either way, so the saving is smaller than it looks.

### Recommendation

**B**, with C available later as the install path for anyone else and as the fallback if the
two-repository dance turns out to cost more than it saves. B is the only option that satisfies both
recent rulings without a scheduled extraction, and its mechanism is already running on this machine
for scalebar, which means the risky part is copied rather than invented.

## The recommended design

### Four homes, one sentence each

| Home | Owns | Never holds |
| --- | --- | --- |
| `webdavis/vpp` | All application code, its own config schema and defaults, its own tests and fixtures, its own design documents and runbooks, its `--help` text | Any path into this checkout, any dotfiles-only assumption, the operator's recordings or notes |
| `webdavis/dotfiles` | Installation and service configuration for THIS Mac: the source and install path declarations, the builder, the LaunchAgent plist and its loader, the rendered config file with secrets from KeePassXC, runtime package declarations, the fresh-machine notes | vpp source code, vpp domain logic, vpp output |
| The configured output directory (`~/workspaces/Ivy` for this operator) | The operator's content: notes, transcripts and the links between them, under the existing `agent-processing-pipeline/` layout | Application code, machine configuration, committed audio |
| `webdavis/homelab` | Any server-side deployment vpp may optionally use: Open Notebook (L6), a remote transcription host, credential brokering | Anything vpp needs in order to run on the laptop |

### What dotfiles gains, file by file

Named so the change can be reviewed before it is written, and so nothing else gets smuggled in:

1. `.chezmoidata/vpp.yaml`, declaring `source_dir` (`workspaces/Ivy/webdavis/vpp`) and `install_dir`
   (`.cargo/bin`, read from `rust_tools.yaml` if the operator prefers one declaration for every Rust
   binary). Home-relative, because launchd resolves no `~`.
1. `.chezmoiscripts/run_onchange_after_5X-build-vpp.sh.tmpl`, the scalebar builder with `cargo`
   substituted for `swift`: guarded `glob`/`stat` hashes over the clone's sources and manifests, a
   retry marker so a deferred build retries on the next apply, and two deferrals (no clone, no cargo)
   that exit 0 with one reported line.
1. `Library/LaunchAgents/com.webdavis.vpp.plist.tmpl` plus its `run_onchange_after_*` loader, if vpp
   runs on a schedule or as a watcher. Absolute paths built from the declared install directory.
1. `dot_config/vpp/private_config.toml.tmpl`, the machine's configuration: output directory, archive
   directory, engine selection, and any API key pulled with `keepassxc`. `private_` because a
   rendered secret must not be world-readable.
1. Additions to `.chezmoidata/system_packages_autoinstall.yaml` for runtime dependencies vpp shells
   out to rather than links against (`ffmpeg` is the likely one), alphabetically, after installing
   them by hand first.
1. One paragraph in `docs/runbooks/macos-fresh-machine-quickstart.md` if a Transparency, Consent and
   Control grant turns out to be needed, because that runbook already owns the grant list.

Not in dotfiles: no `vpp` entry in `.chezmoiignore` (the clone is outside the source tree, so there
is nothing to exclude), no vpp workspace, no vpp tests, no vpp documents beyond the ones this wave
already wrote and the migration note below.

### What the content boundary says

The vault cannot hold the audio, and no backup exists, so "preserve the original audio" has to be
answered with a specific copy rather than a folder name:

1. **Apple's container stays the canonical original** and is read-only to vpp. vpp never moves,
   rewrites, renames or deletes anything inside it, and any SQLite read there opens
   `file:<path>?mode=ro&immutable=1` so that a plain read connection cannot create journal files
   inside Apple's directory.
1. **vpp keeps one archive copy** of the original bytes, byte-identical and with its capture
   metadata, in a configured archive directory. Default: outside the vault and outside git, attached
   to the vault by a symlink if the operator wants it visible there, which is exactly what `minutes`
   already does with `~/meetings`.
1. **The vault holds notes and links**, git-tracked Markdown under `transcripts/` and `analysis/`,
   each note naming the recording identifier and the archive path. Frontmatter key names come from
   configuration, not from code, so the vault's schema (`hub`, `status`, `startDate`, `description`)
   is this operator's configuration rather than vpp's contract.
1. **vpp writes only under its configured directories** and creates a missing output directory only
   when its parent already exists. A typo'd path fails with the path in the message instead of
   building a tree somewhere unexpected.

### Behaviors worth pinning, and the check that pins each

These are boundary behaviors of code we own, which is what earns a test under the 2026-08-05 scope
ruling. Each is written as the sentence the test asserts, with the failure it catches.

1. **vpp builds with dotfiles absent.** Copy the workspace alone into an empty directory and run
   `cargo build --locked --release` with no `pns`, `posture` or dotfiles checkout on the filesystem.
   Catches the path dependency and the "just read the repo's data file" shortcut. This is the same
   proof the 2026-09-10 un-sharing used, so the method is established.
1. **vpp completes a run with pns absent.** With nothing named `pns` on `PATH`, a discovery through
   note pass finishes, the review item is recorded in vpp's own state, and the missing notifier is
   one log line, not an error exit. Catches a notification path that became a hard dependency.
1. **vpp completes a run with every network destination unreachable.** A local-engine configuration
   produces a note; a cloud-engine configuration fails with a named engine and a named reason and
   leaves the recording queued rather than consumed. Catches silent data loss on an outage and the
   accidental homelab dependency.
1. **vpp completes a run with no vault and no Obsidian.** Point the output directory at an empty
   temporary directory: the notes are portable Markdown, and no vault-specific syntax is emitted.
   Catches Obsidian becoming a requirement.
1. **vpp leaves the source container untouched.** Inventory the container (names, sizes, modification
   times, including the SQLite sidecars) before and after a full run and compare. Catches both an
   accidental write and a read-write SQLite open.
1. **The apply defers instead of failing when the clone or the toolchain is missing.** Render the
   builder with `CI=1 chezmoi --source "$PWD" execute-template --no-tty < <script>` and run it with
   the clone absent: exit 0, one reported deferral line, retry marker created. Catches the fresh
   machine whose whole configuration stops because one optional tool has no clone. This is a dotfiles
   test and mirrors the scalebar shape.

Numbers 1 through 5 live in the vpp repository and run in its own suite. Number 6 lives here.

### Configuration boundary

vpp ships its own defaults with every option present and set to its default value, per the
2026-08-31 ruling that defaulted keys ship uncommented at their default. dotfiles' rendered config
carries only what differs on this machine plus the KeePassXC lookups.

The config file is a hand-written chezmoi template to start with. pns's generated template (values
file plus `pns-config-render` plus a byte-equality test) exists because a hand edit to a large
generated file drifts silently; that machinery is worth copying when vpp's configuration grows past
a handful of options, and not before.

### Failure modes

| Situation | Behavior |
| --- | --- |
| Clone or cargo missing at apply time | Builder defers, exit 0, retry marker, apply continues |
| Output or archive directory's parent missing | vpp refuses with the resolved path in the message, creates nothing |
| pns missing or failing | Log line, run continues, review state kept in vpp |
| Transcription engine unreachable | Configured fallback, provenance records which engine ran; no fallback configured means the recording stays queued |
| Homelab down, Open Notebook absent | No effect on a laptop run; the handoff is optional by design (L6's own constraint) |
| Bob or Forzare absent | No effect; they are consumers of vpp's output, never inputs |
| Interrupted iCloud sync | Discovery design's problem, not this one, but the boundary rule is that a partially synced file is skipped rather than half-ingested |
| vpp uninstalled | Notes and archive stay where they are, readable without vpp; nothing in the vault depends on the binary |

### Security

1. **Secrets come from KeePassXC at apply time** into a `private_` rendered config, mode 0600. No
   secret reaches a command line: the ledger already records moshi's pairing script putting its token
   in argv as a defect to avoid, and argv is world-readable through `ps`.
1. **Egress is explicit.** A cloud transcription engine sends the operator's private recordings to a
   third party, so the engine per sensitivity class is a configuration decision the operator makes,
   with a local engine available. The redacted-sharing draft is a separate ledger item and does not
   soften this.
1. **No privilege.** No sudo, no root helper, no privileged LaunchDaemon. The recordings are
   readable as the user (mode 0700, owned by the operator), so nothing here needs to be.
1. **Read-only at the source**, as specified above, including the SQLite open mode.
1. **A notification is not a review.** Review state lives in vpp; a delivered pns page proves
   delivery, never that the operator corrected anything.

### Proving the independence claim, which is this item's done-means

The done-means asks for "a recorded boundary with vpp shown to work with Bob, Forzare and the
homelab all absent". Bob and Forzare do not exist yet, so the honest form of that proof is the
absence test, not a mock: behaviors 1 through 4 above, run on a machine where Bob and Forzare have
never been installed, with the homelab off the network for the duration. Record the four exit codes
and the produced note in the ledger entry. If any of them needs a stub to pass, the boundary is
already broken and the stub is hiding it.

## Out of scope

1. The discovery mechanism, the sync-completion test and the deduplication key (its own item).
1. Engine selection, redundant transcription and disagreement flagging (its own item).
1. The metadata schema, tags, relationships and filing rules (its own item).
1. Meeting briefs, calendar and Todoist inputs (its own item).
1. The redacted sharing draft, retention and summary format (its own item).
1. Whether `minutes` is adopted, replaced or ignored (belongs to the reconcile item, though this
   document raises it because `minutes` already owns a directory inside the vault).
1. Open Notebook's deployment and the handoff format (homelab L6, plus its own ledger item).
1. Forzare's schedule, which the 2026-09-14 ruling already fixed as after everything except SP8.
1. Any removal of files that already exist elsewhere. This repository builds no removal mechanisms.

## Assumptions made in the operator's place

Each of these is a decision that had to be made to write the document, with the alternative that was
rejected and what it would cost to switch.

1. **vpp is its own GitHub repository from day one.** Alternative: a fifth workspace in dotfiles,
   extracted later with `git subtree split`, which is cheaper this week and schedules a migration.
   Switching later costs one subtree split plus rewriting the builder; switching from B to A costs
   about the same in reverse.
1. **The name stays `vpp` for now.** Alternative: choose a function name before the repository
   exists. Two reasons to reconsider: `VPP` is taken by FD.io's Vector Packet Processing, verified on
   fd.io, which is the first search result anyone will find, and the repository's own rules avoid
   less-common acronyms entirely. The operator's own naming memories ask for self-documenting,
   user-agnostic names. Renaming after install instructions circulate is a breaking change, so this
   is the cheapest decision to make now and one of the more expensive to defer.
1. **dotfiles builds from a local clone, in the scalebar shape.** Alternative: `cargo install --git`
   from a pinned tag, which removes the clone dependency and adds a pin to bump and a network
   dependency at apply time.
1. **Apple's container stays the canonical original; vpp keeps one archive copy outside git.**
   Alternative: treat the copy in the vault's gitignored `raw/audio/` as the archive, which puts the
   audio where the notes are at the cost of one unbacked copy on one machine until a backup exists.
   A third option, tracking audio in git, contradicts the vault's own ignore rules.
1. **Notes go into the existing `transcripts/` and `analysis/` directories rather than a new `vpp/`
   subtree.** Alternative: give vpp its own subtree, which reads more cleanly beside the existing
   `minutes` symlink and against "reuse the existing vault layout" in the ledger.
1. **pns notification is opt-in in vpp's configuration and degrades to a log line when pns is
   absent.** Alternative: require pns, which would contradict the shippable-product rule and fail
   behavior 2.
1. **Frontmatter key names are configuration, not code.** Alternative: hardcode the vault's schema,
   which is faster and makes the tool this operator's only.
1. **The design documents written in this wave stay in `docs/superpowers/specs/` until the vpp
   repository exists, then move there with a pointer left behind.** Alternative: keep every vpp
   document in dotfiles permanently, which contradicts this document's own boundary.
1. **No new Todoist tasks.** The existing `6hVpPJC2cjJW3V9M` and `6gjGcHp69phXmXj3` are the
   transcription tasks this work reuses, per the ledger's instruction.
1. **`minutes` is left exactly as it is.** It already lists and searches voice memos and already owns
   `agent-processing-pipeline/minutes`. This document neither wires it in nor removes it, and only
   requires that vpp not write into that symlinked tree.

## Open questions for the operator

**Decided 2026-09-15:** question 5 below is answered. `minutes` is out: vpp does not use or depend on it
in any form. See `docs/decisions/2026-09-15-vpp-architecture-decisions.md`, decision 1. The remaining six
questions are untouched by that decision and stand as written.

1. **Own repository, or a fifth workspace in dotfiles?** Recommendation: own repository
   (`webdavis/vpp`). This is the one answer everything else in the document hangs from.
1. **Does the tool keep the name `vpp`?** The acronym is taken by a well-known networking project and
   the repository's own rules discourage introducing uncommon acronyms. Recommendation: pick the
   shipping name before the repository is created.
1. **Which copy of the audio is canonical, and is one unbacked copy acceptable until a backup
   exists?** Recommendation: Apple's container is canonical, the archive copy lives outside git, and
   a real backup stays a separate ledger item rather than a vpp feature.
1. **Do vpp's notes share `transcripts/` and `analysis/` with everything else, or get their own
   subtree?** Recommendation: share them, since the ledger asks for the existing layout, with a
   per-note provenance field.
1. **Is `minutes` in or out?** It is installed, declared, already symlinked into the vault, and
   already handles voice memos. If it is in, vpp's scope shrinks; if it is out, its ledger evaluation
   should record that vpp supersedes it. Recommendation: answer this before vpp's ingestion design is
   approved, because it can remove a whole layer. **Decided 2026-09-15: out.** vpp does not use or depend
   on `minutes`, in any form. The operator's reasoning: `minutes` is poorly designed, though it has good
   features worth learning from. See `docs/decisions/2026-09-15-vpp-architecture-decisions.md`,
   decision 1.
1. **Do the vault's folder-note and frontmatter conventions apply to machine-written notes, and who
   maintains the folder note for a directory a tool writes into?** This is a vault-governance
   question that vpp's output format depends on.
1. **Should vpp's binary, configuration and LaunchAgent join posture's user-configured watch list?**
   They are outside the osquery known-good manifests by default, verified above, so this is an
   addition the operator opts into rather than a consequence.
