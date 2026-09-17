# Reconciling the posture approval interface against tonight's contracts

Status: reconciliation, written 2026-09-17. Nothing was built and nothing was changed. No Rust file,
no route, no allowlist entry and no line of `docs/remaining-work.md` was touched. Every choice made
in the operator's place is listed under "Assumptions".

This re-reads the two pieces of PR #24's design that the ledger still carries, against the code as it
stands tonight rather than against the code of 2026-09-14:

1. tap-to-approve from the phone, scoped to pending findings;
2. an `/osquery allow|deny|list` skill on the hermes side.

`docs/superpowers/specs/2026-09-14-osquery-approval-authority-design.md` proposed a shape for both.
This document does not repeat that proposal. It answers the four narrower questions the ledger asks,
and it retires what the current contracts will not carry. Three things changed under that earlier
document and each changes an answer: the `explain` agent route now exists, the webhook agent sandbox
is now declared empty, and the `posture` route's 404 is gone because the route was renamed and
declared.

## What the ledger asks

> Reconcile the approval interface separately: Butters tap-to-approve scoped to pending findings and
> the `/osquery allow|deny|list` Hermes skill. Verify the current posture command and trust contracts;
> investigation must not grant the analyst approval authority.

The hard constraint is the last clause. An agent that looks at a finding must not be able to approve
it. A design that lets an investigation path reach the allow path is wrong however convenient it is.

## The posture command surface, verified

`posture` is one binary with one argv dispatcher. The whole surface is declared in
`posture/crates/posture/src/lib.rs:18-24`: `alert`, `poll`, `funnel`, `watchdog`, `digest`,
`heartbeat`, `converge`, `doctor`, `enrich`, `ssh`, and `allowlist add|deny|list`.

The allowlist subcommand parses three words and nothing else, in
`posture/crates/posture/src/allowlist.rs:58-60`, and an unrecognised word prints usage and exits 2 at
`posture/crates/posture/src/allowlist.rs:61-64`. `add` and `deny` both take one label; `list` takes
none.

`list` is already a pure read. It returns from
`posture/crates/posture-application/src/allowlist.rs:46-52`, which is before the write lock is
acquired on line 55, and it reads the deployed file straight off disk
(`posture/crates/posture-adapters/src/allowlist_file.rs:97-99`, which calls `listed_bytes` at lines
8-24). No lock, no `chezmoi`, no privileged call.

`add` and `deny` share one path after the lock:

- the label is validated in `posture/crates/posture-domain/src/allowlist.rs:63-76`, which requires an
  alphanumeric first byte, a restricted character set, and refuses `com.apple` and anything under it;
- `add` captures the live launchd agent before it writes anything
  (`posture/crates/posture-application/src/allowlist.rs:59-67`), so a label with no installed agent
  is refused rather than granted;
- `deny` refuses a label the source does not already carry, returning `NotPresent`
  (`posture/crates/posture-application/src/allowlist.rs:69-71`), which is why `deny` means undo an
  allow and not decline a finding;
- the captured agent's plist is pinned by its SHA-256 (secure hash algorithm, 256-bit) digest, carried
  into the entry at `posture/crates/posture-application/src/allowlist.rs:92`.

## The trust contracts, verified

### Who writes the allowlist

`posture allowlist add|deny` is the only writer in this repository. It writes the chezmoi SOURCE file
by rename, applies that one target, and then refreshes the root-owned known-good manifest, all in
`posture/crates/posture-adapters/src/publisher.rs:45-95`. The apply is
`chezmoi apply --force <deployed>` (lines 47-57) and the manifest refresh runs
`run_after_05-osquery-known-good-manifests.sh` through `/bin/bash` (lines 80-95). Publication carries
a fifteen-minute budget explicitly sized for a terminal password wait
(`posture/crates/posture/src/allowlist.rs:15`).

### What the lock is, and what it is not

