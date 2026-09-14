# babysitter defect release check, 2026-09-14

The `docs/remaining-work.md` entry under "Start gates and retained deferrals" asks one question before
the babysitter hold can be lifted: has upstream fix #1777 reached a released artifact past the pinned
software development kit (SDK) version `6.0.3`, and does the recorded A/B reproducer still reproduce?

Nothing was installed, nothing was upgraded, no template was edited, and nothing was filed upstream. The
two reproducers ran against throwaway `HOME` directories under `$TMPDIR`. The pin, the two enablement
holds and the marketplace declarations are exactly as they were before this check.

Two lines in the fenced blocks below run past 105 columns: a registered hook command line and one line of
reproducer output. Both are verbatim records and are left unwrapped on purpose.

## Verdict

**Defer, and keep both holds.**

- The fix is **merged on `main` and present in no released artifact**. Pull request #1777 merged
  `2026-08-23T19:13:06Z`. The newest publish of any of the three packages the defect lives in is
  `2026-08-10T21:38:04Z`, thirteen days *before* the merge. That holds for the `latest` tag, for the
  released `6.0.3`, and for the newest `staging` prerelease. There is nothing newer to move the pin to.
- The recorded A/B reproducer **still reproduces**, rerun today against installed `6.0.3`. A returns
  `continue: true` with an empty reason; B returns `decision: "block"` with the continuation reason.
- The separate execute-bit defect **also still reproduces**, and it has no upstream report at all, so it
  cannot have an upstream fix. A mode-644 script in `.a5c/hooks/on-run-start/` ran.
- No duplicate of #1761 was filed, and no merged source fix was treated as an installed release.

The operator's half is not closed by this document, and one finding makes it sharper than the ledger
entry assumes. **The enablement condition in both templates requires both defects resolved, and only one
of the two is even reported upstream.** As written, the condition cannot be satisfied by any release
upstream is currently capable of cutting. See the open questions.

## Environment

Measured on this machine, 2026-09-13 local time, 2026-09-14 Coordinated Universal Time (UTC).

| Thing                                   | Value                                             |
| --------------------------------------- | ------------------------------------------------- |
| `@a5c-ai/babysitter-sdk` installed      | `6.0.3` (global, on the fnm node lane)            |
| `adapters-hooks --version`              | `6.0.3`                                           |
| `@a5c-ai/hooks-adapter-claude` vendored | `6.0.3`                                           |
| `@a5c-ai/hooks-adapter-core` vendored   | `6.0.3`                                           |
| Claude Code plugin bundle               | `~/.claude/plugins/cache/a5c-ai/babysitter/6.0.3` |
| Plugin `versions.json`                  | `sdkVersion 6.0.3`, `extensionVersion 6.0.3`      |
| Claude Code                             | `2.1.270`                                         |
| Node                                    | `v24.21.0`                                        |
| macOS                                   | `26.2` build `25C56`                              |

The draft issue recorded Claude Code `2.1.257` and Node `24.20.0`. Both moved since; the babysitter
versions did not.

## What was checked, and how

Registry state, all three packages the fix touches:

```bash
npm view @a5c-ai/babysitter-sdk version dist-tags
npm view @a5c-ai/hooks-adapter-claude version dist-tags
npm view @a5c-ai/hooks-adapter-core version dist-tags
# newest publish across EVERY version, not just the tagged ones:
npm view <pkg> time --json | jq -r 'if type=="array" then .[0] else . end
  | to_entries | map(select(.key|test("^(created|modified)$")|not))
  | sort_by(.value) | last | "\(.key) @ \(.value)"'
```

Upstream state, through the `gh-axi` skill:

```bash
npx -y gh-axi issue view 1761 --repo a5c-ai/babysitter
npx -y gh-axi pr view 1777 --repo a5c-ai/babysitter
npx -y gh-axi api repos/a5c-ai/babysitter/pulls/1777/files
npx -y gh-axi api repos/a5c-ai/babysitter/contents/<path>?ref=main
npx -y gh-axi api repos/a5c-ai/babysitter/releases?per_page=3
npx -y gh-axi api repos/a5c-ai/babysitter-claude/commits?per_page=5
```

Installed-artifact state, by reading the deployed JavaScript rather than trusting a version string:
`hooks-adapter-claude/dist/renderer.js`, `hooks-adapter-core/dist/merge-engine/merge.js`,
`hooks-adapter-codex/dist/renderer.js`, and `babysitter-sdk/dist/hooks/dispatcher.js`.

