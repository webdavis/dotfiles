# Harness wait events and the answered-wait race

Status: design, written 2026-09-14. NOT approved, and no code was written or changed for it. Every
choice made in the operator's place is listed under Assumptions with the alternative it beat, and the
decisions the operator owes are the last section.

## Why this exists

One ledger item covers three backlog rows:

> Resolve the related B6/B20/B39 hook design: the answered-wait race, when `AskUserQuestion` should
> arm a waiting indicator and what its notification contains, and alerts for sandbox network approval
> requests. The `AskUserQuestion`-specific `asked` wiring runs after the tool completes, and network
> permission waits remain explicitly uncovered. Inspect current harness events and agree behavior
> before changing hooks; a prompt-only guess does not establish an actual permission wait.

The three rows, as filed:

| Row | Filed as |
| --- | --- |
| B6 | the elicitation-answered sidecar, with the older-answer residual named |
| B20 | `asked` and `plan-ready` fire on PostToolUse, so the blue marker arms after the answer |
| B39 | sandbox network permission requests have no repository-managed alert or nag |

The investigation changed the shape of all three. B20's premise turns out to be half wrong, B6's
sidecar turns out to be unnecessary, and B39 turns out to have zero exposure on this machine today.
The rest of this document is what the evidence says instead.

## What a wait is in this system, before anything changes

A wait is one file. `~/.local/state/pns/lights-blocked/<session id>` holds the second the wait was
armed, written through `update_blocked_marker`, and the blocked lamp is on while any such file is
live. Four event words arm one and every other word ends one, which is
`pns_domain::lights::phase::blocked_marker_action` reading `pulse::LAMP_BLOCKED`:

```
LAMP_BLOCKED = ["blocked", "asked", "plan-ready", "denied", "asking"]
```

Three bounds sit around it. `[lights.blocked] give_up_after_secs = 57600` (sixteen hours) is the
backstop that expires an abandoned wait. `[nag] after_secs = 300` is when an unanswered approval is
carded again. Config load refuses a backstop shorter than the nag schedule, because that would give
up on a wait before ever nudging about it.

Three things end a wait today: `prompt` (the operator typed), `resolved` (the tool batch came back),
and any other event from that session, whose Stop is the last arm to get there. None of them is the
operator's answer. `update_blocked_marker`'s own doc comment says so plainly, and names the residual:
"the marker clears at the NEXT event from that session, never at the instant the operator answered,
because no event reports the answer itself."

That last clause is the sentence this design falsifies.

## What Claude Code 2.1.270 actually offers

Verified against the installed package rather than from memory: the hook event vocabulary was read
out of the bundle inside `/opt/homebrew/Caskroom/claude-code@latest/2.1.270/claude`, where it appears
as a literal list, together with the input schema of each event. Thirty-four events:

```
PreToolUse, PostToolUse, PostToolUseFailure, PostToolBatch, Notification, UserPromptSubmit,
UserPromptExpansion, SessionStart, SessionEnd, Stop, StopFailure, SubagentStart, SubagentStop,
PreCompact, PostCompact, PreModelSwitch, PostModelSwitch, PermissionRequest, PermissionDenied,
Setup, TeammateIdle, TaskCreated, TaskCompleted, Elicitation, ElicitationResult, ConfigChange,
WorktreeCreate, WorktreeRemove, InstructionsLoaded, CwdChanged, FileChanged, DirectoryAdded,
MessageDisplay
```

Several are newer than the survey the current declarations were written against, and two of them
matter here: `ElicitationResult` and `PostToolUseFailure`. The exact count of additions was not
established, because the declarations cite two different earlier versions.

### Which events can observe a real wait

This is the table the ledger item asks for. "Real wait" means the event fires while the operator has
not answered yet, so a lamp armed from it is telling the truth at the moment it lights.