`AllowlistWriteLock` is a blocking `flock(LOCK_EX)` on `<deployed>.lock`, opened append plus create
with `O_CLOEXEC`, in `posture/crates/posture-adapters/src/locks.rs:23-41`. It is mutual exclusion
between two concurrent curations. It records no identity, no timestamp and no verb, and it holds
nothing after the guard drops. **The thread this repository has around `AllowlistWriteLock` is a
concurrency contract, not an audit trail.** Nothing on this host records who granted a suppression, or
when, outside the git history of the chezmoi source file.

### What an approval can actually suppress

The allowlist is consulted in exactly one branch of the gate:
`posture/crates/posture-domain/src/gate.rs:136-145`, the `PersistenceLaunchd` detector. Everything
else in that `match` reaches its outcome without asking. Inside that branch the allowlist is only
reached when the action is `Added`, the path is not under `/System/Library/`, and the path does not
contain `/LaunchDaemons/`. A LaunchDaemon pages whether or not it is allowlisted.

So the universe of approvable findings is one detector and one action, bounded to user LaunchAgents.
That is a code fact, not a policy, and it is the most useful scoping the design has available.

Suppression also spends a vouch before it takes effect
(`posture/crates/posture-domain/src/allowlist.rs:49-60`): the entry's pinned hash must match the
current plist, or where the entry carries no hash the plist itself must be vouched for by the
manifest, and the allowlist file must be vouched for too. A hand-edited allowlist suppresses nothing.

### The grant is silent, and the writer is what silences it

The allowlist file is itself watched, as `FileCategory::AllowlistFile` in
`posture/crates/posture-domain/src/gate.rs:152-159`, and its verdict comes from
`integrity_verdict` in `posture/crates/posture-domain/src/integrity.rs:16-39`. A write whose new
content the manifest vouches for returns `LogOnly` (lines 34-35). The publisher refreshes that
manifest as its last step, so a grant made through the writer produces no page at all. A hand edit
would page; the supported path does not.

This matters more than it reads. There is no announcement on grant anywhere in posture today. The
2026-09-14 document proposed one and it has not been built.

### Privilege

`.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh:457` and `:460` install the refreshed
manifest with `sudo install`, and that file's own note at lines 114-116 records that the operator
account has passwordless sudo. Measured tonight: `sudo -n true` exits 0. Any process already running
as the operator is root on request, which is the fact that decides both questions below.

## What a pending finding is in the code: nothing

There is no pending-findings concept in posture, in pns, or in the state tree. Three code facts
together:

- a finding is a borrowed transient. `PageFinding` in
  `posture/crates/posture-domain/src/page.rs:62-76` borrows out of one results-log row for the length
  of one judging pass and carries no identifier;
- the only thing the alerter persists about its progress is a cursor, an inode plus an offset, written
  by rename in `posture/crates/posture-adapters/src/results_cursor.rs:32-58`. Once the cursor moves
  past a row, nothing can name that row again;
- the digest spool is not a substitute. `posture/crates/posture-adapters/src/judge_batch.rs:96-103`
  spools only `GateOutcome::Digest` rows; a `GateOutcome::Page` row goes into the page and is never
  spooled. The spool's own header says the same thing
  (`posture/crates/posture-adapters/src/digest_spool.rs:3-7`). **The spool holds exactly the findings
  that did not page, which is the complement of the set an approval would act on.**

This retires one assumption of the 2026-09-14 document, which proposed deriving the pending set from
"the results log plus the deployed allowlist plus a live launchd capture" over a window defaulting to
the digest's own day. The results log is a rotating append-only file read by inode and offset; a
bounded re-read of it can reconstruct candidate rows, but it cannot tell a row that paged from a row
that was log-only without re-running the gate, and it cannot tell a row already acted on from one that
was not, because no record of an action exists.

## The hermes side, as of tonight

