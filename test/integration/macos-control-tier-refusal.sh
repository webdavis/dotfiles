#!/usr/bin/env bash
# macos-control-tier-refusal.sh -- the tiered control model: every record in
# macos_defaults.yaml and macos_system_setup.yaml declares which of three tiers
# it belongs to, and every consumer refuses, fail-closed, anything that does
# not match its declared tier.
#
#   enforce  settable from the command line; the runner renders the write.
#   verify   readable but NOT settable; drift detection only. The runner
#            renders NO mutating command: the branch that emits one is not
#            taken, so the runner physically cannot set it.
#   manual   needs an interactive step or a management profile; carries a
#            REQUIRED runbook field and renders a pointer line, no command.
#
# The refusal is the load-bearing part, so it is pinned as TOTAL:
#   1. A record with no tier ABORTS the render, naming the record. Not a
#      warning, not a skipped record: a fixture that skipped the offender
#      would render cleanly, and the required failure catches it. (That a
#      failed render emits no write at all is chezmoi's own guarantee; see
#      the note on the reject helpers.)
#   2. An unrecognized tier value aborts the render naming the value, from
#      the VALIDATION pass, never the render loop's distinctly-marked
#      fail-closed arm (whose presence is pinned in source form). A blank
#      tier and a set-but-empty tier are rejected as their own cases, never
#      conflated with absent, and nothing ever renders the literal text
#      <nil>. Blank is pinned against BOTH templates.
#   3. A verify record renders no mutating command, asserted as the ABSENCE of
#      the specific write string beside a control record that proves writes
#      render, and contributes no sudo prelude.
#   4. A verify or manual record carrying a mutating payload aborts the render
#      (defaults: a manual record carrying any of the five forbidden write
#      fields, one fixture per member so the list is pinned by completeness;
#      system_setup: a command or sudo on any non-enforce record). On a verify
#      defaults record type/value are the READ expectation the drift checker
#      compares, the one payload shape that is legitimate there. The inverse
#      lie aborts too: an enforce system_setup record with no command (absent,
#      blank, or empty).
#   5. A manual record without a runbook (absent, blank, or empty) aborts,
#      empty pinned against BOTH templates.
#   6. A manual record renders a runbook pointer and no command, and the
#      pointer goes through the single-quoting helper, so hostile runbook or
#      description text arrives literal and executes nothing.
#   7. The tools inherit the same refusal through the shared record stream:
#      apply writes ONLY enforce records, drift checks enforce AND verify but
#      never manual, capture appends tier: enforce, and an unknown tier makes
#      every one of them refuse the whole file before acting on any of it.
#      Behind that gate, apply's and drift's own in-loop refusal is reached
#      via a doctored stream and refuses fail-closed on its own.
#
# Real chezmoi and yq; `defaults`, `sudo`, `osascript`, and `killall` are
# stubbed. Never runs real sudo, never touches /Library.
#
# This test deals in LITERAL shell-injection payloads, so $(...) inside single
# quotes is deliberate (it must NOT expand here).
# shellcheck disable=SC2016
set -euo pipefail

# Scrubbed at SCRIPT scope, before any git or chezmoi call. Git exports GIT_DIR
# to every hook it runs and this suite runs from the pre-push hook.
unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TIER1_TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl"
TIER2_TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_41-macos-system-setup.sh.tmpl"
LIB="$REPO_ROOT/dot_local/bin/macos-defaults-lib.sh"
APPLY="$REPO_ROOT/dot_local/bin/executable_macos-defaults-apply.sh"
CAPTURE="$REPO_ROOT/dot_local/bin/executable_macos-defaults-capture.sh"
DRIFT="$REPO_ROOT/dot_local/bin/executable_macos-defaults-drift.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# A bare `! grep` is dead under `set -e` unless it happens to be the last
# statement, so every negative below goes through these helpers.
refute_file_contains() { # <file> <fixed-string> <message>
  if grep -qF -- "$2" "$1"; then
    fail "$3"
  fi
}

refute_file_matches() { # <file> <regex> <message>
  if grep -qE -- "$2" "$1"; then
    fail "$3"
  fi
}

assert_file_contains() { # <file> <fixed-string> <message>
  grep -qF -- "$2" "$1" || fail "$3"
}

for tool in chezmoi yq; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'SKIP: %s not on PATH; cannot exercise the tier model\n' "$tool"
    exit 0
  }
done
for required_file in "$TIER1_TEMPLATE" "$TIER2_TEMPLATE" "$LIB" "$APPLY" "$CAPTURE" "$DRIFT"; do
  [[ -f $required_file ]] || fail "missing file: $required_file"
done

# Canonicalize away macOS's /var -> /private/var symlink.
sandbox="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$sandbox"' EXIT
render_home="$sandbox/render-home"
mkdir -p "$render_home" "$sandbox/home"
render_error="$sandbox/render.err"

# make_defaults_source / make_setup_source -- create a chezmoi source tree
# whose one data file is read from stdin; print the tree's path.
make_defaults_source() {
  local source_dir
  source_dir="$(mktemp -d "$sandbox/defaults-src.XXXXXX")"
  mkdir -p "$source_dir/.chezmoidata"
  cat >"$source_dir/.chezmoidata/macos_defaults.yaml"
  printf '%s\n' "$source_dir"
}

make_setup_source() {
  local source_dir
  source_dir="$(mktemp -d "$sandbox/setup-src.XXXXXX")"
  mkdir -p "$source_dir/.chezmoidata"
  cat >"$source_dir/.chezmoidata/macos_system_setup.yaml"
  printf '%s\n' "$source_dir"
}

