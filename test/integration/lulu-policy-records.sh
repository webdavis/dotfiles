#!/usr/bin/env bash
# lulu-policy-records.sh -- slice 10: LuLu policy and posture.
#
# LuLu is the outbound firewall on this machine. Its policy surface
# (/Library/Objective-See/LuLu/preferences.plist, plain XML at an absolute
# path) is declaratively manageable; its rules surface (rules.plist, an
# NSKeyedArchiver archive of LuLu's private Rule class) is interactive-only.
# The slice therefore declares:
#
#   - the six policy preferences as enforce records in macos_defaults.yaml,
#     scope system with an explicit plist_path, each carrying the value the
#     machine holds today (the slice codifies posture, it never changes it);
#   - the system-extension approval and rule creation as manual records in
#     macos_system_setup.yaml with runbook sections that exist;
#   - the extension running and the required allow rules existing as verify
#     records in macos_posture_controls.yaml, read by the security-posture
#     poller.
#
# The behaviours, each against the REAL repo data (so a fixture cannot drift
# from what ships), with the poller driven against STUBS (a stubbed rule-file
# reader, a stubbed extension probe), so the suite passes on any machine
# regardless of LuLu's state:
#
#   A  the six policy records: enforce, scope system, the LuLu plist_path,
#      todays live values, and the allowLocalHost record carries the
#      alerting-path reason inline so it cannot be quietly flipped or dropped.
#      The rendered Tier 1 runner writes each record to the absolute plist
#      path under sudo.
#   B  containment: a system record naming a plist_path outside the permitted
#      plist directories (LuLu's install directory) aborts the render.
#   C  manual records: the system-extension approval and rule creation are
#      declared manual with no mutating payload, their runbook sections
#      exist and are non-empty, and the rendered Tier 2 runner emits a
#      pointer and no command for either.
#   D  the talker table: every enumerated outbound talker carries exactly one
#      of a declared verify control or a written no-rule reason, and the set
#      of named controls equals the set of declared lulu_rule records; the
#      table and the control set are diffed, so adding a talker without a
#      control fails here.
#   E  extension lifecycle: the poller probes the extension with one exact
#      user-scoped pgrep, pages exactly one CRIT when it stops (naming the
#      control and its remedy), stays quiet while stopped, re-arms silently
#      on restore, and pages again on a later stop.
#   F  missing rule: with the stubbed rule reader reporting a required rule
#      missing, the poller pages once, names the missing rule, and stays
#      quiet while it remains missing; a restored rule re-arms silently.
#   G  a changed declared target re-arms the control: the baseline records
#      the target each value was observed under, so repointing a rule
#      control at a new binary pages a fresh first observation instead of
#      riding the old baseline.
#   H  honesty: the rule-existence records and the runbook state the
#      existence-only limitation (the archive proves a rule mentioning the
#      binary exists; the check does not prove the rule allows).
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite runs from the pre-push hook.
unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEFAULTS_YAML="$REPO_ROOT/.chezmoidata/macos_defaults.yaml"
SETUP_YAML="$REPO_ROOT/.chezmoidata/macos_system_setup.yaml"
CONTROLS_YAML="$REPO_ROOT/.chezmoidata/macos_posture_controls.yaml"
TIER1_TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl"
TIER2_TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_41-macos-system-setup.sh.tmpl"
POSTURE_TEMPLATE="$REPO_ROOT/dot_local/libexec/osquery/posture-controls.json.tmpl"
RUNBOOK="$REPO_ROOT/docs/runbooks/macos-fresh-machine-quickstart.md"
LULU_PLIST_PATH="/Library/Objective-See/LuLu/preferences.plist"

# shellcheck source=../fixtures/osquery-poller-lib.bash
source "$REPO_ROOT/test/fixtures/osquery-poller-lib.bash"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# refute_file_contains <file> <fixed-string> <message> -- an explicit refute
# helper (never a bare negated grep in a test body).
refute_file_contains() {
  if grep -qF -- "$2" "$1"; then
    fail "$3"
  fi
}