Both reproducers were rerun from scripts kept beside this document, each `shellcheck`-clean:

- `scratchpad/ab-repro.sh`, the recorded A/B Stop reproducer, verbatim from the draft issue.
- `scratchpad/execbit-repro.sh`, the recorded execute-bit reproducer, with `GIT_DIR` and friends scrubbed
  and `gtimeout 180` around the one long call.

## Findings

### 1. The fix is on `main`, byte for byte, and in nothing published

Pull request #1777, "fix(hooks): surface block decisions through the claude Stop renderer", merged
`2026-08-23T19:13:06Z`, closing issue #1761 as completed at the same second. It changes four files, two
of them product code:

- `packages/adapters/hooks/adapter-claude/src/renderer.ts`, adding a `decision === 'block'` branch to
  `renderStopOutput` and stopping an empty `stopReason` from shadowing a real `reason`.
- `packages/adapters/hooks/core/src/merge-engine/merge.ts`, mapping a block decision onto
  `continueSession = false`.

Both blobs are current on `main`. The pull request's post-merge blob hashes and `main`'s blob hashes are
the same objects:

| File                             | Blob hash on `main`                        |
| -------------------------------- | ------------------------------------------ |
| `adapter-claude/src/renderer.ts` | `86ccee387e3fe5bf28a3f0cb88feeb6623ebab95` |
| `core/src/merge-engine/merge.ts` | `e971ed05024e300f2d657be55cc2394df1dfb6b2` |

`main` has moved on since (its head is `feb68abe`, `2026-08-31T09:36:00Z`, a continuous-integration
chore), and the repository is still active (`pushed_at` `2026-09-05`, 427 open issues). Only publishing
is stalled.

Publishing state, newest publish per package across every version including prereleases:

| Package                        | `latest` | Newest publish                                       |
| ------------------------------ | -------- | ---------------------------------------------------- |
| `@a5c-ai/babysitter-sdk`       | `6.0.0`  | `6.0.3-staging.f5f113c68e2c`, `2026-08-10T21:38:04Z` |
| `@a5c-ai/hooks-adapter-claude` | `6.0.0`  | `6.0.3-staging.f5f113c68e2c`, `2026-08-10T21:23:46Z` |
| `@a5c-ai/hooks-adapter-core`   | `6.0.0`  | `6.0.3-staging.f5f113c68e2c`, `2026-08-10T21:20:16Z` |

The released `6.0.3` of the SDK was published `2026-08-10T20:47:06Z`. Every one of those timestamps is
before the merge, so **no channel carries the fix**: not `latest`, not the released `6.0.3` the source
pins, and not the `staging` prerelease. Checking `staging` matters because it is the one channel that
could plausibly have run ahead of a release, and it did not.

The GitHub release track is on its own numbering and is staler still: the newest release is `v0.0.188`,
published `2026-06-26T10:46:10Z`. The Claude Code plugin release source repository
(`a5c-ai/babysitter-claude`) has its newest commit at `2026-08-10T21:10:22Z`, "chore: sync claude plugin
release source". So the plugin bundle has not moved either, which matters because the bundle is what
`hooks.json` ships.

### 2. The installed artifact still contains the defect, read directly

Installed `hooks-adapter-claude/dist/renderer.js`, `renderStopOutput`, is the pre-fix body. There is no
branch that can emit a decision:

```js
function renderStopOutput(result) {
    const output = {};
    if (result.continueSession != null) {
        output['continue'] = result.continueSession;
    }
    if (result.stopReason != null) {
        output['reason'] = result.stopReason;
    }
    else if (result.reason != null) {
        output['reason'] = result.reason;
    }
    ...
```

Installed `hooks-adapter-core/dist/merge-engine/merge.js:247` is likewise pre-fix:
`continueSession === false` with no decision mapping, and no `extractDecision(r) === 'block'` anywhere in
`dist/`.

### 3. The A/B reproducer reproduces, unchanged

The production command line is confirmed as A. The plugin bundle's `hooks/hooks.json` registers, for
`Stop`:

```
adapters-hooks invoke --adapter claude --handler "bash ${CLAUDE_PLUGIN_ROOT}/hooks/babysitter-proxied-stop.sh" --json
```

Rerun output, session `stop-repro-1789360660`, throwaway `HOME`, both legs exit 0.

A, the registered production command line:

```json
{
  "continue": true,
  "reason": "",
  "followUpMessage": "",
  "additionalContext": "",
  "metadata": {
    "AGENT_SESSION_ID": "stop-repro-1789360660",
    "AGENT_ADAPTER": "claude"
  }
}
```

