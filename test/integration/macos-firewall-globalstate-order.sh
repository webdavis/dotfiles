#!/usr/bin/env bash
# macos-firewall-globalstate-order.sh -- the firewall baseline declared in
# .chezmoidata/macos_system_setup.yaml renders through the Tier 2 runner as
# four sudo-prefixed, idempotent socketfilterfw commands with the global-state
# command STRICTLY first.
#
# Why the order is a correctness property, not cosmetics: partial-execution
# safety. The subordinate setters store their configuration independently and
# enabling the global state consumes it, so a run that completes lands
# identically in any order; a run that dies partway does not. Global-first
# leaves the machine with the firewall ON (at worst missing a subordinate
# policy), while any other order can stop with subordinate policies stored
# and the firewall still OFF.
#
# The properties pinned, one per acceptance criterion:
#   1. Global state renders strictly before stealth (and before both
#      signed-software commands, the same subordinate-policy class), compared
#      by LINE NUMBER in the render; a presence-only assertion would pass with
#      the order reversed. Each command is pinned sudo-prefixed as a whole
#      line.
#   2. BOTH signed-software commands render, each asserted individually:
#      --setallowsigned (built-in) and --setallowsignedapp (downloaded) are
#      independent policies, and a single "a signed-app command is present"
#      assertion passes with one of the two missing.
#   3. The render's socketfilterfw lines are EXACTLY the four declared
#      commands, byte for byte; a completeness diff, so an extra or altered
#      line is reported by name.
#   4. Every socketfilterfw command in the data is an idempotent set-to-state
#      form with an explicit trailing state, never a toggle: the runner
#      re-runs the whole list on any data change, so a toggle would flip
#      protection OFF on the second run. Asserted over EVERY declared firewall
#      command, not a count.
#   5. Every manual record's runbook field resolves to a real markdown
#      heading in the runbook, so no manual pointer can dangle. No FIREWALL
#      manual record may exist at all: the only one ever declared here
#      (firewall logging) described an action that does not exist on this
#      macOS version, and a manual record no reader can complete is worse
#      than no record. (Non-firewall manual records are legitimate; the
#      OverSight permission-grant record's own pins live in
#      oversight-posture.sh.)
#
# Real chezmoi against the REAL .chezmoidata; nothing is executed, only
# rendered.
set -euo pipefail

# Scrubbed at SCRIPT scope, before any chezmoi call. Git exports GIT_DIR to
# every hook it runs and this suite runs from the pre-push hook.
unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_41-macos-system-setup.sh.tmpl"
SETUP_YAML="$REPO_ROOT/.chezmoidata/macos_system_setup.yaml"
RUNBOOK="$REPO_ROOT/docs/runbooks/macos-fresh-machine-quickstart.md"

# The tool is not on PATH; every declared command carries the absolute path.
SOCKETFILTERFW="/usr/libexec/ApplicationFirewall/socketfilterfw"
GLOBAL_STATE_COMMAND="$SOCKETFILTERFW --setglobalstate on"
STEALTH_COMMAND="$SOCKETFILTERFW --setstealthmode on"
ALLOW_BUILTIN_SIGNED_COMMAND="$SOCKETFILTERFW --setallowsigned on"
ALLOW_DOWNLOADED_SIGNED_COMMAND="$SOCKETFILTERFW --setallowsignedapp on"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

for tool in chezmoi yq; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'SKIP: %s not on PATH; cannot exercise the firewall baseline\n' "$tool"
    exit 0
  }
done
for required_file in "$TEMPLATE" "$SETUP_YAML"; do
  [[ -f $required_file ]] || fail "missing file: $required_file"
done

work="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$work"' EXIT

# ---- data-level properties (criteria 4 and 5; OS-independent) ---------------