for tool in yq jq chezmoi; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'SKIP: %s not on PATH; cannot exercise the LuLu policy records\n' "$tool"
    exit 0
  }
done
for required_file in "$DEFAULTS_YAML" "$SETUP_YAML" "$CONTROLS_YAML" "$TIER1_TEMPLATE" \
  "$TIER2_TEMPLATE" "$POSTURE_TEMPLATE" "$RUNBOOK"; do
  [[ -f $required_file ]] || fail "missing file: $required_file"
done

sandbox="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$sandbox"' EXIT
render_home="$sandbox/render-home"
mkdir -p "$render_home"
render_error="$sandbox/render.err"

# ---- A: the six policy records, todays values, the inline reason -------------

# The declared values ARE the values the live machine carries today (measured
# 2026-07-27, plutil -p on the live plist). A record whose declared value
# silently differed from the live one would turn a codification into a change.
declare -a policy_expectations=(
  "allowLocalHost|true"
  "allowApple|true"
  "allowDNS|true"
  "allowInstalled|true"
  "blockMode|false"
  "passiveMode|false"
)

lulu_records="$(yq -o=json '[.macos.defaults[] | select(.domain == "com.objective-see.lulu")]' "$DEFAULTS_YAML")" ||
  fail "A: could not read the LuLu records from $DEFAULTS_YAML"
jq -e 'length == 6' <<<"$lulu_records" >/dev/null ||
  fail "A: exactly six LuLu policy records must be declared; got $(jq 'length' <<<"$lulu_records")"

for expectation in "${policy_expectations[@]}"; do
  policy_key="${expectation%%|*}"
  policy_value="${expectation##*|}"
  record="$(jq --arg key "$policy_key" '[.[] | select(.key == $key)]' <<<"$lulu_records")"
  jq -e 'length == 1' <<<"$record" >/dev/null ||
    fail "A: exactly one record for $policy_key must be declared"
  jq -e '.[0].tier == "enforce"' <<<"$record" >/dev/null ||
    fail "A: the $policy_key record must be tier: enforce (got $(jq -r '.[0].tier' <<<"$record"))"
  jq -e '.[0].scope == "system"' <<<"$record" >/dev/null ||
    fail "A: the $policy_key record must be scope: system"
  jq -e --arg path "$LULU_PLIST_PATH" '.[0].plist_path == $path' <<<"$record" >/dev/null ||
    fail "A: the $policy_key record must declare plist_path: $LULU_PLIST_PATH"
  jq -e '.[0].type == "bool"' <<<"$record" >/dev/null ||
    fail "A: the $policy_key record must be type: bool"
  jq -e --arg value "$policy_value" '(.[0].value | tostring) == $value' <<<"$record" >/dev/null ||
    fail "A: the $policy_key record must declare todays live value $policy_value (got $(jq -r '.[0].value' <<<"$record"))"
done

# The allowLocalHost reason, inline where anyone tempted to tidy it will read
# it: the alerting path itself rides loopback, so this preference is the most
# safety-critical one on the machine. Both fragments must survive.
grep -qF 'alert-dispatch.sh POSTs every page to a Hermes gateway on 127.0.0.1:8644' "$DEFAULTS_YAML" ||
  fail "A: the allowLocalHost record must carry the alerting-path reason (the loopback POST target)"
grep -qF 'Flipping it to false would put LuLu in front of the alerting path itself' "$DEFAULTS_YAML" ||
  fail "A: the allowLocalHost record must carry the alerting-path consequence inline"

# The rendered Tier 1 runner writes each record to the absolute plist path,
# under sudo (the slice 2 system-scope form, confirmed for this exact path).
if [[ "$(uname)" == Darwin ]]; then
  tier1_rendered="$sandbox/tier1.rendered"
  HOME="$render_home" chezmoi --source "$REPO_ROOT" execute-template --no-tty \
    <"$TIER1_TEMPLATE" >"$tier1_rendered" 2>"$render_error" ||
    fail "A: the Tier 1 runner must render against the real data (stderr: $(cat "$render_error"))"
  for expectation in "${policy_expectations[@]}"; do
    policy_key="${expectation%%|*}"
    policy_value="${expectation##*|}"
    write_line="sudo defaults write '$LULU_PLIST_PATH' '$policy_key' -bool '$policy_value'"
    grep -qxF -- "$write_line" "$tier1_rendered" ||
      fail "A: the rendered runner must write $policy_key to the absolute plist path under sudo: [$write_line]"
  done