render_template() { # <template> <source-dir> <out-file> <err-file>
  HOME="$render_home" chezmoi --source "$2" execute-template --no-tty <"$1" >"$3" 2>"$4"
}

# Non-darwin hosts render an empty body, so there is nothing to exercise.
probe_src="$(
  make_defaults_source <<'EOF'
macos:
  defaults:
    - domain: com.example.probe
      key: ProbeKey
      type: bool
      value: true
      tier: enforce
  killall: []
EOF
)"
render_template "$TIER1_TEMPLATE" "$probe_src" "$sandbox/rendered-probe" "$render_error" ||
  fail "the probe render must succeed (stderr: $(cat "$render_error"))"
if [[ -z "$(tr -d '[:space:]' <"$sandbox/rendered-probe")" ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# assert_tier1_rejects / assert_tier2_rejects <label> <stderr-fragment...> --
# feed a fixture on stdin, require the render to FAIL, and require the message
# to carry every named fragment.
#
# Deliberately NOT asserted: that the rejected render's stdout carries no
# write or command line. chezmoi buffers the whole render and discards it on
# failure (verified empirically: a template that emits two lines and then
# calls fail exits 1 with ZERO bytes on stdout), so that assertion is
# satisfied by chezmoi's buffering no matter where our validation sits, and
# an assertion that cannot fail reads as coverage while pinning nothing.
assert_tier1_rejects() { # <label> <stderr-fragment...>   (fixture on stdin)
  local label="$1"
  shift
  local source_dir out_file status=0 fragment
  source_dir="$(make_defaults_source)"
  out_file="$sandbox/rejected-tier1"
  render_template "$TIER1_TEMPLATE" "$source_dir" "$out_file" "$render_error" || status=$?
  [[ $status -ne 0 ]] ||
    fail "$label: the render must fail (got 0, render: $(cat "$out_file"))"
  for fragment in "$@"; do
    assert_file_contains "$render_error" "$fragment" \
      "$label: the failure must name '$fragment' (stderr: $(cat "$render_error"))"
  done
}

assert_tier2_rejects() { # <label> <stderr-fragment...>   (fixture on stdin)
  local label="$1"
  shift
  local source_dir out_file status=0 fragment
  source_dir="$(make_setup_source)"
  out_file="$sandbox/rejected-tier2"
  render_template "$TIER2_TEMPLATE" "$source_dir" "$out_file" "$render_error" || status=$?
  [[ $status -ne 0 ]] ||
    fail "$label: the render must fail (got 0, render: $(cat "$out_file"))"
  for fragment in "$@"; do
    assert_file_contains "$render_error" "$fragment" \
      "$label: the failure must name '$fragment' (stderr: $(cat "$render_error"))"
  done
}

# ---- 1: a record with no tier aborts the render -------------------------------

# The valid enforce record comes FIRST, so a template that warned and skipped
# the offender instead of aborting would render the file cleanly, and the
# helper's required render failure is what catches it.
assert_tier1_rejects 'defaults record with no tier' \
  'has no tier' 'com.example.untiered' 'UntieredKey' <<'EOF'
macos:
  defaults:
    - domain: com.example.tiered
      key: TieredKey
      type: bool
      value: true
      tier: enforce
    - domain: com.example.untiered
      key: UntieredKey
      type: bool
      value: true
  killall: []
EOF

assert_tier2_rejects 'system_setup record with no tier' \
  'has no tier' 'untiered setup record' <<'EOF'
macos:
  system_setup:
    - description: "tiered setup record"
      command: 'printf tiered-ran'
      tier: enforce
    - description: "untiered setup record"
      command: 'printf untiered-ran'
EOF

# ---- 2: an unrecognized tier aborts the render, naming the value --------------

assert_tier1_rejects 'unrecognized defaults tier' \
  'unrecognized tier "enforced"' 'com.example.typo' <<'EOF'
macos:
  defaults:
    - domain: com.example.typo
      key: TypoKey
      type: bool
      value: true
      tier: enforced
  killall: []
EOF
# The unrecognized-tier check exists at TWO sites per template: the validation
# pass and the render loop's fail-closed else arm, with deliberately distinct
# messages. The refusal above must come from the VALIDATION pass: if the loop
# marker shows up here, the validation copy is gone and the loop copy is
# masking its absence.
refute_file_contains "$render_error" 'reached the render loop' \
  'an unrecognized defaults tier must be refused by the validation pass, not the render loop'

# A blank scalar is nil, and stringifying nil yields the literal text <nil>.
# It must be rejected as its own case, and the message must not carry <nil>.
blank_tier_src="$(
  make_defaults_source <<'EOF'
macos:
  defaults:
    - domain: com.example.blanktier
      key: BlankTierKey
      type: bool
      value: true
      tier:
  killall: []
EOF
)"
blank_tier_status=0
render_template "$TIER1_TEMPLATE" "$blank_tier_src" "$sandbox/rendered-blank-tier" "$render_error" ||
  blank_tier_status=$?
[[ $blank_tier_status -ne 0 ]] || fail 'a blank tier must fail the render'
assert_file_contains "$render_error" 'has a blank tier' \
  "a blank tier must be named as blank (stderr: $(cat "$render_error"))"
refute_file_contains "$render_error" '<nil>' \
  'a blank tier must never surface as the stringified literal <nil>'

# A set-but-empty tier is an unrecognized VALUE, never treated as absent.
empty_tier_src="$(
  make_defaults_source <<'EOF'
macos:
  defaults:
    - domain: com.example.emptytier
      key: EmptyTierKey
      type: bool
      value: true
      tier: ""
  killall: []
EOF
)"
empty_tier_status=0
render_template "$TIER1_TEMPLATE" "$empty_tier_src" "$sandbox/rendered-empty-tier" "$render_error" ||
  empty_tier_status=$?
