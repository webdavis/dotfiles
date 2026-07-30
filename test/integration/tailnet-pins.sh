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
#   - the helper path the render names is a path chezmoi actually deploys.
#
# LAYER 2, BEHAVIOR (the real reconciler against temp hosts files): idempotence,
# stale-IP correction, an exact line PLUS a stale duplicate collapses to exactly
# one line, the filter keys on hosts FIELD STRUCTURE (grep word-boundary victims
# like pin.example.test.evil survive), the pin owns its short name, comments and
# blank lines survive a rebuild, every refusal exits NONZERO and says why on
# stderr, a failed install never truncates the target and leaves no temp, a
# SIGNAL leaves no temp either, the installed file keeps the target's mode, a
# symlinked target keeps its indirection and its referent's metadata, a missing
# trailing newline never produces a joined line, and a field that is not one
# hosts column is refused at the entry point too.
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
# shellcheck disable=SC2016  # the literal $tailnet_pin_helper IS the property
# under test; expanding it here would assert the opposite.
helper_assignment='tailnet_pin_helper="$HOME/'"$RECONCILER_TARGET_SUFFIX"'"'
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

# GNU stat first, BSD fallback: GNU's -f means "filesystem status" and would
# SUCCEED with useless output, so the GNU form must be the one tried first.
file_mode() { # <file>
  stat -L -c '%a' "$1" 2>/dev/null || stat -L -f '%Lp' "$1"
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
# satisfies ^127\.0\.0\.1[[:space:]] and maps NOTHING: hosts(5) says characters
# from # to end of line are not interpreted, and a record needs an official host
# name after the address. With the real loopback line carrying the pin's owned
# short name (so the rebuild drops it), the old gate installed a hosts file with
# ZERO valid loopback records, exit 0, no stderr. The machine loses localhost.
comment_decoy="$work/comment-decoy-hosts"
printf '127.0.0.1\tlocalhost\tpin\n127.0.0.1\t# comment-only decoy\n' >"$comment_decoy"
cp "$comment_decoy" "$work/comment-decoy.before"
run_expect_refusal "$comment_decoy" "lost its loopback entry" comment-decoy
cmp -s "$comment_decoy" "$work/comment-decoy.before" ||
  fail "a rebuild whose only 127.0.0.1 line is COMMENT-ONLY was installed instead of refused; the machine would have no localhost: $(cat "$comment_decoy")"

# The same hole with no comment at all: a bare address is not a mapping either.
address_only="$work/address-only-hosts"
printf '127.0.0.1\tlocalhost\tpin\n127.0.0.1\t\n' >"$address_only"
cp "$address_only" "$work/address-only.before"
run_expect_refusal "$address_only" "lost its loopback entry" address-only
cmp -s "$address_only" "$work/address-only.before" ||
  fail "a rebuild whose only 127.0.0.1 line has NO host name was installed instead of refused: $(cat "$address_only")"

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
signal_dir="$work/signal"
mkdir -p "$signal_dir"
signal_hosts="$signal_dir/hosts"
printf '127.0.0.1\tlocalhost\n198.51.100.1\tpin.example.test\tpin\n' >"$signal_hosts"
cp "$signal_hosts" "$work/signal.before"
signal_shimdir="$work/signal-shims"
mkdir -p "$signal_shimdir"
# shellcheck disable=SC2016  # $PPID must stay literal: it is the SHIM's own
# parent (the reconciler), resolved when the shim runs, not this test's parent.
printf '#!/bin/sh\nkill -TERM "$PPID"\nsleep 1\n' >"$signal_shimdir/chmod"
chmod +x "$signal_shimdir/chmod"
signal_status=0
PATH="$signal_shimdir:$PATH" TAILNET_PIN_HOSTS_FILE="$signal_hosts" \
  "$RECONCILER" pin.example.test 192.0.2.7 pin >/dev/null 2>&1 || signal_status=$?
[[ $signal_status -ne 0 ]] ||
  fail "a SIGTERM mid-install still exited 0"
cmp -s "$signal_hosts" "$work/signal.before" ||
  fail "a SIGTERM mid-install altered the target hosts file: $(cat "$signal_hosts")"
[[ $(find "$signal_dir" -type f | wc -l) -eq 1 ]] ||
  fail "a SIGTERM mid-install left temp droppings next to the target: $(ls "$signal_dir")"

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

echo "tailnet-pins: OK (exact render per pin against the deployed reconciler; malformed and non-string pin fields refuse to render; the real reconciler converges, refuses loudly, survives signals and symlinks; hostile fields stay inert; $pin_count real pin(s) well-formed)"