# Criteria 2 and 4, data half: the declared socketfilterfw commands are
# EXACTLY the four baseline commands. Set equality via a sorted byte compare
# asserts each command individually and reports a missing or an extra one by
# name, where a count or a single-presence grep would stay green.
declared_firewall_commands="$work/declared-firewall-commands"
yq eval -r \
  '[.macos.system_setup[] | select(has("command")) | .command | select(test("socketfilterfw"))] | .[]' \
  "$SETUP_YAML" | LC_ALL=C sort >"$declared_firewall_commands"

expected_firewall_commands="$work/expected-firewall-commands"
LC_ALL=C sort >"$expected_firewall_commands" <<EOF
$GLOBAL_STATE_COMMAND
$STEALTH_COMMAND
$ALLOW_BUILTIN_SIGNED_COMMAND
$ALLOW_DOWNLOADED_SIGNED_COMMAND
EOF

cmp -s "$expected_firewall_commands" "$declared_firewall_commands" ||
  fail "the declared socketfilterfw commands must be exactly the four baseline commands (diff: $(diff "$expected_firewall_commands" "$declared_firewall_commands" || true))"

# Criterion 4: EVERY declared firewall command is a set-to-state form with an
# explicit trailing state. A completeness loop, not a count: a fifth command
# sneaking in as a toggle must fail here by name. Against TODAY'S data this
# loop is shadowed: the exact-command set compare above fires first on any
# deviation from the four baseline commands. It is a forward-looking layer,
# not dead code: it fails when a future toggle command lands alongside a
# co-evolved whitelist, so keep it.
set_to_state_pattern='^/usr/libexec/ApplicationFirewall/socketfilterfw --set[a-z]+ (on|off)$'
while IFS= read -r firewall_command; do
  [[ $firewall_command =~ $set_to_state_pattern ]] ||
    fail "firewall command is not an idempotent set-to-state form: $firewall_command"
done <"$declared_firewall_commands"

# Criterion 5: every manual record's runbook field resolves to a real
# markdown heading in the runbook, so no manual pointer can dangle; and no
# FIREWALL manual record exists. The only firewall manual record ever
# declared (firewall logging) described an action that cannot be performed:
# socketfilterfw on 26.2 has no logging flag, and firewall activity flows to
# the unified log by default, so there is nothing to enable by hand.
# Legitimate non-firewall manual records (the OverSight permission grants)
# are validated by the resolution loop, not refused.
firewall_manual_descriptions="$(yq eval -r \
  '[.macos.system_setup[] | select(.tier == "manual" and (.description | test("(?i)firewall"))) | .description] | join(", ")' \
  "$SETUP_YAML")"
[[ -z $firewall_manual_descriptions ]] ||
  fail "no firewall manual record may exist (nothing firewall-manual is performable on this macOS version); found: $firewall_manual_descriptions"

manual_runbook_sections="$work/manual-runbook-sections"
yq eval -r \
  '.macos.system_setup[] | select(.tier == "manual") | .runbook // ""' \
  "$SETUP_YAML" >"$manual_runbook_sections" ||
  fail "could not enumerate the manual records' runbook fields"
while IFS= read -r manual_runbook_section; do
  [[ -n $manual_runbook_section ]] ||
    fail "a manual record has an empty runbook field (the render would refuse it; this names the gap first)"
  grep -qxF "### $manual_runbook_section" "$RUNBOOK" ||
    fail "manual runbook pointer dangles: no '### $manual_runbook_section' heading in $RUNBOOK"
done <"$manual_runbook_sections"

# ---- render properties (criteria 1, 2, 3; darwin render only) ---------------

render_home="$work/render-home"
mkdir -p "$render_home"
rendered="$work/rendered"
render_error="$work/render.err"
HOME="$render_home" chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$TEMPLATE" >"$rendered" 2>"$render_error" ||
  fail "the Tier 2 runner must render against the real data (stderr: $(cat "$render_error"))"