B, the same handler alone, same session, one line exactly as emitted:

```text
{"decision":"block","reason":"Babysitter iteration 3 | Continue orchestration (run:iterate).\n\n","systemMessage":"🔄 Babysitter iteration 3/65000 [created]"}
```

A and B disagree, which is the whole report, and they disagree the same way they did on 2026-09-05. The
iteration number differs (3 rather than 4) because this is a fresh throwaway run; nothing else changed.

Consequence, in the upstream author's own words on #1761: the continuation loop "is inert for every
Claude Code user of the plugin". Enabling the plugin today buys an orchestration plugin whose
orchestration does not resume.

### 4. The execute-bit defect reproduces too, and has no upstream report

Rerun today, mode-644 script, throwaway `HOME` and scratch repository:

```
mode before run: -rw-r--r--  1 stephen  staff  91 .a5c/hooks/on-run-start/marker.sh
EXECUTED present: 2026-09-14T04:41:38Z executed uid=501
```

The installed dispatcher explains it, and the explanation is not version-dependent.
`babysitter-sdk/dist/hooks/dispatcher.js` selects candidates by extension alone (`HOOK_SCRIPT_EXTENSIONS`
at line 49, `path.extname(entry.name)` at line 93) and then runs each with `spawn("bash", [hook.path])`
at line 133, which ignores the file mode. No `X_OK` test exists in the file.

This defect was never filed. The ledger records its issue draft as deferred on 2026-09-06 and still
sitting in `~/.claude/pipeline/babysitter-issues/babysitter-issue-execute-bit-v2.md`, and searches of the
upstream tracker (`repo:a5c-ai/babysitter execute bit hook` and
`repo:a5c-ai/babysitter on-run-start executable`) return zero issues. An unreported defect has no
upstream fix, no merged pull request, and no release that could carry one.

One caveat on both this search and the Codex one below: GitHub issue search is keyword-based over a
repository with 427 open issues, so zero hits is strong evidence of no report rather than proof of one.
It is not evidence either way about a fix, since a fix would show as a merged pull request touching
`dist/hooks/dispatcher.js`'s source, and no such change is on `main`.

That is the finding the operator's half turns on, and it is stated plainly because it is the one thing
this check found that the ledger entry does not already assume.

### 5. Even a released #1777 would not lift the Codex hold, and might move it sideways

The Codex adapter drops `decision` on Stop by a deliberate per-event field allowlist, not by the omission
#1777 fixes. Read from installed `hooks-adapter-codex/dist/renderer.js`:

```js
/** Output fields supported on Stop. */
const STOP_FIELDS = new Set([
    'continueSession',
    'stopReason',
    'reason',
]);
```

`decision` is absent, so `renderCodexOutput` routes it to `droppedFields` instead of the output. The
file's own header says why: "Codex output semantics are limited -- many fields fail open (spec section
17.2). Only emit fields that are documented." The draft issue called this out of scope, correctly, and
searches of the upstream tracker (`codex stop decision`, `STOP_FIELDS`, `adapter-codex stop`) return zero
issues, subject to the caveat above.

There is a second-order effect worth flagging before the hold is lifted anywhere. #1777's core half maps
a block decision onto `continueSession = false`. `continueSession` **is** in the Codex allowlist, and
`renderCodexOutput` treats only `continueSession === true` as empty. So once that change ships, the Codex
Stop output for a block gains `continueSession: false` where today it carries nothing. Whether Codex
reads that as "hold the turn" or as "end the session" is **not** established here: it is a reading of the
merged diff against the installed renderer, not a measured behavior, and the fix is not installable yet
so it cannot be measured on this machine. Treat it as a risk to retest, not as a finding.

### 6. The `latest` tag is behind, and the pin is durable against both writers

`latest` for all three packages is `6.0.0`, while the source pins `6.0.3`, so the pin cannot be replaced
with a floating spec: an unpinned install would move the command-line interface two minor patches
*backwards* relative to the plugin bundles that call it. The source comment in
`.chezmoidata/system_packages_autoinstall.yaml` already records this, and it is still true.

Two writers could disturb the pin, and neither does:

- **The apply path.** `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl:293` tests
  `npm ls -g '@a5c-ai/babysitter-sdk@6.0.3'`, which is exact, then reinstalls that exact spec on the
  `elif` branch. An apply converges on the pin.