Six webhook routes are declared whole by `private_dot_hermes/modify_private_config.yaml:181-188`:
`general`, `pns-events`, `priority`, `posture-pages`, `uu-runs` and `explain`. Five get
`deliver_only: true` at line 204; `explain` declares `agent: true` and therefore omits it, which is
what makes the gateway hand the posted page to an agent and deliver the agent's reply.
`.chezmoiscripts/run_after_68-hermes-log-route-status.sh.tmpl:63-73` carves `explain` out of the
route-status check by giving it its own expectation, and lines 99-113 report a wrong mode in either
direction.

posture posts its own pages. `dot_config/posture/private_config.toml.tmpl:53` sets
`mode = "hermes"`, line 61 sets the default route to `posture-pages`, and line 94 copies a critical
page to `critical_copy_route = "explain"`. The `posture` route's 404 recorded on 2026-09-14 is gone
with the rename.

Two sandbox facts decide the rest.

**The webhook agent sandbox is empty.** `private_dot_hermes/modify_private_config.yaml:227` declares
`platform_toolsets.webhook` as the single sentinel `["no_mcp"]`, and
`~/.hermes/hermes-agent/hermes_cli/tools_config.py:1589-1601` is the code that reads it: the sentinel
sets `explicit_mcp_servers` to the empty set and subtracts every enabled Model Context Protocol server
from the passthrough. The live config agrees (`platform_toolsets.webhook: [no_mcp]`). So the `explain`
agent, which is the investigation path that exists today, has no terminal, no file tool and no Model
Context Protocol server. **Tonight, the investigation path is fenced from the allow path by an empty
toolset rather than by prose.** That is a real boundary and it is the one worth keeping.

**The Discord agent sandbox is not empty.** The live `platform_toolsets.discord` list carries `web`,
`terminal`, `file`, `skills`, `memory`, `session_search`, `clarify`, `delegation`, `cronjob`,
`discord`, `discord_admin`, `kanban` and `qmd`. `terminal.backend` is `local` and
`terminal.auto_source_bashrc` is `true`, and `dot_bashrc.tmpl:157` prepends `~/.cargo/bin`, where
`posture` is installed. Measured tonight against the installed hermes:
`tools.approval.detect_dangerous_command("posture allowlist add com.webdavis.example")` returns
`(False, None, None)` and `tools.tirith_security.check_command_security` on the same string returns
`{'action': 'allow', 'findings': [], 'summary': ''}`. With no warnings collected,
`~/.hermes/hermes-agent/tools/approval.py:1624-1626` returns approved with no prompt.

So a hermes Discord agent can already run the writer, unprompted, and the grant it makes is silent.

## Answers

### Is tap-to-approve reachable at all, and by which path

No, not as designed, and the blocker is not the trust boundary. It is the delivery surface.

A page reaches Discord through a `deliver_only` route, and `deliver_only` delivery is
`_direct_deliver` in `~/.hermes/hermes-agent/gateway/platforms/webhook.py:897-923`: it takes a
`content` string and hands it to a cross-platform dispatcher. There is no components or view
parameter anywhere on that path. A delivered page cannot carry a button.

The one button surface hermes owns is `ExecApprovalView`
(`~/.hermes/hermes-agent/plugins/platforms/discord/adapter.py:5797-5806`, instantiated at
`:4726-4730`). It is constructed with a `session_key` and an allowed-user set, its four buttons are
Allow Once, Allow Session, Always Allow and Deny, and clicking one unblocks a waiting agent thread. It
approves a COMMAND an agent has already proposed inside a live session, not a FINDING. It is reached
only when `detect_dangerous_command` or tirith flags that command, and `posture allowlist add` flags
neither, measured tonight.

Making a page carry an approve button therefore means modifying third-party code, which the repository
forbids. Reusing `ExecApprovalView` means routing approval through an agent session, which is the exact
coupling the ledger's constraint forbids: the thing that would tap the button is an agent, and the
thing being approved would be that agent's own command.

There is no path. Tap-to-approve from the phone is not reachable.

### What "scoped to pending findings" has to mean in code

Three properties, and the current code supplies one of them.