| Event | When it fires | Real wait | Wired today |
| --- | --- | --- | --- |
| `PermissionRequest` | before the prompt is drawn, and awaited | yes, the seam | `blocked` |
| `Elicitation` | before the dialog, and a decision is read | yes | `asked` |
| `Notification` | ~6s after a dialog with no activity | yes, subject unknown | quota only |
| `PostToolUse` | after the tool returns | no, it is the ANSWER | `asked`, `plan-ready` |
| `ElicitationResult` | after the operator responds | no, it is the ANSWER | nothing |
| `PostToolBatch` | after a batch fully resolved | no, batch resolution | `resolved` |
| `UserPromptSubmit` | the operator typed | no, but answers one | `prompt` |
| `PermissionDenied` | after the classifier refused a call | no, nobody waits | `denied` |
| `SubagentStop` | a subagent finished | no, but bounds one | nothing |
| `TeammateIdle` | a teammate session went idle | out of scope | nothing |
| `Stop`, `StopFailure` | the turn ended or died | no, the backstop | `stop`, `stop-failure` |
| everything else | lifecycle, config, display, files, tasks | no | two observations |

Two facts behind the table are worth stating on their own, because both were surprises.

**A tool that owns its own dialog goes down the ask path even so.** `AskUserQuestion` declares
`requiresUserInteraction()` true, and the permission resolver turns that into an `ask` before it
consults the permission mode. `ExitPlanMode` declares the same thing outside remote sessions. So
`PermissionRequest` fires for both, before the question card or the plan card is drawn. The hook's
`allow` is then ignored for such a tool, deliberately, because the dialog IS the user interaction and
a hook may not answer it, but the hook still runs.

This is not read off the bundle alone. It is in the live durable ledger at
`~/.local/state/pns/pns.db`, `ledger_events`, which holds 2005 events:

| `claude` events with state `blocked` | count |
| --- | --- |
| detail begins `AskUserQuestion: que` | 23 |
| nag nudge `still waiting 5m: As` | 6 |
| detail begins `ExitPlanMode: plan=#` | 3 |
| nag nudge `still waiting 5m: Ex` | 1 |
| detail begins `Bash: command=` | 2 |

Twenty-six of thirty-five `blocked` events on this machine are a question or a plan, not a tool
approval. The nag already nudges them. The lamp already lights for them at the right moment.

**The sandbox network dialog has no event of its own.** `SandboxNetworkPrompts` calls the dialog host
directly with kind `sandbox_network_access` and payload `{host, port}`. There is no
`PermissionRequest` on that path, and Claude Code's own remote-control bridge, not a hook, is what
carries `Allow network connection to <host>?` to a phone. The one hook-visible trace is the dialog
host's notification, whose registry entry is `{text: "A sandboxed command needs network access"}`
with no `type`, and the host defaults a missing type to `permission_prompt`. The `Notification` hook
matcher matches the notification type, so no matcher can separate that from a tool approval, which
uses the same default type.

## The three defects, restated against current source

### 1. `asked` and `plan-ready` re-arm a wait after the operator answered

`Attempt::First` reaches `BlockedMarker::update` with the event's state word, and `asked` and
`plan-ready` are both in `LAMP_BLOCKED`. So for one answered question the sequence is:

1. `PermissionRequest` runs synchronously and arms the wait. Correct, and already the case.
2. The operator answers on the card.
3. `PostToolUse`/`AskUserQuestion` runs asynchronously as `asked` and arms the wait again.
4. `PostToolBatch` runs asynchronously as `resolved` and ends it.

Steps 3 and 4 are both asynchronous and therefore unordered. `hook_dispatch`'s own comment on the
`resolved` arm already names this: "a late End can unlink a newer wait's marker, an early one can
leave an answered `asked` lit." When `asked` wins, the blue lamp stays on for a question that was
answered, until the session's next event, or for up to sixteen hours if there is none.

There is a second cost. The `asked` card is delivered after the answer and its detail is the answer.
A live example, trimmed:

```
asked | AskUserQuestion: annotations= answers=78 worktrees sit inside the repo under `.worktrees/` ...
        How should I clear them out?=Move all, then delete merged (Recommended) questions=header=...
```

The operator is carded twice for one question, and the second card reads back the choice they just
made.

### 2. An answer signal exists per class and none of it is wired

- For an elicitation, `ElicitationResult` fires after the operator responds, carries `action`
  (`accept`, `decline`, `cancel`) and carries `elicitation_id`, the same identifier `Elicitation`
  carries. That is the exact signal B6's sidecar was invented to approximate, and it is better than
  the sidecar because it is correlatable.