[[ $empty_tier_status -ne 0 ]] || fail 'a set-but-empty tier must fail the render'
assert_file_contains "$render_error" 'unrecognized tier ""' \
  "an empty tier must be rejected as an unrecognized value (stderr: $(cat "$render_error"))"
refute_file_contains "$render_error" 'has no tier' \
  'a set-but-empty tier must not be conflated with an absent one'

assert_tier2_rejects 'unrecognized system_setup tier' \
  'unrecognized tier "bogus"' 'bogus-tier record' <<'EOF'
macos:
  system_setup:
    - description: "bogus-tier record"
      command: 'printf bogus-ran'
      tier: bogus
EOF
# Same two-site rule as Tier 1: the refusal must come from the validation
# pass, never the render loop's distinctly-marked else arm.
refute_file_contains "$render_error" 'reached the render loop' \
  'an unrecognized system_setup tier must be refused by the validation pass, not the render loop'

# The Tier 2 blank tier is its own case too, tested here directly rather than
# assumed covered by the Tier 1 case above: the two templates carry separate
# copies of the branch, and one of them silently rotting is exactly what
# per-template pins exist to catch.
blank_tier_setup_src="$(
  make_setup_source <<'EOF'
macos:
  system_setup:
    - description: "blank-tier setup record"
      tier:
EOF
)"
blank_tier_setup_status=0
render_template "$TIER2_TEMPLATE" "$blank_tier_setup_src" "$sandbox/rendered-blank-tier-setup" "$render_error" ||
  blank_tier_setup_status=$?
[[ $blank_tier_setup_status -ne 0 ]] || fail 'a blank system_setup tier must fail the render'
assert_file_contains "$render_error" 'has a blank tier' \
  "a blank system_setup tier must be named as blank (stderr: $(cat "$render_error"))"
refute_file_contains "$render_error" '<nil>' \
  'a blank system_setup tier must never surface as the stringified literal <nil>'

# ---- 3: a verify record renders no mutating command ---------------------------

# The verify record carries the FULL write-shaped payload (its read
# expectation), so the write branch has everything it would need; the enforce
# record beside it proves the write branch renders. The absent string is the
# SPECIFIC write, not a shorter render.
verify_render_src="$(
  make_defaults_source <<'EOF'
macos:
  defaults:
    - domain: com.example.enforced
      key: EnforcedKey
      type: bool
      value: true
      tier: enforce
    - domain: com.example.verifyme
      key: VerifyKey
      type: bool
      value: false
      tier: verify
  killall:
    - Dock
EOF
)"
rendered_verify="$sandbox/rendered-verify"
render_template "$TIER1_TEMPLATE" "$verify_render_src" "$rendered_verify" "$render_error" ||
  fail "a verify defaults record must render cleanly (stderr: $(cat "$render_error"))"
grep -qxF "defaults write 'com.example.enforced' 'EnforcedKey' -bool 'true'" "$rendered_verify" ||
  fail "the enforce control record must still render its write (render: $(cat "$rendered_verify"))"
refute_file_contains "$rendered_verify" "defaults write 'com.example.verifyme' 'VerifyKey' -bool 'false'" \
  'a verify record must not render its specific defaults write'
refute_file_contains "$rendered_verify" 'com.example.verifyme' \
  'a verify record must contribute nothing at all to the rendered runner'

# A verify record at system scope is a READ of a root plist, not a write, so
# it must not pull in the sudo prelude either.
verify_system_src="$(
  make_defaults_source <<'EOF'
macos:
  defaults:
    - domain: com.example.sysverify
      key: SysVerifyKey
      type: bool
      value: true
      tier: verify
      scope: system
  killall: []
EOF
)"
rendered_verify_system="$sandbox/rendered-verify-system"
render_template "$TIER1_TEMPLATE" "$verify_system_src" "$rendered_verify_system" "$render_error" ||
  fail "a system-scope verify record must render cleanly (stderr: $(cat "$render_error"))"
refute_file_matches "$rendered_verify_system" '^sudo' \
  'a verify-only data file must render no sudo invocation, the -v prelude included'
refute_file_contains "$rendered_verify_system" 'sudo -v' \
  'a verify-only data file must render no sudo prelude'
refute_file_matches "$rendered_verify_system" '^(sudo )?defaults ' \
  'a verify-only data file must render no defaults invocation at all'

# system_setup: a verify record contributes nothing to the render, and a
# verify-only file must not render the spurious sudo -v prelude.
verify_setup_src="$(
  make_setup_source <<'EOF'
macos:
  system_setup:
    - description: "enforced setup control"
      command: 'printf enforced-setup-ran'
      tier: enforce
    - description: "verify-only setup control"
      tier: verify
EOF
)"
rendered_verify_setup="$sandbox/rendered-verify-setup"
render_template "$TIER2_TEMPLATE" "$verify_setup_src" "$rendered_verify_setup" "$render_error" ||
  fail "a verify system_setup record must render cleanly (stderr: $(cat "$render_error"))"
grep -qxF 'printf enforced-setup-ran' "$rendered_verify_setup" ||
  fail "the enforce control record must still render its command (render: $(cat "$rendered_verify_setup"))"
