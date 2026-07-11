#!/usr/bin/env bash
# update-skills-first-install-retry.sh, a failed first-install pass must stay
# RETRYABLE across applies without ever aborting the apply, and the retry marker
# must never be able to inject shell into the rendered chezmoiscript.
#
# The runner is exercised by RENDERING the chezmoiscript (chezmoi
# execute-template, with .chezmoi.homeDir pointed at a sandbox via $HOME) and
# then EXECUTING the rendered bytes with a stubbed updater on PATH. Assertions:
#   1. updater success  -> no marker, exit 0;
#   2. updater failure  -> a monotonic marker is created, exit 0 (NOT aborted);
#   3. repeated failure  -> the marker counter increments;
#   4. success after failure -> the marker is removed;
#   5. SECURITY: a hostile marker (a digit then a second shell line) renders as a
#      digits-only comment and neither executes nor aborts the render/apply;
#   6. template-content: the digits-only marker interpolation exists in the .tmpl;
#   7. mutating the UPDATER copy (not just the lock) changes the rendered
#      trigger content (the updater-hash line), so run_onchange re-fires on an
#      installer change too.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMPL="$REPO_ROOT/.chezmoiscripts/run_onchange_after_64-update-skills-first-install.sh.tmpl"
UPDATER_SRC="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# ── 6. template-content: the digits-only sanitizer is present in the source. ──
grep -q 'regexFind "\^\[0-9\]+"' "$TMPL" ||
  fail "the chezmoiscript does not sanitize the marker to digits with regexFind"
grep -q 'first-install-pending' "$TMPL" ||
  fail "the chezmoiscript does not reference the pending-marker file"
# The marker is read through a shell guard (cat ... || true) so an unreadable
# (mode-000) marker yields the safe zero path instead of a render abort.
grep -q 'output "sh"' "$TMPL" ||
  fail "the chezmoiscript does not read the marker via a guarded output call"
grep -q '10#' "$TMPL" ||
  fail "the chezmoiscript does not force base-10 on the retry counter"

# Sandbox home; render the runner against it (marker path resolves under here).
sbox="$tmp/home"
mkdir -p "$sbox/.local/bin"
MARKER="$sbox/.local/state/skills/first-install-pending"

# A stub updater whose exit code we control via FAKE_UPDATER_RC.
cat >"$sbox/.local/bin/update-skills.sh" <<'EOF'
#!/usr/bin/env bash
exit "${FAKE_UPDATER_RC:-0}"
EOF
chmod +x "$sbox/.local/bin/update-skills.sh"