- For `AskUserQuestion` and `ExitPlanMode`, `PostToolUse` is the answer: the tool returns when the
  dialog is answered.
- For an ordinary tool approval there is still no answer event, so `resolved` at batch resolution
  remains the earliest clear.

### 3. Sandbox network approvals are uncovered, and the exposure is currently zero

No `sandbox` block exists in `private_dot_claude/modify_settings.json` or in the live
`~/.claude/settings.json`, so no sandbox network dialog can occur on dresden today. That is a
material change to B39's HIGH rating. It is not a permanent reprieve: `sandbox.enabled` is readable
from flag settings and policy settings as well as from user settings, so the gap can open without any
local change.

## Approaches

### For the waiting indicator (B20)

**A. Keep both arms and add a sidecar that remembers which wait is live.** This is the filed plan. It
carries one identifier per session, which is why review R6-5 named the residual that answering an
older elicitation after a newer one still mis-clears the shared marker. Rejected: it adds a state
file to work around two declarations that are pointed at the wrong events.

**B. Point the post-answer events at the clearing word that already exists.** `PostToolUse` for
`AskUserQuestion` and `ExitPlanMode` is the answer, and pns already has an arm whose entire job is
"end this session's wait and clear the nag, guarded against a subagent": `pns hook resolved`. Route
them there and delete the `asked` and `plan-ready` cards. Recommended.

**C. Move `asked` to `PreToolUse`.** This was the row's own suggestion. It is now unnecessary:
`PermissionRequest` already fires earlier than `PreToolUse` would help with, and it already arms the
wait with better detail (the question text rather than the tool name). `PreToolUse` would add a third
event per question that arms a marker two other events already own.

### For ending a wait at the answer (B6)

**A. Leave the End as it is.** An unconditional `remove_file`. The accepted limit recorded in
`pns/docs/decisions/0001-ownership-by-rename-not-by-unlink.md`: "a blocked event that publishes a new
wait while a previous Stop is still condensing loses that wait when the Stop reaches its removal."

**B. An End never removes a marker armed after its own moment.** The marker already holds the arming
second, and every event already carries its own decision clock (`decision.inputs.now_secs`), so no
new state is needed. The End claims the marker by rename, reads the epoch off the claim, and either
removes the claim (the marker is not newer) or puts it back by rename (it is). This is the exact
claim, read, restore shape `protocols/markers/sweep.rs` already implements, and it is what
decision record 0001 means by ownership by rename: concurrent `unlink` reports success to every
caller on this filesystem, so a read-then-unlink End cannot be made safe. Recommended.

**C. One marker per wait, keyed by an identifier.** Faithful, and it would let an elicitation's answer
clear exactly its own wait. It costs a directory per session, it does not remove the stale-End problem
(a Stop must still clear every marker in the set, with the same race at the directory level), and it
is a state layout migration on a live file protocol. Rejected for now; approach B closes the same
residual class for less.

### For sandbox network approvals (B39)

**A. Match `Notification` on type `permission_prompt` and discriminate on the message text.** The
binary would keep an exact allowlist of verified message strings, the way `config_source_label` and
`quota_label` already keep allowlists their declarations mirror rather than trust. Everything else on
that type is silence. Cost: a wording change upstream silently turns the alert off. The failure is
quiet, which is this repository's usual direction, but it means a security-adjacent alert can die
without a signal.

**B. Match `Notification` on type `permission_prompt` and discriminate on state.** Card the
notification only when this session has no live wait already, so a tool or plan approval (which
`PermissionRequest` already reported) is dropped as a duplicate and anything else is carded with the
harness's own message as the detail. This depends on nothing upstream writes in English. It does
depend on the wait marker existing, and the marker's Start is gated on the `[lights]` table, so on a
machine with no lamps configured the dedup read always finds nothing and every approval is carded
twice. dresden has lamps configured; a fresh machine does not.

**C. Record the wiring, build nothing yet.** The exposure is zero until the sandbox is switched on,
the discriminator problem is upstream's to fix properly (the dialog needs its own notification type),
and both A and B are an hour of work the day it is needed. Recommended, with A written out below so
that hour is already spent.

## The recommended design

Three declaration edits, one behavior change in the marker protocol, two deletions. No new state
file, no new arm, no sidecar.