refute_file_contains "$rendered_verify_setup" 'verify-only setup control' \
  'a verify system_setup record must contribute nothing to the rendered runner'

verify_only_setup_src="$(
  make_setup_source <<'EOF'
macos:
  system_setup:
    - description: "verify-only setup control"
      tier: verify
EOF
)"
rendered_verify_only_setup="$sandbox/rendered-verify-only-setup"
render_template "$TIER2_TEMPLATE" "$verify_only_setup_src" "$rendered_verify_only_setup" "$render_error" ||
  fail "a verify-only system_setup file must render cleanly (stderr: $(cat "$render_error"))"
refute_file_matches "$rendered_verify_only_setup" '^sudo' \
  'a verify-only system_setup file must render no sudo invocation'
refute_file_contains "$rendered_verify_only_setup" 'sudo -v' \
  'a verify-only system_setup file must render no sudo prelude'

# ---- 4: a verify or manual record carrying a mutating payload aborts ----------

assert_tier1_rejects 'manual defaults record carrying value' \
  'carries value' 'com.example.manualvalue' <<'EOF'
macos:
  defaults:
    - domain: com.example.manualvalue
      key: ManualValueKey
      value: true
      tier: manual
      runbook: Some section
  killall: []
EOF

assert_tier1_rejects 'manual defaults record carrying scope' \
  'carries scope' 'com.example.manualscope' <<'EOF'
macos:
  defaults:
    - domain: com.example.manualscope
      key: ManualScopeKey
      tier: manual
      runbook: Some section
      scope: system
  killall: []
EOF

# The forbidden-field list on a manual defaults record is pinned by
# COMPLETENESS: one fixture per member (type, value, host, scope, plist_path,
# each carried alone so the refusal names exactly that field), so removing ANY
# single member from the template's list fails the member's own case here.
assert_tier1_rejects 'manual defaults record carrying type' \
  'carries type' 'com.example.manualtype' <<'EOF'
macos:
  defaults:
    - domain: com.example.manualtype
      key: ManualTypeKey
      type: bool
      tier: manual
      runbook: Some section
  killall: []
EOF

assert_tier1_rejects 'manual defaults record carrying host' \
  'carries host' 'com.example.manualhost' <<'EOF'
macos:
  defaults:
    - domain: com.example.manualhost
      key: ManualHostKey
      tier: manual
      runbook: Some section
      host: current
  killall: []
EOF

assert_tier1_rejects 'manual defaults record carrying plist_path' \
  'carries plist_path' 'com.example.manualplist' <<'EOF'
macos:
  defaults:
    - domain: com.example.manualplist
      key: ManualPlistKey
      tier: manual
      runbook: Some section
      plist_path: /Library/Preferences/com.example.manualplist.plist
  killall: []
EOF

verify_payload_src="$(
  make_setup_source <<'EOF'
macos:
  system_setup:
    - description: "verify record smuggling a command"
      command: 'printf smuggled-write-ran'
      tier: verify
EOF
)"
verify_payload_status=0
render_template "$TIER2_TEMPLATE" "$verify_payload_src" "$sandbox/rendered-smuggled" "$render_error" ||
  verify_payload_status=$?
[[ $verify_payload_status -ne 0 ]] ||
  fail 'a verify system_setup record carrying a command must fail the render'
assert_file_contains "$render_error" 'carries command' \
  "the refusal must name the smuggled command field (stderr: $(cat "$render_error"))"
# Deliberately NOT asserted: that the rejected render's output file lacks the
# smuggled command. The required failure above already dies if the render
# succeeds, and a failed render's stdout is empty by chezmoi's buffering (see
# the note on the reject helpers), so that refute could never fail.

assert_tier2_rejects 'manual system_setup record carrying command' \
  'carries command' 'manual record smuggling a command' <<'EOF'
macos:
  system_setup:
    - description: "manual record smuggling a command"
      command: 'printf manual-smuggled-ran'
      tier: manual
      runbook: Some section
EOF

assert_tier2_rejects 'verify system_setup record carrying sudo' \
  'carries sudo' 'verify record smuggling sudo' <<'EOF'
macos:
  system_setup:
    - description: "verify record smuggling sudo"
      sudo: true
      tier: verify
EOF

# The inverse lie: an enforce record IS a command, so a record that declares
# the tier without one aborts. Absent, blank (nil), and empty-string are three
# ways of declaring no command; each aborts on its own fixture, so the branch
# cannot rot down to a sampled subset.
assert_tier2_rejects 'enforce system_setup record with no command' \
  'has no command' 'enforce record without a command' <<'EOF'
macos:
  system_setup:
    - description: "enforce record without a command"
      tier: enforce
EOF

assert_tier2_rejects 'enforce system_setup record with a blank command' \
  'has no command' 'enforce record with a blank command' <<'EOF'
macos:
  system_setup:
    - description: "enforce record with a blank command"
      command:
      tier: enforce
EOF

assert_tier2_rejects 'enforce system_setup record with an empty command' \
  'has no command' 'enforce record with an empty command' <<'EOF'
macos:
  system_setup:
    - description: "enforce record with an empty command"
      command: ""
      tier: enforce
EOF

# ---- 5: a manual record without a runbook aborts -------------------------------

assert_tier1_rejects 'manual defaults record with no runbook' \
  'has no runbook' 'com.example.norunbook' <<'EOF'
macos:
  defaults:
    - domain: com.example.norunbook
      key: NoRunbookKey
      tier: manual
  killall: []
EOF

blank_runbook_src="$(
  make_defaults_source <<'EOF'
