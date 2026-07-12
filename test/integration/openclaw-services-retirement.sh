#!/usr/bin/env bash
# openclaw-services-retirement.sh -- proves the convergent retirement
# chezmoiscript (run_after_61-retire-openclaw-services) boots out the three
# known ai.openclaw.* launchd agents, deletes their plists, and uninstalls the
# global npm `openclaw` package -- convergently, loud one line per action, never
# failing the apply, and NEVER touching ~/.openclaw or any other launchd label.
#
# The script is a run_after (not run_once): run_once records success permanently
# even when a probe or action transiently failed, so it instead gates on a
# quiescence marker at ~/.local/state/openclaw/retired. Steady state is one file
# check; the marker is written ONLY after a full postcondition sweep confirms
# all three labels absent, all three plists absent, and npm openclaw absent.
#
# The REAL template is rendered with the host chezmoi (CI=1, scratch HOME) and
# the rendered body is run against a PATH-prepended stub dir whose `launchctl`,
# `npm`, and `rm` record their FULL argv line and mutate shared state files:
# a successful `launchctl bootout` drops the label from the loaded set (so a
# later `launchctl print` of it reports absent), a successful `npm uninstall`
# clears the installed flag, and `rm` really deletes. Failure injection is via
# FAIL_BOOTOUT / FAIL_RM / FAIL_NPM_UNINSTALL env vars the stubs honor. Each
# case asserts the COMPLETE expected argv multiset (sorted diff -- extras fail,
# no unanchored substring matches). Mirrors the render+stub approach in
# test/tailscaled-status.sh and the argv-recording stubs in
# test/update-skills-cua-driver-refresh.sh.
#
# Cases:
#   A. loaded + plists present -> 3 bootouts + 3 plist removals + npm uninstall,
#      each logged; a postcondition sweep writes the marker; two decoy labels
#      (happy-daemon, atuin-daemon) and ~/.openclaw untouched.
#   B. idempotence (marker) -> re-run on A's SAME state: the marker fast path
#      short-circuits everything, ZERO stub invocations.
#   C. nothing present -> no bootout, no removal, no uninstall; marker written.
#   D. plist-only (not loaded) -> plist removed, NO bootout; marker written.
#   E. bootout fails -> its plist KEPT, other labels still processed, NO marker.
#   F. plist removal fails -> other labels processed, NO marker.
#   G. npm uninstall fails -> labels processed, NO marker.
#   H. transient failure then retry -> re-run of E's home (injection cleared)
#      converges and writes the marker.
#   Guard (static) -> the rendered text names ONLY the three ai.openclaw.*
#      labels, never `launchctl list`, never a wildcard bootout.
set -euo pipefail

# git exports GIT_DIR/GIT_INDEX_FILE when this runs under the pre-commit hook;
# unset so nothing here can reach the outer repository.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_after_61-retire-openclaw-services.sh.tmpl"

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

labels=(ai.openclaw.gateway ai.openclaw.node ai.openclaw.rescue)
uid="$(id -u)"

# ── Guard: static checks on the rendered text ───────────────────────────────
# Only the three exact labels, no bare `launchctl list`, no wildcard target.
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
# Stub bodies are written with `printf '%s'` (NOT heredocs): homebrew bash
# 5.3.15 can deadlock on heredoc writes when a sibling test suite runs
# concurrently, so this suite stays heredoc-free. The stubs read their log and
# state paths from the exported env below, and the per-case failure switches
# (FAIL_BOOTOUT / FAIL_RM / FAIL_NPM_UNINSTALL) from the run env.
stub_dir="$work/stubs"
mkdir -p "$stub_dir"
export LAUNCHCTL_LOG="$work/launchctl.log"
export NPM_LOG="$work/npm.log"
export RM_LOG="$work/rm.log"
export LOADED_FILE="$work/loaded"     # one label per line = "loaded" in gui/<uid>
export NPM_FLAG="$work/npm-installed" # exists = openclaw globally installed
launchctl_log="$LAUNCHCTL_LOG"
npm_log="$NPM_LOG"
rm_log="$RM_LOG"
loaded_file="$LOADED_FILE"
npm_installed_flag="$NPM_FLAG"
out_file="$work/stdout"
err_file="$work/stderr"