### The declarations

In `private_dot_claude/modify_settings.json`:

1. `PostToolUse` matcher `AskUserQuestion`: command becomes `pns hook resolved`.
2. `PostToolUse` matcher `ExitPlanMode`: command becomes `pns hook resolved`.
3. New `ElicitationResult` entry, no matcher, async, command `pns hook resolved`.
4. `Elicitation` keeps `pns hook asked` unchanged. It is the one genuine pre-answer wait among the
   four, and its card is the only one worth delivering.

All three entries stay asynchronous. `ElicitationResult` is documented as able to
"observe or override the response before it is sent to the server", so a synchronous hook there could
answer a question pns has no business answering. That is the same reasoning the `Elicitation` entry
already carries, and it is the strongest reason on the whole hook table.

No matcher on `ElicitationResult`. A matcher there filters by server name, and every answered
elicitation is a clearing signal.

### The Rust side

- `plan-ready` becomes unreachable: nothing declares it after edit 2. Delete the arm and remove
  `plan-ready` from `LAMP_BLOCKED`. Dead code gets deleted.
- `asked` stays, reached only by `Elicitation`, and stays in `LAMP_BLOCKED`.
- `update_blocked_marker`'s End branch takes the caller's moment and refuses to remove a marker whose
  epoch is strictly greater. `end_blocked_wait` takes the same argument. Both use claim by rename.
- Nothing else changes. `resolved` already clears the nag and the marker, already guards on
  `agent_id`, already loads no config and delivers nothing, and its per-event cost stays a payload
  read, a parse and at most two file operations.

### What the two ends buy, stated exactly

The clearing edits are a FAST path and the existing signals remain the guarantee, which is the same
split `arm_quota_stale_wait` documents for the quota wait. `PostToolUse` for `ExitPlanMode` fired once
in the ledger against four `blocked` events for that tool, so it does not cover every plan wait; what
it covers, it covers sooner. `Stop`, `StopFailure`, `prompt` and `resolved` are unchanged and still
end every wait they ended before.

The named nag residual shrinks for exactly the two tools where the tool IS the dialog. Today
`clear_nag`'s comment says the marker "records the BATCH'S RESOLUTION, which is the only per-batch
fact the harness's hook vocabulary carries: an approval answered at ten seconds whose tool then runs
past the schedule is nudged about anyway." For `AskUserQuestion` and `ExitPlanMode` the tool returns
at the answer, so after this change the nag for a question is cleared by the answer rather than by
the slowest sibling in the batch.

### Failure modes

**A second question's `PermissionRequest` arms while the first question's clear is in flight.** The
epoch compare keeps the newer marker. A collision inside the same whole second still loses it, and
that is closed by the session's next event.

**An older `Stop` reaches the End while a newer wait is live.** The same compare, the same
same-second window. This is the residual decision record 0001 names, and approach B closes the wide
part of it.

**`ElicitationResult` for an older elicitation clears a newer elicitation's wait.** The lamp goes
dark on a live wait. The session's next event re-publishes it, and the backstop bounds the worst
case. This is R6-5's residual, unfixed and named rather than papered over. Approach C is the fix if
it is ever observed.

**A subagent's approval arms the parent session's marker.** `resolved` skips a subagent batch by
design, so the marker holds until the parent's own Stop. Unchanged by this design; see the
`SubagentStop` open question.

**`ElicitationResult` never arrives**, because the server timed out or the session died. The wait
holds until `Stop`, `StopFailure` or the backstop.

**A payload over the one-megabyte stdin cap.** Nothing is parsed and nothing is cleared, exactly as
today, and `Stop` is the backstop.

The residual this design does NOT fix, and must not be claimed to: an ordinary tool approval still has
no answer event, so `resolved` at batch resolution remains the earliest clear for a `Bash` approval.
Two of thirty-five live `blocked` events are in that class.

### Behaviors to pin, test first

Each line is one behavior, in the repository's own hook test style
(`pns/crates/pns/tests/hooks/lights_waits.rs`, `nag_clearing.rs`, `elicitation.rs`):

1. A `PostToolUse` payload for `AskUserQuestion` routed to `resolved` removes the session's wait
   marker.