fi

# ---- B: a plist_path outside the permitted directories aborts the render ----

make_defaults_source() { # fixture YAML on stdin; prints the source dir
  local source_dir
  source_dir="$(mktemp -d "$sandbox/defaults-src.XXXXXX")"
  mkdir -p "$source_dir/.chezmoidata"
  cat >"$source_dir/.chezmoidata/macos_defaults.yaml"
  printf '%s\n' "$source_dir"
}

escape_source="$(
  make_defaults_source <<'EOF'
macos:
  defaults:
    - domain: com.evil.example
      key: EvilKey
      type: bool
      value: true
      tier: enforce
      scope: system
      plist_path: /etc/evil.plist
  killall: []
EOF
)"
escape_status=0
HOME="$render_home" chezmoi --source "$escape_source" execute-template --no-tty \
  <"$TIER1_TEMPLATE" >"$sandbox/escape.rendered" 2>"$render_error" || escape_status=$?
[[ $escape_status -ne 0 ]] ||
  fail "B: a plist_path outside the permitted plist directories must abort the render (rendered: $(grep -F EvilKey "$sandbox/escape.rendered" || true))"
grep -qF '/etc/evil.plist' "$render_error" ||
  fail "B: the refusal must name the offending path (stderr: $(cat "$render_error"))"
grep -qF 'permitted plist director' "$render_error" ||
  fail "B: the refusal must name the containment rule (stderr: $(cat "$render_error"))"

# Positive control: the same fixture shape with the LuLu path renders.
inside_source="$(
  make_defaults_source <<EOF
macos:
  defaults:
    - domain: com.objective-see.lulu
      key: allowApple
      type: bool
      value: true
      tier: enforce
      scope: system
      plist_path: $LULU_PLIST_PATH
  killall: []
EOF
)"
HOME="$render_home" chezmoi --source "$inside_source" execute-template --no-tty \
  <"$TIER1_TEMPLATE" >"$sandbox/inside.rendered" 2>"$render_error" ||
  fail "B: the LuLu plist_path must render cleanly (stderr: $(cat "$render_error"))"

# ---- B2: the target pairing is refused in both directions, at both layers ----

make_posture_source() { # fixture YAML on stdin; prints the source dir
  local source_dir
  source_dir="$(mktemp -d "$sandbox/posture-src.XXXXXX")"
  mkdir -p "$source_dir/.chezmoidata"
  cat >"$source_dir/.chezmoidata/macos_posture_controls.yaml"
  printf '%s\n' "$source_dir"
}

assert_posture_render_rejects() { # <label> <stderr-fragment>...
  local label="$1" source_dir status=0 fragment
  shift
  source_dir="$(make_posture_source)"
  HOME="$render_home" chezmoi --source "$source_dir" execute-template --no-tty \
    <"$POSTURE_TEMPLATE" >"$sandbox/posture.rendered" 2>"$render_error" || status=$?
  [[ $status -ne 0 ]] ||
    fail "B2 $label: the posture render must refuse (rendered: $(cat "$sandbox/posture.rendered"))"
  for fragment in "$@"; do
    grep -qF -- "$fragment" "$render_error" ||
      fail "B2 $label: the refusal must name '$fragment' (stderr: $(cat "$render_error"))"
  done
}

assert_posture_render_rejects 'rule reader without a target' 'requires a target' <<'EOF'
macos:
  posture_controls:
    - id: demo_rule
      description: "A rule control with no target"
      tier: verify
      reader: lulu_rule_present
      expect: "present"
EOF