# The skip is gated on the ACTUAL host OS, never on the render coming out
# empty: emptiness conflates "non-darwin host" (skip, by design) with "the
# template's OS guard is broken on darwin" (a failure this test exists to
# catch). An empty render on darwin must fail loudly, not skip.
[[ "$(uname)" == Darwin ]] || {
  printf 'SKIP: non-darwin host; render assertions not exercised\n'
  exit 0
}
[[ -n "$(tr -d '[:space:]' <"$rendered")" ]] ||
  fail "the Tier 2 runner rendered EMPTY on a darwin host; its OS guard is broken (template: $TEMPLATE; stderr: $(cat "$render_error"))"

# Whole-line match (-x) pins the sudo prefix; uniqueness makes the returned
# line number meaningful for the order comparison.
line_number_of_unique_line() { # <exact-line> <label>
  local hits
  hits="$(grep -nxF -- "$1" "$rendered")" ||
    fail "$2 must render exactly once; not found: $1"
  [[ $(printf '%s\n' "$hits" | wc -l) -eq 1 ]] ||
    fail "$2 must render exactly once; found multiple: $hits"
  printf '%s' "${hits%%:*}"
}

# Criterion 1: global state strictly before stealth, by line number. The two
# signed-software policies are held behind the global-state line too: they are
# the same subordinate-policy class, with the same partial-run exposure.
global_state_line="$(line_number_of_unique_line "sudo $GLOBAL_STATE_COMMAND" "the sudo-prefixed global-state command")"
stealth_line="$(line_number_of_unique_line "sudo $STEALTH_COMMAND" "the sudo-prefixed stealth command")"
((global_state_line < stealth_line)) ||
  fail "global state (line $global_state_line) must render STRICTLY BEFORE stealth (line $stealth_line); a partial run must not stop with stealth stored and the firewall still off"

# Criterion 2: each signed-software command individually, sudo-prefixed.
allow_builtin_signed_line="$(line_number_of_unique_line "sudo $ALLOW_BUILTIN_SIGNED_COMMAND" "the sudo-prefixed built-in signed-software command")"
allow_downloaded_signed_line="$(line_number_of_unique_line "sudo $ALLOW_DOWNLOADED_SIGNED_COMMAND" "the sudo-prefixed downloaded signed-software command")"
((global_state_line < allow_builtin_signed_line)) ||
  fail "global state (line $global_state_line) must render before the built-in signed-software policy (line $allow_builtin_signed_line)"
((global_state_line < allow_downloaded_signed_line)) ||
  fail "global state (line $global_state_line) must render before the downloaded signed-software policy (line $allow_downloaded_signed_line)"

# Criterion 3: every socketfilterfw line in the render is one of the four
# declared sudo-prefixed commands, byte for byte. A completeness diff, not a
# count, so an extra or altered line is reported by name.
rendered_firewall_lines="$work/rendered-firewall-lines"
{ grep -F 'socketfilterfw' "$rendered" || true; } | LC_ALL=C sort >"$rendered_firewall_lines"
expected_rendered_firewall_lines="$work/expected-rendered-firewall-lines"
LC_ALL=C sort >"$expected_rendered_firewall_lines" <<EOF
sudo $GLOBAL_STATE_COMMAND
sudo $STEALTH_COMMAND
sudo $ALLOW_BUILTIN_SIGNED_COMMAND
sudo $ALLOW_DOWNLOADED_SIGNED_COMMAND
EOF
cmp -s "$expected_rendered_firewall_lines" "$rendered_firewall_lines" ||
  fail "the render must contain exactly the four declared socketfilterfw command lines (diff: $(diff "$expected_rendered_firewall_lines" "$rendered_firewall_lines" || true))"

printf 'macos-firewall-globalstate-order: OK (global state renders first; both signed-software policies render sudo-prefixed; the render carries exactly the four declared commands; every declared firewall command is set-to-state; no firewall manual record exists and every manual runbook pointer resolves)\n'