macos:
  defaults:
    - domain: com.example.blankrunbook
      key: BlankRunbookKey
      tier: manual
      runbook:
  killall: []
EOF
)"
blank_runbook_status=0
render_template "$TIER1_TEMPLATE" "$blank_runbook_src" "$sandbox/rendered-blank-runbook" "$render_error" ||
  blank_runbook_status=$?
[[ $blank_runbook_status -ne 0 ]] || fail 'a blank runbook must fail the render'
assert_file_contains "$render_error" 'has a blank runbook' \
  "a blank runbook must be named as blank (stderr: $(cat "$render_error"))"
refute_file_contains "$render_error" '<nil>' \
  'a blank runbook must never surface as the stringified literal <nil>'

assert_tier1_rejects 'manual defaults record with an empty runbook' \
  'has an empty runbook' 'com.example.emptyrunbook' <<'EOF'
macos:
  defaults:
    - domain: com.example.emptyrunbook
      key: EmptyRunbookKey
      tier: manual
      runbook: ""
  killall: []
EOF

assert_tier2_rejects 'manual system_setup record with no runbook' \
  'has no runbook' 'manual record without a runbook' <<'EOF'
macos:
  system_setup:
    - description: "manual record without a runbook"
      tier: manual
EOF

# Pinned against Tier 2 directly, not assumed covered by the Tier 1 case
# above: the empty-runbook branch is a separate copy in each template.
assert_tier2_rejects 'manual system_setup record with an empty runbook' \
  'has an empty runbook' 'manual record with an empty runbook' <<'EOF'
macos:
  system_setup:
    - description: "manual record with an empty runbook"
      tier: manual
      runbook: ""
EOF

# An enforce record has no consumer for a runbook, and silently ignored data
# is how a mislabeled tier hides; carrying one aborts.
assert_tier1_rejects 'enforce defaults record carrying runbook' \
  'carries runbook' 'com.example.enforcebook' <<'EOF'
macos:
  defaults:
    - domain: com.example.enforcebook
      key: EnforceBookKey
      type: bool
      value: true
      tier: enforce
      runbook: Some section
  killall: []
EOF

assert_tier2_rejects 'enforce system_setup record carrying runbook' \
  'carries runbook' 'enforce record with a runbook' <<'EOF'
macos:
  system_setup:
    - description: "enforce record with a runbook"
      command: 'printf enforce-book-ran'
      tier: enforce
      runbook: Some section
EOF

# ---- 6: a manual record renders a runbook pointer and no command ---------------

manual_render_src="$(
  make_defaults_source <<'EOF'
macos:
  defaults:
    - domain: com.example.enforced
      key: EnforcedKey
      type: bool
      value: true
      tier: enforce
    - domain: com.example.byhand
      key: ByHandKey
      tier: manual
      runbook: Firewall logging
  killall: []
EOF
)"
rendered_manual="$sandbox/rendered-manual"
render_template "$TIER1_TEMPLATE" "$manual_render_src" "$rendered_manual" "$render_error" ||
  fail "a manual defaults record must render cleanly (stderr: $(cat "$render_error"))"
grep -qxF "echo 'manual control com.example.byhand ByHandKey: not settable here; see the runbook section Firewall logging'" "$rendered_manual" ||
  fail "a manual record must render its exact runbook pointer (render: $(cat "$rendered_manual"))"
refute_file_contains "$rendered_manual" "defaults write 'com.example.byhand'" \
  'a manual record must render no defaults write'
grep -qxF "defaults write 'com.example.enforced' 'EnforcedKey' -bool 'true'" "$rendered_manual" ||
  fail 'the enforce record beside a manual one must keep its write'

# The pointer is a NEW data-to-shell emission path, so it gets the same
# injection proof as the write path: hostile runbook text must arrive literal
# and execute nothing, apostrophe included.
manual_injection_src="$(
  make_defaults_source <<'EOF'
macos:
  defaults:
    - domain: com.example.hostilebook
      key: HostileBookKey
      tier: manual
      runbook: "it's $(touch runbook-marker)`touch runbook-tick`$INJECTED_VARIABLE"
  killall: []
EOF
)"
rendered_hostile_manual="$sandbox/rendered-hostile-manual"
render_template "$TIER1_TEMPLATE" "$manual_injection_src" "$rendered_hostile_manual" "$render_error" ||
  fail "a hostile runbook is a legal, if nasty, value and must render (stderr: $(cat "$render_error"))"
stub_bin="$sandbox/bin"
mkdir -p "$stub_bin"
printf '#!/bin/bash\nexit 0\n' >"$stub_bin/osascript"
printf '#!/bin/bash\nexit 0\n' >"$stub_bin/killall"
chmod +x "$stub_bin/osascript" "$stub_bin/killall"
marker_dir="$sandbox/markers"
mkdir -p "$marker_dir"
manual_pointer_output="$sandbox/manual-pointer.out"
(
  cd "$marker_dir" || exit 1
  PATH="$stub_bin:$PATH" INJECTED_VARIABLE=EXPANDED_VARIABLE_VALUE \
    bash "$rendered_hostile_manual"
) >"$manual_pointer_output" 2>&1 ||
  fail "the rendered manual pointer must run cleanly (output: $(cat "$manual_pointer_output"))"
created_markers="$(find "$marker_dir" -mindepth 1 -print)"
[[ -z $created_markers ]] ||
  fail "no runbook payload may execute; the rendered script created: $created_markers"