assert_posture_render_rejects 'target on a targetless reader' 'does not consume' <<'EOF'
macos:
  posture_controls:
    - id: demo_guest
      description: "A guest control carrying a target"
      tier: verify
      reader: sysadminctl_guest
      expect: "disabled"
      target: /usr/local/bin/tailscaled
EOF

assert_posture_render_rejects 'relative target' 'must be an absolute path' <<'EOF'
macos:
  posture_controls:
    - id: demo_rule
      description: "A rule control with a relative target"
      tier: verify
      reader: lulu_rule_present
      expect: "present"
      target: usr/local/bin/tailscaled
EOF

# The poller re-validates the deployed file itself: a target its reader does
# not consume is refused BEFORE any probe runs, as a monitoring gap.
setup_poller_harness
trap 'teardown_poller_harness' EXIT
set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
set_posture_controls '[{"id":"demo_guest","description":"A guest control carrying a target","tier":"verify","reader":"sysadminctl_guest","expect":"disabled","target":"/usr/local/bin/tailscaled"}]'
seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
run_poller >/dev/null 2>&1 || fail "B2 poller: expected exit 0 after paging the refused file"
assert_page_count 1 || fail "B2 poller: a mis-paired deployed file must page a monitoring gap"
assert_page_body_has 'demo_guest' || fail "B2 poller: the refusal page must name the record"
assert_no_probe_calls || fail "B2 poller: a refused file must be rejected BEFORE any probe runs"
teardown_poller_harness
trap - EXIT

# ---- C: the manual records and their runbook sections ------------------------

