#!/usr/bin/env bash
# tailnet-pins.sh, MagicDNS /etc/hosts fallback pins are STRUCTURED DATA
# (`macos.tailnet_pins` in .chezmoidata/macos_system_setup.yaml); the Tier-2 sudo
# runner template (run_onchange_after_41) generates the convergence command for
# each pin. Test layers:
#
# LAYER 1, MACHINERY (fixture): copy the REAL template into a temp chezmoi
# source dir with fixture chezmoidata carrying test-owned pins (TEST-NET-1
# addresses, never real tailnet data), render it, and assert:
#   - the EXACT generated command string per pin (expectation hardcoded here,
#     an independent derivation, never re-implemented from the template logic);
#   - `sudo -v` is emitted even when the system_setup commands list is EMPTY
#     (pins must still apply; the upfront timestamp covers them);
#   - a fixture with NO tailnet_pins key still renders (the `index` absent-key
#     gotcha) and keeps the `exit 0` early-return;
#   - a pin field that is not a single hosts column (whitespace including
#     newlines, a # that starts a hosts comment, empty, or missing) REFUSES to
#     render, so one pin can never smuggle extra columns or extra LINES;
#   - executing the generated commands against temp hosts files: idempotence,
#     stale-IP correction, an exact line PLUS a stale duplicate collapses to
#     exactly one line, the filter keys on hosts FIELD STRUCTURE (grep
#     word-boundary victims like pin.example.test.evil survive), the pin owns
#     its short name, comments and blank lines survive a rebuild, every
#     refusal (lost loopback, loopback decoy, missing file) exits NONZERO and
#     says why on stderr, a failed install never truncates the target, the
#     installed file keeps the target's mode, and a missing trailing newline
#     never produces a joined line.
#
# LAYER 1d, INERTNESS: hostile pin data (command substitution, backticks, a %s
# printf directive, and single quotes, the one character shellSingleQuoted
# exists to rewrite) must ride as positional arguments, never execute in either
# shell, and arrive in the hosts file byte-exact.
#
# LAYER 2, SHAPE (real data): read the real YAML's pins via yq and validate form
# only, fields non-empty, ip inside the proper Tailscale CGNAT range
# 100.64.0.0/10, fqdn ends .ts.net, short == the fqdn's first label. No
# behavioral expectations are derived from real data.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_41-macos-system-setup.sh.tmpl"
YAML="$REPO_ROOT/.chezmoidata/macos_system_setup.yaml"

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

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# render_fixture <name> <fixture-yaml-body...on stdin> -> $work/<name>.rendered
render_fixture() {
  local name="$1"
  local src="$work/$name-src"
  mkdir -p "$src/.chezmoiscripts" "$src/.chezmoidata"
  cp "$TEMPLATE" "$src/.chezmoiscripts/"
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
  local src="$work/$name-src"
  mkdir -p "$src/.chezmoiscripts" "$src/.chezmoidata"
  cp "$TEMPLATE" "$src/.chezmoiscripts/"
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
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# Independent expectation: the exact command the template must generate per pin.
# The sh -c script is a CONSTANT, identical for every pin; only the trailing
# arguments differ. Single-quoted here so $1/$2/$3 stay literal, which is the
# property under test: they must reach the inner shell as parameters, not as
# text the template substituted.
# shellcheck disable=SC2016  # the literal $1/$2/$3 ARE the property under test:
# they must survive into the inner shell as parameters, so expanding them here
# would assert the opposite of what this pins.
pin_script='f=$1; s=$3; tmp=; bail(){ [ -n "$tmp" ] && rm -f "$tmp"; exit 1; }; w=$(printf "%s\t%s\t%s" "$2" "$1" "$3"); [ -f /etc/hosts ] || { echo "refusing to edit /etc/hosts for $f: the file is missing" >&2; exit 1; }; tmp=$(mktemp /etc/hosts.XXXXXXXX) || exit 1; mc=0; ex=0; set -f; while IFS= read -r l || [ -n "$l" ]; do hit=0; set -- ${l%%#*}; if [ $# -gt 1 ]; then shift; for n in "$@"; do if [ "$n" = "$f" ] || [ "$n" = "$s" ]; then hit=1; fi; done; fi; if [ "$hit" = 1 ]; then mc=$((mc+1)); if [ "$l" = "$w" ]; then ex=1; fi; else printf "%s\n" "$l" || bail; fi; done </etc/hosts >"$tmp" || bail; if [ "$mc" = 1 ] && [ "$ex" = 1 ]; then rm -f "$tmp"; exit 0; fi; printf "%s\n" "$w" >>"$tmp" || bail; grep -qE "^127\.0\.0\.1[[:space:]]" "$tmp" || { echo "refusing to rewrite /etc/hosts for $f: the filtered result lost its loopback entry" >&2; bail; }; m=$(stat -c "%a" /etc/hosts 2>/dev/null) || m=$(stat -f "%Lp" /etc/hosts) || bail; o=$(stat -c "%u:%g" /etc/hosts 2>/dev/null) || o=$(stat -f "%u:%g" /etc/hosts) || bail; chmod "$m" "$tmp" || bail; chown "$o" "$tmp" || bail; mv -f "$tmp" /etc/hosts || bail'
expected_1="sudo sh -c '$pin_script' sh 'pin.example.test' '192.0.2.7' 'pin'"
expected_2="sudo sh -c '$pin_script' sh 'pin2.example.test' '192.0.2.8' 'pin2'"
grep -qxF "$expected_1" "$rendered" ||
  fail "generated pin command 1 missing or wrong; expected exactly: $expected_1 (rendered: $(cat "$rendered"))"
grep -qxF "$expected_2" "$rendered" ||
  fail "generated pin command 2 missing or wrong; expected exactly: $expected_2"
grep -qxF 'sudo -v' "$rendered" ||
  fail "sudo -v not emitted with an empty commands list, the upfront timestamp must cover pin commands"
if grep -qxF 'exit 0' "$rendered"; then
  fail "early-return emitted despite pins being configured, pins would never apply"
fi

# The exact desired line for pin 1, reused by the execution tests below.
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

# ---------- execution helpers ------------------------------------------------
run_pin_command() { # strip the sudo prefix, point at the temp hosts file, run
  local cmd="${1#sudo }"
  cmd="${cmd///etc\/hosts/$hosts}"
  bash -c "$cmd" || fail "generated pin command failed (rc=$?): $cmd"
}

run_against() { # <command> <hosts-file>
  local cmd="${1#sudo }"
  cmd="${cmd///etc\/hosts/$2}"
  bash -c "$cmd" || fail "pin command failed (rc=$?) against $2"
}

# A refusal must be LOUD: nonzero exit (the rendered runner runs under set -e,
# so chezmoi apply fails instead of reporting success over a pin that did not
# apply) and a stderr line saying why.
run_expect_refusal() { # <command> <hosts-file> <stderr-substring> <label>
  local cmd="${1#sudo }" refusal_err="$work/refuse-$4.err"
  cmd="${cmd///etc\/hosts/$2}"
  if bash -c "$cmd" >/dev/null 2>"$refusal_err"; then
    fail "$4: expected a nonzero refusal, but the pin command exited 0 against $2"
  fi
  grep -qF -- "$3" "$refusal_err" ||
    fail "$4: refusal ran silent or with the wrong message; wanted stderr to contain '$3', got: $(cat "$refusal_err")"
}

run_with_shims() { # <command> <hosts-file> <shim-dir>: force tool failures
  local cmd="${1#sudo }"
  cmd="${cmd///etc\/hosts/$2}"
  PATH="$3:$PATH" bash -c "$cmd"
}

# GNU stat first, BSD fallback: GNU's -f means "filesystem status" and would
# SUCCEED with useless output, so the GNU form must be the one tried first.
file_mode() { # <file>
  stat -c '%a' "$1" 2>/dev/null || stat -f '%Lp' "$1"
}

# ---------- LAYER 1c: execute the generated commands (idempotence) ----------
hosts="$work/hosts"
printf '127.0.0.1\tlocalhost\n255.255.255.255\tbroadcasthost\n' >"$hosts"

for round in 1 2; do
  run_pin_command "$expected_1"
  run_pin_command "$expected_2"
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

# ---------- LAYER 1c2: a pin whose IP changed must be CORRECTED --------------
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
run_against "$expected_1" "$stale"
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
run_against "$expected_1" "$stale"
cmp -s "$stale" "$work/stale.after1" || fail "correcting a pin is not idempotent on a second run"

# ---------- LAYER 1c3: an exact line PLUS a stale duplicate is NOT converged -
# "At least one correct line exists" is the wrong property: it reads a correct
# line plus a stale duplicate as converged and leaves two lines naming the pin.
# Convergence is "exactly one line names this pin, and it is exactly right".
dup="$work/dup-hosts"
printf '127.0.0.1\tlocalhost\n198.51.100.1\tpin.example.test\tpin\n%s\n' "$want1" >"$dup"
run_against "$expected_1" "$dup"
dup_expected="$work/dup-expected"
printf '127.0.0.1\tlocalhost\n%s\n' "$want1" >"$dup_expected"
cmp -s "$dup" "$dup_expected" ||
  fail "an exact line plus a stale duplicate must collapse to exactly one pin line; got: $(diff "$dup_expected" "$dup" | head -5)"

# ---------- LAYER 1c4: comments and blank lines survive; a commented-out copy
# of the exact pin line must not count as convergence (an ACTIVE stale line
# would then be left uncorrected, reopening the exact bug this command fixes).
commented="$work/commented-hosts"
printf '127.0.0.1\tlocalhost\n\n# plain comment\n#%s\n198.51.100.1\tpin.example.test\tpin\n' "$want1" >"$commented"
run_against "$expected_1" "$commented"
commented_expected="$work/commented-expected"
printf '127.0.0.1\tlocalhost\n\n# plain comment\n#%s\n%s\n' "$want1" "$want1" >"$commented_expected"
cmp -s "$commented" "$commented_expected" ||
  fail "a commented-out copy of the pin line must be preserved AND never satisfy convergence; got: $(diff "$commented_expected" "$commented" | head -5)"

# ---------- LAYER 1c5: the pin owns its short name ---------------------------
# The pin exists so BOTH names answer with the tailnet address when MagicDNS is
# down. An unrelated line claiming only the short name would compete with the
# pin for that name, so it is dropped, by decision, not by accident.
shortclaim="$work/shortclaim-hosts"
printf '127.0.0.1\tlocalhost\n192.168.1.5\tpin\n' >"$shortclaim"
run_against "$expected_1" "$shortclaim"
shortclaim_expected="$work/shortclaim-expected"
printf '127.0.0.1\tlocalhost\n%s\n' "$want1" >"$shortclaim_expected"
cmp -s "$shortclaim" "$shortclaim_expected" ||
  fail "a line claiming the pin's short name must be replaced by the pin; got: $(diff "$shortclaim_expected" "$shortclaim" | head -5)"

# ---------- LAYER 1c6: refusals are LOUD and change nothing ------------------
# Rewriting /etc/hosts as root is the one step here that can break the machine.
# A rebuild that lost its loopback entry must be refused: file untouched, exit
# NONZERO (so chezmoi apply fails instead of reporting success), reason on
# stderr.
noloop="$work/noloop-hosts"
printf '198.51.100.1\tpin.example.test\tpin\n' >"$noloop"
cp "$noloop" "$work/noloop.before"
run_expect_refusal "$expected_1" "$noloop" "lost its loopback entry" noloop
cmp -s "$noloop" "$work/noloop.before" ||
  fail "a rewrite that would drop the loopback entry was installed instead of refused: $(cat "$noloop")"

# The gate must match a REAL loopback line, anchored: 127.0.0.100 is a decoy
# that an unanchored 127.0.0.1 match would accept, installing a hosts file with
# no working localhost when the actual loopback line was filtered away (here it
# also names the pin, so the rebuild drops it).
decoy="$work/decoy-hosts"
printf '127.0.0.1\tlocalhost pin.example.test\n127.0.0.100\tdev.local\n' >"$decoy"
cp "$decoy" "$work/decoy.before"
run_expect_refusal "$expected_1" "$decoy" "lost its loopback entry" decoy
cmp -s "$decoy" "$work/decoy.before" ||
  fail "a rebuild whose only 127.0.0.1 match is the decoy 127.0.0.100 was installed instead of refused: $(cat "$decoy")"

# A missing hosts file is refused the same way: nothing safe exists to rewrite,
# and seeding a pin-only /etc/hosts with no loopback would break the machine.
absent="$work/absent-hosts"
run_expect_refusal "$expected_1" "$absent" "the file is missing" absent
[[ ! -e $absent ]] ||
  fail "a missing hosts file was seeded with pin content instead of refused: $(cat "$absent")"

# ---------- LAYER 1c7: a failed install must never destroy the target --------
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
if run_with_shims "$expected_1" "$atomic_hosts" "$shimdir" >/dev/null 2>&1; then
  fail "the install step failed but the pin command still exited 0; install status must be checked"
fi
cmp -s "$atomic_hosts" "$work/atomic.before" ||
  fail "a failed install altered or truncated the target hosts file: $(cat "$atomic_hosts")"
[[ $(find "$atomic_dir" -type f | wc -l) -eq 1 ]] ||
  fail "a failed install left temp droppings next to the target: $(ls "$atomic_dir")"

# ---------- LAYER 1c8: the installed file keeps the target's mode ------------
# The template comment claims the install preserves the file's mode. Pin it:
# mktemp creates 0600, /etc/hosts is 0644, and an install that ships the temp
# file's mode would leave a hosts file other daemons cannot read.
modehosts="$work/mode-hosts"
printf '127.0.0.1\tlocalhost\n198.51.100.1\tpin.example.test\tpin\n' >"$modehosts"
chmod 644 "$modehosts"
run_against "$expected_1" "$modehosts"
grep -qxF "$want1" "$modehosts" || fail "mode-preservation run did not converge the pin"
mode_after="$(file_mode "$modehosts")" || fail "could not stat the installed hosts file"
[[ $mode_after == 644 ]] ||
  fail "the install changed the hosts file mode from 644 to $mode_after (mktemp's private mode leaking through?)"

# ---------- LAYER 1c9: a missing trailing newline never joins lines ----------
# Appending to a file whose last line is unterminated used to produce one
# joined line (localhost192.0.2.7...). The rebuild must terminate every line.
nonl="$work/nonl-hosts"
printf '127.0.0.1\tlocalhost\n198.51.100.1\tpin.example.test\tpin\n10.0.0.9\tkeeper.example.test' >"$nonl"
run_against "$expected_1" "$nonl"
nonl_expected="$work/nonl-expected"
printf '127.0.0.1\tlocalhost\n10.0.0.9\tkeeper.example.test\n%s\n' "$want1" >"$nonl_expected"
cmp -s "$nonl" "$nonl_expected" ||
  fail "a hosts file without a trailing newline was corrupted by the rebuild; got: $(diff "$nonl_expected" "$nonl" | head -5)"

# ---------- LAYER 1c10: a leading-dash field stays DATA in every parser ------
# Shell-inert is not enough: a field that reaches grep unbound became an OPTION
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
expected_dash="sudo sh -c '$pin_script' sh '-eunrelated.ts.net' '192.0.2.11' 'dashpin'"
grep -qxF "$expected_dash" "$work/dashpin.rendered" ||
  fail "generated leading-dash pin command missing or wrong; expected exactly: $expected_dash"
dashhosts="$work/dash-hosts"
printf '127.0.0.1\tlocalhost\n9.9.9.9\tunrelated.ts.net\tunrelated\n' >"$dashhosts"
run_against "$expected_dash" "$dashhosts"
dash_expected="$work/dash-expected"
printf '127.0.0.1\tlocalhost\n9.9.9.9\tunrelated.ts.net\tunrelated\n192.0.2.11\t-eunrelated.ts.net\tdashpin\n' >"$dash_expected"
cmp -s "$dashhosts" "$dash_expected" ||
  fail "a leading-dash fqdn steered a downstream parser; the REAL unrelated.ts.net line must survive; got: $(diff "$dash_expected" "$dashhosts" | head -5)"

# ---------- LAYER 1d: pin data is DATA, never source, in either shell --------
# The generated command runs `sudo sh -c`, so there are TWO shells: the outer one
# this runner is written in, and the inner ROOT one sudo starts. Interpolating a
# pin field into the sh -c string put it inside double quotes in that inner root
# shell, so a command substitution in the data executed AS ROOT. The fix carries
# each field as a positional argument, which no shell re-parses as source.
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
  # The sh -c script text must stay the FIXED constant even for hostile data:
  # fields ride as arguments AFTER it, never inside it.
  grep -qF "sudo sh -c '$pin_script' sh '" "$hostile_rendered" ||
    fail "the sh -c script text is no longer the fixed constant when hostile pin data renders; fields must ride as arguments only"

  # Executing it must move bytes, not run them. sudo stripped, hosts redirected.
  rm -f "$marker"
  hostile_hosts="$work/hostile-hosts"
  # Seeded with loopback because installing the result is gated on it: an edit
  # that would leave /etc/hosts without 127.0.0.1 is refused by design.
  printf '127.0.0.1\tlocalhost\n' >"$hostile_hosts"
  # Both emitted lines, not just the sudo one. The trace `echo` runs in the OUTER
  # user shell, so an unquoted field there is its own injection, at user rather
  # than root privilege. Running only the sh -c line would leave that uncovered.
  while IFS= read -r line; do
    cmd="${line#sudo }"
    cmd="${cmd///etc\/hosts/$hostile_hosts}"
    bash -c "$cmd" >/dev/null 2>&1 || true
  done < <(grep -E '^(echo |sudo sh -c )' "$hostile_rendered")
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

# ---------- LAYER 2: shape of the REAL pins data -----------------------------
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

echo "tailnet-pins: OK (exact idempotent convergence commands from fixture data; malformed pin fields refuse to render; refusals are loud and nonzero; hostile fields stay inert; $pin_count real pin(s) well-formed)"
