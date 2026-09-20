#!/usr/bin/env bash
#
# pns-reminder-extension -- the pi and omp reminder extensions must arm pns's
# unanswered-approval reminder when the harness opens a question and clear it
# when the harness closes one.
#
# Arming without clearing is the failure that matters: a reminder nobody can
# clear nags the operator about a question they already answered, so both ends
# of each pair are driven here against a stub engine that records its argv, its
# producer and the payload it was handed.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# ONE RENDER FOR THE WHOLE FILE. chezmoi's template render is the expensive
# part here and nothing a test does changes its input, so both extensions are
# rendered once and each test only clears the engine's log.
set_up_before_script() {
  unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE
  SANDBOX="$(mktemp -d)"
  PNS_STUB_LOG="$SANDBOX/pns.log"
  export PNS_STUB_LOG

  # The engine the extension calls, recording its argv, its producer and the
  # payload it was handed. It sits where the template joins the install dir
  # onto the home the render is given, which is what puts it in the extension.
  mkdir -p "$SANDBOX/.cargo/bin"
  cat >"$SANDBOX/.cargo/bin/pns" <<'STUB'
#!/bin/bash
{
  printf 'argv:%s\n' "$*"
  printf 'producer:%s\n' "$PNS_PRODUCER"
  printf 'payload:'
  cat
} >>"$PNS_STUB_LOG"
STUB
  chmod +x "$SANDBOX/.cargo/bin/pns"

  HOME="$SANDBOX" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
    <"$REPO_ROOT/dot_pi/agent/private_extensions/pns-reminders.ts.tmpl" \
    >"$SANDBOX/pi-reminders.ts"
  HOME="$SANDBOX" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
    <"$REPO_ROOT/private_dot_omp/private_agent/private_extensions/pns-reminders.ts.tmpl" \
    >"$SANDBOX/omp-reminders.ts"
}

tear_down_after_script() {
  rm -rf "$SANDBOX"
}

set_up() {
  : >"$PNS_STUB_LOG"
}

# fire <harness> <event> <event-json>: load the extension the way the harness
# does, hand it a recording `on`, and run the one handler. The engine call is
# detached, so the driver waits for the log to gain a line before it returns.
fire() {
  local harness="$1" event="$2" payload="$3"
  cat >"$SANDBOX/drive.mjs" <<'DRIVER'
const [, , modulePath, event, json, logPath] = process.argv
const { readFileSync } = await import("node:fs")
const handlers = {}
const module = await import(modulePath)
module.default({ on: (name, fn) => { handlers[name] = fn } })
if (!handlers[event]) {
  console.error(`no handler registered for ${event}`)
  process.exit(1)
}
await handlers[event](JSON.parse(json), { cwd: "/tmp/project" })
for (let i = 0; i < 200; i += 1) {
  if (readFileSync(logPath, "utf8").includes("payload:")) break
  await new Promise((resolve) => setTimeout(resolve, 5))
}
DRIVER
  node "$SANDBOX/drive.mjs" "$SANDBOX/$harness-reminders.ts" "$event" "$payload" "$PNS_STUB_LOG"
}

function test_pi_prompt_start_arms_a_reminder() {
  fire pi ui_prompt_start '{"sessionId":"s1","title":"Run bash?"}'
  assert_contains "argv:hook blocked --remind" "$(cat "$PNS_STUB_LOG")"
  assert_contains "producer:pi" "$(cat "$PNS_STUB_LOG")"
  assert_contains '"session_id":"s1"' "$(cat "$PNS_STUB_LOG")"
  assert_contains '"message":"Run bash?"' "$(cat "$PNS_STUB_LOG")"
}

function test_pi_prompt_end_clears_the_reminder() {
  fire pi ui_prompt_end '{"sessionId":"s1"}'
  assert_contains "argv:hook resolved" "$(cat "$PNS_STUB_LOG")"
  assert_contains "producer:pi" "$(cat "$PNS_STUB_LOG")"
}

function test_omp_approval_requested_arms_a_reminder() {
  fire omp tool_approval_requested '{"sessionId":"s2","toolName":"bash","reason":"writes"}'
  assert_contains "argv:hook blocked --remind" "$(cat "$PNS_STUB_LOG")"
  assert_contains "producer:omp" "$(cat "$PNS_STUB_LOG")"
  assert_contains '"tool_name":"bash"' "$(cat "$PNS_STUB_LOG")"
}

function test_omp_approval_resolved_clears_the_reminder() {
  fire omp tool_approval_resolved '{"sessionId":"s2","toolName":"bash"}'
  assert_contains "argv:hook resolved" "$(cat "$PNS_STUB_LOG")"
  assert_contains "producer:omp" "$(cat "$PNS_STUB_LOG")"
}

function test_a_session_with_no_id_anywhere_falls_back_to_a_per_session_id_not_the_bare_producer() {
  # No sessionManager on ctx and no session id on the event: the bare
  # producer name must never be the fallback, or two such sessions running at
  # once would share one pns wait and clear each other's reminder.
  fire omp tool_approval_requested '{"toolName":"bash"}'
  local logged
  logged="$(cat "$PNS_STUB_LOG")"
  assert_contains '"session_id":"omp-' "$logged"
  assert_not_contains '"session_id":"omp"' "$logged"
}