# launchctl: records argv; `print` is TRI-STATE -- it exits 0 for a loaded label
# (in LOADED_FILE), 113 for a confirmed-absent one (the known not-found status),
# and 112 (an operational error, e.g. an invalid GUI domain) for any label named
# in FAIL_PRINT. A successful `bootout` drops the label (stateful, so a later
# print reports 113). FAIL_BOOTOUT names a label whose bootout fails and leaves
# it loaded.
# shellcheck disable=SC2016  # stub body is literal; $vars resolve when it runs
launchctl_stub='#!/usr/bin/env bash
printf "%s\n" "$*" >>"$LAUNCHCTL_LOG"
if [[ "$1" == "print" ]]; then
  target="$2"
  label="${target##*/}"
  if [[ -n "${FAIL_PRINT:-}" && "$label" == "$FAIL_PRINT" ]]; then
    exit 112
  fi
  while IFS= read -r loaded; do
    [[ -n "$loaded" && "$target" == *"$loaded" ]] && exit 0
  done <"$LOADED_FILE"
  exit 113
fi
if [[ "$1" == "bootout" ]]; then
  target="$2"
  label="${target##*/}"
  if [[ -n "${FAIL_BOOTOUT:-}" && "$label" == "$FAIL_BOOTOUT" ]]; then
    exit 1
  fi
  grep -vFx -- "$label" "$LOADED_FILE" >"$LOADED_FILE.tmp" 2>/dev/null || true
  mv "$LOADED_FILE.tmp" "$LOADED_FILE"
  exit 0
fi
exit 0'

# npm: records argv; `ls -g openclaw --json` emits the npm --json tree so the
# script parses the verdict from it (not the exit code): a .dependencies.openclaw
# entry when NPM_FLAG exists (installed), an entry-less object otherwise
# (confirmed absent). NPM_LS_BROKEN makes `ls` emit non-JSON on stderr and exit
# non-zero (could-not-determine -> unknown). A successful `uninstall` clears the
# flag (stateful); FAIL_NPM_UNINSTALL makes uninstall fail with the flag intact.
# shellcheck disable=SC2016  # stub body is literal; $vars resolve when it runs
npm_stub='#!/usr/bin/env bash
printf "%s\n" "$*" >>"$NPM_LOG"
if [[ "$1" == "ls" ]]; then
  if [[ -n "${NPM_LS_BROKEN:-}" ]]; then
    printf "npm error: registry unreachable\n" >&2
    exit 1
  fi
  if [[ -e "$NPM_FLAG" ]]; then
    printf "{\"name\":\"lib\",\"dependencies\":{\"openclaw\":{\"version\":\"1.0.0\"}}}\n"
    exit 0
  fi
  printf "{\"name\":\"lib\"}\n"
  exit 1
fi
if [[ "$1" == "uninstall" ]]; then
  [[ -n "${FAIL_NPM_UNINSTALL:-}" ]] && exit 1
  /bin/rm -f "$NPM_FLAG"
  exit 0
fi
exit 0'

# rm records argv, then really removes (so idempotence sees a true no-op).
# FAIL_RM names a path whose removal fails (the file survives).
# shellcheck disable=SC2016  # stub body is literal; $vars resolve when it runs
rm_stub='#!/usr/bin/env bash
printf "%s\n" "$*" >>"$RM_LOG"
if [[ -n "${FAIL_RM:-}" ]]; then
  for arg in "$@"; do
    [[ "$arg" == "$FAIL_RM" ]] && exit 1
  done
fi
exec /bin/rm "$@"'

