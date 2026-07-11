#!/usr/bin/env bash
# openclaw-services-retirement.sh -- proves the one-time convergent retirement
# chezmoiscript (run_once_after_61-retire-openclaw-services) boots out the three
# known ai.openclaw.* launchd agents, deletes their plists, and uninstalls the
# global npm `openclaw` package -- idempotently, loud one line per action, never
# failing the apply, and NEVER touching ~/.openclaw or any other launchd label.
#
# The REAL template is rendered with the host chezmoi (CI=1, scratch HOME) and
# the rendered body is run against a PATH-prepended stub dir whose `launchctl`,
# `npm`, and `rm` record argv. `launchctl print` reports "loaded" only for the
# labels listed in a per-case file; `rm` records then really deletes (so the
# idempotence case observes a genuine second-run no-op). Mirrors the render+stub
# approach in test/tailscaled-status.sh and the argv-recording stubs in
# test/update-skills-cua-driver-refresh.sh.
#
# Cases:
#   1. loaded + plists present -> 3 bootouts + 3 plist removals + npm uninstall,
#      each logged; ~/.openclaw and a decoy label/plist untouched.
#   2. nothing present         -> no bootout, no removal, no uninstall recorded.
#   3. plist-only (not loaded) -> plist removed, NO bootout recorded.
#   4. idempotence             -> a second run after case 1 records no actions.
#   5. guard (static)          -> the script text names ONLY the three
#      ai.openclaw.* labels, never `launchctl list`, never a wildcard bootout.
set -euo pipefail

