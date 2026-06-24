# Relay notifications — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Build the `relay` pipeline — agents (Claude, Codex) and long shell commands send state-aware
notifications to three failure-separated channels (moshi push, Hermes→Discord, clickable-local).

**Architecture:** A shared sender `relay.sh` fans out to the three channels, each isolated. `relay-agent.sh`
builds the agent message and calls `relay.sh`; the shell notifier calls `relay.sh` directly. Hooks map each
agent event to a state.

**Tech Stack:** bash, jq, python3 (HMAC), curl, terminal-notifier, herdr CLI, chezmoi, KeePassXC.

## Global Constraints

- Every commit passes the pre-commit hook: `just lint-check` (shellcheck, `shfmt -i 2 -ci -s`, mdformat
  105-col, taplo, jq, yq) **and** `just test`. Both must pass.
- Shell: `set -euo pipefail`; double-quote expansions; prefer coreutils/jq/python3/openssl.
- `relay.sh` and `relay-agent.sh` **always `exit 0`** (async hooks must never delay/fail a hook or prompt).
- **The webhook secret never appears in argv *or* env.** `jq`/`python3` read the secret straight from the
  0600 file (never into a shell variable or env var), and the request body is sent on stdin via
  `curl --data @-` (never `curl --data "$body"`, which would put the moshi token on argv). The HMAC key is
  read from the file by `python3` (not `openssl -hmac <key>`, which puts the key on argv).
- New `.sh.tmpl` files get added to `find_shell_templates` in `scripts/lint.sh`; they're shellchecked via
  `CI=1 chezmoi execute-template --no-tty <file | shellcheck -`. Plain `.sh` files are auto-shellchecked.
- Never bare `chezmoi apply` (KeePassXC TTY templates). Apply specific non-template files by name; the
  operator applies KeePassXC templates (the new `~/.config/relay/auth.json`) interactively.
- Do **not** break the currently-working Claude `Stop`→moshi push during the refactor.
- Tests are `test/*.sh` (build-tool style; exit non-zero on failure), mocking `curl` / `terminal-notifier`
  on `PATH` like `test/homebrew-weekly-upgrade.sh` mocks `brew`.

## File structure

- Create `dot_local/bin/executable_relay.sh` — the sender (3 channels, failure-separated, exit 0).
- Create `dot_local/bin/executable_relay-agent.sh` — agent state→message builder (calls relay.sh).
- Delete `dot_local/bin/executable_claude-moshi-notify.sh` — superseded by the two above.
- Create `dot_config/relay/private_auth.json.tmpl` — `{moshi_secret, hermes_secret}` from KeePassXC, 0600.
- Delete `dot_config/moshi/private_auth.json.tmpl` — moshi secret migrates into relay's.
- Modify `private_dot_claude/modify_settings.json` — Claude hooks → `relay-agent.sh <state>`.
- Create `dot_codex/private_hooks.json.tmpl` — Codex `Stop`/`PermissionRequest` → `relay-agent.sh`, plus the
  preserved herdr agent-state `SessionStart` hook.