assert_manual_record() { # <description-fragment> <expected-runbook-section>
  local fragment="$1" expected_runbook="$2" records
  records="$(yq -o=json "[.macos.system_setup[] | select(.tier == \"manual\" and (.description | contains(\"$fragment\")))]" "$SETUP_YAML")" ||
    fail "C: could not read $SETUP_YAML"
  jq -e 'length == 1' <<<"$records" >/dev/null ||
    fail "C: exactly one manual record matching '$fragment' must be declared; got $(jq 'length' <<<"$records")"
  jq -e '.[0] | (has("command") or has("sudo")) | not' <<<"$records" >/dev/null ||
    fail "C: the '$fragment' manual record must carry no mutating payload"
  [[ "$(jq -r '.[0].runbook' <<<"$records")" == "$expected_runbook" ]] ||
    fail "C: the '$fragment' manual record must name the runbook section '$expected_runbook'"
  grep -qxF "### $expected_runbook" "$RUNBOOK" ||
    fail "C: the runbook section '### $expected_runbook' must exist in $RUNBOOK"
  local section_body
  section_body="$(awk -v heading="### $expected_runbook" '
    $0 == heading { in_section = 1; next }
    in_section && /^### / { exit }
    in_section { print }
  ' "$RUNBOOK")"
  [[ -n ${section_body//[[:space:]]/} ]] ||
    fail "C: the runbook section '### $expected_runbook' has an empty body"
  printf '%s\n' "$section_body"
}

approval_body="$(assert_manual_record 'approve its system extension' 'LuLu system extension approval')"
grep -qF 'Login Items & Extensions' <<<"$approval_body" ||
  fail "C: the approval section must name the System Settings pane the operator uses"

rules_body="$(assert_manual_record 'outbound allow rules' 'LuLu rule creation')"
grep -qF 'Do NOT create blanket allow rules for shared interpreters' <<<"$rules_body" ||
  fail "C: the rule-creation section must carry the lean-narrow shared-client warning"
grep -qiF 'rules cannot be pre-seeded' <<<"$rules_body" ||
  fail "C: the rule-creation section must state why creation is interactive-only"

# The rendered Tier 2 runner emits a pointer and no command for either record.
if [[ "$(uname)" == Darwin ]]; then
  tier2_rendered="$sandbox/tier2.rendered"
  HOME="$render_home" chezmoi --source "$REPO_ROOT" execute-template --no-tty \
    <"$TIER2_TEMPLATE" >"$tier2_rendered" 2>"$render_error" ||
    fail "C: the Tier 2 runner must render against the real data (stderr: $(cat "$render_error"))"
  grep -qF 'see the runbook section LuLu system extension approval' "$tier2_rendered" ||
    fail "C: the rendered runner must point at the approval runbook section"
  grep -qF 'see the runbook section LuLu rule creation' "$tier2_rendered" ||
    fail "C: the rendered runner must point at the rule-creation runbook section"
  refute_file_contains "$tier2_rendered" 'systemextensionsctl' \
    "C: the runner must emit no command for the manual extension approval"
fi

# ---- D: the talker table and the declared control set are diffed -------------

talkers_json="$(yq -o=json '.macos.lulu_talkers' "$CONTROLS_YAML")" ||
  fail "D: could not read the talker table from $CONTROLS_YAML"
jq -e 'type == "array" and length > 0' <<<"$talkers_json" >/dev/null ||
  fail "D: macos.lulu_talkers must enumerate the outbound talker set"

# The enumerated set from the spec, by name: each must appear exactly once.
for talker_name in "Hermes gateway" "the alerter curl" "tailscaled" "Homebrew" "npm" "nix" "gh"; do
  jq -e --arg name "$talker_name" '[.[] | select(.talker == $name)] | length == 1' <<<"$talkers_json" >/dev/null ||
    fail "D: the talker table must enumerate '$talker_name' exactly once"
done

# Exactly one of control / no_rule_reason per talker, never both, never neither.
bad_talkers="$(jq -r '[.[] | select((has("control")) == (has("no_rule_reason"))) | .talker] | join(", ")' <<<"$talkers_json")"
[[ -z $bad_talkers ]] ||
  fail "D: every talker needs exactly one of control or no_rule_reason; violated by: $bad_talkers"

# THE DIFF: the set of controls the table names == the set of declared
# lulu_rule records. A talker added without a control fails here, and so does
# a rule control no talker claims.
controls_json="$(yq -o=json '.macos.posture_controls' "$CONTROLS_YAML")" ||
  fail "D: could not read the posture controls"
jq -r '[.[] | select(has("control")) | .control] | sort | .[]' <<<"$talkers_json" >"$sandbox/talker-controls"
jq -r '[.[] | select(.reader | startswith("lulu_rule")) | .id] | sort | .[]' <<<"$controls_json" >"$sandbox/declared-rule-controls"
diff -u "$sandbox/talker-controls" "$sandbox/declared-rule-controls" >&2 ||
  fail "D: the talker tables named controls and the declared lulu_rule records have drifted apart"

# The alerter curl needs no rule BECAUSE its hop is loopback under the
# allowLocalHost preference; anyone writing a curl rule has solved the wrong
# problem, and the reason must say so.
curl_reason="$(jq -r '.[] | select(.talker == "the alerter curl") | .no_rule_reason' <<<"$talkers_json")"
grep -qF 'allowLocalHost' <<<"$curl_reason" ||
  fail "D: the alerter-curl no-rule reason must rest on the allowLocalHost preference, not on a rule"

# The extension record and the two rule records exist with their exact shape.
jq -e '[.[] | select(.id == "lulu_extension")] | length == 1' <<<"$controls_json" >/dev/null ||
  fail "D: the lulu_extension verify record must be declared"
jq -e '[.[] | select(.id == "lulu_extension")] | .[0].reader == "pgrep_lulu_extension" and .[0].expect == "running" and .[0].tier == "verify"' <<<"$controls_json" >/dev/null ||
  fail "D: lulu_extension must be verify tier, read by pgrep_lulu_extension, expecting running"
jq -e '[.[] | select(.id == "lulu_rule_tailscaled")] | .[0].reader == "lulu_rule_present" and .[0].expect == "present" and .[0].target == "/usr/local/bin/tailscaled"' <<<"$controls_json" >/dev/null ||
  fail "D: lulu_rule_tailscaled must check the stable tailscaled path via lulu_rule_present"
jq -e '[.[] | select(.id == "lulu_rule_hermes_gateway")] | .[0].reader == "lulu_rule_resolved_present" and .[0].expect == "present" and (.[0].target | startswith("/"))' <<<"$controls_json" >/dev/null ||
  fail "D: lulu_rule_hermes_gateway must resolve the venv launcher via lulu_rule_resolved_present"

# ---- E: extension lifecycle against a stubbed probe --------------------------

extension_record="$(jq -c '[.[] | select(.id == "lulu_extension")]' <<<"$controls_json")"
extension_description="$(jq -r '.[0].description' <<<"$extension_record")"
extension_remedy="$(jq -r '.[0].remedy // empty' <<<"$extension_record")"
[[ -n $extension_remedy ]] ||
  fail "E: the lulu_extension record must carry a remedy"

extension_probe_argv="pgrep -x -U 0 com.objective-see.lulu.extension"
extension_healthy_seed='{"firewall":"1","gatekeeper":"1","screenlock":"1","lulu_extension":"running","lulu_extension:expect":"running"}'

setup_poller_harness
trap 'teardown_poller_harness' EXIT
set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
set_posture_controls "$extension_record"
export POLLER_PGREP_LULU_MODE=running
poller_status=0
poller_output="$(run_poller 2>&1)" || poller_status=$?
[[ $poller_status -eq 0 ]] ||
  fail "E healthy: expected exit 0, got $poller_status: $poller_output"
assert_no_page || fail "E healthy: a running extension must page nothing"
assert_probe_argv "$extension_probe_argv" 1 ||
  fail "E healthy: the poller must probe the extension process exactly once, root-scoped by exact name"
assert_baseline_scalar lulu_extension running ||
  fail "E healthy: the baseline must record the extension as running"
assert_no_mutation_attempt || fail "E healthy: a posture read must never mutate"
teardown_poller_harness
trap - EXIT

setup_poller_harness
trap 'teardown_poller_harness' EXIT
set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
set_posture_controls "$extension_record"
seed_baseline "$extension_healthy_seed"
export POLLER_PGREP_LULU_MODE=stopped
run_poller >/dev/null 2>&1 || fail "E stop: expected exit 0"
assert_page_count 1 || fail "E stop: the stop must page exactly once"
assert_page_severity_is CRIT || fail "E stop: the stop page must be CRIT"
assert_page_body_has "\`$extension_description\`: now stopped, declared running" ||
  fail "E stop: the page must name the control and the stopped-vs-declared state"
assert_page_body_has "$extension_remedy" ||
  fail "E stop: the page must carry the declared remedy"
run_poller >/dev/null 2>&1 || fail "E quiet: expected exit 0"
assert_page_count 1 || fail "E quiet: an ONGOING stop must stay quiet (page-once)"
export POLLER_PGREP_LULU_MODE=running
run_poller >/dev/null 2>&1 || fail "E restore: expected exit 0"
assert_page_count 1 || fail "E restore: a restore is silent recovery, not a page"
assert_baseline_scalar lulu_extension running ||
  fail "E restore: the baseline must return to running"
export POLLER_PGREP_LULU_MODE=stopped
run_poller >/dev/null 2>&1 || fail "E re-stop: expected exit 0"
assert_page_count 2 || fail "E re-stop: a LATER stop must page again"
assert_no_mutation_attempt || fail "E: the lifecycle must never mutate"
unset POLLER_PGREP_LULU_MODE
teardown_poller_harness
trap - EXIT

# ---- F: a missing required rule pages once, names the rule, stays quiet ------

tailscaled_record="$(jq -c '[.[] | select(.id == "lulu_rule_tailscaled")]' <<<"$controls_json")"
tailscaled_description="$(jq -r '.[0].description' <<<"$tailscaled_record")"
tailscaled_target="$(jq -r '.[0].target' <<<"$tailscaled_record")"
rule_healthy_seed="{\"firewall\":\"1\",\"gatekeeper\":\"1\",\"screenlock\":\"1\",\"lulu_rule_tailscaled\":\"present\",\"lulu_rule_tailscaled:expect\":\"present\",\"lulu_rule_tailscaled:target\":\"$tailscaled_target\"}"

# archive_xml_mentioning <path>... -- a minimal plutil-shaped XML body whose
# $objects array mentions each path, the way LuLu's keyed archive does.
archive_xml_mentioning() {
  local body="" mentioned_path
  for mentioned_path in "$@"; do
    body+="		<string>$mentioned_path</string>"$'\n'
  done
  # shellcheck disable=SC2016 # the literal $objects key is the archive format
  printf '<?xml version="1.0" encoding="UTF-8"?>\n<plist version="1.0">\n<dict>\n	<key>$objects</key>\n	<array>\n%s	</array>\n</dict>\n</plist>\n' "$body"
}

setup_poller_harness
trap 'teardown_poller_harness' EXIT
set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
set_posture_controls "$tailscaled_record"
seed_baseline "$rule_healthy_seed"
POLLER_PLUTIL_XML="$(archive_xml_mentioning /opt/unrelated/binary)"
export POLLER_PLUTIL_XML
run_poller >/dev/null 2>&1 || fail "F missing: expected exit 0"
assert_page_count 1 || fail "F missing: a missing required rule must page exactly once"
assert_page_severity_is CRIT || fail "F missing: the missing-rule page must be CRIT"
assert_page_body_has "\`$tailscaled_description\`: now absent, declared present" ||
  fail "F missing: the page must name the missing rule control"
run_poller >/dev/null 2>&1 || fail "F quiet: expected exit 0"
assert_page_count 1 || fail "F quiet: the rule STAYING missing must not re-page"
POLLER_PLUTIL_XML="$(archive_xml_mentioning "$tailscaled_target")"
export POLLER_PLUTIL_XML
run_poller >/dev/null 2>&1 || fail "F restore: expected exit 0"
assert_page_count 1 || fail "F restore: a restored rule is silent recovery"
assert_baseline_scalar lulu_rule_tailscaled present ||
  fail "F restore: the baseline must return to present"
assert_no_mutation_attempt || fail "F: the rule check must never mutate"
unset POLLER_PLUTIL_XML
teardown_poller_harness
trap - EXIT

# ---- G: a changed declared target re-arms the control ------------------------

setup_poller_harness
trap 'teardown_poller_harness' EXIT
set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
set_posture_controls "$tailscaled_record"
POLLER_PLUTIL_XML="$(archive_xml_mentioning "$tailscaled_target")"
export POLLER_PLUTIL_XML
run_poller >/dev/null 2>&1 || fail "G seed: expected exit 0"
assert_no_page || fail "G seed: a present rule must page nothing"
assert_baseline_scalar "lulu_rule_tailscaled:target" "$tailscaled_target" ||
  fail "G seed: the baseline must record the target the value was observed under"
# Repoint the control at a binary the archive does NOT mention: the prior was
# recorded under the old target, so it must not be trusted, and the deviation
# pages as a fresh first observation rather than riding the stale baseline.
set_posture_controls "$(jq -c '.[0].target = "/usr/local/bin/relocated-tailscaled" | [.[0]]' <<<"$tailscaled_record")"
run_poller >/dev/null 2>&1 || fail "G repoint: expected exit 0"
assert_page_count 1 || fail "G repoint: a repointed control whose rule is absent must page"
assert_page_body_has 'first observation' ||
  fail "G repoint: the page must be a first observation under the new target, not a transition from the old one"
unset POLLER_PLUTIL_XML
teardown_poller_harness
trap - EXIT

# ---- H: the existence-only limitation is stated, in data and runbook ---------

grep -qF 'existence-only' "$CONTROLS_YAML" ||
  fail "H: the rule records must state the existence-only limitation in the data file"
grep -qF 'does not prove the rule allows' "$CONTROLS_YAML" ||
  fail "H: the data file must state the check cannot see the rule action"
grep -qF 'existence-only' "$RUNBOOK" ||
  fail "H: the runbook must state the existence-only limitation"

printf 'ok: LuLu policy and posture (six enforce policy records with the inline alerting-path reason, plist_path containment, manual approval and rule-creation records, talker-vs-control diff, extension lifecycle, missing-rule paging, target re-arm, existence-only honesty)\n'
