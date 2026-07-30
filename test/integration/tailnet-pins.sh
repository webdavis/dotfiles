#!/usr/bin/env bash
# tailnet-pins.sh, MagicDNS /etc/hosts fallback pins are STRUCTURED DATA
# (`macos.tailnet_pins` in .chezmoidata/macos_system_setup.yaml). The Tier-2
# sudo runner template (run_onchange_after_41) renders ONE sudo call per pin
# into the deployed reconciler
# (dot_local/libexec/tailnet/executable_reconcile-hosts-pin.sh ->
# ~/.local/libexec/tailnet/reconcile-hosts-pin.sh), which owns the
# reconciliation.
#
# WHAT CHANGED AND WHY THE SUITE IS SHAPED THIS WAY. The reconciliation used to
# be a ~1KB single-line `sudo sh -c` body inside the template, and this suite
# re-declared that entire body as a literal expectation, then executed the
# literal after rewriting /etc/hosts into it with string surgery. So the suite
# exercised a COPY of the implementation and the two could drift. They did: a
# fail-open loopback gate lived in the body through three review passes. Now
# the behavior tests invoke the REAL script with the hosts path pointed at a
# fixture through its documented seam (TAILNET_PIN_HOSTS_FILE), and the render
# test asserts only the short line the template emits. There is no copy left to
# drift.
#
# LAYER 1, RENDER (fixture): copy the REAL template and the REAL reconciler into
# a temp chezmoi source dir with fixture chezmoidata carrying test-owned pins
# (TEST-NET-1 addresses, never real tailnet data), render it, and assert:
#   - the EXACT generated command string per pin, plus the helper-path
#     assignment and the guard that refuses when the helper is not deployed;
#   - `sudo -v` is emitted even when the system_setup commands list is EMPTY
#     (pins must still apply; the upfront timestamp covers them);
#   - a fixture with NO tailnet_pins key still renders (the `index` absent-key
#     gotcha) and keeps the `exit 0` early-return;
#   - a pin field that is not a single hosts column (whitespace including
#     newlines, a # that starts a hosts comment, empty, or missing) or is not a
#     STRING (a bare YAML number or boolean) REFUSES to render;
#   - two pins claiming the SAME name refuse to render, because a pin owns both
#     of its names and the second would delete the first's record forever;
#   - the helper path the render names is a path chezmoi actually deploys, and
#     the runner GATES on the helper's sha256 rather than only embedding it.
#
# LAYER 2, BEHAVIOR (the real reconciler against temp hosts files): idempotence,
# stale-IP correction, an exact line PLUS a stale duplicate collapses to exactly
# one line, the filter keys on hosts FIELD STRUCTURE (grep word-boundary victims
# like pin.example.test.evil survive), the pin owns its short name, comments and
# blank lines survive a rebuild, every refusal exits NONZERO and says why on
# stderr, a failed install never truncates the target and leaves no temp, EVERY
# signal the reconciler declares for cleanup leaves no temp either (the declared
# list and this suite's own copy are enumerated and diffed, and the cases are
# driven by the reconciler's), an UNREADABLE source is refused rather than read
# as an empty file, the loopback gate cannot be satisfied by the record the
# reconciler itself just appended, the gate HONOURS an indented loopback record
# because the resolver does, and one that does not name `localhost`, while still
# refusing an indented comment-only one and a line whose NAME is the loopback
# address, a set-but-empty seam never falls back to the real /etc/hosts, the
# installed file keeps the target's mode AND its owner, the temporary file is
# created beside the target rather than in $TMPDIR, a symlinked target keeps its
# indirection and its referent's metadata, a missing trailing newline never
# produces a joined line and never reads as CONVERGED, and a field that is not
# one hosts column is refused at the entry point too.
#
# TWO INTERPRETERS. sudo on this machine has no secure_path, so the `bash` that
# runs the deployed helper as root is whichever the invoking PATH resolves:
# /opt/homebrew/bin/bash 5.3 with Homebrew ahead of the system paths, /bin/bash
# 3.2 without it. The two disagree about when a failed redirect aborts a
# function and about which fatal signals let an EXIT trap run, and both
# disagreements have produced live defects here, so the cases that turn on them
# run under EVERY interpreter this host has rather than under the one that
# happens to be first on PATH.
#
# LAYER 2d, INERTNESS: hostile pin data (command substitution, backticks, a %s
# printf directive, and single quotes, the one character shellSingleQuoted
# exists to rewrite) must ride as positional arguments, never execute in either
# shell, and arrive in the hosts file byte-exact.
#
# LAYER 3, SHAPE (real data): read the real YAML's pins via yq and validate form
# only, fields non-empty, ip inside the proper Tailscale CGNAT range
# 100.64.0.0/10, fqdn ends .ts.net, short == the fqdn's first label. No
# behavioral expectations are derived from real data.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_41-macos-system-setup.sh.tmpl"
YAML="$REPO_ROOT/.chezmoidata/macos_system_setup.yaml"
# The reconciler under test, addressed by its chezmoi SOURCE path. Nothing here
# re-derives its body; it is executed as-is.
RECONCILER_SOURCE_PATH="dot_local/libexec/tailnet/executable_reconcile-hosts-pin.sh"
RECONCILER="$REPO_ROOT/$RECONCILER_SOURCE_PATH"
# The target path the rendered runner must name for that source file.
RECONCILER_TARGET_SUFFIX=".local/libexec/tailnet/reconcile-hosts-pin.sh"
# Declared here rather than read from the reconciler: this suite must be able to
# disagree with the implementation, which is the whole reason the old suite's
# copy-of-the-body approach was thrown away.
LOOPBACK_ADDRESS_UNDER_TEST="127.0.0.1"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# Host-tool guards: plain test/*.sh scripts run outside the Nix shell.
for tool in chezmoi yq; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'SKIP: %s not on PATH; cannot exercise the tailnet-pins machinery\n' "$tool"
    exit 0
  fi
done
[[ -f $TEMPLATE ]] || fail "missing template: $TEMPLATE"
[[ -f $YAML ]] || fail "missing data file: $YAML"
[[ -x $RECONCILER ]] || fail "missing or non-executable reconciler: $RECONCILER"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Every bash on this host, because which one runs the helper as root is decided
# by the invoking PATH (see TWO INTERPRETERS above). The list is built from what
# actually exists, so a machine with only the system bash still runs the suite;
# it is never empty, because this script is itself running under a bash.
INTERPRETERS=()
for candidate in /bin/bash /opt/homebrew/bin/bash /usr/local/bin/bash; do
  [[ -x $candidate ]] && INTERPRETERS+=("$candidate")