- **The weekly uu lane.** `uu/crates/uu-adapters/src/lanes/npm.rs` runs `npm update -g`. Measured rather
  than reasoned: the pin landed in `f89a9dae` on 2026-09-05, the npm lane's state under
  `~/.local/state/uu/lanes/npm/` was last written 2026-09-13, and the installed SDK is still `6.0.3`. The
  lane has run since the pin and has not moved it.

So the pin needs a deliberate edit to move, which is the behavior the hold wants.

## Assumptions made in the operator's place

Each of these was a choice this document made rather than leave the check unfinished. The alternative is
stated so the operator can overturn it cheaply.

1. **"Released artifact" means a non-prerelease version on the npm registry, or a GitHub release asset.**
   A `staging` dist-tag does not count. *Alternative:* accept `staging` as a release channel for this
   package. This is moot today (the newest `staging` publish predates the merge by thirteen days), so
   overturning it changes nothing now, but it would change what a future check accepts.
1. **Nothing was filed upstream.** Not a duplicate of #1761, which the ledger forbids, and also not the
   two unreported items: the Codex Stop allowlist and the execute-bit defect. *Alternative:* file the
   Codex Stop omission now, since #1761 is closed, no duplicate exists, and it is a distinct mechanism in
   a distinct package. Filing was withheld because both holds are the operator's and the ledger keeps the
   execute-bit draft behind an explicit deferral.
1. **Neither template was edited.** `private_dot_claude/modify_settings.json` keeps `babysitter@a5c.ai`
   in `$defaultDisabledPlugins`, and `private_dot_codex/modify_private_config.toml` keeps
   `plugins."babysitter@babysitter".enabled = false`. *Alternative:* split the coupled condition in both
   comments now, so each hold names the one defect it waits on. This is the operator's explicit half of
   the task, so it was not done silently.
1. **No release watcher was built.** Learning that a fix shipped stays a manual recheck of the three
   `npm view` lines above. *Alternative:* a uu probe or a weekly lane step that compares the published
   versions against the pin. Withheld under the standing "no speculative mechanism" preference and
   because the check is three commands.
1. **The recommendation is "keep the hold", not "enable with an inert loop".** *Alternative:* enable
   babysitter for its non-orchestration features and accept that the continuation loop does not resume,
   plus the execute-bit exposure. Not recommended, because the loop is the reason the plugin exists and
   the execute-bit behavior runs repository-supplied scripts under one unrelated approval.

## What would change the verdict

- **A publish, on any of the three packages, dated after `2026-08-23T19:13:06Z`.** Check it with the
  `npm view <pkg> time --json` line above, then confirm the artifact rather than the version string:
  installed `hooks-adapter-claude/dist/renderer.js` must contain a `result.decision === 'block'` branch
  in `renderStopOutput`, and installed `hooks-adapter-core/dist/merge-engine/merge.js` must contain an
  `extractDecision` block mapping. A version bump with the old bytes is not a fix.
- **A rerun where A and B agree**, A carrying `decision: "block"` with the handler's reason. That is the
  one test that closes the Stop half, and it needs no live session.
- **An upstream fix to the execute-bit dispatcher**, which cannot exist before the defect is reported.
  Filing the deferred draft is the prerequisite, not an optional extra.
- **An operator ruling that decouples the two holds**, which would let a Stop-only release lift the
  Claude Code hold without waiting on the unreported defect.
- **A retest of the Codex Stop path once the fix is installable**, because `continueSession: false` will
  start reaching Codex on a block and its meaning there is untested.

## Open questions for the operator

1. **Does the enablement condition stay "both defects resolved"?** As written it cannot be satisfied: the
   execute-bit defect has no upstream report, so no release can fix it. Recommendation: decouple, so each
   hold names the defect it waits on.
1. **Is the execute-bit draft filed now, releasing its 2026-09-06 deferral?** Without that, "both
   resolved" has no path forward at any date.
1. **Is the Codex Stop allowlist omission reported, and does it join the Codex hold's condition
   explicitly?** Today the Codex comment cites the same "adapters both drop Stop's decision" reason as
   the Claude comment, but a released #1777 fixes only the Claude side.
1. **When a release lands that fixes only the Stop defect, does babysitter get enabled in Claude Code
   while the execute-bit behavior stands?** The two defects differ in kind: one is a broken feature, the
   other runs repository-supplied scripts under an unrelated approval.
1. **Does the repository get a release watcher, or does this stay a manual recheck?** If manual, on what
   cadence, and does it live in the ledger entry or in a Todoist task?
1. **Is a `staging` dist-tag ever acceptable as the pin?** The answer decides what a future check is
   allowed to accept, even though it changes nothing today.