assert_file_contains "$manual_pointer_output" 'it'\''s $(touch runbook-marker)`touch runbook-tick`$INJECTED_VARIABLE' \
  "the hostile runbook must arrive as literal text (output: $(cat "$manual_pointer_output"))"
refute_file_contains "$manual_pointer_output" 'EXPANDED_VARIABLE_VALUE' \
  'a $VAR in a runbook must never be expanded against the runner environment'

# system_setup: a manual-only file renders the pointer, no sudo of any kind,
# and survives the same hostile text through the same single-quoting helper.
manual_setup_src="$(
  make_setup_source <<'EOF'
macos:
  system_setup:
    - description: "Turn on firewall logging"
      tier: manual
      runbook: Firewall logging
    - description: "hostile pointer it's $(touch setup-marker)`touch setup-tick`$INJECTED_VARIABLE"
      tier: manual
      runbook: "Section $(touch setup-runbook-marker)"
EOF
)"
rendered_manual_setup="$sandbox/rendered-manual-setup"
render_template "$TIER2_TEMPLATE" "$manual_setup_src" "$rendered_manual_setup" "$render_error" ||
  fail "a manual system_setup record must render cleanly (stderr: $(cat "$render_error"))"
grep -qxF "echo '→ MANUAL Turn on firewall logging: see the runbook section Firewall logging'" "$rendered_manual_setup" ||
  fail "a manual system_setup record must render its exact runbook pointer (render: $(cat "$rendered_manual_setup"))"
refute_file_matches "$rendered_manual_setup" '^sudo' \
  'a manual-only system_setup file must render no sudo invocation'
refute_file_contains "$rendered_manual_setup" 'sudo -v' \
  'a manual-only system_setup file must render no sudo prelude'
manual_setup_output="$sandbox/manual-setup.out"
(
  cd "$marker_dir" || exit 1
  INJECTED_VARIABLE=EXPANDED_VARIABLE_VALUE bash "$rendered_manual_setup"
) >"$manual_setup_output" 2>&1 ||
  fail "the rendered manual-only system_setup runner must run cleanly (output: $(cat "$manual_setup_output"))"
created_markers="$(find "$marker_dir" -mindepth 1 -print)"
[[ -z $created_markers ]] ||
  fail "no system_setup pointer payload may execute; created: $created_markers"
assert_file_contains "$manual_setup_output" 'Section $(touch setup-runbook-marker)' \
  "the hostile system_setup runbook must arrive as literal text (output: $(cat "$manual_setup_output"))"
refute_file_contains "$manual_setup_output" 'EXPANDED_VARIABLE_VALUE' \
  'a $VAR in a system_setup pointer must never be expanded against the runner environment'

# ---- source form: the tier is read through index, never the dotted form --------

for guarded_template in "$TIER1_TEMPLATE" "$TIER2_TEMPLATE"; do
  assert_file_contains "$guarded_template" 'index . "tier"' \
    "$(basename "$guarded_template") must read the tier through index"
  refute_file_matches "$guarded_template" '\.tier' \
    "$(basename "$guarded_template") must not use the bare dotted tier field form anywhere"
  refute_file_matches "$guarded_template" '\.runbook' \
    "$(basename "$guarded_template") must not use the bare dotted runbook field form anywhere"
  # The render loop's fail-closed else arm is unreachable while the validation
  # pass holds, so no fixture can reach it; its distinctly-marked fail is
  # pinned here in source form. Together with the validation-site refutes in
  # section 2, each of the two copies is individually removable only by
  # turning a test red.
  assert_file_contains "$guarded_template" 'reached the render loop' \
    "$(basename "$guarded_template") must keep the render loop's distinctly-marked fail-closed tier refusal"
done

# ---- 7: the tools inherit the refusal through the shared record stream ---------

# The stream refuses a file with an untiered record, names it, and emits
# NOTHING a caller could act on.
untiered_stream_src="$(
  make_defaults_source <<'EOF'
macos:
  defaults:
    - domain: com.example.streamok
      key: StreamOkKey
      type: bool
      value: true
      tier: enforce
    - domain: com.example.streambad
      key: StreamBadKey
      type: bool
      value: true
  killall: []
EOF
)"
stream_status=0
stream_output="$(bash -c 'source "$1"; defaults_records_unit_separated "$2"' _ \
  "$LIB" "$untiered_stream_src/.chezmoidata/macos_defaults.yaml" 2>"$sandbox/stream.err")" || stream_status=$?
[[ $stream_status -eq 2 ]] ||
  fail "the record stream must refuse an untiered record with status 2 (got $stream_status)"
[[ -z $stream_output ]] ||
  fail "a refused stream must emit nothing, not a partial record list (got: $(printf '%s' "$stream_output" | cat -v))"
assert_file_contains "$sandbox/stream.err" 'com.example.streambad' \
  "the stream refusal must name the untiered record (stderr: $(cat "$sandbox/stream.err"))"
assert_file_contains "$sandbox/stream.err" 'tier' \
  "the stream refusal must say the tier is the problem (stderr: $(cat "$sandbox/stream.err"))"

unknown_tier_stream_src="$(
  make_defaults_source <<'EOF'
macos:
  defaults:
    - domain: com.example.streamodd
      key: StreamOddKey
      type: bool
      value: true
      tier: bogus
  killall: []
EOF
)"
stream_status=0
stream_output="$(bash -c 'source "$1"; defaults_records_unit_separated "$2"' _ \
  "$LIB" "$unknown_tier_stream_src/.chezmoidata/macos_defaults.yaml" 2>"$sandbox/stream.err")" || stream_status=$?