- Create `dot_codex/executable_herdr-agent-state.sh` — tracked copy (Task 0 confirms origin/content).
- Modify `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl` — drop `codex` from `--target`.
- Modify `dot_bashrc.tmpl` — shell notifier thresholds + skip-list + relay.sh call.
- Create `test/relay.sh`, `test/relay-agent.sh`.
- Modify `scripts/lint.sh` — add the codex hooks `.sh.tmpl` if templated (the `.json.tmpl` is not shell).
- Create `dot_hermes/create_private_dot_env.tmpl` (`WEBHOOK_ENABLED`/`WEBHOOK_PORT` platform switch) +
  `dot_hermes/create_private_config.yaml.tmpl` (relay route; secret is a keepassxc literal — route `${VAR}`
  isn't expanded; no global — `create_` so neither clobbers live state).
- Modify `CLAUDE.md` — replace the moshi/notify sections with the relay description; document the Hermes
  tracking method (`.env` secret + `create_` config baseline + manual route-add on existing hosts).

---

## Task 0: Verify the two open questions (gather evidence; no code yet)

**Files:** none (investigation; record findings inline in this plan or the commit message of Task 3).

- [ ] **Step 1 — Codex hook payload.** Add a temporary echo hook OR inspect a real Codex run: confirm
  whether Codex's `Stop` and `PermissionRequest` hooks receive stdin JSON with `cwd` and `transcript_path`.
  Run: trigger a Codex session and capture stdin, e.g. temporarily set a Codex `Stop` hook to
  `cat > /tmp/codex-stop.json` and inspect it. Expected: learn which fields exist.
- [ ] **Step 2 — Record the decision.** If `transcript_path` is present and JSONL like Claude's,
  `relay-agent.sh` reuses the snippet parser for Codex. If absent, Codex's `done` message uses
  project/branch only (no snippet). Write the finding into Task 2's snippet step as a guard.
- [ ] **Step 3 — herdr-agent-state.sh origin.** Run
  `grep -rl herdr-agent-state ~/.codex ~/.config/herdr /opt/homebrew/bin/moshi-hook 2>/dev/null` and
  `file ~/.codex/herdr-agent-state.sh`; check whether moshi-hook or a herdr integration wrote it
  (`herdr integration --help`). Expected: identify the writer.
- [ ] **Step 4 — Decide tracking.** If it's a static helper, copy it to
  `dot_codex/executable_herdr-agent-state.sh` verbatim so the chezmoi `hooks.json` reference resolves. If a
  tool regenerates it, note that the tool must run before the hooks.json is used, and still track a copy.

---

## Task 1: `relay.sh` sender + secret template + test

**Files:**
- Create `dot_local/bin/executable_relay.sh`
- Create `dot_config/relay/private_auth.json.tmpl`
- Create `test/relay.sh`

**Produces:** `relay.sh --agent <l> --state <s> --project <p> [--branch <b>] [--detail <t>] [--pane <id>]`,
reading secrets from `${RELAY_AUTH_FILE:-~/.config/relay/auth.json}`, endpoints overridable via
`RELAY_MOSHI_URL` / `RELAY_HERMES_URL`.

- [ ] **Step 1 — failing test `test/relay.sh`.** Mocks `curl` + `terminal-notifier` on `PATH` (each records
  its argv + stdin to files under a tempdir), points `RELAY_AUTH_FILE` at a fixture with both secrets and
  `RELAY_MOSHI_URL`/`RELAY_HERMES_URL` at dummy URLs, runs `relay.sh`, and asserts:

```bash
#!/usr/bin/env bash
# relay.sh: fans out to moshi + hermes + local, failure-separated, exits 0.
set -uo pipefail
relay="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/dot_local/bin/executable_relay.sh"
[[ -x $relay ]] || { echo "relay: FAIL -- not executable: $relay" >&2; exit 1; }
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
# record-only mocks
cat >"$tmp/curl" <<MOCK
#!/usr/bin/env bash
echo "\$*" >>"$tmp/curl-argv.log"
cat >>"$tmp/curl-stdin.log"
MOCK
cat >"$tmp/terminal-notifier" <<MOCK
#!/usr/bin/env bash
echo "ARGV: \$*" >>"$tmp/tn.log"
MOCK
chmod +x "$tmp/curl" "$tmp/terminal-notifier"
printf '{"moshi_secret":"MSECRET","hermes_secret":"HSECRET"}' >"$tmp/auth.json"
out="$(PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/auth.json" \
  RELAY_MOSHI_URL="http://moshi.test/hook" RELAY_HERMES_URL="http://hermes.test/relay" \
  bash "$relay" --agent claude --state done --project dotfiles --branch main \
  --detail "all green" --pane wW:p8 2>&1; echo "rc=$?")"
wait 2>/dev/null
grep -q "rc=0" <<<"$out" || { echo "relay: FAIL -- exit not 0" >&2; exit 1; }
grep -q "moshi.test/hook" "$tmp/curl-argv.log" || { echo "relay: FAIL -- moshi not called" >&2; exit 1; }
grep -q "hermes.test/relay" "$tmp/curl-argv.log" || { echo "relay: FAIL -- hermes not called" >&2; exit 1; }
grep -q "X-Webhook-Signature:" "$tmp/curl-argv.log" || { echo "relay: FAIL -- no HMAC header" >&2; exit 1; }
grep -q "MSECRET" "$tmp/curl-stdin.log" || { echo "relay: FAIL -- moshi token not sent in body" >&2; exit 1; }
grep -q "MSECRET\|HSECRET" "$tmp/curl-argv.log" && { echo "relay: FAIL -- secret leaked to argv" >&2; exit 1; }
grep -q "HSECRET" "$tmp/curl-stdin.log" && { echo "relay: FAIL -- hermes secret leaked into the body" >&2; exit 1; }
grep -q "herdr agent focus wW:p8" "$tmp/tn.log" || { echo "relay: FAIL -- local focus cmd wrong" >&2; exit 1; }
# failure separation: hermes curl fails, moshi + local still happen, exit 0
cat >"$tmp/curl" <<MOCK
#!/usr/bin/env bash
echo "ARGV: \$*" >>"$tmp/curl2.log"
[[ "\$*" == *hermes.test* ]] && exit 7
MOCK
chmod +x "$tmp/curl"
out2="$(PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/auth.json" \
  RELAY_MOSHI_URL="http://moshi.test/hook" RELAY_HERMES_URL="http://hermes.test/relay" \
  bash "$relay" --agent claude --state done --project x --pane wW:p8 2>&1; echo "rc=$?")"
wait 2>/dev/null
grep -q "rc=0" <<<"$out2" || { echo "relay: FAIL -- exit not 0 on hermes failure" >&2; exit 1; }
grep -q "moshi.test/hook" "$tmp/curl2.log" || { echo "relay: FAIL -- moshi dropped on hermes failure" >&2; exit 1; }
# no secret -> no-op, exit 0
printf '{}' >"$tmp/empty.json"
out3="$(PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/empty.json" bash "$relay" --state done --project x 2>&1; echo "rc=$?")"
grep -q "rc=0" <<<"$out3" || { echo "relay: FAIL -- exit not 0 with no secrets" >&2; exit 1; }
echo "relay: OK"
```

- [ ] **Step 2 — run it, expect FAIL** (`relay.sh` missing): `just test 2>&1 | grep relay`.
- [ ] **Step 3 — implement `dot_local/bin/executable_relay.sh`:**

```bash
#!/usr/bin/env bash
# relay: fan a notification to moshi (phone) + Hermes (Discord paper trail) + a
# clickable local macOS notification (focus the herdr pane on click). Each channel
# is isolated (|| true, backgrounded); always exits 0. Secret never on argv.
set -euo pipefail

agent="" state="" project="" branch="" detail="" pane=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --agent) agent="${2:-}"; shift 2 ;;
    --state) state="${2:-}"; shift 2 ;;
    --project) project="${2:-}"; shift 2 ;;
    --branch) branch="${2:-}"; shift 2 ;;
    --detail) detail="${2:-}"; shift 2 ;;
    --pane) pane="${2:-}"; shift 2 ;;
    *) shift ;;
  esac
done

auth_file="${RELAY_AUTH_FILE:-$HOME/.config/relay/auth.json}"
moshi_url="${RELAY_MOSHI_URL:-https://api.getmoshi.app/api/webhook}"
hermes_url="${RELAY_HERMES_URL:-http://127.0.0.1:8644/webhooks/relay}"

loc="$project"
[[ -n $branch ]] && loc="${loc:+$loc }($branch)"
title="${agent:-relay} · ${state:-done}${project:+ · $project}"
message="${state:-done}${loc:+ — $loc}"
[[ -n $detail ]] && message="$message"$'\n'"$detail"

# moshi — token read from the 0600 file by jq; body sent on stdin (never on argv)
moshi_body="$(jq -c --arg t "$title" --arg m "$message" \
  'if .moshi_secret then {token: .moshi_secret, title: $t, message: $m} else empty end' "$auth_file" 2>/dev/null || true)"
if [[ -n $moshi_body ]]; then
  (curl -fsS -m 10 -X POST "$moshi_url" -H 'Content-Type: application/json' --data @- <<<"$moshi_body" >/dev/null 2>&1 || true) &
fi

# hermes — body carries no secret; HMAC key read from the file by python (never argv/env); body on stdin
hermes_body="$(jq -cn --arg a "$agent" --arg s "$state" --arg p "$project" --arg d "$message" \
  '{agent: $a, state: $s, project: $p, detail: $d}')"
sig="$(printf '%s' "$hermes_body" | python3 -c 'import hmac, hashlib, json, sys
secret = json.load(open(sys.argv[1])).get("hermes_secret") or ""
sys.stdout.write(hmac.new(secret.encode(), sys.stdin.buffer.read(), hashlib.sha256).hexdigest() if secret else "")' "$auth_file" 2>/dev/null || true)"
if [[ -n $sig ]]; then
  (curl -fsS -m 10 -X POST "$hermes_url" -H 'Content-Type: application/json' \
    -H "X-Webhook-Signature: $sig" --data @- <<<"$hermes_body" >/dev/null 2>&1 || true) &
fi

# local clickable notification -> focus the exact herdr pane on click
if command -v terminal-notifier >/dev/null 2>&1; then
  exec_cmd=":"
  [[ -n $pane ]] && exec_cmd="herdr agent focus $pane"
  (terminal-notifier -title "$title" -message "$message" \
    -activate com.mitchellh.ghostty -execute "$exec_cmd" >/dev/null 2>&1 || true) &
fi

exit 0
```

- [ ] **Step 4 — secret template `dot_config/relay/private_auth.json.tmpl`:**

```json
{
  "moshi_secret": {{ (keepassxc "Moshi :: Webhook Secret").Password | quote }},
  "hermes_secret": {{ (keepassxc "Hermes :: Relay Webhook Secret").Password | quote }}
}
```

(`| quote` emits the JSON-quoted string, so no surrounding quotes. Create the KeePassXC entry
**Hermes :: Relay Webhook Secret** — the same entry renders the Hermes route's literal secret in Task 5.)

- [ ] **Step 5 — run the test, expect PASS:** `just test 2>&1 | grep relay` → `relay: OK`.
- [ ] **Step 6 — commit:**

```bash
git add dot_local/bin/executable_relay.sh dot_config/relay/private_auth.json.tmpl test/relay.sh
git commit -m "feat(relay): fan-out sender (moshi + Hermes + clickable local), failure-separated"
```

---

## Task 2: `relay-agent.sh` + rewire Claude hooks (keep the Stop push working)

**Files:**
- Create `dot_local/bin/executable_relay-agent.sh`
- Create `test/relay-agent.sh`
- Modify `private_dot_claude/modify_settings.json`
- Delete `dot_local/bin/executable_claude-moshi-notify.sh`

**Consumes:** `relay.sh` (Task 1). **Produces:** `relay-agent.sh <state>` reading the hook stdin JSON.

- [ ] **Step 1 — failing test `test/relay-agent.sh`.** Mocks `relay.sh` on `PATH` (records argv), feeds a
  fixture transcript + a `{"cwd":...,"transcript_path":...}` stdin, asserts the right `--state`, `--project`,
  `--pane`, and (for `done`) a `--detail` snippet:

```bash
#!/usr/bin/env bash
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
agent="$root/dot_local/bin/executable_relay-agent.sh"
[[ -x $agent ]] || { echo "relay-agent: FAIL -- not executable" >&2; exit 1; }
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
cat >"$tmp/relay.sh" <<'MOCK'
#!/usr/bin/env bash
printf '%s\0' "$@" >"$RELAY_ARGS_FILE"
MOCK
chmod +x "$tmp/relay.sh"
# minimal assistant-text transcript (last text block = "done thinking")
printf '%s\n' '{"type":"assistant","message":{"content":[{"type":"text","text":"done thinking"}]}}' >"$tmp/t.jsonl"
out="$(PATH="$tmp:$PATH" RELAY_BIN="$tmp/relay.sh" RELAY_ARGS_FILE="$tmp/args" HERDR_PANE_ID="wW:p8" \
  bash -c 'printf "{\"cwd\":\"/x/dotfiles\",\"transcript_path\":\"'"$tmp"'/t.jsonl\"}" | "$0" done' "$agent" 2>&1; echo "rc=$?")"
grep -q "rc=0" <<<"$out" || { echo "relay-agent: FAIL -- exit not 0" >&2; exit 1; }
args="$(tr '\0' '\n' <"$tmp/args")"
grep -qx -- "--state" <<<"$args" && grep -qx -- "done" <<<"$args" || { echo "FAIL state" >&2; exit 1; }
grep -qx -- "dotfiles" <<<"$args" || { echo "relay-agent: FAIL -- project" >&2; exit 1; }
grep -qx -- "wW:p8" <<<"$args" || { echo "relay-agent: FAIL -- pane" >&2; exit 1; }
grep -qx -- "done thinking" <<<"$args" || { echo "relay-agent: FAIL -- snippet" >&2; exit 1; }
echo "relay-agent: OK"
```

- [ ] **Step 2 — run it, expect FAIL.**
- [ ] **Step 3 — implement `dot_local/bin/executable_relay-agent.sh`** (reuse the transcript parse from the
  current `claude-moshi-notify.sh`; delegate delivery to relay.sh via `${RELAY_BIN:-$HOME/.local/bin/relay.sh}`):

```bash
#!/usr/bin/env bash
# relay-agent: build an agent state message from a hook payload, hand to relay.sh.
# Arg 1 = state (done|blocked|asked|plan-ready). Always exits 0.
set -euo pipefail
state="${1:-done}"
relay="${RELAY_BIN:-$HOME/.local/bin/relay.sh}"
input="$(cat 2>/dev/null || true)"
cwd="$(printf '%s' "$input" | jq -r '.cwd // empty' 2>/dev/null || true)"
transcript="$(printf '%s' "$input" | jq -r '.transcript_path // empty' 2>/dev/null || true)"
agent="${RELAY_AGENT:-claude}"
project="${cwd##*/}"
branch=""
[[ -n $cwd && -d $cwd ]] && branch="$(git -C "$cwd" branch --show-current 2>/dev/null || true)"
detail=""
if [[ $state == done && -n $transcript && -f $transcript ]]; then
  detail="$(jq -rs '[.[] | select(.type=="assistant" and (.message.content? != null))
    | .message.content[] | select(.type=="text") | .text] | last // empty' "$transcript" 2>/dev/null || true)"
  detail="$(printf '%s' "$detail" | tr '\n\r\t' '   ' | tr -s ' ' | sed 's/^ *//; s/ *$//')"
  [[ ${#detail} -gt 240 ]] && detail="${detail:0:240}…"
else
  detail="$(printf '%s' "$input" | jq -r '.message // .detail // empty' 2>/dev/null || true)"
fi
"$relay" --agent "$agent" --state "$state" --project "$project" \
  ${branch:+--branch "$branch"} ${detail:+--detail "$detail"} ${HERDR_PANE_ID:+--pane "$HERDR_PANE_ID"} || true
exit 0
```

- [ ] **Step 4 — run the test, expect PASS.**
- [ ] **Step 5 — rewire Claude hooks in `private_dot_claude/modify_settings.json`.** In the `$hooks` dict,
  change the `Stop` moshi entry's command from `claude-moshi-notify.sh` to
  `printf '%s/.local/bin/relay-agent.sh' $home` with arg `done`, and add `Notification[permission_prompt]`,
  `PostToolUse[AskUserQuestion]`, `PostToolUse[ExitPlanMode]` entries calling `relay-agent.sh blocked|asked|plan-ready`.
  Exact template fragment for the Stop entry:

```gotemplate
        (dict
          "type" "command"
          "async" true
          "command" (printf "%s/.local/bin/relay-agent.sh done" $home))
```

(Add the other three states as new hook arrays for `PostToolUse` with the `AskUserQuestion`/`ExitPlanMode`
matchers, and extend the existing `Notification[permission_prompt]` array with a `relay-agent.sh blocked`
command alongside the alerter.)

- [ ] **Step 6 — render-check (no KeePassXC needed; settings modify-template is keepassxc-free):**
  `chezmoi cat ~/.claude/settings.json | jq '.hooks.Stop'` → shows `relay-agent.sh done`.
- [ ] **Step 7 — delete the old script:** `git rm dot_local/bin/executable_claude-moshi-notify.sh`.
- [ ] **Step 8 — commit:**

```bash
git add dot_local/bin/executable_relay-agent.sh test/relay-agent.sh private_dot_claude/modify_settings.json
git commit -m "feat(relay): agent message-builder + Claude done/blocked/asked/plan-ready hooks"
```

---

## Task 3: Codex — chezmoi-owned hooks.json + moshi-hook exclusion + herdr-agent-state

**Files:**
- Create `dot_codex/private_hooks.json.tmpl`
- Create `dot_codex/executable_herdr-agent-state.sh` (from Task 0)
- Modify `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl`

**Consumes:** `relay-agent.sh` (Task 2), Task 0 findings.

- [ ] **Step 1 — `dot_codex/private_hooks.json.tmpl`** (`$home` = `.chezmoi.homeDir`); `Stop`→`done`,
  `PermissionRequest`→`blocked`, preserve the herdr agent-state `SessionStart`:

```gotemplate
{{- $home := .chezmoi.homeDir -}}
{
  "hooks": {
    "SessionStart": [
      { "hooks": [ { "type": "command", "timeout": 10,
        "command": "bash '{{ $home }}/.codex/herdr-agent-state.sh' session" } ] }
    ],
    "Stop": [
      { "hooks": [ { "type": "command",
        "command": "RELAY_AGENT=codex '{{ $home }}/.local/bin/relay-agent.sh' done" } ] }
    ],
    "PermissionRequest": [
      { "hooks": [ { "type": "command",
        "command": "RELAY_AGENT=codex '{{ $home }}/.local/bin/relay-agent.sh' blocked" } ] }
    ]
  }
}
```

- [ ] **Step 2 — drop `codex` from `--target`** in
  `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl` (change the install line to
  `--target opencode,gemini,cursor,kimi,qwen,grok,omp,pi`), and add a guarded one-time
  `moshi-hook uninstall --target codex` before it (mirrors the Claude-exclusion note already in the file).
- [ ] **Step 3 — operator step (document in the commit body):** run `moshi-hook uninstall --target codex`
  once, then apply the new hooks: `chezmoi apply ~/.codex/hooks.json ~/.codex/herdr-agent-state.sh`.
- [ ] **Step 4 — verify:** `jq '.hooks | keys' ~/.codex/hooks.json` shows the three events;
  `moshi-hook status` shows `codex` as **not found** (no longer managed); a Codex `Stop` fires `relay-agent`.
- [ ] **Step 5 — commit:**

```bash
git add dot_codex/private_hooks.json.tmpl dot_codex/executable_herdr-agent-state.sh \
  .chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl
git commit -m "feat(relay): own Codex hooks (done/blocked) + remove Codex from moshi-hook"
```

---

## Task 4: Shell long-command notifier

**Files:** Modify `dot_bashrc.tmpl` (the `__cmd_notify_precmd` block).

**Consumes:** `relay.sh`.

- [ ] **Step 1 — edit `__cmd_notify_precmd`** (lines ~301-309): add `codex` to the skip-list regex, raise
  the alerter tier to `>= 60`, and at `>= 300` add a relay call. New body:

```bash
    [[ $__cmd_notify_name =~ ^(vim|nvim|less|man|top|btop|ssh|herdr|claude|codex|fzf) ]] && return
    local cmd="${__cmd_notify_name%% *}"
    if ((elapsed >= 300)); then
      (alerter --timeout 10 --title "Command finished" --message "$cmd (${elapsed}s)" --sound default >/dev/null 2>&1 &)
      ("$HOME/.local/bin/hue-pulse.sh" "$exit_code" >/dev/null 2>&1 &)
      ("$HOME/.local/bin/relay.sh" --agent shell --state done --project "${PWD##*/}" \
        --detail "$cmd (${elapsed}s)" ${HERDR_PANE_ID:+--pane "$HERDR_PANE_ID"} >/dev/null 2>&1 &)
    elif ((elapsed >= 60)); then
      ("$HOME/.local/bin/relay.sh" --agent shell --state done --project "${PWD##*/}" \
        --detail "$cmd (${elapsed}s)" ${HERDR_PANE_ID:+--pane "$HERDR_PANE_ID"} >/dev/null 2>&1 &)
    fi
```

(The ≥60 tier sends the clickable local notification via relay's local channel; ≥300 adds moshi+Hermes+Hue.
relay.sh decides channels by which secrets exist — so for a local-only tier, point it at the local channel:
see Step 2.)

- [ ] **Step 2 — add a `--local-only` flag to `relay.sh`** so the ≥60 tier sends only the local notification
  (no phone/Discord spam under 5m). In `relay.sh` arg parse add `--local-only) local_only=1; shift ;;` and
  guard the moshi + hermes blocks with `[[ -z ${local_only:-} ]] &&`. Update `test/relay.sh` with a case:
  `--local-only` produces a `tn.log` entry but no `curl.log` entry. Use `--local-only` in the ≥60 tier and
  full fan-out in the ≥300 tier.
- [ ] **Step 3 — render + shellcheck** `dot_bashrc.tmpl`:
  `CI=1 chezmoi execute-template --no-tty dot_bashrc.tmpl | shellcheck -` → clean.
- [ ] **Step 4 — commit:**

```bash
git add dot_bashrc.tmpl dot_local/bin/executable_relay.sh test/relay.sh
git commit -m "feat(relay): long-command notifications (local >=1m, full fan-out >=5m); skip codex"
```

---

## Task 5: Track the Hermes webhook activation + relay route (secrets-safe) + secret migration + docs

**Verified webhook model** (Hermes v0.14.0, confirmed against the installed gateway source, the osquery
branch, and the docs): the `.env` is the platform **switch** — `WEBHOOK_ENABLED=true` and `WEBHOOK_PORT=8644`
(read via `os.getenv`, `gateway/config.py:1470-1471`) turn the webhook gateway on; without them no route
runs. **Secrets are not env-injected for routes** — the gateway does not expand `${VAR}` in route secrets, so
each route carries an explicit secret in `config.yaml`. We use **no global `WEBHOOK_SECRET`**: every route is
explicit, and a route missing its secret **fails closed** (the platform refuses to start, `webhook.py:154`).
`config.yaml` is runtime-rewritten (open bug #4775 can resolve placeholders to plaintext), so both files are
chezmoi **`create_`** templates (written once on a fresh host, never re-synced or read back). **Only the
`relay` route is committed here** — not the osquery routes (unmerged osquery branch, which tracks no Hermes
source and signs from its own runtime secret file, so there's no source collision); the live `config.yaml`
holds both route sets and `create_` never overwrites it.

**Files:** Create `dot_hermes/create_private_dot_env.tmpl`, `dot_hermes/create_private_config.yaml.tmpl`
(both done — committed with this task); delete `dot_config/moshi/private_auth.json.tmpl`; modify `CLAUDE.md`.

- [ ] **Step 1 — platform switch in `.env`: `dot_hermes/create_private_dot_env.tmpl`** (`create_` — the live
  `.env` holds ~25 Hermes-owned keys; a normal template would wipe them). Two non-secret activation vars (no
  keepassxc, so the template is automation-safe):

```gotemplate
WEBHOOK_ENABLED=true
WEBHOOK_PORT=8644
```

- [ ] **Step 2 — relay route config: `dot_hermes/create_private_config.yaml.tmpl`** (`create_` — never
  overwrites the live osquery-bearing config). Route `${VAR}` is NOT expanded (v0.14.0), so the secret is a
  keepassxc-rendered LITERAL; the committed source holds the keepassxc call, never the value:

```yaml
platforms:
  webhook:
    enabled: true
    extra:
      host: 127.0.0.1
      port: 8644
      routes:
        relay:
          secret: {{ (keepassxc "Hermes :: Relay Webhook Secret").Password | quote }}
          deliver: discord
          deliver_only: true
          prompt: "{agent} · {state} · {project}\n\n{detail}"
          deliver_extra:
            chat_id: "<#notify-log channel id>"
```

(No osquery routes; no global `secret:` under `extra` — every route is explicit. Never `chezmoi add` the live
`config.yaml`; #4775 rewrites it and would capture the resolved literal.)

- [ ] **Step 3 — migrate the moshi secret.** `grep -rn 'config/moshi/auth' .`; delete
  `dot_config/moshi/private_auth.json.tmpl` (moshi's secret now lives in `~/.config/relay/auth.json`,
  Task 1). Operator removes the stale `~/.config/moshi/auth.json`.
- [ ] **Step 4 — operator wiring (existing host).** `create_` won't touch the live `.env` or `config.yaml`,
  so on dresden: confirm `WEBHOOK_ENABLED=true` + `WEBHOOK_PORT=8644` are in the live `~/.hermes/.env` (they
  already are if the osquery webhook works); create the KeePassXC entry **Hermes :: Relay Webhook Secret**;
  hand-add the `routes.relay` block (Step 2, real secret from KeePassXC) to the live `config.yaml` beside the
  osquery routes. Restart Hermes if it doesn't hot-reload. Never `chezmoi add` the live `config.yaml`/`.env`.
- [ ] **Step 5 — live end-to-end check** (after `relay.sh` exists, Task 1):
  `~/.local/bin/relay.sh --agent test --state done --project relay --detail hi --pane "$HERDR_PANE_ID"` →
  phone push + `#notify-log` (HMAC validated) + clickable local.
- [ ] **Step 6 — CLAUDE.md.** Replace the moshi/notify sections with the relay section; document the Hermes
  tracking method (`.env` = `WEBHOOK_ENABLED`/`WEBHOOK_PORT` platform switch; explicit per-route secret as a
  keepassxc literal in a `create_` config; no global; the live-file clobber caution; the pending osquery
  merge), and add only `create_private_config.yaml.tmpl` to the KeePassXC-template list (the `.env` template
  has no secret, so it's automation-safe).
- [ ] **Step 7 — commit:**

```bash
git add dot_hermes/create_private_dot_env.tmpl dot_hermes/create_private_config.yaml.tmpl CLAUDE.md
git rm dot_config/moshi/private_auth.json.tmpl
git commit -m "feat(relay): track Hermes webhook activation + relay route secret (secrets-safe); retire moshi auth"
```

---

## Self-review

- **Spec coverage:** sender (T1), agent builder + Claude 4 states (T2), Codex done/blocked + moshi-hook
  exclusion + herdr-agent-state (T3), shell thresholds + skip-list (T4), Hermes route + secret migration +
  docs (T5), exact-pane via `herdr agent focus` (T1 local channel), failure separation (T1), open questions
  (T0). All covered.
- **Placeholders:** none — every script/test/edit shows complete code; `<...>` tokens are operator-supplied
  secrets/ids, not code gaps.
- **Type consistency:** `relay.sh` flags (`--agent/--state/--project/--branch/--detail/--pane/--local-only`)
  are used identically by `relay-agent.sh`, the shell notifier, and the tests; `RELAY_AGENT`/`RELAY_BIN`/
  `RELAY_AUTH_FILE`/`RELAY_MOSHI_URL`/`RELAY_HERMES_URL` are consistent across tasks.