2. The same payload writes the answered marker and drops the nag record.
3. The same payload carrying an `agent_id` key, whatever its value, removes nothing.
4. An `ElicitationResult` payload removes the session's wait marker and clears the nag.
5. An `Elicitation` payload still ARMS the wait marker (the one arm that must not regress).
6. An End whose moment is older than the marker's epoch leaves the marker in place.
7. An End whose moment is equal to or newer than the marker's epoch removes it.
8. An End that claims a marker and then finds it newer restores it at its original path.
9. `plan-ready` is no longer a state word: an event carrying it is refused as an unknown hook event,
   and `state_behaviour("plan-ready", true)` is `Done` rather than `Blocked`.
10. A question answered inside the nag schedule produces no nudge (this is behavior 2 read from the
    fire's side, and it is the one behavior an operator would actually notice).

Behaviors 6 through 8 are the marker protocol and belong beside the existing
`protocols/markers/blocked/tests.rs` cases rather than in the hook tests.

### Security

Nothing here widens what pns reads or writes. `ElicitationResult` adds one payload whose `message`
field pns does not read at all (the arm reads `session_id` and the presence of `agent_id`), so no new
remote text reaches a card. The existing `elicitation_request` composer, which does put a connected
server's own name in front of its prompt, is on the `Elicitation` path and is unchanged. Every marker
path keeps `session_id_is_safe`, so a harness identifier that cannot be a filename still writes
nothing rather than escaping the state directory.

### The interim wiring for B39, written out but not built

If the operator wants the alert now, or on the day the sandbox is switched on:

- Declaration: `Notification`, matcher `permission_prompt`, async, `pns hook waiting`.
- Arm: an exact allowlist in the binary, mirroring rather than trusting the matcher, of the message
  strings verified against a Claude Code version recorded in the comment beside it. Today that is
  exactly one string, `A sandboxed command needs network access`. Every other message on that type is
  silence, because it is a wait `PermissionRequest` already reported.
- Routing: `Attempt::First` with a state word in `LAMP_BLOCKED`, so `run_event` arms the marker
  itself, plus `arm_nag`, which is what the filed row asks for. This is `blocking_event`'s shape
  without the moshi forward.
- No moshi forward. There is no permission-request payload to hand moshi, and Claude Code already
  carries this dialog to a phone over its own remote-control bridge, so a second answerable card would
  be two bridges to one dialog.
- The card cannot name the host. The registry text is static and the payload carries no host, so the
  card says that a sandboxed command needs network access and stops there. Anything more specific
  would be invented.
- One test, pinning the allowlist by exact string, so a version bump has somewhere to fail rather
  than somewhere to go quiet.

## Out of scope

- **Suppressing subagent approvals.** Explicitly out, per the ledger item. The subagent residual is
  addressed only by the open question below, which makes a subagent's wait end sooner, never by making
  it invisible.
- **A per-wait marker keyed by an identifier.** Approach C above. It is the right answer if the
  older-elicitation residual is ever observed rather than reasoned about.
- **Millisecond marker epochs.** They would close the one-second compare window, and they would change
  the unit the backstop reads on a live file protocol. Not worth it for a window the next event closes.
- **`TeammateIdle`, `TaskCreated`, `TaskCompleted`, `MessageDisplay`.** New events, none of which
  reports a wait, and the teammate feature is not in use here.
- **`PostToolUseFailure`.** A genuinely useful new event (a tool that errored is news the operator
  currently reads off the pane) and entirely unrelated to a wait. It should be filed on its own.
- **Anything about `pns quiet` or Focus and the blocked lamp.** That is B18, already decided and
  merged.

## Assumptions made in the operator's place

1. **A post-answer event should clear a wait rather than card the operator.** The alternative is an
   observation card reading "question answered", routed marker-neutral like `config-change`. Rejected
   because nobody needs to be told what they just did, and the current `asked` card already proves how
   that reads (it recites the operator's own choice back to them).
2. **`asked` keeps its name and its card, serving `Elicitation` alone.** The alternative is renaming
   it now that it means one thing instead of two. Rejected: the word appears in the durable ledger,
   `LAMP_BLOCKED`, the config comments and the doctor output, and a rename buys nothing an operator
   can see.
3. **The clearing arm is the existing `resolved`, not a new word per class.** The alternative is
   `answered` for a dialog tool and `elicitation-answered` for an elicitation, which would read better
   in the decision ring. Rejected because `resolved` already does exactly this job and three words for
   one behavior is three places for the subagent guard to drift.
4. **The epoch compare uses whole seconds and a same-second collision still loses.** The alternative
   is milliseconds in the marker, which is a unit change on a live protocol the backstop also reads.
   The residual is named under Failure modes rather than hidden.
5. **B39 is designed and not built.** The alternative is building approach A now. Assumed deferred
   because the sandbox is off on this machine, so the code would ship untested against a real dialog,
   which is the worst state for a security-adjacent alert.
6. **`plan-ready` is deleted rather than kept as an unwired word.** The alternative is leaving the arm
   in place in case a future declaration wants it. Rejected under the repository's own rule that dead
   code gets deleted and that unshipped code gets no compatibility hacks.

## Open questions

1. **Does `denied` belong in `LAMP_BLOCKED`?** `PermissionDenied` fires after the auto-mode classifier
   refused a call on its own. Nobody is waiting on an answer, yet the word arms a wait that only the
   session's next event ends. There is one live `denied` event, so this is nearly theoretical, but it
   is the same defect class as `asked`. Recommendation: route `denied` as an observation, which keeps
   the card and stops it colouring a lamp that claims someone is waiting. Not done here because it was
   not in the three filed rows.
2. **Should `SubagentStop` end a subagent's wait?** Today a subagent's approval arms the parent
   session's marker and `resolved` deliberately skips subagent batches, so the marker holds until the
   parent's own Stop. `SubagentStop` would bound it at the subagent's own end instead, which makes a
   subagent's wait shorter without making it invisible. Recommendation: yes, as a fifth declaration
   routed to `resolved`, if the operator agrees the reduced residual is worth one more declaration.
3. **B39: build now with the text allowlist, or wait?** The recommendation is to wait, and the trigger
   to revisit is either switching the sandbox on locally or Claude Code giving the dialog its own
   notification type. If the operator would rather have the alert standing, approach A is written out
   above and is about an hour.
4. **Should the sandbox-network gap be reported upstream?** The clean fix is a distinct
   `notification_type` for `sandbox_network_access`, which would make every option here robust instead
   of text-matched. Worth one issue, and it is the operator's call whether to file it.
5. **Is the `[lights]` gate on arming a marker still right?** Approach B for B39 fails on a machine
   with no lamps configured purely because the marker's Start is lamp-gated. Nothing needs changing
   today, but the gate is the reason the state-based discriminator cannot be the recommendation.

## What was verified, and what was not

Verified against the installed package and live state:

- the thirty-four event names and the input schema of each, read out of the 2.1.270 bundle;
- `ElicitationResult`'s fields, including `elicitation_id` and `action`;
- the dialog host defaulting a typeless notification to `permission_prompt`, and the `Notification`
  matcher matching the notification type;
- `sandbox_network_access` reaching the dialog host directly with no `PermissionRequest`, and its
  static notification text;
- `AskUserQuestion` and `ExitPlanMode` both declaring `requiresUserInteraction`, and both appearing as
  `blocked` events in the live ledger with their question and plan text;
- the event counts and details quoted above, from `~/.local/state/pns/pns.db`;
- the absence of any `sandbox` block in the managed template and in the live settings file.

Not verified, and stated as such:

- that a `requiresUserInteraction` tool still reaches the ask path under `bypassPermissions`. The
  resolver's ordering says it does at the call site read, and the live evidence is from sessions whose
  surviving decision records all read `mode=auto`, so the `bypassPermissions` case is inferred rather
  than measured.
- the default value of `sandbox.enabled` in 2.1.270. It is optional in the schema and absent from
  every settings file on this machine, which is what the zero-exposure claim rests on; the default
  itself was not established.
- the exact activity timestamp the notification's roughly six second idle gate measures from. The
  interval constant is 6000 milliseconds and the gate compares against the later of the dialog's mount
  and a session activity read that this pass did not trace.
- why `PostToolUse` for `ExitPlanMode` appears once against four `blocked` events for that tool. The
  asymmetry is recorded, not explained, and it is why the clearing edits are described as a fast path
  rather than a replacement.