[[ $stream_status -eq 2 ]] ||
  fail "the record stream must refuse an unrecognized tier with status 2 (got $stream_status)"
[[ -z $stream_output ]] ||
  fail 'a stream refused for an unrecognized tier must emit nothing'
assert_file_contains "$sandbox/stream.err" 'bogus' \
  "the stream refusal must name the unrecognized tier value (stderr: $(cat "$sandbox/stream.err"))"

# A valid mixed file streams every record with the tier as field 8, so the
# tools decide by DECLARED tier, not by which payload fields happen to exist.
mixed_stream_src="$(
  make_defaults_source <<'EOF'
macos:
  defaults:
    - domain: com.example.applyenforce
      key: ApplyKey
      type: bool
      value: true
      tier: enforce
    - domain: com.example.applyverify
      key: VerifyKey
      type: bool
      value: true
      tier: verify
      scope: system
    - domain: com.example.applymanual
      key: ManualKey
      tier: manual
      runbook: Some section
  killall: []
EOF
)"
mixed_stream="$(bash -c 'source "$1"; defaults_records_unit_separated "$2"' _ \
  "$LIB" "$mixed_stream_src/.chezmoidata/macos_defaults.yaml")" ||
  fail 'the stream must accept a valid mixed-tier file'
IFS=$'\x1f' read -r got_domain got_key got_type got_value got_host got_scope got_plist_path got_tier \
  <<<"$(printf '%s\n' "$mixed_stream" | sed -n '1p')"
[[ $got_domain == com.example.applyenforce ]] || fail "stream field 1 must be the domain (got '$got_domain')"
[[ $got_key == ApplyKey ]] || fail "stream field 2 must be the key (got '$got_key')"
[[ $got_type == bool ]] || fail "stream field 3 must be the type (got '$got_type')"
[[ $got_value == true ]] || fail "stream field 4 must be the value (got '$got_value')"
[[ -z $got_host ]] || fail "stream field 5 must be the empty absent host (got '$got_host')"
[[ $got_scope == user ]] || fail "stream field 6 must default to user scope (got '$got_scope')"
[[ -z $got_plist_path ]] || fail "stream field 7 must be the empty absent plist path (got '$got_plist_path')"
[[ $got_tier == enforce ]] || fail "stream field 8 must be the tier (got '$got_tier')"
IFS=$'\x1f' read -r _ _ _ _ _ _ _ got_tier \
  <<<"$(printf '%s\n' "$mixed_stream" | sed -n '3p')"
[[ $got_tier == manual ]] ||
  fail "a manual record must stream with its tier intact (got '$got_tier')"

# ---- apply writes ONLY enforce records ------------------------------------------

defaults_log="$sandbox/defaults.log"
sudo_log="$sandbox/sudo.log"
cat >"$stub_bin/sudo" <<EOF
#!/bin/bash
printf '%s\n' "\$*" >>"$sudo_log"
exec "\$@"
EOF
cat >"$stub_bin/defaults" <<EOF
#!/bin/bash
printf '%s\n' "\$*" >>"$defaults_log"
if [[ \$1 == read-type ]]; then printf 'Type is boolean\n'; exit 0; fi
if [[ \$1 == read ]]; then
  case "\$2" in
    com.example.driftenforce | com.example.captured) printf '1\n' ;;
    *) printf 'off\n' ;;
  esac
  exit 0
fi
exit 0
EOF
chmod +x "$stub_bin/sudo" "$stub_bin/defaults"

run_tool() { # <source-dir> <script> [args...]
  local source_dir="$1"
  shift
  (
    cd "$sandbox" || exit 1
    MACOS_DEFAULTS_SOURCE_DIR="$source_dir" HOME="$sandbox/home" \
      PATH="$stub_bin:$PATH" bash "$@"
  )
}

: >"$defaults_log"
: >"$sudo_log"
run_tool "$mixed_stream_src" "$APPLY" ||
  fail 'apply must succeed on a valid mixed-tier file'
assert_file_contains "$defaults_log" 'write com.example.applyenforce ApplyKey -bool true' \
  "apply must write the enforce record (defaults log: $(cat "$defaults_log"))"
refute_file_contains "$defaults_log" 'com.example.applyverify' \
  'apply must never touch a verify record; it is not settable, that is the tier'
refute_file_contains "$sudo_log" 'com.example.applyverify' \
  'apply must never sudo-write a verify record, system scope or not'
refute_file_contains "$defaults_log" 'com.example.applymanual' \
  'apply must never touch a manual record'
refute_file_contains "$sudo_log" 'write' \
  'apply must perform no root write at all when no enforce record is system scope'

# apply refuses the whole file on an unknown tier, before any write.
: >"$defaults_log"
apply_unknown_status=0
run_tool "$unknown_tier_stream_src" "$APPLY" 2>"$sandbox/apply-unknown.err" || apply_unknown_status=$?
[[ $apply_unknown_status -ne 0 ]] ||
  fail 'apply must refuse a file carrying an unrecognized tier'
[[ ! -s $defaults_log ]] ||
  fail "apply must write nothing from a file it refused (defaults log: $(cat "$defaults_log"))"

# ---- drift checks enforce AND verify, skips manual -------------------------------

drift_src="$(
  make_defaults_source <<'EOF'
macos:
  defaults:
    - domain: com.example.driftenforce
      key: EnforceKey
      type: bool
      value: true
      tier: enforce
    - domain: com.example.driftverify
      key: VerifyKey
      type: bool
      value: true
      tier: verify
    - domain: com.example.driftmanual
      key: ManualKey
      tier: manual
      runbook: Some section
  killall: []