1. **Scoped to the one detector the allowlist can affect.** Already bound, in
   `posture/crates/posture-domain/src/gate.rs:136-145`: `PersistenceLaunchd`, action `Added`, not
   under `/System/Library/`, not a LaunchDaemon. This is free and it is real.
2. **Scoped to one identity, not one label.** A grant must pin the label, the plist path, the program
   and the plist's hash together, and that is what the entry already carries
   (`posture/crates/posture-application/src/allowlist.rs:83-94`), enforced on every later read by
   `posture/crates/posture-domain/src/allowlist.rs:46-52`, where a changed path or program returns
   `ReusedLabel` rather than `Suppress`. A label whose plist is later swapped pages again. **This is
   the property that keeps approving one finding from widening into approving a class, and it already
   holds.** Any interface that lets a caller supply the path, the program or the hash instead of
   capturing them from the live launchd table breaks it, which is why `add` refuses a label with no
   installed agent.
3. **Scoped to a finding that is actually outstanding.** This does not exist and cannot be added
   without new persisted state. There is no record that a finding paged and no record that anyone acted
   on one; see "What a pending finding is in the code" above. A scope check of this kind needs an
   append-only decision record written by the writer, keyed by the same four-part identity, which is a
   new file in the state tree and the only genuinely new thing either piece would need.

Property 3 is also the only one that would make an approval interface safer than the bare command. 1
and 2 bind every caller already.

### Can `/osquery allow|deny|list` exist without giving investigation a route to trust

Not on this host as configured, and the reason is that the skill would grant nothing that is not
already granted.

`platform_toolsets.discord` includes `terminal`, `terminal.backend` is `local`, the operator account
has passwordless sudo, `posture` is on the PATH a hermes shell sources, and
`posture allowlist add` matches no dangerous pattern and no tirith rule. Every Discord hermes agent
already holds unconditional authority to write the allowlist and to refresh the manifest that makes
the write invisible. A skill named `/osquery allow` would be a label on a door that is standing open.

Two consequences follow, and they point in opposite directions from the ones the design assumed.

- **The `allow` and `deny` verbs cannot be made safe by designing the skill.** Adding the skill does
  not widen the boundary and removing the skill does not narrow it. The boundary is
  `platform_toolsets.discord`, and only a change there, or a change to `terminal.backend`, moves it.
- **The `list` verb is a different animal.** It is a pure read with no lock and no privilege
  (`posture/crates/posture-application/src/allowlist.rs:46-52`), of a file at mode 0600 in the
  operator's own home. A read-only surface adds no authority to a platform that already has `terminal`,
  and it is the only third of the design that the trust boundary has no opinion about.

Stated plainly, as the ledger asks for: **`/osquery allow|deny` is not buildable without breaking the
trust boundary, and it is not buildable as a fence either, because the boundary it would fence is
already open. The boundary is the Discord platform toolset, not the skill.**

One more thing worth naming, because it is the live hazard rather than a hypothetical. The `explain`
agent route is fenced by `["no_mcp"]`. A future investigator moved onto the Discord platform, or a
webhook toolset edited away from the sentinel, inherits `terminal` and therefore inherits the allow
path in the same session in which it reads attacker-chosen launchd labels and file paths. The
sentinel is also fragile in a specific way the template records at
`private_dot_hermes/modify_private_config.yaml:225-227`: saving from the `hermes tools` picker drops
it, because the picker has no checkbox for it, and it stays dropped until the next apply.

### The smallest honest version of each piece

**Tap-to-approve.** The smallest honest version is not an approval at all. It is one more line on the
page that already arrives: the exact `posture allowlist add <label>` command, with the label the
finding carries, rendered by the page builder so the operator can paste it into a terminal they
already trust. The tap becomes a copy. Nothing new is stored, no route changes, no button exists, and
the authority stays where it is. The next honest step up, if the operator wants one, is the decision
record from property 3 above plus an announcement through pns on every grant and every denial, which
buys detection rather than prevention and is worth naming as such.