done
if [[ ${#INTERPRETERS[@]} -eq 0 ]]; then
  INTERPRETERS=("$(command -v bash)")
fi

interpreter_label() { # <path>
  # shellcheck disable=SC2016  # $BASH_VERSION must be read by the interpreter
  # being labelled, not by this shell, so it stays unexpanded here.
  printf '%s (%s)' "$1" "$("$1" -c 'echo "$BASH_VERSION"')"
}

# stage_fixture_source <name>: a chezmoi source dir holding the REAL template
# and the REAL reconciler. The reconciler must be there because the template
# embeds its hash (`include ... | sha256sum`), which is what makes a change to
# the reconciliation logic re-trigger the hash-gated runner.
stage_fixture_source() {
  local src="$work/$1-src"
  mkdir -p "$src/.chezmoiscripts" "$src/.chezmoidata" \
    "$src/$(dirname "$RECONCILER_SOURCE_PATH")"
  cp "$TEMPLATE" "$src/.chezmoiscripts/"
  cp "$RECONCILER" "$src/$RECONCILER_SOURCE_PATH"
  printf '%s\n' "$src"
}

# render_fixture <name> <fixture-yaml-body...on stdin> -> $work/<name>.rendered
render_fixture() {
  local name="$1"
  local src
  src="$(stage_fixture_source "$name")"
  cat >"$src/.chezmoidata/macos_system_setup.yaml"
  local render_home="$work/$name-home"
  mkdir -p "$render_home"
  HOME="$render_home" CI=1 chezmoi --source "$src" execute-template --no-tty \
    <"$src/.chezmoiscripts/$(basename "$TEMPLATE")" >"$work/$name.rendered" ||
    fail "$name: chezmoi failed to render the template (absent-key gotcha? see {{ index }})"
}

# render_fixture_must_fail <name> <error-substring> <fixture-yaml on stdin>:
# the render itself must ABORT (malformed pin data is refused at render time,
# before a root shell ever sees it), and the refusal must name the reason.
render_fixture_must_fail() {
  local name="$1" want_error="$2"
  local src
  src="$(stage_fixture_source "$name")"
  cat >"$src/.chezmoidata/macos_system_setup.yaml"
  local render_home="$work/$name-home"
  mkdir -p "$render_home"
  local render_error="$work/$name.render-err"
  if HOME="$render_home" CI=1 chezmoi --source "$src" execute-template --no-tty \
    <"$src/.chezmoiscripts/$(basename "$TEMPLATE")" >/dev/null 2>"$render_error"; then
    fail "$name: the render SUCCEEDED but malformed pin data must refuse to render at all"
  fi
  grep -qF -- "$want_error" "$render_error" ||
    fail "$name: render refused for the wrong reason; wanted '$want_error', got: $(cat "$render_error")"
}

# ---------- LAYER 1a: pins render with an EMPTY commands list ----------------
render_fixture pins <<'EOF'
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: pin.example.test
      ip: "192.0.2.7"
      short: pin
    - fqdn: pin2.example.test
      ip: "192.0.2.8"
      short: pin2
EOF
rendered="$work/pins.rendered"
if [[ ! -s $rendered ]]; then
  # An empty render is only legitimate OFF darwin. On darwin this template
  # must produce output, so empty means the template broke, and skipping
  # would report a broken render as a pass. Assert the host, do not infer it.
  if [[ $(uname -s) == Darwin ]]; then
    printf 'FAIL: the render came back EMPTY on darwin, where this template must produce output; a broken darwin render must not pass as a skip\n' >&2
    exit 1
  fi
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# Independent expectation: the exact line the template must generate per pin.
# Short enough to review at a glance, which the ~1KB inline body it replaced was
# not. Single-quoted so $tailnet_pin_helper stays literal: it must reach the
# rendered script as a shell variable reference, not as text the template
# expanded at render time.
#
# CHEZMOI_DEST_DIR, never $HOME: under `chezmoi apply --destination X` with an
# unchanged $HOME the two differ, and a $HOME-derived path would look for the
# helper where nothing was deployed and abort the apply. The `:?` makes running
# the rendered body outside chezmoi fail loudly instead of resolving to
# /.local/libexec/... . That CHEZMOI_DEST_DIR really does name the destination
# is executed in tailnet-pin-helper-deploy-order.sh; here the property is that
# the runner asks for it.
# shellcheck disable=SC2016  # the literal $tailnet_pin_helper and the literal
# ${CHEZMOI_DEST_DIR:?...} ARE the property under test; expanding either here
# would assert the opposite.
helper_assignment='tailnet_pin_helper="${CHEZMOI_DEST_DIR:?chezmoi exports this to every script it runs}/'"$RECONCILER_TARGET_SUFFIX"'"'
# shellcheck disable=SC2016
expected_1='sudo "$tailnet_pin_helper" '"'pin.example.test' '192.0.2.7' 'pin'"
# shellcheck disable=SC2016
expected_2='sudo "$tailnet_pin_helper" '"'pin2.example.test' '192.0.2.8' 'pin2'"
grep -qxF "$helper_assignment" "$rendered" ||
  fail "the runner must name the deployed reconciler in one place; expected exactly: $helper_assignment (rendered: $(cat "$rendered"))"
grep -qxF "$expected_1" "$rendered" ||
  fail "generated pin command 1 missing or wrong; expected exactly: $expected_1 (rendered: $(cat "$rendered"))"
grep -qxF "$expected_2" "$rendered" ||
  fail "generated pin command 2 missing or wrong; expected exactly: $expected_2"
# A partial or hand-driven apply could leave the reconciler undeployed. That must
# name the missing file, not surface as sudo's "command not found".
grep -qF 'is missing or not executable' "$rendered" ||
  fail "the runner must refuse loudly when the reconciler is not deployed"
# The hash of the reconciler is embedded so a change to the reconciliation logic
# re-triggers this run_onchange_ script; chezmoi hash-gates on the body alone.
reconciler_hash="$(shasum -a 256 "$RECONCILER" | cut -d' ' -f1)"
grep -qF "$reconciler_hash" "$rendered" ||
  fail "the rendered body does not carry the reconciler's sha256, so editing the reconciliation logic would never re-run the pins"
# ... and the hash is CHECKED, not merely carried. `[[ -x ]]` alone let a
# locally modified helper run as root: `chezmoi apply` without --force prompts
# rather than overwriting a modified target, and answering "skip" leaves the
# modified copy in place with no signal. An embedded-but-unverified hash is a
# re-trigger token; these two lines are what make it a gate.
grep -qxF "tailnet_pin_helper_expected_sha256='$reconciler_hash'" "$rendered" ||
  fail "the runner does not bind the reconciler's sha256 to a variable it can compare against; expected exactly: tailnet_pin_helper_expected_sha256='$reconciler_hash'"
grep -qF 'is not the helper this run was rendered for' "$rendered" ||
  fail "the runner embeds the reconciler's sha256 but never refuses on a mismatch, so root would execute a locally modified helper with no signal"
# Executed, not just grepped: the emitted gate must actually refuse a helper
# whose bytes differ, and must pass the one it was rendered for.
gate_block="$work/hash-gate.sh"
{
  printf '#!/usr/bin/env bash\nset -euo pipefail\n'
  # Everything the runner emits between the helper assignment and the first
  # generated pin command: both guards, not just the first `fi`.
  awk '/^tailnet_pin_helper=/ {inside = 1} /^echo /  {inside = 0} inside' "$rendered"
  printf 'echo GATE-PASSED\n'
} >"$gate_block"
grep -qF 'tailnet_pin_helper_actual_sha256' "$gate_block" ||
  fail "the extracted guard block does not contain the hash check, so exercising it proves nothing: $(cat "$gate_block")"
gate_dest="$work/gate-dest"
mkdir -p "$gate_dest/$(dirname "$RECONCILER_TARGET_SUFFIX")"
cp "$RECONCILER" "$gate_dest/$RECONCILER_TARGET_SUFFIX"
chmod +x "$gate_dest/$RECONCILER_TARGET_SUFFIX"
CHEZMOI_DEST_DIR="$gate_dest" bash "$gate_block" 2>"$work/hash-gate.err" |
  grep -qxF 'GATE-PASSED' ||
  fail "the rendered hash gate refused the very helper it was rendered for: $(cat "$work/hash-gate.err")"
printf '\n# locally modified\n' >>"$gate_dest/$RECONCILER_TARGET_SUFFIX"
if CHEZMOI_DEST_DIR="$gate_dest" bash "$gate_block" >/dev/null 2>"$work/hash-gate-modified.err"; then
  fail "the rendered hash gate accepted a MODIFIED helper; root would execute it"
fi
grep -qF 'is not the helper this run was rendered for' "$work/hash-gate-modified.err" ||
  fail "the hash gate refused a modified helper without saying why: $(cat "$work/hash-gate-modified.err")"
grep -qxF 'sudo -v' "$rendered" ||
  fail "sudo -v not emitted with an empty commands list, the upfront timestamp must cover pin commands"
if grep -qxF 'exit 0' "$rendered"; then
  fail "early-return emitted despite pins being configured, pins would never apply"
fi
# The rendered body is a bash script; a syntax error in it would only surface at
# apply time, as root, with sudo already primed.
bash -n "$rendered" || fail "the rendered runner is not valid bash"

# The exact desired line for pin 1, reused by the behavior tests below.
want1=$'192.0.2.7\tpin.example.test\tpin'

# ---------- LAYER 1b: no tailnet_pins key -> early-return survives ----------
render_fixture nopins <<'EOF'
macos:
  system_setup: []
EOF
grep -qxF 'exit 0' "$work/nopins.rendered" ||
  fail "with no commands and no pins the runner must keep its exit-0 early-return"
if grep -qxF 'sudo -v' "$work/nopins.rendered"; then
  fail "spurious sudo -v emitted when there is nothing to run"
fi
if grep -qF "$RECONCILER_TARGET_SUFFIX" "$work/nopins.rendered"; then
  fail "the reconciler guard rendered with no pins configured; nothing would use it"
fi

# ---------- LAYER 1b2: malformed pin fields REFUSE to render -----------------
# One pin must be exactly one hosts record of three columns. A newline in a
# field would write EXTRA LINES into /etc/hosts as root, any other whitespace
# splits the record into extra columns, and # starts a hosts comment that
# silently truncates it. Empty or missing fields render a malformed line.
# All of these must abort the render, never reach the generated command.
render_fixture_must_fail nl-ip "is not a single hosts column" <<'EOF'
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: nl.example.test
      ip: "192.0.2.7\n203.0.113.9"
      short: nl
EOF
render_fixture_must_fail cr-ip "is not a single hosts column" <<'EOF'
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: cr.example.test
      ip: "192.0.2.7\r"
      short: cr
EOF
render_fixture_must_fail sp-fqdn "is not a single hosts column" <<'EOF'
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: "pin space.example.test"
      ip: "192.0.2.7"
      short: pin
EOF
render_fixture_must_fail hash-short "is not a single hosts column" <<'EOF'
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: pin.example.test
      ip: "192.0.2.7"
      short: "pin#note"
EOF
render_fixture_must_fail empty-fqdn "missing fqdn" <<'EOF'
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: ""
      ip: "192.0.2.7"
      short: pin
EOF
render_fixture_must_fail no-short "missing short" <<'EOF'
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: pin.example.test
      ip: "192.0.2.7"
EOF

# The schema says every pin field is a STRING. It used to say so and nothing
# checked, so a boolean fqdn with numeric ip and short rendered a command from
# 'false' '192002007' '7' without complaint.
render_fixture_must_fail bool-fqdn "not a string" <<'EOF'
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: false
      ip: "192.0.2.7"
      short: pin
EOF
render_fixture_must_fail numeric-ip "not a string" <<'EOF'
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: pin.example.test
      ip: 192002007
      short: pin
EOF
render_fixture_must_fail numeric-short "not a string" <<'EOF'
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: pin.example.test
      ip: "192.0.2.7"
      short: 7
EOF

# ---------- LAYER 1b2b: two pins may not claim the same NAME -----------------
# A pin OWNS both of its names: the reconciler drops every line claiming either
# one before writing its own record. Two pins sharing a name therefore delete
# each other's record on every apply, forever. Measured before this refusal
# existed: pin a reported "written", pin b deleted it and reported "written",
# and after two full apply rounds the file held only b's record while both
# commands exited 0. Nothing downstream can detect that, so it is refused here.
render_fixture_must_fail duplicate-short "claimed by two different pins" <<'EOF'
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: a.example.test
      ip: "192.0.2.7"
      short: nas
    - fqdn: b.example.test
      ip: "192.0.2.8"
      short: nas
EOF
render_fixture_must_fail duplicate-fqdn "claimed by two different pins" <<'EOF'
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: a.example.test
      ip: "192.0.2.7"
      short: aa
    - fqdn: a.example.test
      ip: "192.0.2.8"
      short: bb
EOF
# A short name colliding with a DIFFERENT pin's fqdn is the same collision.
render_fixture_must_fail short-collides-with-fqdn "claimed by two different pins" <<'EOF'
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: nas
      ip: "192.0.2.7"
      short: nx
    - fqdn: b.example.test
      ip: "192.0.2.8"
      short: nas
EOF
# The false-positive direction, twice: distinct pins must still render, and a
# single pin whose fqdn EQUALS its own short is not a collision (one owner, both
# names). Without these the uniqueness rule could be "refuse every second pin"
# and every assertion above would still pass.
render_fixture distinct-pins <<'EOF'
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: a.example.test
      ip: "192.0.2.7"
      short: aa
    - fqdn: b.example.test
      ip: "192.0.2.8"
      short: bb
EOF
# shellcheck disable=SC2016
grep -qxF 'sudo "$tailnet_pin_helper" '"'a.example.test' '192.0.2.7' 'aa'" "$work/distinct-pins.rendered" ||
  fail "two pins with distinct names must both render; the uniqueness rule is refusing legitimate data"
# shellcheck disable=SC2016
grep -qxF 'sudo "$tailnet_pin_helper" '"'b.example.test' '192.0.2.8' 'bb'" "$work/distinct-pins.rendered" ||
  fail "the second of two distinct pins did not render; the uniqueness rule is refusing legitimate data"
render_fixture self-named-pin <<'EOF'
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: nas
      ip: "192.0.2.7"
      short: nas
EOF
# shellcheck disable=SC2016
grep -qxF 'sudo "$tailnet_pin_helper" '"'nas' '192.0.2.7' 'nas'" "$work/self-named-pin.rendered" ||
  fail "a single pin whose fqdn equals its own short is one owner claiming one name, not a collision"

# ---------- LAYER 1b3: the rendered path is a path chezmoi deploys -----------
# The render can only name the reconciler correctly by accident unless the two
# are checked against each other: a renamed source file with an unchanged
# template renders a sudo call at a path nothing deploys, and the guard would
# then abort every apply.
managed_home="$work/managed-home"
mkdir -p "$managed_home"
managed_target="$(HOME="$managed_home" chezmoi --source "$REPO_ROOT" managed \
  --path-style=absolute --include=files 2>/dev/null |
  grep -F "$RECONCILER_TARGET_SUFFIX" || true)"
[[ -n $managed_target ]] ||
  fail "chezmoi does not manage any file at $RECONCILER_TARGET_SUFFIX, but the rendered runner sudo-executes one there"

# ---------- behavior helpers -------------------------------------------------
# The REAL reconciler, pointed at a fixture through its documented seam. No
# string surgery, so this exercises the shipped code rather than a copy of it.
run_pin() { # <hosts-file> <fqdn> <ip> <short>
  local hosts_file="$1"
  shift
  TAILNET_PIN_HOSTS_FILE="$hosts_file" "$RECONCILER" "$@"
}

run_pin1() { # <hosts-file>
  run_pin "$1" pin.example.test 192.0.2.7 pin ||
    fail "reconciler failed (rc=$?) against $1"
}

# A refusal must be LOUD: nonzero exit (the rendered runner runs under set -e,
# so chezmoi apply fails instead of reporting success over a pin that did not
# apply) and a stderr line saying why.
run_expect_refusal() { # <hosts-file> <stderr-substring> <label> [<fqdn> <ip> <short>]
  local hosts_file="$1" want_error="$2" label="$3"
  shift 3
  local refusal_err="$work/refuse-$label.err"
  if [[ $# -eq 0 ]]; then
    set -- pin.example.test 192.0.2.7 pin
  fi
  if run_pin "$hosts_file" "$@" >/dev/null 2>"$refusal_err"; then
    fail "$label: expected a nonzero refusal, but the reconciler exited 0 against $hosts_file"
  fi
  grep -qF -- "$want_error" "$refusal_err" ||
    fail "$label: refusal ran silent or with the wrong message; wanted stderr to contain '$want_error', got: $(cat "$refusal_err")"
}

run_with_shims() { # <hosts-file> <shim-dir>: force tool failures
  PATH="$2:$PATH" TAILNET_PIN_HOSTS_FILE="$1" "$RECONCILER" \
    pin.example.test 192.0.2.7 pin
}

# The same reconciler under a NAMED interpreter, bypassing its shebang. Used by
# every case whose outcome the two bash versions disagreed about.
run_pin_under() { # <interpreter> <hosts-file> <fqdn> <ip> <short>
  local interpreter="$1" hosts_file="$2"
  shift 2
  TAILNET_PIN_HOSTS_FILE="$hosts_file" "$interpreter" "$RECONCILER" "$@"
}

# GNU stat first, BSD fallback: GNU's -f means "filesystem status" and would
# SUCCEED with useless output, so the GNU form must be the one tried first.
file_mode() { # <file>
  stat -L -c '%a' "$1" 2>/dev/null || stat -L -f '%Lp' "$1"
}

file_owner() { # <file> -> uid:gid
  stat -L -c '%u:%g' "$1" 2>/dev/null || stat -L -f '%u:%g' "$1"
}

# The inode is how "was this file replaced?" is asked without reading it: an
# install that happened to produce identical bytes still renamed a new file into
# place, so bytes alone cannot tell a converged run from a rebuilt one.
file_inode() { # <file>
  stat -L -c '%i' "$1" 2>/dev/null || stat -L -f '%i' "$1"
}

# ---------- LAYER 2a: idempotence -------------------------------------------
hosts="$work/hosts"
printf '127.0.0.1\tlocalhost\n255.255.255.255\tbroadcasthost\n' >"$hosts"

for round in 1 2; do
  run_pin1 "$hosts"
  run_pin "$hosts" pin2.example.test 192.0.2.8 pin2 || fail "pin 2 failed on round $round"
  if [[ $round -eq 1 ]]; then
    cp "$hosts" "$work/hosts.after1"
  fi
done
grep -qxF "$want1" "$hosts" ||
  fail "pin 1 line missing or malformed after execution: $(grep -F pin.example.test "$hosts" || echo '<absent>')"
grep -qxF $'192.0.2.8\tpin2.example.test\tpin2' "$hosts" ||
  fail "pin 2 line missing or malformed after execution"
cmp -s "$hosts" "$work/hosts.after1" ||
  fail "NOT idempotent: round 2 changed the file ($(grep -cF example.test "$hosts") pin lines)"
grep -qxF $'127.0.0.1\tlocalhost' "$hosts" || fail "pre-existing hosts content was clobbered"
[[ $(find "$work" -maxdepth 1 -name 'hosts.*' -type f | wc -l) -eq 1 ]] ||
  fail "a converged run left temp droppings beside the target: $(find "$work" -maxdepth 1 -name 'hosts.*')"

# ---------- LAYER 2b: a pin whose IP changed must be CORRECTED ----------------
# The guard used to ask only "does this fqdn appear anywhere", so once a line
# existed the pin was never touched again. A tailnet address that changed left
# the OLD one in place forever. That is worse than having no pin: the pin exists
# to be the fallback when MagicDNS is down, and a stale fallback resolves
# confidently to the wrong host.
#
# The filter must key on the FIELD STRUCTURE of /etc/hosts, not on grep word
# boundaries: to grep, `.` and `-` end a word, so a -w filter kept the prefix
# case (xpin) but deleted pin.example.test.evil and other-pin.example.test,
# both DIFFERENT hosts that merely contain the pinned name.
stale="$work/stale-hosts"
printf '127.0.0.1\tlocalhost\n198.51.100.1\tpin.example.test\tpin\n10.0.0.1\txpin.example.test\tunrelated\n203.0.113.5\tpin.example.test.evil\tevil\n203.0.113.6\tother-pin.example.test\totherpin\n' >"$stale"
run_pin1 "$stale"
grep -qxF "$want1" "$stale" ||
  fail "a changed pin IP was not corrected; hosts still reads: $(grep -F pin.example.test "$stale")"
if grep -qF '198.51.100.1' "$stale"; then
  fail "the stale pin line survived alongside the new one, so resolution is now ambiguous"
fi
grep -qxF $'127.0.0.1\tlocalhost' "$stale" || fail "correcting a pin clobbered unrelated hosts content"
grep -qxF $'10.0.0.1\txpin.example.test\tunrelated' "$stale" ||
  fail "correcting a pin removed a DIFFERENT host whose name merely contains the pinned one (prefix case)"
grep -qxF $'203.0.113.5\tpin.example.test.evil\tevil' "$stale" ||
  fail "correcting a pin removed pin.example.test.evil, a DIFFERENT host; the filter must match whole hosts fields, not grep words"
grep -qxF $'203.0.113.6\tother-pin.example.test\totherpin' "$stale" ||
  fail "correcting a pin removed other-pin.example.test, a DIFFERENT host; the filter must match whole hosts fields, not grep words"
cp "$stale" "$work/stale.after1"
run_pin1 "$stale"
cmp -s "$stale" "$work/stale.after1" || fail "correcting a pin is not idempotent on a second run"

# ---------- LAYER 2c: an exact line PLUS a stale duplicate is NOT converged ---
# "At least one correct line exists" is the wrong property: it reads a correct
# line plus a stale duplicate as converged and leaves two lines naming the pin.
# Convergence is "exactly one line names this pin, and it is exactly right".
dup="$work/dup-hosts"
printf '127.0.0.1\tlocalhost\n198.51.100.1\tpin.example.test\tpin\n%s\n' "$want1" >"$dup"
run_pin1 "$dup"
dup_expected="$work/dup-expected"
printf '127.0.0.1\tlocalhost\n%s\n' "$want1" >"$dup_expected"
cmp -s "$dup" "$dup_expected" ||
  fail "an exact line plus a stale duplicate must collapse to exactly one pin line; got: $(diff "$dup_expected" "$dup" | head -5)"

# ---------- LAYER 2d: comments and blank lines survive -----------------------
# A commented-out copy of the exact pin line must not count as convergence (an
# ACTIVE stale line would then be left uncorrected, reopening the exact bug this
# command fixes).
commented="$work/commented-hosts"
printf '127.0.0.1\tlocalhost\n\n# plain comment\n#%s\n198.51.100.1\tpin.example.test\tpin\n' "$want1" >"$commented"
run_pin1 "$commented"
commented_expected="$work/commented-expected"
printf '127.0.0.1\tlocalhost\n\n# plain comment\n#%s\n%s\n' "$want1" "$want1" >"$commented_expected"
cmp -s "$commented" "$commented_expected" ||
  fail "a commented-out copy of the pin line must be preserved AND never satisfy convergence; got: $(diff "$commented_expected" "$commented" | head -5)"

# ---------- LAYER 2e: the pin owns its short name ----------------------------
# The pin exists so BOTH names answer with the tailnet address when MagicDNS is
# down. An unrelated line claiming only the short name would compete with the
# pin for that name, so it is dropped, by decision, not by accident.
shortclaim="$work/shortclaim-hosts"
printf '127.0.0.1\tlocalhost\n192.168.1.5\tpin\n' >"$shortclaim"
run_pin1 "$shortclaim"
shortclaim_expected="$work/shortclaim-expected"
printf '127.0.0.1\tlocalhost\n%s\n' "$want1" >"$shortclaim_expected"
cmp -s "$shortclaim" "$shortclaim_expected" ||
  fail "a line claiming the pin's short name must be replaced by the pin; got: $(diff "$shortclaim_expected" "$shortclaim" | head -5)"

# ---------- LAYER 2f: refusals are LOUD and change nothing -------------------
# Rewriting /etc/hosts as root is the one step here that can break the machine.
# A rebuild that lost its loopback entry must be refused: file untouched, exit
# NONZERO (so chezmoi apply fails instead of reporting success), reason on
# stderr.
noloop="$work/noloop-hosts"
printf '198.51.100.1\tpin.example.test\tpin\n' >"$noloop"
cp "$noloop" "$work/noloop.before"
run_expect_refusal "$noloop" "lost its loopback entry" noloop
cmp -s "$noloop" "$work/noloop.before" ||
  fail "a rewrite that would drop the loopback entry was installed instead of refused: $(cat "$noloop")"

# The gate must match a REAL loopback line, anchored: 127.0.0.100 is a decoy
# that an unanchored 127.0.0.1 match would accept, installing a hosts file with
# no working localhost when the actual loopback line was filtered away (here it
# also names the pin, so the rebuild drops it). 127.0.0.100 IS a loopback
# address (lo0 carries netmask 0xff000000, so all of 127.0.0.0/8 is loopback);
# it is simply not the exact address localhost must map to.
decoy="$work/decoy-hosts"
printf '127.0.0.1\tlocalhost pin.example.test\n127.0.0.100\tdev.local\n' >"$decoy"
cp "$decoy" "$work/decoy.before"
run_expect_refusal "$decoy" "lost its loopback entry" decoy
cmp -s "$decoy" "$work/decoy.before" ||
  fail "a rebuild whose only 127.0.0.1 match is the decoy 127.0.0.100 was installed instead of refused: $(cat "$decoy")"

# THE DEFECT THIS FILE WAS REOPENED FOR. `127.0.0.1  # comment-only decoy`
# satisfies ^127\.0\.0\.1[[:space:]] while naming nothing a machine could
# resolve localhost through. With the real loopback line carrying the pin's
# owned short name (so the rebuild drops it), the old gate installed a hosts
# file with ZERO records naming localhost, exit 0, no stderr.
#
# WHY IT IS REFUSED, corrected. The reconciler used to say such a line "maps
# nothing", citing hosts(5). The resolver disagrees, measured against Libinfo's
# own parser: only a line whose FIRST character is '#' is skipped, so this one
# parses as a valid record mapping 127.0.0.1 to the official name "#" with the
# alias "decoy". It is refused because it does not map localhost, not because it
# maps nothing, and the reconciler's message now says so.
comment_decoy="$work/comment-decoy-hosts"
printf '127.0.0.1\tlocalhost\tpin\n127.0.0.1\t# comment-only decoy\n' >"$comment_decoy"
cp "$comment_decoy" "$work/comment-decoy.before"
run_expect_refusal "$comment_decoy" "lost its loopback entry" comment-decoy
cmp -s "$comment_decoy" "$work/comment-decoy.before" ||
  fail "a rebuild whose only 127.0.0.1 line is COMMENT-ONLY was installed instead of refused; the machine would have no localhost: $(cat "$comment_decoy")"

# And the refusal must give the REAL reason. Pinned because nothing else can
# catch a regression to the false one: the outcome is byte-identical either way,
# so only the sentence the operator reads changes, and a wrong explanation sends
# whoever hits this looking for a record that is right there in the file.
run_expect_refusal "$comment_decoy" "name written before a #" comment-decoy-reason

# The same hole with no comment at all: a bare address is not a mapping either.
address_only="$work/address-only-hosts"
printf '127.0.0.1\tlocalhost\tpin\n127.0.0.1\t\n' >"$address_only"
cp "$address_only" "$work/address-only.before"
run_expect_refusal "$address_only" "lost its loopback entry" address-only
cmp -s "$address_only" "$work/address-only.before" ||
  fail "a rebuild whose only 127.0.0.1 line has NO host name was installed instead of refused: $(cat "$address_only")"

# THE OTHER DIRECTION, which a safety gate gets no credit for passing by
# accident: the gate must not refuse a file the machine resolves perfectly well.
# The resolver skips leading blanks before the FIRST token of a hosts line
# (Libinfo `_fsi_tokenize`), so an INDENTED localhost record works, and the
# predicate refusing one would have blocked every pin on such a file, loudly and
# for no reason. That refusal used to be deliberate, on the stated grounds that
# the question was unmeasured. It is measured now. The indented line must
# survive the rebuild BYTE-EXACT as well: it is not the pin's to normalize.
indented="$work/indented-hosts"
printf '  \t127.0.0.1   localhost   broadcasthost\n198.51.100.1\tpin.example.test\tpin\n' >"$indented"
run_pin1 "$indented"
indented_expected="$work/indented-expected"
printf '  \t127.0.0.1   localhost   broadcasthost\n%s\n' "$want1" >"$indented_expected"
cmp -s "$indented" "$indented_expected" ||
  fail "an indented localhost record must satisfy the gate and survive byte-exact; got: $(diff "$indented_expected" "$indented" | head -5)"

# Indentation is not what made the decoy invalid, so an indented one is still
# refused. Without this, relaxing the anchor could have been over-relaxed into
# accepting any line CONTAINING the address and nothing here would have noticed.
indented_decoy="$work/indented-decoy-hosts"
printf '127.0.0.1\tlocalhost\tpin\n  127.0.0.1\t# comment-only decoy\n' >"$indented_decoy"
cp "$indented_decoy" "$work/indented-decoy.before"
run_expect_refusal "$indented_decoy" "lost its loopback entry" indented-decoy
cmp -s "$indented_decoy" "$work/indented-decoy.before" ||
  fail "an indented comment-only 127.0.0.1 line satisfied the gate: $(cat "$indented_decoy")"

# An address sitting in a NAME column is not a loopback record either, indented
# or not: the gate wants the FIRST FIELD, which is what the old line-anchor was
# really buying. The resolver reads this as 10.0.0.1 named "127.0.0.1".
name_position="$work/name-position-hosts"
printf '127.0.0.1\tlocalhost\tpin\n  10.0.0.1\t127.0.0.1\n' >"$name_position"
cp "$name_position" "$work/name-position.before"
run_expect_refusal "$name_position" "lost its loopback entry" name-position
cmp -s "$name_position" "$work/name-position.before" ||
  fail "a line whose NAME is 127.0.0.1 satisfied the loopback gate: $(cat "$name_position")"

# A missing hosts file is refused the same way: nothing safe exists to rewrite,
# and seeding a pin-only /etc/hosts with no loopback would break the machine.
absent="$work/absent-hosts"
run_expect_refusal "$absent" "the file is missing" absent
[[ ! -e $absent ]] ||
  fail "a missing hosts file was seeded with pin content instead of refused: $(cat "$absent")"

# A dangling symlink is a missing file, not a file to create through.
dangling="$work/dangling-hosts"
ln -s "$work/nowhere-hosts" "$dangling"
run_expect_refusal "$dangling" "the file is missing" dangling
[[ ! -e "$work/nowhere-hosts" ]] ||
  fail "a dangling symlink was followed into creating a new hosts file"

# The reconciler is a standalone component now, so its own entry contract is
# checked at its own boundary rather than trusted from the renderer.
entry="$work/entry-hosts"
printf '127.0.0.1\tlocalhost\n' >"$entry"
run_expect_refusal "$entry" "not a single hosts column" entry-space \
  "pin space.example.test" 192.0.2.7 pin
run_expect_refusal "$entry" "not a single hosts column" entry-hash \
  pin.example.test 192.0.2.7 "pin#note"
run_expect_refusal "$entry" "not a single hosts column" entry-empty \
  pin.example.test 192.0.2.7 ""
if TAILNET_PIN_HOSTS_FILE="$entry" "$RECONCILER" only-one-argument >/dev/null 2>&1; then
  fail "the reconciler accepted the wrong number of arguments"
fi
cmp -s "$entry" <(printf '127.0.0.1\tlocalhost\n') ||
  fail "an entry-contract refusal still touched the hosts file"

# ---------- LAYER 2g: a failed install must never destroy the target ---------
# The old install (`cat "$tmp" > target`) truncated the target BEFORE writing,
# so a failure or interrupt during it left an EMPTY /etc/hosts and still exited
# 0. Force the install tools to fail: the target must be byte-identical after,
# the command must exit nonzero, and no temp droppings may remain.
shimdir="$work/shims"
mkdir -p "$shimdir"
printf '#!/bin/sh\nexit 75\n' >"$shimdir/mv"
printf '#!/bin/sh\nexit 75\n' >"$shimdir/cat"
chmod +x "$shimdir/mv" "$shimdir/cat"
atomic_dir="$work/atomic"
mkdir -p "$atomic_dir"
atomic_hosts="$atomic_dir/hosts"
printf '127.0.0.1\tlocalhost\n198.51.100.1\tpin.example.test\tpin\n' >"$atomic_hosts"
cp "$atomic_hosts" "$work/atomic.before"
if run_with_shims "$atomic_hosts" "$shimdir" >/dev/null 2>&1; then
  fail "the install step failed but the reconciler still exited 0; install status must be checked"
fi
cmp -s "$atomic_hosts" "$work/atomic.before" ||
  fail "a failed install altered or truncated the target hosts file: $(cat "$atomic_hosts")"
[[ $(find "$atomic_dir" -type f | wc -l) -eq 1 ]] ||
  fail "a failed install left temp droppings next to the target: $(ls "$atomic_dir")"

# A SIGNAL is the case bail-only cleanup could never cover: SIGTERM after mktemp
# returned 143 and preserved the target, but left a mode-0600 hosts.XXXXXXXX
# beside it forever. The shim fires the signal from inside the install step, so
# the temp file definitely exists when it arrives.
#
# EVERY trappable signal, under EVERY interpreter, because a set that "should"
# be covered by the EXIT trap is exactly what hid the last two leaks: an EXIT
# trap alone runs before bash 3.2 dies from SIGQUIT and before bash 5.3 dies
# from a write-triggered SIGXFSZ in neither case, and each left a mode-0600
# hosts.XXXXXXXX beside the target while the file's own comment claimed
# "signals included". This is a completeness guard, not a count: every member of
# the list must leave no temp, exit nonzero, and preserve the target.
# SIGKILL and SIGSTOP are absent because no process can trap them; the helper's
# header says so rather than claiming otherwise.
#
# TWO LISTS THAT MUST DESCRIBE THE SAME SET, ENUMERATED AND DIFFED. This suite
# used to iterate a literal of its own and never look at the constant, so three
# of the six members could be deleted from the reconciler, and members could be
# added to it, with every assertion below still passing: the loop simply stopped
# visiting them. The expectation stays declared HERE, so the suite can disagree
# with the implementation, but the disagreement is now reported by name and the
# cases are driven by the constant the reconciler actually traps on.
EXPECTED_CLEANED_UP_SIGNAL_NAMES=(HUP INT QUIT TERM PIPE XFSZ)

# The reconciler's own list. Sourcing it defines functions and constants and
# reconciles nothing (its entry point is guarded on BASH_SOURCE; the unit suite
# pins that guard), and the subshell keeps its readonly declarations out of this
# shell.
reconciler_cleaned_up_signal_names() {
  (
    # shellcheck source=/dev/null
    source "$RECONCILER"
    printf '%s\n' "${CLEANED_UP_SIGNAL_NAMES[@]}"
  )
}

actual_cleaned_up_signal_names=()
while IFS= read -r signal_name; do
  actual_cleaned_up_signal_names+=("$signal_name")
done < <(reconciler_cleaned_up_signal_names)
[[ ${#actual_cleaned_up_signal_names[@]} -gt 0 ]] ||
  fail "could not read CLEANED_UP_SIGNAL_NAMES out of the reconciler; the per-signal cases below would then exercise nothing"

signal_list_contains() { # <needle> <haystack...>
  local needle="$1" candidate
  shift
  for candidate in "$@"; do
    [[ $candidate == "$needle" ]] && return 0
  done
  return 1
}
for signal_name in "${EXPECTED_CLEANED_UP_SIGNAL_NAMES[@]}"; do
  signal_list_contains "$signal_name" "${actual_cleaned_up_signal_names[@]}" ||
    fail "SIG$signal_name is in this suite's expected cleanup set but NOT in the reconciler's CLEANED_UP_SIGNAL_NAMES (it declares: ${actual_cleaned_up_signal_names[*]}); a signal nothing handles leaves a mode-0600 temp beside the target"
done
for signal_name in "${actual_cleaned_up_signal_names[@]}"; do
  signal_list_contains "$signal_name" "${EXPECTED_CLEANED_UP_SIGNAL_NAMES[@]}" ||
    fail "the reconciler traps SIG$signal_name for cleanup but this suite does not expect it (it declares: ${actual_cleaned_up_signal_names[*]}); either the new member is deliberate and belongs in EXPECTED_CLEANED_UP_SIGNAL_NAMES, or a handler was added that nothing exercises"
done

signal_case_number=0
for signal_name in "${actual_cleaned_up_signal_names[@]}"; do
  for interpreter in "${INTERPRETERS[@]}"; do
    signal_case_number=$((signal_case_number + 1))
    signal_dir="$work/signal-$signal_case_number"
    mkdir -p "$signal_dir"
    signal_hosts="$signal_dir/hosts"
    printf '127.0.0.1\tlocalhost\n198.51.100.1\tpin.example.test\tpin\n' >"$signal_hosts"
    cp "$signal_hosts" "$signal_dir/before"
    signal_shimdir="$signal_dir/shims"
    mkdir -p "$signal_shimdir"
    # shellcheck disable=SC2016  # $PPID must stay literal: it is the SHIM's own
    # parent (the reconciler), resolved when the shim runs, not this test's.
    printf '#!/bin/sh\nkill -%s "$PPID"\nsleep 1\n' "$signal_name" \
      >"$signal_shimdir/chmod"
    chmod +x "$signal_shimdir/chmod"
    signal_status=0
    PATH="$signal_shimdir:$PATH" TAILNET_PIN_HOSTS_FILE="$signal_hosts" \
      "$interpreter" "$RECONCILER" pin.example.test 192.0.2.7 pin \
      >/dev/null 2>&1 || signal_status=$?
    [[ $signal_status -ne 0 ]] ||
      fail "a SIG$signal_name mid-install still exited 0 under $(interpreter_label "$interpreter")"
    cmp -s "$signal_hosts" "$signal_dir/before" ||
      fail "a SIG$signal_name mid-install altered the target hosts file under $(interpreter_label "$interpreter"): $(cat "$signal_hosts")"
    [[ $(find "$signal_dir" -maxdepth 1 -type f -name 'hosts.*' | wc -l) -eq 0 ]] ||
      fail "a SIG$signal_name mid-install left temp droppings next to the target under $(interpreter_label "$interpreter"): $(ls "$signal_dir")"
  done
done

# SIGXFSZ a second way, and the way that matters. The loop above delivers it
# with `kill`, where the handler turns out to be inert on both interpreters;
# `ulimit -f 0` stands in for a full filesystem so the kernel raises it from
# inside the rebuild's OWN write, which is where the handler earns its place:
# with the per-signal traps removed, bash 5.3 leaves a mode-0600 hosts.XXXXXXXX
# beside the target here (measured), while bash 3.2 surfaces the failed write
# through the normal refusal path and exits 1. Both are nonzero, so this asserts
# only that.
for interpreter in "${INTERPRETERS[@]}"; do
  xfsz_dir="$work/xfsz-$(basename "$(dirname "$interpreter")")"
  mkdir -p "$xfsz_dir"
  xfsz_hosts="$xfsz_dir/hosts"
  printf '127.0.0.1\tlocalhost\n198.51.100.1\tpin.example.test\tpin\n' >"$xfsz_hosts"
  cp "$xfsz_hosts" "$xfsz_dir/before"
  xfsz_status=0
  (
    ulimit -f 0 2>/dev/null || exit 0
    TAILNET_PIN_HOSTS_FILE="$xfsz_hosts" "$interpreter" "$RECONCILER" \
      pin.example.test 192.0.2.7 pin >/dev/null 2>&1
  ) || xfsz_status=$?
  [[ $xfsz_status -ne 0 ]] ||
    fail "a write-triggered SIGXFSZ still exited 0 under $(interpreter_label "$interpreter")"
  cmp -s "$xfsz_hosts" "$xfsz_dir/before" ||
    fail "a write-triggered SIGXFSZ altered the target hosts file under $(interpreter_label "$interpreter"): $(cat "$xfsz_hosts")"
  [[ $(find "$xfsz_dir" -maxdepth 1 -type f -name 'hosts.*' | wc -l) -eq 0 ]] ||
    fail "a write-triggered SIGXFSZ left temp droppings next to the target under $(interpreter_label "$interpreter"): $(ls "$xfsz_dir")"
done

# ---------- LAYER 2g2: an UNREADABLE source is refused, never read as empty ---
# The walkers ended in a `printf` after their loop, so a failed `done <"$path"`
# redirect never became the function's status. Under bash 3.2 a chmod-000 hosts
# file with four records surveyed as zero claiming lines, the rebuild became the
# pin record ALONE, and the run printed a success message and exited 0.
#
# The second fixture is the COMPOSITION that made it destructive rather than
# merely wrong, and it is the case that must not regress: with the pin's ip
# mistyped as 127.0.0.1, the rebuild's own appended record satisfied the
# loopback gate, so nothing stood between an unreadable /etc/hosts and a
# one-line replacement of it. Both halves are pinned together, because either
# fix alone leaves the pair reachable.
if [[ $(id -u) -eq 0 ]]; then
  printf 'NOTE: skipping the unreadable-source cases; uid 0 reads a mode-000 fixture regardless\n'
else
  unreadable_case_number=0
  for interpreter in "${INTERPRETERS[@]}"; do
    for pin_ip in 192.0.2.7 127.0.0.1; do
      unreadable_case_number=$((unreadable_case_number + 1))
      unreadable_dir="$work/unreadable-$unreadable_case_number"
      mkdir -p "$unreadable_dir"
      unreadable_hosts="$unreadable_dir/hosts"
      printf '127.0.0.1\tlocalhost\n255.255.255.255\tbroadcasthost\n::1\tlocalhost\n10.0.0.5\tnas.home\n' \
        >"$unreadable_hosts"
      cp "$unreadable_hosts" "$unreadable_dir/before"
      chmod 000 "$unreadable_hosts"
      unreadable_status=0
      unreadable_output="$unreadable_dir/out"
      run_pin_under "$interpreter" "$unreadable_hosts" \
        pin.example.test "$pin_ip" pin >"$unreadable_output" 2>&1 ||
        unreadable_status=$?
      chmod 644 "$unreadable_hosts"
      [[ $unreadable_status -ne 0 ]] ||
        fail "an unreadable hosts file was reconciled 'successfully' (ip $pin_ip, $(interpreter_label "$interpreter")): $(cat "$unreadable_output")"
      grep -qF 'an unreadable hosts file is not an empty one' "$unreadable_output" ||
        fail "the refusal for an unreadable hosts file did not say why (ip $pin_ip, $(interpreter_label "$interpreter")): $(cat "$unreadable_output")"
      if grep -qF 'written to' "$unreadable_output"; then
        fail "an unreadable hosts file produced a SUCCESS message (ip $pin_ip, $(interpreter_label "$interpreter")): $(cat "$unreadable_output")"
      fi
      cmp -s "$unreadable_hosts" "$unreadable_dir/before" ||
        fail "an unreadable hosts file was REWRITTEN (ip $pin_ip, $(interpreter_label "$interpreter")); it now reads: $(cat "$unreadable_hosts")"
      [[ $(find "$unreadable_dir" -maxdepth 1 -type f -name 'hosts.*' | wc -l) -eq 0 ]] ||
        fail "the refusal left temp droppings beside the target: $(ls "$unreadable_dir")"
    done
  done
fi

# ---------- LAYER 2g3: the gate may not be satisfied by the pin's own record --
# The loopback gate ran on the temp file AFTER the desired record was appended,
# and nothing constrains a pin's ip. One mistyped YAML field (ip: "127.0.0.1")
# plus a localhost line carrying the pin's owned short name therefore passed the
# gate on the very line the rebuild had just written: localhost's record was
# deleted, the file's only 127.0.0.1 record became the pin's, and the run exited
# 0 with a success message. That is the same catastrophic outcome the gate
# exists to prevent, reached one door over. The gate now runs on the lines the
# rebuild KEEPS, before its own record joins them.
for interpreter in "${INTERPRETERS[@]}"; do
  self_gate_dir="$work/self-gate-$(basename "$(dirname "$interpreter")")"
  mkdir -p "$self_gate_dir"
  self_gate_hosts="$self_gate_dir/hosts"
  printf '127.0.0.1\tlocalhost\tpin\n' >"$self_gate_hosts"
  cp "$self_gate_hosts" "$self_gate_dir/before"
  self_gate_status=0
  self_gate_output="$self_gate_dir/out"
  run_pin_under "$interpreter" "$self_gate_hosts" \
    pin.example.test 127.0.0.1 pin >"$self_gate_output" 2>&1 || self_gate_status=$?
  [[ $self_gate_status -ne 0 ]] ||
    fail "a pin whose ip is $LOOPBACK_ADDRESS_UNDER_TEST satisfied the loopback gate with its own appended record under $(interpreter_label "$interpreter"): $(cat "$self_gate_output")"
  grep -qF 'lost its loopback entry' "$self_gate_output" ||
    fail "the loopback refusal did not say why under $(interpreter_label "$interpreter"): $(cat "$self_gate_output")"
  cmp -s "$self_gate_hosts" "$self_gate_dir/before" ||
    fail "localhost's record was deleted by a pin claiming its address under $(interpreter_label "$interpreter"); the file now reads: $(cat "$self_gate_hosts")"
done

# The false-positive direction: a pin whose ip is the loopback address is still
# reconciled when a loopback record the pin does NOT own survives the filter. It
# is the DESTRUCTION that is refused, not the address.
loop_ok="$work/loopback-ok-hosts"
printf '127.0.0.1\tlocalhost\n192.168.1.5\tpin\n' >"$loop_ok"
run_pin "$loop_ok" pin.example.test 127.0.0.1 pin >/dev/null ||
  fail "a pin at the loopback address must still apply when a loopback record it does not own survives the rebuild"
loop_ok_expected="$work/loopback-ok-expected"
printf '127.0.0.1\tlocalhost\n127.0.0.1\tpin.example.test\tpin\n' >"$loop_ok_expected"
cmp -s "$loop_ok" "$loop_ok_expected" ||
  fail "the loopback gate is now refusing legitimate rebuilds; got: $(diff "$loop_ok_expected" "$loop_ok" | head -5)"

# ---------- LAYER 2g4: a set-but-empty seam never falls back to /etc/hosts ----
# `${VAR:-$DEFAULT}` reads an empty value as "not configured", so one caller
# passing an empty first argument aimed a root-capable rewrite at the machine's
# real /etc/hosts. SET and EMPTY are different states and the empty one is
# refused. The real /etc/hosts is hashed either side of the call as the proof
# that nothing touched it.
etc_hosts_before=""
[[ -r /etc/hosts ]] && etc_hosts_before="$(shasum -a 256 /etc/hosts)"
empty_seam_output="$work/empty-seam.out"
empty_seam_status=0
TAILNET_PIN_HOSTS_FILE="" "$RECONCILER" pin.example.test 192.0.2.7 pin \
  >"$empty_seam_output" 2>&1 || empty_seam_status=$?
[[ $empty_seam_status -ne 0 ]] ||
  fail "an empty TAILNET_PIN_HOSTS_FILE was accepted: $(cat "$empty_seam_output")"
grep -qF 'is set but EMPTY' "$empty_seam_output" ||
  fail "the empty-seam refusal did not name the variable or the reason: $(cat "$empty_seam_output")"
if [[ -n $etc_hosts_before ]]; then
  [[ "$(shasum -a 256 /etc/hosts)" == "$etc_hosts_before" ]] ||
    fail "an empty TAILNET_PIN_HOSTS_FILE reached the real /etc/hosts and changed it"
fi

# ---------- LAYER 2h: the installed file keeps the target's mode -------------
# mktemp creates 0600 and /etc/hosts is 0644, so an install that ships the temp
# file's mode would leave a hosts file other daemons cannot read.
modehosts="$work/mode-hosts"
printf '127.0.0.1\tlocalhost\n198.51.100.1\tpin.example.test\tpin\n' >"$modehosts"
chmod 644 "$modehosts"
run_pin1 "$modehosts"
grep -qxF "$want1" "$modehosts" || fail "mode-preservation run did not converge the pin"
mode_after="$(file_mode "$modehosts")" || fail "could not stat the installed hosts file"
[[ $mode_after == 644 ]] ||
  fail "the install changed the hosts file mode from 644 to $mode_after (mktemp's private mode leaking through?)"

# ---------- LAYER 2i: a symlinked target keeps its indirection ---------------
# Metadata used to be read from the LINK rather than its referent, so a
# hosts -> real-hosts pair whose referent was 0640 became a REGULAR 0755 file
# (a symlink's own mode on macOS) and the referent stayed stale, still holding
# the address the resolver would have answered with had anyone restored the
# link.
link_dir="$work/symlink"
mkdir -p "$link_dir"
link_referent="$link_dir/real-hosts"
link_path="$link_dir/hosts"
printf '127.0.0.1\tlocalhost\n198.51.100.1\tpin.example.test\tpin\n' >"$link_referent"
chmod 640 "$link_referent"
ln -s real-hosts "$link_path"
run_pin1 "$link_path"
[[ -L $link_path ]] ||
  fail "the symlinked hosts path was replaced by a regular file; the operator's indirection was destroyed"
[[ $(readlink "$link_path") == real-hosts ]] ||
  fail "the hosts symlink no longer points at its referent"
grep -qxF "$want1" "$link_referent" ||
  fail "the pin was not written through the symlink into its referent, which is the file the resolver reads"
if grep -qF '198.51.100.1' "$link_referent"; then
  fail "the stale pin line survived in the symlink's referent"
fi
link_mode="$(file_mode "$link_referent")"
[[ $link_mode == 640 ]] ||
  fail "the install took its mode from the LINK (0755 on macOS) instead of the referent; referent is now $link_mode"
[[ $(find "$link_dir" -type f | wc -l) -eq 1 ]] ||
  fail "installing through a symlink left temp droppings: $(ls "$link_dir")"

# ---------- LAYER 2j: a missing trailing newline never joins lines -----------
# Appending to a file whose last line is unterminated used to produce one
# joined line (localhost192.0.2.7...). The rebuild must terminate every line.
nonl="$work/nonl-hosts"
printf '127.0.0.1\tlocalhost\n198.51.100.1\tpin.example.test\tpin\n10.0.0.9\tkeeper.example.test' >"$nonl"
run_pin1 "$nonl"
nonl_expected="$work/nonl-expected"
printf '127.0.0.1\tlocalhost\n10.0.0.9\tkeeper.example.test\n%s\n' "$want1" >"$nonl_expected"
cmp -s "$nonl" "$nonl_expected" ||
  fail "a hosts file without a trailing newline was corrupted by the rebuild; got: $(diff "$nonl_expected" "$nonl" | head -5)"

# ---------- LAYER 2j2: an unterminated final line is NOT convergence ---------
# THE LIVE DEFECT THIS LAYER EXISTS FOR. A hosts file whose final line is the
# pin's exact record with no trailing newline satisfied both convergence
# conditions, so the run printed "already converged", exited 0 and changed
# nothing. Apple's `_fsi_get_line` runs
# `if (s[0] != '#') s[strlen(s) - 1] = '\0';` over whatever fgets returned, so
# it eats a REAL character when there is no newline to eat: the resolver read
# that record as naming "pi", and the pin's short name did not answer. The
# rebuild is the only thing that writes a terminator and the converged path
# never reaches the rebuild, so the repair the reconciler's header promises was
# unreachable exactly where it was promised.
#
# Each fixture below is asserted on its MESSAGE as well as its bytes. Without
# that, "converged" and "written" are indistinguishable whenever the resulting
# file is the same, which is precisely this case.
run_pin_capture() { # <hosts-file> <label>: output lands in $work/<label>.out
  local hosts_file="$1" label="$2"
  TAILNET_PIN_HOSTS_FILE="$hosts_file" "$RECONCILER" \
    pin.example.test 192.0.2.7 pin >"$work/$label.out" 2>&1 ||
    fail "$label: the reconciler failed (rc=$?) against $hosts_file: $(cat "$work/$label.out")"
}

unterminated_dir="$work/unterminated"
mkdir -p "$unterminated_dir"
unterminated="$unterminated_dir/hosts"
printf '127.0.0.1\tlocalhost\n%s' "$want1" >"$unterminated"
run_pin_capture "$unterminated" unterminated
grep -qF 'written to' "$work/unterminated.out" ||
  fail "a hosts file whose UNTERMINATED final line is the pin record was reported converged instead of repaired; the resolver reads that record as naming 'pi': $(cat "$work/unterminated.out")"
unterminated_expected="$work/unterminated-expected"
printf '127.0.0.1\tlocalhost\n%s\n' "$want1" >"$unterminated_expected"
cmp -s "$unterminated" "$unterminated_expected" ||
  fail "the repaired file is not the pin record plus a terminator; got: $(diff "$unterminated_expected" "$unterminated" | head -5)"
[[ $(find "$unterminated_dir" -maxdepth 1 -type f -name 'hosts.*' | wc -l) -eq 0 ]] ||
  fail "repairing an unterminated final line left temp droppings: $(ls "$unterminated_dir")"

# IDEMPOTENCE, the other half. One rebuild repairs it; the next run must report
# converged and rewrite nothing, or every apply would rewrite /etc/hosts forever.
cp "$unterminated" "$work/unterminated.after1"
run_pin_capture "$unterminated" unterminated-again
grep -qF 'already converged' "$work/unterminated-again.out" ||
  fail "the run after the terminator repair did not report the file converged: $(cat "$work/unterminated-again.out")"
cmp -s "$unterminated" "$work/unterminated.after1" ||
  fail "the run after the terminator repair rewrote the file again; the repair does not converge"

# The false-positive direction, and the assertion that keeps the fix from being
# "always rebuild": a file that is genuinely converged AND properly terminated
# must report converged and be left alone, byte for byte and inode for inode.
# The inode is what proves no rebuild ran at all, since an install that produced
# identical bytes would still have renamed a new file into place.
terminated_converged="$work/terminated-converged-hosts"
printf '127.0.0.1\tlocalhost\n%s\n' "$want1" >"$terminated_converged"
cp "$terminated_converged" "$work/terminated-converged.before"
converged_inode_before="$(file_inode "$terminated_converged")"
run_pin_capture "$terminated_converged" terminated-converged
grep -qF 'already converged' "$work/terminated-converged.out" ||
  fail "a converged, properly terminated hosts file was rebuilt instead of reported converged: $(cat "$work/terminated-converged.out")"
cmp -s "$terminated_converged" "$work/terminated-converged.before" ||
  fail "a converged, properly terminated hosts file was modified"
[[ $(file_inode "$terminated_converged") == "$converged_inode_before" ]] ||
  fail "a converged, properly terminated hosts file was reinstalled (its inode changed), so every apply would rewrite /etc/hosts"

# An unterminated final line that is NOT the pin's is repaired too, and this is
# the case that makes the whole-file rule worth having rather than a narrower
# "the pin's own record must be terminated". Measured with Libinfo's parser: a
# file ending "127.0.0.1<tab>localhost" with no newline resolves as naming
# "localhos", so the machine has no localhost at all while the pin itself is
# perfect. Reporting that file converged is the same fail-quiet one door over.
unterminated_loopback="$work/unterminated-loopback-hosts"
printf '%s\n127.0.0.1\tlocalhost' "$want1" >"$unterminated_loopback"
run_pin_capture "$unterminated_loopback" unterminated-loopback
grep -qF 'written to' "$work/unterminated-loopback.out" ||
  fail "a hosts file whose UNTERMINATED final line is the LOOPBACK record was reported converged; the resolver reads it as naming 'localhos': $(cat "$work/unterminated-loopback.out")"
unterminated_loopback_expected="$work/unterminated-loopback-expected"
printf '127.0.0.1\tlocalhost\n%s\n' "$want1" >"$unterminated_loopback_expected"
cmp -s "$unterminated_loopback" "$unterminated_loopback_expected" ||
  fail "the unterminated loopback record was not repaired; got: $(diff "$unterminated_loopback_expected" "$unterminated_loopback" | head -5)"

# An unterminated final line still COUNTS as a claiming line. Drop it from the
# survey instead and this file reads as one exact claiming line in a terminated
# file, so it reports converged with the stale duplicate still in it.
unterminated_duplicate="$work/unterminated-duplicate-hosts"
printf '127.0.0.1\tlocalhost\n%s\n10.0.0.9\tpin.example.test' "$want1" \
  >"$unterminated_duplicate"
run_pin_capture "$unterminated_duplicate" unterminated-duplicate
unterminated_duplicate_expected="$work/unterminated-duplicate-expected"
printf '127.0.0.1\tlocalhost\n%s\n' "$want1" >"$unterminated_duplicate_expected"
cmp -s "$unterminated_duplicate" "$unterminated_duplicate_expected" ||
  fail "an unterminated final line claiming the pin survived the rebuild; got: $(diff "$unterminated_duplicate_expected" "$unterminated_duplicate" | head -5)"

# ---------- LAYER 2m: the temporary file is created BESIDE the target --------
# The reconciler's atomicity rests on `mv` never crossing a filesystem, which
# rests on mktemp creating the temp in the TARGET's directory. Every "no temp
# droppings" assertion in this suite looks only next to the target, so moving
# the temp into $TMPDIR passed all of them while a real mode-0600 leak sat
# somewhere nobody counted, and a cross-device rename would silently degrade to
# copy-then-unlink on a root rewrite of /etc/hosts.
#
# This pins the LOCATION directly and without timing: point the seam at a
# readable but NON-WRITABLE directory. Creating the temp beside the target is
# then impossible and the refusal must say exactly that. A temp created anywhere
# else succeeds, gets as far as the install, and fails with the install's
# message instead (measured), so the two are never confusable.
if [[ $(id -u) -eq 0 ]]; then
  printf 'NOTE: skipping the temp-location case; uid 0 writes a mode-555 directory regardless\n'
else
  unwritable_dir="$work/unwritable-dir"
  mkdir -p "$unwritable_dir"
  unwritable_hosts="$unwritable_dir/hosts"
  printf '127.0.0.1\tlocalhost\n198.51.100.1\tpin.example.test\tpin\n' >"$unwritable_hosts"
  cp "$unwritable_hosts" "$work/unwritable.before"
  # The mode is restored BEFORE any assertion runs: a `fail` while the directory
  # is still 0555 would abort the suite with a temp tree its own EXIT trap
  # cannot remove.
  chmod 555 "$unwritable_dir"
  temp_location_status=0
  temp_location_error="$work/temp-beside-target.err"
  TAILNET_PIN_HOSTS_FILE="$unwritable_hosts" "$RECONCILER" \
    pin.example.test 192.0.2.7 pin >/dev/null 2>"$temp_location_error" ||
    temp_location_status=$?
  chmod 755 "$unwritable_dir"
  [[ $temp_location_status -ne 0 ]] ||
    fail "a hosts file in a directory the process cannot write was reconciled 'successfully'"
  grep -qF 'no temporary file could be created beside it' "$temp_location_error" ||
    fail "the temp file was not created beside the target: creating it there is impossible in a 0555 directory, so this run should have refused with 'no temporary file could be created beside it' and instead said: $(cat "$temp_location_error")"
  cmp -s "$unwritable_hosts" "$work/unwritable.before" ||
    fail "the temp-location refusal still altered the target hosts file: $(cat "$unwritable_hosts")"
fi

# ---------- LAYER 2n: the install preserves the target's OWNER ---------------
# `chmod` and `chown` are separate steps and only the mode half was covered, so
# deleting the chown line outright left this suite green. As a non-root test the
# uid cannot change, but the GROUP can and it is the half that actually moves:
# macOS gives a new file its DIRECTORY's group, so the temp file never inherits
# the target's group and only the chown puts it back.
owner_group_ids=()
read -r -a owner_group_ids <<<"$(id -G)"
owner_dir_group=""
owner_file_group=""
for candidate_group_id in "${owner_group_ids[@]}"; do
  if [[ -z $owner_dir_group ]]; then
    owner_dir_group="$candidate_group_id"
  elif [[ $candidate_group_id != "$owner_dir_group" ]]; then
    owner_file_group="$candidate_group_id"
    break
  fi
done
[[ -n $owner_file_group ]] ||
  fail "this user belongs to only one group (${owner_group_ids[*]}), so the owner-preservation fixture cannot distinguish the temp file's group from the target's"
owner_dir="$work/owner"
mkdir -p "$owner_dir"
chgrp "$owner_dir_group" "$owner_dir" ||
  fail "could not set the fixture directory's group to $owner_dir_group"
owner_hosts="$owner_dir/hosts"
printf '127.0.0.1\tlocalhost\n198.51.100.1\tpin.example.test\tpin\n' >"$owner_hosts"
chgrp "$owner_file_group" "$owner_hosts" ||
  fail "could not set the fixture hosts file's group to $owner_file_group"
# The fixture only proves something while these two differ.
[[ "$(file_owner "$owner_dir")" != "$(file_owner "$owner_hosts")" ]] ||
  fail "the owner fixture's directory and hosts file ended up in the same group, so a deleted chown would be invisible"
owner_before="$(file_owner "$owner_hosts")"
run_pin1 "$owner_hosts"
grep -qxF "$want1" "$owner_hosts" || fail "the owner-preservation run did not converge the pin"
owner_after="$(file_owner "$owner_hosts")"
[[ $owner_after == "$owner_before" ]] ||
  fail "the install changed the hosts file's owner from $owner_before to $owner_after; the temp file's own owner (it takes the DIRECTORY's group) leaked through, so /etc/hosts would come back owned by whoever ran the apply"

# ---------- LAYER 2o: the loopback record need not be named localhost --------
# The gate refuses a rebuild that leaves 127.0.0.1 mapping NOTHING. It
# deliberately does not demand the name `localhost`, because nothing here knows
# which name a given machine resolves localhost through. That latitude had no
# coverage: every fixture that must SATISFY the gate happened to name localhost,
# so narrowing it to require that literal name passed the whole suite while
# refusing every pin on a machine whose loopback record reads
# "127.0.0.1 loopback my-mac", which aborts `chezmoi apply` outright.
other_name="$work/other-loopback-name-hosts"
printf '127.0.0.1\tloopback\tmy-mac\n198.51.100.1\tpin.example.test\tpin\n' >"$other_name"
run_pin1 "$other_name"
other_name_expected="$work/other-loopback-name-expected"
printf '127.0.0.1\tloopback\tmy-mac\n%s\n' "$want1" >"$other_name_expected"
cmp -s "$other_name" "$other_name_expected" ||
  fail "a loopback record that does not name localhost must satisfy the gate and survive byte-exact; got: $(diff "$other_name_expected" "$other_name" | head -5)"

# ---------- LAYER 2k: a leading-dash field stays DATA in every parser --------
# Shell-inert is not enough: a field that reached grep unbound became an OPTION
# (-eunrelated.ts.net turned into `-e unrelated.ts.net` and deleted the REAL
# unrelated.ts.net line). No parser downstream of the shell may read pin data
# as anything but bytes.
render_fixture dashpin <<'EOF'
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: "-eunrelated.ts.net"
      ip: "192.0.2.11"
      short: dashpin
EOF
# shellcheck disable=SC2016
expected_dash='sudo "$tailnet_pin_helper" '"'-eunrelated.ts.net' '192.0.2.11' 'dashpin'"
grep -qxF "$expected_dash" "$work/dashpin.rendered" ||
  fail "generated leading-dash pin command missing or wrong; expected exactly: $expected_dash"
dashhosts="$work/dash-hosts"
printf '127.0.0.1\tlocalhost\n9.9.9.9\tunrelated.ts.net\tunrelated\n' >"$dashhosts"
run_pin "$dashhosts" "-eunrelated.ts.net" "192.0.2.11" "dashpin" ||
  fail "leading-dash pin failed against $dashhosts"
dash_expected="$work/dash-expected"
printf '127.0.0.1\tlocalhost\n9.9.9.9\tunrelated.ts.net\tunrelated\n192.0.2.11\t-eunrelated.ts.net\tdashpin\n' >"$dash_expected"
cmp -s "$dashhosts" "$dash_expected" ||
  fail "a leading-dash fqdn steered a downstream parser; the REAL unrelated.ts.net line must survive; got: $(diff "$dash_expected" "$dashhosts" | head -5)"

# ---------- LAYER 2l: pin data is DATA, never source, in either shell --------
# The generated command runs under `sudo`, so there are TWO shells: the outer one
# this runner is written in, and the inner ROOT one. Interpolating a pin field
# into an sh -c string put it inside double quotes in that inner root shell, so a
# command substitution in the data executed AS ROOT. Each field now travels as a
# positional argument, which no shell re-parses as source.
#
# Four hostile shapes across the three fields, so a fix that closes only one is
# caught: command substitution, backtick substitution, a printf format
# directive, and SINGLE QUOTES. The quotes are load-bearing: shellSingleQuoted's
# entire job is the ' -> '\'' rewrite, so without a quote in a fixture field a
# naive literal-quote regression would render byte-identical commands for clean
# data and this suite would never notice. TWO adjacent quotes, so the naive
# render still balances its quotes and PARSES (one quote would unbalance the
# line into a syntax error and nothing would run at all): the outer shell then
# EATS the data quotes as its own syntax and delivers a corrupted line, which
# the byte-exact assertion below catches; the marker assertion stands guard
# for arrangements where a substitution lands unquoted and executes. The
# payload is `:>file` (the : builtin plus a redirect) because it creates the
# marker WITHOUT any whitespace: the render-time column validation rightly
# refuses whitespace-bearing fields, and an injection needs no spaces to do
# damage.
marker="$work/PIN_INJECTION_EXECUTED"
hostile_fqdn="a''\$(:>$marker).example.test"
hostile_ip='192.0.2.9%s'
hostile_short="s\`:>$marker\`"
render_fixture hostile <<EOF
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: "$hostile_fqdn"
      ip: "$hostile_ip"
      short: "$hostile_short"
EOF
hostile_rendered="$work/hostile.rendered"
if [[ -s $hostile_rendered ]]; then
  rm -f "$marker"
  hostile_hosts="$work/hostile-hosts"
  # Seeded with loopback because installing the result is gated on it: an edit
  # that would leave /etc/hosts without 127.0.0.1 is refused by design.
  printf '127.0.0.1\tlocalhost\n' >"$hostile_hosts"
  # Both emitted lines, not just the sudo one. The trace `echo` runs in the OUTER
  # user shell, so an unquoted field there is its own injection, at user rather
  # than root privilege. Running only the sudo line would leave that uncovered.
  # sudo is stripped and the reconciler is bound to the fixture through the same
  # seam the behavior tests use.
  while IFS= read -r line; do
    cmd="${line#sudo }"
    cmd="${cmd/\"\$tailnet_pin_helper\"/\"$RECONCILER\"}"
    TAILNET_PIN_HOSTS_FILE="$hostile_hosts" bash -c "$cmd" >/dev/null 2>&1 || true
  done < <(grep -E '^(echo |sudo )' "$hostile_rendered")
  [[ -e $marker ]] &&
    fail "pin data EXECUTED: the generated command ran a substitution from the data (this is root under sudo)"

  # Every field must arrive byte-for-byte, which is what proves each was carried
  # as data rather than as text some shell re-read. The whole-line comparison is
  # also what pins the printf FORMAT: move data back into the format position and
  # the %s in the ip is consumed as a directive, so the line comes out truncated.
  expected_line="$(printf '%s\t%s\t%s' "$hostile_ip" "$hostile_fqdn" "$hostile_short")"
  grep -qxF -- "$expected_line" "$hostile_hosts" ||
    fail "hostile pin line wrong; expected exactly $(printf '%q' "$expected_line"), got $(printf '%q' "$(cat "$hostile_hosts")")"
fi

# ---------- LAYER 3: shape of the REAL pins data -----------------------------
pin_count="$(yq eval '.macos.tailnet_pins | length' "$YAML")"
[[ $pin_count =~ ^[0-9]+$ && $pin_count -ge 1 ]] ||
  fail "real YAML must declare at least one tailnet pin (got: $pin_count)"

# Tailscale node IPs live in CGNAT 100.64.0.0/10 -> second octet 64-127.
cgnat_regex='^100\.(6[4-9]|[789][0-9]|1[01][0-9]|12[0-7])\.[0-9]{1,3}\.[0-9]{1,3}$'
while IFS=$'\t' read -r fqdn ip short; do
  [[ -n $fqdn && -n $ip && -n $short && $fqdn != "null" && $ip != "null" && $short != "null" ]] ||
    fail "pin has empty/missing fields: fqdn='$fqdn' ip='$ip' short='$short'"
  [[ $ip =~ $cgnat_regex ]] ||
    fail "pin IP '$ip' is not inside the Tailscale CGNAT range 100.64.0.0/10 ($fqdn)"
  [[ $fqdn == *.ts.net ]] || fail "pin FQDN '$fqdn' is not a MagicDNS .ts.net name"
  [[ $fqdn == "$short".* ]] ||
    fail "pin short name '$short' is not the first label of '$fqdn'"
done < <(yq eval '.macos.tailnet_pins[] | [.fqdn, .ip, .short] | @tsv' "$YAML")

echo "tailnet-pins: OK (exact render per pin against the deployed reconciler, gated on its sha256; malformed, non-string and name-colliding pins refuse to render; the real reconciler converges, repairs an unterminated final line rather than reporting it converged, refuses loudly, refuses an unreadable source and a self-satisfied loopback gate, accepts an indented loopback record and one that does not name localhost while still refusing an indented decoy, creates its temp beside the target, preserves the target's mode and owner, leaves no temp after any of the ${#actual_cleaned_up_signal_names[@]} signals it declares for cleanup under ${#INTERPRETERS[@]} interpreter(s), survives symlinks; hostile fields stay inert; $pin_count real pin(s) well-formed)"