EOF
)"
drift_out="$sandbox/drift.out"
drift_status=0
run_tool "$drift_src" "$DRIFT" >"$drift_out" 2>"$sandbox/drift.err" || drift_status=$?
[[ $drift_status -eq 1 ]] ||
  fail "drift must exit 1 when a verify control has drifted (got $drift_status, stderr: $(cat "$sandbox/drift.err"))"
assert_file_contains "$drift_out" 'com.example.driftverify' \
  "a drifted verify control must be reported; detecting it is the tier's whole job (stdout: $(cat "$drift_out"))"
refute_file_contains "$drift_out" 'com.example.driftenforce' \
  'a matching enforce control must not be reported as drift'
refute_file_contains "$drift_out" 'com.example.driftmanual' \
  'a manual control has no check and must never appear in the drift report'

drift_unknown_status=0
run_tool "$unknown_tier_stream_src" "$DRIFT" >"$drift_out" 2>"$sandbox/drift.err" || drift_unknown_status=$?
[[ $drift_unknown_status -eq 2 ]] ||
  fail "drift must exit 2 on a file carrying an unrecognized tier (got $drift_unknown_status)"

# ---- the tools' OWN in-loop tier refusal, reached by bypassing the stream --------

# The refusals above pin the shared stream's gate: an unknown tier never
# reaches a tool's loop through the real library. Apply and drift each carry
# their own case arm behind that gate as defence in depth for the day the
# stream's rules drift, and an arm no test can reach reads as protection
# while providing none. So reach it: copy each tool beside a doctored lib
# whose stream emits the one record the real gate refuses, and require the
# tool's own arm to refuse it, fail-closed, before acting on the record.
bypass_dir="$sandbox/bypass-tools"
mkdir -p "$bypass_dir"
cp "$APPLY" "$bypass_dir/macos-defaults-apply.sh"
cp "$DRIFT" "$bypass_dir/macos-defaults-drift.sh"
cp "$LIB" "$bypass_dir/macos-defaults-lib.sh"
cat >>"$bypass_dir/macos-defaults-lib.sh" <<'EOF'

# TEST DOUBLE: emit one record with a tier the real stream's gate refuses,
# so the calling tool's own case arm is the only guard left standing.
defaults_records_unit_separated() {
  printf 'com.example.bypassgate\x1fBypassGateKey\x1fbool\x1ftrue\x1f\x1fuser\x1f\x1fmystery\n'
}
EOF

: >"$defaults_log"
: >"$sudo_log"
apply_bypass_status=0
run_tool "$mixed_stream_src" "$bypass_dir/macos-defaults-apply.sh" \
  2>"$sandbox/apply-bypass.err" || apply_bypass_status=$?
[[ $apply_bypass_status -eq 2 ]] ||
  fail "apply's own loop must refuse an unknown tier with status 2 when the stream gate is bypassed (got $apply_bypass_status)"
assert_file_contains "$sandbox/apply-bypass.err" 'unrecognized tier' \
  "apply's own refusal must name the tier as the problem (stderr: $(cat "$sandbox/apply-bypass.err"))"
assert_file_contains "$sandbox/apply-bypass.err" 'refusing to write' \
  "the refusal must be apply's own arm, not the stream's (stderr: $(cat "$sandbox/apply-bypass.err"))"
refute_file_contains "$defaults_log" 'com.example.bypassgate' \
  'apply must not fall through to the write path on a tier it cannot classify'

drift_bypass_status=0
run_tool "$mixed_stream_src" "$bypass_dir/macos-defaults-drift.sh" \
  >"$drift_out" 2>"$sandbox/drift-bypass.err" || drift_bypass_status=$?
[[ $drift_bypass_status -eq 2 ]] ||
  fail "drift's own loop must refuse an unknown tier with status 2 when the stream gate is bypassed (got $drift_bypass_status)"
assert_file_contains "$sandbox/drift-bypass.err" 'refusing to report on it' \
  "the refusal must be drift's own arm, not the stream's (stderr: $(cat "$sandbox/drift-bypass.err"))"
refute_file_contains "$drift_out" 'com.example.bypassgate' \
  'drift must not report a comparison for a tier it cannot classify'

# ---- capture appends tier: enforce, and the result round-trips -------------------

capture_src="$(
  make_defaults_source <<'EOF'
macos:
  defaults: []
  killall: []
EOF
)"
capture_data_file="$capture_src/.chezmoidata/macos_defaults.yaml"
run_tool "$capture_src" "$CAPTURE" com.example.captured CapturedKey ||
  fail 'capture must succeed against the stubbed defaults'
captured_tier="$(yq eval -r \
  '.macos.defaults[] | select(.domain == "com.example.captured") | .tier' \
  "$capture_data_file")"
[[ $captured_tier == enforce ]] ||
  fail "capture must declare tier: enforce on the record it appends (got '$captured_tier')"
bash -c 'source "$1"; defaults_records_unit_separated "$2"' _ \
  "$LIB" "$capture_data_file" >/dev/null ||
  fail 'a captured record must round-trip through the tier-validating stream'

printf 'macos-control-tier-refusal: OK (missing, blank, empty, and unrecognized tiers abort both renders naming the record; every forbidden manual field and a commandless enforce record abort; verify renders no write and no sudo; manual requires a runbook and renders a literal-quoted pointer only; apply writes only enforce, drift checks verify and skips manual, capture appends tier: enforce; the stream refuses whole files fail-closed and apply/drift refuse on their own arms behind it)\n'