# git exports GIT_DIR/GIT_INDEX_FILE when this runs under the pre-commit hook;
# unset so nothing here can reach the outer repository.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_once_after_61-retire-openclaw-services.sh.tmpl"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not on PATH; cannot render the retirement script\n'
  exit 0
}
[[ -f $SCRIPT ]] || fail "missing template: $SCRIPT"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Render the darwin-only script once (scratch HOME, CI=1 -- same mechanics as
# the treefmt rendered-template lint and the tailscaled status test). An empty
# render means a non-darwin host: skip.
rendered="$work/rendered.sh"
render_home="$(mktemp -d)"
HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$SCRIPT" >"$rendered" || fail "chezmoi failed to render $SCRIPT"
rm -rf "$render_home"
if [[ ! -s $rendered ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# ── Case 5: static guard on the rendered text ──────────────────────────────
# Only the three exact labels, no bare `launchctl list`, no wildcard target.
labels=(ai.openclaw.gateway ai.openclaw.node ai.openclaw.rescue)
for label in "${labels[@]}"; do
  grep -qF -- "$label" "$rendered" || fail "guard: rendered script never mentions $label"
done
if grep -qE 'launchctl[[:space:]]+list' "$rendered"; then
  fail "guard: rendered script uses 'launchctl list' (must probe with 'launchctl print')"
fi
if grep -qE 'openclaw[.*]\*|gui/[^/[:space:]]*/[^[:space:]]*\*' "$rendered"; then
  fail "guard: rendered script contains a wildcard launchd target"
fi
# Every ai.openclaw.<token> in the file must be one of the three known labels.
while IFS= read -r token; do
  case "$token" in
    ai.openclaw.gateway | ai.openclaw.node | ai.openclaw.rescue) ;;
    *) fail "guard: rendered script references an unexpected label root '$token'" ;;
  esac
done < <(grep -oE 'ai\.openclaw\.(gateway|node|rescue|[a-z]+)' "$rendered" | sort -u)

# ── Stub harness ────────────────────────────────────────────────────────────
stub_dir="$work/stubs"
mkdir -p "$stub_dir"
launchctl_log="$work/launchctl.log"
npm_log="$work/npm.log"
rm_log="$work/rm.log"
loaded_file="$work/loaded" # one label per line = "loaded" in gui/<uid>
npm_installed_flag="$work/npm-installed"

cat >"$stub_dir/launchctl" <<STUB
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$launchctl_log"
if [[ "\$1" == "print" ]]; then
  target="\$2"
  while IFS= read -r label; do
    [[ -n "\$label" && "\$target" == *"\$label" ]] && exit 0
  done <"$loaded_file"
  exit 1
fi
if [[ "\$1" == "bootout" ]]; then
  exit 0
fi
exit 0
STUB

cat >"$stub_dir/npm" <<STUB
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$npm_log"
if [[ "\$1" == "ls" ]]; then
  [[ -e "$npm_installed_flag" ]] && exit 0
  exit 1
fi
exit 0
STUB

# rm records argv, then really removes (so idempotence sees a true no-op).
cat >"$stub_dir/rm" <<STUB
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$rm_log"
exec /bin/rm "\$@"
STUB

chmod +x "$stub_dir/launchctl" "$stub_dir/npm" "$stub_dir/rm"

# run_case <case-home> : renders a fresh HOME layout is the caller's job; this
# runs the rendered script with the stubs prepended and captures stdout+rc.
run_script() {
  local case_home="$1"
  : >"$launchctl_log"
  : >"$npm_log"
  : >"$rm_log"
  RC=0
  OUT="$(HOME="$case_home" PATH="$stub_dir:$PATH" bash "$rendered" 2>/dev/null)" || RC=$?
}

failures=0
report() {
  local status="$1" msg="$2"
  if [[ $status == ok ]]; then
    printf '  ok   %s\n' "$msg"
  else
    printf '  FAIL %s\n' "$msg"
    failures=$((failures + 1))
  fi
}

# assert_rc0 <case> : the retirement chore must always exit 0.
assert_rc0() {
  if [[ $RC -eq 0 ]]; then
    report ok "$1: exits 0"
  else
    report bad "$1: exits 0 (rc=$RC, out: $OUT)"
  fi
}

# assert_absent <case> <path> <label>
assert_absent() {
  if [[ ! -e $2 ]]; then
    report ok "$1: removed $3"
  else
    report bad "$1: $3 still present"
  fi
}

# assert_present <case> <path> <label>
assert_present() {
  if [[ -e $2 ]]; then
    report ok "$1: left $3 untouched"
  else
    report bad "$1: touched $3"
  fi
}

# assert_logged <case> <substring> : the loud one-line-per-action log reached stdout.
assert_logged() {
  if grep -q -- "$2" <<<"$OUT"; then
    report ok "$1: logged '$2'"
  else
    report bad "$1: missing log '$2' (out: $OUT)"
  fi
}

printf 'openclaw-services-retirement cases:\n'

# ── Case 1: loaded + plists present ─────────────────────────────────────────
h1="$work/home1"
mkdir -p "$h1/Library/LaunchAgents" "$h1/.openclaw/elevenlabs"
printf 'skill data\n' >"$h1/.openclaw/elevenlabs/config"
for label in "${labels[@]}"; do
  printf '<plist/>\n' >"$h1/Library/LaunchAgents/$label.plist"
  printf '%s\n' "$label" >>"$loaded_file"
done
# Decoy: an unrelated loaded label + plist that MUST survive.
printf 'com.webdavis.happy-daemon\n' >>"$loaded_file"
printf '<plist/>\n' >"$h1/Library/LaunchAgents/com.webdavis.happy-daemon.plist"
touch "$npm_installed_flag"

run_script "$h1"
assert_rc0 case1
for label in "${labels[@]}"; do
  if grep -q "bootout gui/.*/$label" "$launchctl_log"; then
    report ok "case1: booted out $label"
  else
    report bad "case1: no bootout for $label (log: $(cat "$launchctl_log"))"
  fi
  assert_absent case1 "$h1/Library/LaunchAgents/$label.plist" "$label.plist"
  # Loud one-line log per action (the requirement): bootout + removal reached stdout.
  assert_logged case1 "booted out gui/.*/$label"
  assert_logged case1 "removed .*$label.plist"
done
if grep -q 'uninstall .*openclaw' "$npm_log"; then
  report ok "case1: uninstalled npm openclaw"
else
  report bad "case1: no npm uninstall (log: $(cat "$npm_log"))"
fi
assert_logged case1 "uninstalled global npm package openclaw"
# The decoy label must NOT be booted out and its plist must survive.
if grep -q 'happy-daemon' "$launchctl_log"; then
  report bad "case1: touched an unrelated label (happy-daemon)"
else
  report ok "case1: left unrelated label untouched"
fi
assert_present case1 "$h1/Library/LaunchAgents/com.webdavis.happy-daemon.plist" "the unrelated plist"
# ~/.openclaw must be untouched.
assert_present case1 "$h1/.openclaw/elevenlabs/config" "the .openclaw skill data dir"

# ── Case 4 (uses case 1's now-drained home): idempotence ────────────────────
: >"$loaded_file" # nothing loaded anymore
rm -f "$npm_installed_flag"
run_script "$h1"
assert_rc0 case4
if grep -q bootout "$launchctl_log"; then
  report bad "case4: second run booted out something (log: $(cat "$launchctl_log"))"
else
  report ok "case4: second run records no bootout"
fi
if [[ -s $rm_log ]]; then
  report bad "case4: second run removed a plist (log: $(cat "$rm_log"))"
else
  report ok "case4: second run removes nothing"
fi
if grep -q uninstall "$npm_log"; then
  report bad "case4: second run uninstalled npm"
else
  report ok "case4: second run no npm uninstall"
fi

# ── Case 2: nothing present ─────────────────────────────────────────────────
h2="$work/home2"
mkdir -p "$h2/Library/LaunchAgents"
: >"$loaded_file"
rm -f "$npm_installed_flag"
run_script "$h2"
assert_rc0 case2
if grep -q bootout "$launchctl_log"; then
  report bad "case2: booted out something on an empty host"
else
  report ok "case2: no bootout"
fi
if [[ -s $rm_log ]]; then
  report bad "case2: removed something on an empty host"
else
  report ok "case2: no removal"
fi
if grep -q uninstall "$npm_log"; then
  report bad "case2: uninstalled npm on an empty host"
else
  report ok "case2: no npm uninstall"
fi

# ── Case 3: plist-only (not loaded) ─────────────────────────────────────────
h3="$work/home3"
mkdir -p "$h3/Library/LaunchAgents"
printf '<plist/>\n' >"$h3/Library/LaunchAgents/ai.openclaw.gateway.plist"
: >"$loaded_file" # gateway present on disk but NOT loaded
rm -f "$npm_installed_flag"
run_script "$h3"
assert_rc0 case3
if grep -q bootout "$launchctl_log"; then
  report bad "case3: booted out a not-loaded label"
else
  report ok "case3: no bootout for a not-loaded label"
fi
assert_absent case3 "$h3/Library/LaunchAgents/ai.openclaw.gateway.plist" "the orphan plist"

if [[ $failures -gt 0 ]]; then
  printf 'openclaw-services-retirement: %d assertion(s) FAILED\n' "$failures" >&2
  exit 1
fi
printf 'openclaw-services-retirement: OK (retire loaded+plist, no-op empty, plist-only, idempotent, guarded)\n'