**The skill.** The smallest honest version is `list` alone, and even that is worth building only if the
operator wants an allowlist readout in Discord, because it answers a question an agent with `terminal`
can already answer. If it is built, it should be built as a deterministic command that runs
`posture allowlist list` and posts the output, never as a prompt that asks an agent to run a posture
command, because the second shape teaches the model that the binary is available and reachable.

## Dispositions

**Piece 1, tap-to-approve scoped to pending findings: NOT BUILDABLE WITHOUT BREAKING THE TRUST
BOUNDARY.** Two boundaries, either one sufficient. The delivery boundary: a `deliver_only` route posts
a string and hermes's only button view approves a command inside an agent session, so a page cannot
carry an approve button without modifying third-party code
(`~/.hermes/hermes-agent/gateway/platforms/webhook.py:897-923`,
`~/.hermes/hermes-agent/plugins/platforms/discord/adapter.py:5797-5806`). The investigation boundary:
every route that could carry a tap is served by an agent, so the tapper is the investigator. Retire
the piece. A smaller, different thing survives it, the paste-ready command line on the page, and that
is not the same design.

**Piece 2, `/osquery allow|deny|list`: BUILDABLE SMALLER, and only the `list` third.** What shrinks:
both write verbs go. They are not refused because a skill would be unsafe; they are refused because
they add nothing to a Discord platform whose toolset already carries `terminal` with
`backend: local` against an account with passwordless sudo, and because the writer's own manifest
refresh makes a grant silent. What survives is a deterministic, chezmoi-managed read-only command over
`posture allowlist list`, and it is optional.

**The boundary that actually needs a decision** is neither piece. It is `platform_toolsets.discord`.
Until `terminal` leaves it, or `terminal.backend` stops being `local`, investigation and approval are
the same authority on this host, and no interface design above that line changes the answer.

## Assumptions

Each is a choice made in the operator's place, with its alternative.

1. **A skill on the hermes side means a Discord-platform surface.** The ledger says "the hermes side"
   and Discord is where posture's pages land. Alternative: the command-line interface platform, whose
   toolset also carries `terminal`, which does not change any answer here.
2. **`platform_toolsets.discord` is read as intended rather than as drift.** It is not declared by
   `private_dot_hermes/modify_private_config.yaml`, which writes only the webhook entry at line 227, so
   the Discord list is live configuration hermes owns. It is treated as the operator's standing choice.
   Alternative: treat it as drift worth declaring, which is a separate change with its own blast radius
   across every Discord agent.
3. **"Investigation" means the `explain` agent route and any successor to it.** That is the only
   investigation path that exists tonight. Alternative: read it as the recovered security
   investigator from PR #24, which is not built and which this document only mentions as the thing the
   sentinel currently protects.
4. **The paste-ready command line is offered as the smaller honest version, not designed here.** It
   touches the page builder, which is a Rust file this document may not change. Alternative: leave the
   page as it is and treat the whole piece as retired.
5. **No new persisted state is proposed.** The decision record that property 3 would need is described
   and left unbuilt, because its shape depends on the operator's answer about detection versus
   prevention. Alternative: specify it now and have it discarded if authority moves.

## Open questions

1. Does `terminal` stay in `platform_toolsets.discord`? Every other question here is downstream of it.
   Leaving it means investigation and approval remain one authority; removing it narrows every Discord
   agent at once, which is a behaviour change well outside this reconciliation.
2. Is detection enough? A grant through the writer is silent today. An announcement through pns on
   every grant and denial is cheap and does not prevent anything. Prevention on this host means a
   one-time code an agent cannot read, from the password vault.
3. Is the `list` third wanted at all, given that an agent with `terminal` can already read the file?
4. Should the page carry the paste-ready `posture allowlist add <label>` line? It is the only surviving
   fragment of tap-to-approve, and it is a page-builder change rather than an interface.
5. Does the `hermes tools` picker hazard get a guard? The `["no_mcp"]` sentinel is the entire fence
   around the investigation path, and a picker session drops it until the next apply.