printf '%s\n' "$launchctl_stub" >"$stub_dir/launchctl"
printf '%s\n' "$npm_stub" >"$stub_dir/npm"
printf '%s\n' "$rm_stub" >"$stub_dir/rm"
chmod +x "$stub_dir/launchctl" "$stub_dir/npm" "$stub_dir/rm"

# run_script <case-home> [ENV=val ...] : runs the rendered script with the
# stubs prepended and the given env, capturing stdout (OUT), stderr (ERR), rc.
run_script() {
  local case_home="$1"
  shift
  : >"$launchctl_log"
  : >"$npm_log"
  : >"$rm_log"
  RC=0
  # Capture stdout/stderr to files (grep the files, never `printf | grep -q`):
  # under `set -o pipefail` a `grep -q` early-exit sends the upstream printf
  # SIGPIPE and the pipeline reports failure even on a match.
  HOME="$case_home" PATH="$stub_dir:$PATH" env "$@" bash "$rendered" \
    >"$out_file" 2>"$err_file" || RC=$?
  OUT="$(cat "$out_file")"
  ERR="$(cat "$err_file")"
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
    report bad "$1: exits 0 (rc=$RC, out: $OUT, err: $ERR)"
  fi
}

# assert_log_exact <case> <logfile> <label> <expected-line...> : the recorded
# argv multiset equals exactly the expected lines (sorted diff -- rejects
# extras and missing, full-line match only).
assert_log_exact() {
  local case="$1" logfile="$2" what="$3"
  shift 3
  local expected="$work/expected"
  if [[ $# -eq 0 ]]; then
    : >"$expected"
  else
    printf '%s\n' "$@" >"$expected"
  fi
  if diff <(sort "$expected") <(sort "$logfile") >/dev/null 2>&1; then
    report ok "$case: exact $what argv"
  else
    report bad "$case: $what argv mismatch (got: $(tr '\n' '|' <"$logfile"))"
  fi
}

# assert_empty_log <case> <logfile> <label>
assert_empty_log() {
  if [[ ! -s $2 ]]; then
    report ok "$1: no $3 invocations"
  else
    report bad "$1: unexpected $3 invocations ($(tr '\n' '|' <"$2"))"
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

# assert_logged <case> <substring> : the loud action log reached stdout.
# Greps the captured stdout FILE (no here-string: homebrew bash 5.3.15 can
# deadlock on here-string writes under concurrent-suite load; no pipe: pipefail
# would misreport a `grep -q` early-exit as failure).
assert_logged() {
  if grep -q -- "$2" "$out_file"; then
    report ok "$1: logged '$2'"
  else
    report bad "$1: missing log '$2' (out: $OUT)"
  fi
}

# assert_warned <case> <substring> : a warning reached stderr.
assert_warned() {
  if grep -q -- "$2" "$err_file"; then
    report ok "$1: warned '$2'"
  else
    report bad "$1: missing warning '$2' (err: $ERR)"
  fi
}

# assert_no_npm_uninstall <case> : no `npm uninstall` was attempted (the package
# must not be pulled out from under a still-loaded openclaw job).
assert_no_npm_uninstall() {
  if grep -q '^uninstall ' "$npm_log"; then
    report bad "$1: npm uninstall attempted though the launchd services are not fully retired"
  else
    report ok "$1: no npm uninstall while services are not fully retired"
  fi
}

marker_of() { printf '%s/.local/state/openclaw/retired' "$1"; }
assert_marker() {
  if [[ -f "$(marker_of "$2")" ]]; then
    report ok "$1: marker written"
  else
    report bad "$1: marker NOT written"
  fi
}
assert_no_marker() {
  if [[ ! -e "$(marker_of "$2")" ]]; then
    report ok "$1: marker NOT written (incomplete convergence)"
  else
    report bad "$1: marker written despite incomplete convergence"
  fi
}

seed_loaded() {
  : >"$loaded_file"
  local label
  for label in "$@"; do
    printf '%s\n' "$label" >>"$loaded_file"
  done
}

printf 'openclaw-services-retirement cases:\n'

# ── Case A: loaded + plists present, npm installed, two decoys ──────────────
hA="$work/homeA"
mkdir -p "$hA/Library/LaunchAgents" "$hA/.openclaw/elevenlabs"
printf 'skill data\n' >"$hA/.openclaw/elevenlabs/config"
for label in "${labels[@]}"; do
  printf '<plist/>\n' >"$hA/Library/LaunchAgents/$label.plist"
done
# Two decoys that MUST survive: a smuggled non-openclaw label escapes only a
# fuzzy guard, so both must be untouched.
printf '<plist/>\n' >"$hA/Library/LaunchAgents/com.webdavis.happy-daemon.plist"
printf '<plist/>\n' >"$hA/Library/LaunchAgents/com.webdavis.atuin-daemon.plist"
seed_loaded "${labels[@]}" com.webdavis.happy-daemon com.webdavis.atuin-daemon
touch "$npm_installed_flag"

run_script "$hA"
assert_rc0 caseA
# Per loaded label: initial print, bootout, re-probe print (confirms absence
# before the plist goes); then the sweep re-probes each once more.
assert_log_exact caseA "$launchctl_log" launchctl \
  "print gui/$uid/ai.openclaw.gateway" \
  "bootout gui/$uid/ai.openclaw.gateway" \
  "print gui/$uid/ai.openclaw.gateway" \
  "print gui/$uid/ai.openclaw.node" \
  "bootout gui/$uid/ai.openclaw.node" \
  "print gui/$uid/ai.openclaw.node" \
  "print gui/$uid/ai.openclaw.rescue" \
  "bootout gui/$uid/ai.openclaw.rescue" \
  "print gui/$uid/ai.openclaw.rescue" \
  "print gui/$uid/ai.openclaw.gateway" \
  "print gui/$uid/ai.openclaw.node" \
  "print gui/$uid/ai.openclaw.rescue"
assert_log_exact caseA "$rm_log" rm \
  "-f $hA/Library/LaunchAgents/ai.openclaw.gateway.plist" \
  "-f $hA/Library/LaunchAgents/ai.openclaw.node.plist" \
  "-f $hA/Library/LaunchAgents/ai.openclaw.rescue.plist"
assert_log_exact caseA "$npm_log" npm \
  "ls -g openclaw --json" \
  "uninstall -g openclaw" \
  "ls -g openclaw --json"
for label in "${labels[@]}"; do
  assert_absent caseA "$hA/Library/LaunchAgents/$label.plist" "$label.plist"
  assert_logged caseA "booted out gui/$uid/$label"
  assert_logged caseA "removed .*$label.plist"
done
assert_logged caseA "uninstalled global npm package openclaw"
assert_present caseA "$hA/Library/LaunchAgents/com.webdavis.happy-daemon.plist" "the happy-daemon decoy plist"
assert_present caseA "$hA/Library/LaunchAgents/com.webdavis.atuin-daemon.plist" "the atuin-daemon decoy plist"
assert_present caseA "$hA/.openclaw/elevenlabs/config" "the .openclaw skill data dir"
assert_marker caseA "$hA"

# ── Case B: idempotence via the marker (re-run A's now-drained home) ─────────
run_script "$hA"
assert_rc0 caseB
assert_empty_log caseB "$launchctl_log" launchctl
assert_empty_log caseB "$npm_log" npm
assert_empty_log caseB "$rm_log" rm
assert_marker caseB "$hA"

# ── Case C: nothing present ─────────────────────────────────────────────────
hC="$work/homeC"
mkdir -p "$hC/Library/LaunchAgents"
seed_loaded
/bin/rm -f "$npm_installed_flag"
run_script "$hC"
assert_rc0 caseC
assert_log_exact caseC "$launchctl_log" launchctl \
  "print gui/$uid/ai.openclaw.gateway" \
  "print gui/$uid/ai.openclaw.node" \
  "print gui/$uid/ai.openclaw.rescue" \
  "print gui/$uid/ai.openclaw.gateway" \
  "print gui/$uid/ai.openclaw.node" \
  "print gui/$uid/ai.openclaw.rescue"
assert_empty_log caseC "$rm_log" rm
assert_log_exact caseC "$npm_log" npm "ls -g openclaw --json" "ls -g openclaw --json"
assert_marker caseC "$hC"

# ── Case D: plist-only (not loaded) ─────────────────────────────────────────
hD="$work/homeD"
mkdir -p "$hD/Library/LaunchAgents"
printf '<plist/>\n' >"$hD/Library/LaunchAgents/ai.openclaw.gateway.plist"
seed_loaded
/bin/rm -f "$npm_installed_flag"
run_script "$hD"
assert_rc0 caseD
if grep -q bootout "$launchctl_log"; then
  report bad "caseD: booted out a not-loaded label"
else
  report ok "caseD: no bootout for a not-loaded label"
fi
assert_log_exact caseD "$rm_log" rm "-f $hD/Library/LaunchAgents/ai.openclaw.gateway.plist"
assert_absent caseD "$hD/Library/LaunchAgents/ai.openclaw.gateway.plist" "the orphan plist"
assert_marker caseD "$hD"

# ── Case E: bootout fails -> its plist kept, others processed, no marker ─────
hE="$work/homeE"
mkdir -p "$hE/Library/LaunchAgents"
for label in "${labels[@]}"; do
  printf '<plist/>\n' >"$hE/Library/LaunchAgents/$label.plist"
done
seed_loaded "${labels[@]}"
touch "$npm_installed_flag"
run_script "$hE" FAIL_BOOTOUT=ai.openclaw.gateway
assert_rc0 caseE
assert_warned caseE "failed to bootout gui/$uid/ai.openclaw.gateway"
assert_present caseE "$hE/Library/LaunchAgents/ai.openclaw.gateway.plist" "the still-loaded gateway plist"
assert_absent caseE "$hE/Library/LaunchAgents/ai.openclaw.node.plist" "node.plist (other label still processed)"
assert_absent caseE "$hE/Library/LaunchAgents/ai.openclaw.rescue.plist" "rescue.plist (other label still processed)"
assert_no_marker caseE "$hE"
# gateway's plist removal is skipped while it is still loaded.
assert_log_exact caseE "$rm_log" rm \
  "-f $hE/Library/LaunchAgents/ai.openclaw.node.plist" \
  "-f $hE/Library/LaunchAgents/ai.openclaw.rescue.plist"
# B-F5 (F2): the gateway is STILL loaded, so the npm package must NOT be
# uninstalled -- doing so would pull dist/index.js out from under the running
# KeepAlive job. The uninstall is gated on EVERY label absent AND every plist
# gone; here it must be deferred and the package left installed.
assert_no_npm_uninstall caseE
assert_present caseE "$npm_installed_flag" "the openclaw npm package (a still-loaded job forbids uninstall)"

# ── Case F: plist removal fails -> no marker, others processed ───────────────
hF="$work/homeF"
mkdir -p "$hF/Library/LaunchAgents"
for label in "${labels[@]}"; do
  printf '<plist/>\n' >"$hF/Library/LaunchAgents/$label.plist"
done
seed_loaded "${labels[@]}"
touch "$npm_installed_flag"
run_script "$hF" "FAIL_RM=$hF/Library/LaunchAgents/ai.openclaw.gateway.plist"
assert_rc0 caseF
assert_present caseF "$hF/Library/LaunchAgents/ai.openclaw.gateway.plist" "the un-removable gateway plist"
assert_absent caseF "$hF/Library/LaunchAgents/ai.openclaw.node.plist" "node.plist (other label still processed)"
assert_no_marker caseF "$hF"
# rm was attempted for all three (gateway attempt fails inside the stub).
assert_log_exact caseF "$rm_log" rm \
  "-f $hF/Library/LaunchAgents/ai.openclaw.gateway.plist" \
  "-f $hF/Library/LaunchAgents/ai.openclaw.node.plist" \
  "-f $hF/Library/LaunchAgents/ai.openclaw.rescue.plist"
# B-F5 (F2): the gateway plist survived, so retirement is incomplete -- the npm
# uninstall must be deferred and the package left installed.
assert_no_npm_uninstall caseF
assert_present caseF "$npm_installed_flag" "the openclaw npm package (an un-removed plist forbids uninstall)"

# ── Case G: npm uninstall fails -> no marker, labels processed ───────────────
hG="$work/homeG"
mkdir -p "$hG/Library/LaunchAgents"
for label in "${labels[@]}"; do
  printf '<plist/>\n' >"$hG/Library/LaunchAgents/$label.plist"
done
seed_loaded "${labels[@]}"
touch "$npm_installed_flag"
run_script "$hG" FAIL_NPM_UNINSTALL=1
assert_rc0 caseG
for label in "${labels[@]}"; do
  assert_absent caseG "$hG/Library/LaunchAgents/$label.plist" "$label.plist (labels processed)"
done
assert_no_marker caseG "$hG"
assert_log_exact caseG "$npm_log" npm "ls -g openclaw --json" "uninstall -g openclaw" "ls -g openclaw --json"

# ── Case H: transient failure then retry converges (re-run E's home) ────────
# E left the gateway loaded (bootout injected to fail) and no marker. A second
# apply with the injection gone must converge and write the marker.
run_script "$hE"
assert_rc0 caseH
assert_absent caseH "$hE/Library/LaunchAgents/ai.openclaw.gateway.plist" "gateway.plist on retry"
assert_marker caseH "$hE"

# ── Case I: operational probe error (112, not 113) -> plist KEPT, no marker ──
# The core B-F7 defect: a non-113 launchctl-print error (112 = invalid GUI
# domain) must NOT be misread as "unloaded". The label's state is unknown, so
# the script must NOT bootout it and must NOT delete its plist, and the sweep
# leaves the marker unwritten.
hI="$work/homeI"
mkdir -p "$hI/Library/LaunchAgents"
for label in "${labels[@]}"; do
  printf '<plist/>\n' >"$hI/Library/LaunchAgents/$label.plist"
done
seed_loaded
/bin/rm -f "$npm_installed_flag"
run_script "$hI" FAIL_PRINT=ai.openclaw.gateway
assert_rc0 caseI
if grep -q "bootout gui/$uid/ai.openclaw.gateway" "$launchctl_log"; then
  report bad "caseI: booted out a label whose load state is unknown (112 probe error)"
else
  report ok "caseI: no bootout for an operational-error (112) probe"
fi
assert_present caseI "$hI/Library/LaunchAgents/ai.openclaw.gateway.plist" "the unknown-state gateway plist"
assert_warned caseI "could not determine the load state of gui/$uid/ai.openclaw.gateway"
assert_no_marker caseI "$hI"
# node/rescue were confirmed absent (113) and had their plists removed.
assert_absent caseI "$hI/Library/LaunchAgents/ai.openclaw.node.plist" "node.plist (confirmed absent)"
assert_absent caseI "$hI/Library/LaunchAgents/ai.openclaw.rescue.plist" "rescue.plist (confirmed absent)"

# ── Case J: npm probe cannot determine -> no uninstall, no marker ────────────
# `npm ls` failing indeterminately (registry unreachable) must be distinguished
# from a confirmed absence: the script must NOT treat it as retired. No
# uninstall is attempted, a warning fires, and the marker stays unwritten even
# though every launchd label is clean.
hJ="$work/homeJ"
mkdir -p "$hJ/Library/LaunchAgents"
seed_loaded
touch "$npm_installed_flag"
run_script "$hJ" NPM_LS_BROKEN=1
assert_rc0 caseJ
if grep -q '^uninstall ' "$npm_log"; then
  report bad "caseJ: attempted an uninstall though the npm probe could not determine installation"
else
  report ok "caseJ: no uninstall on an indeterminate npm probe"
fi
assert_warned caseJ "could not determine whether the global npm package openclaw is installed"
assert_no_marker caseJ "$hJ"
/bin/rm -f "$npm_installed_flag"

# ── Case K (F2): an unknown-state label forbids the npm uninstall ────────────
# 112 (operational probe error) leaves the gateway's load state unknown, so the
# services are NOT fully retired. The npm uninstall is gated on EVERY label
# confirmed absent AND every plist gone, so it must be deferred here and the
# package left installed even though the other two labels are clean.
hK="$work/homeK"
mkdir -p "$hK/Library/LaunchAgents"
seed_loaded
touch "$npm_installed_flag"
run_script "$hK" FAIL_PRINT=ai.openclaw.gateway
assert_rc0 caseK
assert_no_npm_uninstall caseK
assert_present caseK "$npm_installed_flag" "the openclaw npm package (an unknown-state label forbids uninstall)"
assert_no_marker caseK "$hK"
/bin/rm -f "$npm_installed_flag"

# ── Case L (F3): a missing npm binary is "unknown", never "absent" ───────────
# A system-only PATH (an automation env) can hide Homebrew's npm. Treating a
# missing npm as confirmed-absent would write the PERMANENT quiescence marker
# while the global package silently survives forever. Missing npm is
# indeterminate: the marker is withheld and the next apply retries. Runs on an
# ISOLATED PATH that carries the launchctl/rm stubs but no npm at all.
nonpm_dir="$work/stubs-nonpm"
mkdir -p "$nonpm_dir"
ln -s "$stub_dir/launchctl" "$nonpm_dir/launchctl"
ln -s "$stub_dir/rm" "$nonpm_dir/rm"
L_PATH="$nonpm_dir:/usr/bin:/bin"
if PATH="$L_PATH" command -v npm >/dev/null 2>&1; then
  report ok "caseL: SKIPPED (npm leaks into the isolated PATH; cannot exercise a missing npm)"
else
  hL="$work/homeL"
  mkdir -p "$hL/Library/LaunchAgents"
  seed_loaded
  : >"$launchctl_log"
  : >"$npm_log"
  : >"$rm_log"
  RCL=0
  HOME="$hL" PATH="$L_PATH" bash "$rendered" >"$out_file" 2>"$err_file" || RCL=$?
  if [[ $RCL -eq 0 ]]; then
    report ok "caseL: exits 0"
  else
    report bad "caseL: exits 0 (rc=$RCL, err: $(cat "$err_file"))"
  fi
  assert_no_marker caseL "$hL"
fi

# ── Case M (F4): an unwritable state dir must not abort the apply ────────────
# A full convergence (nothing loaded, no plists, npm confirmed absent) reaches
# the marker write, but `mkdir -p`/`: >marker` are under `set -euo pipefail`; an
# unwritable state dir (a root-owned leftover) would abort the entire apply. The
# guarded write must warn, write no marker, and still exit 0. The state dir path
# is pre-created as a FILE so mkdir -p and the marker write both fail.
hM="$work/homeM"
mkdir -p "$hM/.local/state" "$hM/Library/LaunchAgents"
: >"$hM/.local/state/openclaw"
seed_loaded
/bin/rm -f "$npm_installed_flag"
run_script "$hM"
assert_rc0 caseM
assert_no_marker caseM "$hM"
assert_warned caseM "could not be written"

if [[ $failures -gt 0 ]]; then
  printf 'openclaw-services-retirement: %d assertion(s) FAILED\n' "$failures" >&2
  exit 1
fi
printf 'openclaw-services-retirement: OK (retire+marker, idempotent fast path, empty/plist-only converge, bootout/plist/npm failures hold the marker, transient retry converges, guarded)\n'