render() { HOME="$sbox" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$TMPL" >"$1"; }
runner="$tmp/runner.sh"
render "$runner" || fail "rendering the chezmoiscript failed"

# ── 1. updater success -> no marker, exit 0. ────────────────────────────────
rm -rf "$sbox/.local/state"
FAKE_UPDATER_RC=0 HOME="$sbox" bash "$runner" || fail "runner exited non-zero on updater success"
[[ ! -e $MARKER ]] || fail "a marker was created on updater success"

# ── 2. updater failure -> marker created (count 1), exit 0. ─────────────────
FAKE_UPDATER_RC=1 HOME="$sbox" bash "$runner" || fail "runner aborted the apply on updater failure (must exit 0)"
[[ -f $MARKER ]] || fail "no pending marker was created on updater failure"
[[ "$(<"$MARKER")" == "1" ]] || fail "first failure did not record attempt 1: $(<"$MARKER")"

# ── 3. repeated failure -> counter increments. ──────────────────────────────
FAKE_UPDATER_RC=1 HOME="$sbox" bash "$runner" || fail "runner aborted on the second failure"
[[ "$(<"$MARKER")" == "2" ]] || fail "second failure did not bump the counter to 2: $(<"$MARKER")"

# ── 4. success after failure -> marker removed. ─────────────────────────────
FAKE_UPDATER_RC=0 HOME="$sbox" bash "$runner" || fail "runner exited non-zero clearing the marker"
[[ ! -e $MARKER ]] || fail "the pending marker was not removed after a success"

# ── 4a. base-10: a leading-zero marker (08) must NOT abort with an octal error;
#     it routes through the failure branch and increments to 9. ─────────────
mkdir -p "$(dirname "$MARKER")"
printf '08\n' >"$MARKER"
FAKE_UPDATER_RC=1 HOME="$sbox" bash "$runner" ||
  fail "an '08' marker aborted the failure branch (octal arithmetic bug: value too great for base)"
[[ "$(<"$MARKER")" == "9" ]] || fail "base-10 bump of '08' did not yield 9: $(<"$MARKER")"

# ── 4b. a multiline marker is inert: only the leading digits count, the second
#     line never executes, and the counter still increments off the first line.
sentinel2="$tmp/PWNED2"
rm -f "$sentinel2"
printf '5\ntouch %s\n' "$sentinel2" >"$MARKER"
FAKE_UPDATER_RC=1 HOME="$sbox" bash "$runner" ||
  fail "a multiline marker aborted the failure branch"
[[ ! -e $sentinel2 ]] || fail "a multiline marker executed its injected second line at run time"
[[ "$(<"$MARKER")" == "6" ]] || fail "a multiline marker did not increment off its leading digit: $(<"$MARKER")"
rm -f "$MARKER"

# ── 4c. render tolerates an UNREADABLE (mode-000) marker: no template abort,
#     the safe zero path is taken. ────────────────────────────────────────
mkdir -p "$(dirname "$MARKER")"
printf '3\n' >"$MARKER"
chmod 000 "$MARKER"
mode000_runner="$tmp/mode000.sh"
render "$mode000_runner" || fail "rendering with a mode-000 marker aborted the render (output of cat errored)"
chmod 644 "$MARKER"
rm -f "$MARKER"

# ── 5. SECURITY: a hostile marker must not inject shell. ────────────────────
sentinel="$tmp/PWNED"
rm -f "$sentinel"
mkdir -p "$(dirname "$MARKER")"
printf '5\ntouch %s\n' "$sentinel" >"$MARKER" # digit then an injected command
hostile_runner="$tmp/hostile.sh"
render "$hostile_runner" || fail "rendering with a hostile marker aborted the render"
# The injected command must not appear as a live (uncommented) line.
if grep -Eq '^[[:space:]]*touch ' "$hostile_runner"; then
  fail "the hostile marker injected a live 'touch' line into the rendered runner"
fi
# The digit still lands, but only inside the comment line.
grep -Eq '^# pending retry: 5$' "$hostile_runner" ||
  fail "the sanitized digit did not land in the pending-retry comment: $(grep -i 'pending retry' "$hostile_runner")"
# Executing the rendered runner must not run the injection.
FAKE_UPDATER_RC=0 HOME="$sbox" bash "$hostile_runner" || fail "the hostile-marker runner exited non-zero"
[[ ! -e $sentinel ]] || fail "the hostile marker executed injected shell (sentinel created)"

# ── 7. mutating the UPDATER copy changes the rendered trigger content. ───────
fix="$tmp/fixture_src"
mkdir -p "$fix/.chezmoiscripts" "$fix/dot_local/bin" "$fix/dot_agents"
cp "$TMPL" "$fix/.chezmoiscripts/$(basename "$TMPL")"
cp "$UPDATER_SRC" "$fix/dot_local/bin/executable_update-skills.sh"
printf '{"version":2,"tiers":{}}\n' >"$fix/dot_agents/custom-skill-lock.json"
render_fix() { CI=1 chezmoi --source "$fix" execute-template --no-tty <"$fix/.chezmoiscripts/$(basename "$TMPL")"; }
before="$(render_fix | grep 'updater hash:')" || fail "fixture render A failed"
printf '\n# a change to the installer\n' >>"$fix/dot_local/bin/executable_update-skills.sh"
after="$(render_fix | grep 'updater hash:')" || fail "fixture render B failed"
[[ $before != "$after" ]] ||
  fail "the rendered updater-hash line did not change when the updater copy changed (run_onchange would miss installer edits)"

echo "update-skills-first-install-retry: OK"
