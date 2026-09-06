#!/usr/bin/env bash
# render-page.sh: the #priority page body built from the enriched CRIT findings.
#
# render_page is display-only and already sourceable, so every test calls the
# function in this process. No alerter, no spawned shell and no clock: one jq
# pass per render is the whole cost, and the fixtures are built with bash
# parameter expansion rather than a jq fork apiece.
#
# Three subjects share the file because they share that one function: the block
# shape and its caps, the basename-only privacy rule for secret and credential
# files, and the two injection defenses (an embedded newline must not open a
# forged markdown line, and a crafted path must not escape a rendered next-step
# command).
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit; test/validate-tests.sh pins that shape. A test body runs
# WITHOUT errexit, so a helper that merely returns non-zero mid-test would
# report nothing: every check below ends in a real bashunit assertion, and the
# reason a check exists is printed beside it, since that is the part bashunit's
# own failure message cannot say.
#
# This file deals in LITERAL shell-injection payloads and stub-script bodies, so
# `$(...)` and `$@` inside single quotes are deliberate: they must NOT expand here.
# shellcheck disable=SC2016

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# shellcheck source=dot_local/libexec/osquery/results-alerter/render-page.sh
source "$REPO_ROOT/dot_local/libexec/osquery/results-alerter/render-page.sh"

# Stub tools for the command-injection test, built once: codesign, cat and shasum
# record every argument they are handed, sudo drops itself and execs the rest, and
# touch drops a PROOF marker so an injected `touch` is detectable.
set_up_before_script() {
  local tool
  FILE_FIXTURE="$(mktemp -d)"
  mkdir -p "$FILE_FIXTURE/bin"
  for tool in codesign cat shasum; do
    printf '#!/bin/sh\nfor a in "$@"; do printf "%%s\\n" "$a" >>"%s/argv"; done\nexit 0\n' \
      "$FILE_FIXTURE" >"$FILE_FIXTURE/bin/$tool"
  done
  printf '#!/bin/sh\nexec "$@"\n' >"$FILE_FIXTURE/bin/sudo"
  # An injected `touch` APPENDS rather than creating, so an assertion can compare
  # the marker before and after instead of deleting it between cases.
  printf '#!/bin/sh\nprintf "ran\\n" >>"%s/PROOF"\n' "$FILE_FIXTURE" >"$FILE_FIXTURE/bin/touch"
  : >"$FILE_FIXTURE/PROOF"
  chmod +x "$FILE_FIXTURE/bin/codesign" "$FILE_FIXTURE/bin/cat" \
    "$FILE_FIXTURE/bin/shasum" "$FILE_FIXTURE/bin/sudo" "$FILE_FIXTURE/bin/touch"
}

tear_down_after_script() { discard_fixture "$FILE_FIXTURE"; }

# discard_fixture <path>: remove one mktemp -d this file created, and nothing
# else. Plain rm -rf, the convention every other test in this repo uses; the
# suite also runs on a CI host with no Trash.
discard_fixture() {
  [[ -n ${1:-} && -d $1 ]] || return 0
  rm -rf "$1"
}

# --- fixture builders (no forks: the payloads are hostile, jq is not needed) ---

# json_string <raw>: the value as a quoted, escaped JSON string.
json_string() {
  local s=$1
  s=${s//\\/\\\\}
  s=${s//\"/\\\"}
  s=${s//$'\n'/\\n}
  s=${s//$'\r'/\\r}
  s=${s//$'\t'/\\t}
  printf '"%s"' "$s"
}

# crit <q> <cols-json> [ep] [extra-members-json]: one CRIT finding.
crit() {
  printf '{"q":"%s","act":"added","sev":"CRIT","cols":%s,"ep":%s%s}' \
    "$1" "$2" "$(json_string "${3:-}")" "${4:+,$4}"
}

# render <finding-ndjson>: render once, into RENDER_COUNT and RENDER_BODY.
render() {
  local out
  out="$(printf '%s\n' "$1" | render_page | jq -r '.pcount, .pbody')"
  RENDER_COUNT=${out%%$'\n'*}
  RENDER_BODY=${out#*$'\n'}
}

# --- assertions -------------------------------------------------------------

# assert_holds <needle> <why>: RENDER_BODY holds the needle. The reason is
# printed first on failure; the assertion is what turns the test red.
assert_holds() {
  [[ $RENDER_BODY == *"$1"* ]] || printf 'why the page body must hold this: %s\n' "$2" >&2
  assert_contains "$1" "$RENDER_BODY"
}

# refute_holds <needle> <why>: RENDER_BODY does not hold the needle. A real
# assertion rather than a `! grep`, which no shell option can fail a test on.
refute_holds() {
  [[ $RENDER_BODY != *"$1"* ]] || printf 'why the page body must NOT hold this: %s\n' "$2" >&2
  assert_not_contains "$1" "$RENDER_BODY"
}

# next_step_command: the shell command inside the backticks on RENDER_BODY's
# next-step line, extracted with parameter expansion rather than sed.
next_step_command() {
  local line rest
  while IFS= read -r line; do
    case "$line" in
      *'**Inspect:**'* | *'**Review:**'* | *'**Compare:**'* | *'**Inspect the writer:**'*)
        rest=${line#*\`}
        printf '%s' "${rest%%\`*}"
        return 0
        ;;
    esac
  done <<<"$RENDER_BODY"
  printf 'no next-step command was rendered\n--- body ---\n%s\n' "$RENDER_BODY" >&2
  return 1
}

# assert_command_is_safe <label> <finding> <payload>: render the finding, run the
# next-step command it suggests under the stub tools, and require that the
# injected clause never ran and that the tool got the whole path as ONE argument.
# `bash -n` would pass the injected form (it is valid bash), so the proof is
# execution.
assert_command_is_safe() {
  local label="$1" payload="$3" command_line argv proof_before proof_after
  render "$2"
  command_line="$(next_step_command)"
  assert_not_empty "$command_line"
  : >"$FILE_FIXTURE/argv"
  proof_before=$(<"$FILE_FIXTURE/PROOF")
  PATH="$FILE_FIXTURE/bin:/usr/bin:/bin" bash -c "$command_line" >/dev/null 2>&1 || true
  proof_after=$(<"$FILE_FIXTURE/PROOF")
  [[ $proof_after == "$proof_before" ]] ||
    printf '%s: COMMAND INJECTION, the crafted path executed touch. command: %s\n' \
      "$label" "$command_line" >&2
  assert_same "$proof_before" "$proof_after"
  argv="$(<"$FILE_FIXTURE/argv")"
  [[ $argv == *"$payload"* ]] ||
    printf '%s: the tool did not receive the whole path as one argument. command: %s\n' \
      "$label" "$command_line" >&2
  assert_contains "$payload" "$argv"
}

# assert_value_stays_on_one_line <label> <finding> <marker>: a backtick ends a
# Discord inline-code span and so does a NEWLINE, so an attacker-controlled column
# carrying one could otherwise open a markdown line of its own. The headline
# forgery is a fabricated signing provenance: these findings carry no real
# .signing, so any line STARTING with "- **Signing:**" is the injected one.
assert_value_stays_on_one_line() {
  local label="$1" marker="$3" line marker_line="" forged_line=""
  render "$2"
  while IFS= read -r line; do
    if [[ -z $forged_line && $line == '- **Signing:**'* ]]; then forged_line=$line; fi
    if [[ $line == *"$marker"* ]]; then marker_line=$line; fi
  done <<<"$RENDER_BODY"
  [[ -z $forged_line ]] ||
    printf '%s: a FORGED signing line was injected\n' "$label" >&2
  assert_empty "$forged_line"
  [[ -n $marker_line ]] ||
    printf '%s: the field value (%s) is missing from the body\n' "$label" "$marker" >&2
  assert_not_empty "$marker_line"
  [[ $marker_line == *'signed: Apple'* ]] ||
    printf '%s: the embedded break was not squashed, the value split across lines\n' "$label" >&2
  assert_contains 'signed: Apple' "$marker_line"
  # A CARRIAGE RETURN needs its own check, and this is the assertion the legacy
  # suite was missing: bash `read` and grep both split on newlines only, so a \r
  # that survived the sanitize left every line-based assertion above green while
  # Discord still broke the line. Verified by mutation: dropping \r from the
  # squash set in render-page.sh passed the old test and fails this one.
  [[ $RENDER_BODY != *$'\r'* ]] ||
    printf '%s: a carriage return survived into the page body, so the value can still break its line\n' "$label" >&2
  assert_not_contains $'\r' "$RENDER_BODY"
}

# --- block shape and the basename-only privacy rule ------------------------

function test_a_crit_finding_renders_a_plain_english_header_its_decision_fields_and_a_next_step() {
  render "$(crit new_admin_user '{"username":"eve","uid":"501"}')"
  assert_holds 'New administrator account' 'the header names the event in plain English'
  assert_holds '**User:**' 'the decision fields are labelled'
  assert_holds 'eve' 'the decision field carries its value'
  assert_holds 'admin access' 'the block ends in one next step'
}

function test_a_secret_or_credential_file_is_rendered_by_basename_never_with_its_path_or_content_hash() {
  render "$(crit agent_secretfile_changed \
    '{"path":"/Users/x/.config/pns/webhook-secret","sha256":"cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe"}')
$(crit agent_authfile_changed \
      '{"path":"/Users/x/.codex/config.toml","sha256":"deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"}')"
  assert_holds 'webhook-secret' 'the secret file is still identified, by basename'
  refute_holds '/Users/x/.config/pns' 'the page fans out to Discord, so the path stays out of it'
  refute_holds 'cafebabe' 'the content hash of a secret stays out of the page'
  assert_holds 'config.toml' 'the credential file is identified by basename'
  refute_holds '/Users/x/.codex/config.toml' 'the credential path stays out of the page'
  refute_holds 'deadbeef' 'the content hash of a credential stays out of the page'
}

function test_a_field_value_over_240_characters_is_truncated_behind_a_marker() {
  local long
  printf -v long 'a%.0s' {1..300}
  render "$(crit new_admin_user "{\"username\":\"$long\",\"uid\":\"1\"}")"
  assert_holds '(truncated)' 'the cap announces itself'
  refute_holds "$long" 'one giant value cannot alone spend the delivery budget'
}

function test_the_page_renders_at_most_eight_blocks_and_counts_every_crit_finding_it_dropped() {
  local findings="" i blocks rest
  for i in {1..10}; do
    findings+="$(crit new_admin_user "{\"username\":\"user$i\",\"uid\":\"$i\"}")"$'\n'
  done
  render "$findings"
  assert_same 10 "$RENDER_COUNT"
  assert_holds 'and 2 more CRITICAL finding(s)' 'the dropped blocks are accounted for'
  blocks=0
  rest="$RENDER_BODY"
  while [[ $rest == *'New administrator account'* ]]; do
    blocks=$((blocks + 1))
    rest=${rest#*'New administrator account'}
  done
  assert_same 8 "$blocks"
}

function test_the_page_body_is_hard_capped_below_the_2000_char_delivery_limit() {
  local wide findings="" i
  printf -v wide 'b%.0s' {1..240}
  for i in {1..8}; do
    findings+="$(crit new_admin_user "{\"username\":\"$wide$i\",\"uid\":\"$i\"}")"$'\n'
  done
  render "$findings"
  assert_holds 'truncated to fit the 2000-char limit' 'the final cap announces itself'
  assert_less_than 2000 "${#RENDER_BODY}"
}

# --- file-integrity triage lines -------------------------------------------

function test_file_integrity_triage_facts_render_exactly_when_the_router_attached_them() {
  local triage
  triage='"triage":{"recorded":"aaaaaaaaaaaa","ondisk":"bbbbbbbbbbbb","upgrade":"recorded upgrade: route.sh 1.0 -> 1.1 at 2026-08-03T12:00:00Z (the name matches this file, which is not proof)"}'
  render "$(crit file_events_recent \
    '{"category":"pipeline_integrity","target_path":"/Users/x/.local/libexec/osquery/route.sh","action":"UPDATED"}' \
    /Users/x/.local/libexec/osquery/route.sh "$triage")"
  assert_holds 'aaaaaaaaaaaa' 'the recorded hash says which bytes were vouched for'
  assert_holds 'bbbbbbbbbbbb' 'the on-disk hash says which bytes are there now'
  assert_holds 'recorded upgrade: route.sh 1.0 -> 1.1' 'the upgrade correlation is a lead the operator can follow'
  assert_holds 'not proof' 'the correlation keeps the qualifier that stops it reading as an all-clear'
  assert_holds 'Security tooling changed' 'the triage facts are additive: the header is unchanged'
  assert_holds '/Users/x/.local/libexec/osquery/route.sh' 'the path is unchanged'
  assert_holds 'shasum -a 256' 'the next step is unchanged'

  # The control, and the reason the assertions above mean anything: the ssh and
  # sshd_config file events the same arm renders carry no triage object, and they
  # must render exactly as they did before.
  render "$(crit file_events_recent \
    '{"category":"sshd_config","target_path":"/etc/ssh/sshd_config","action":"UPDATED"}' \
    /etc/ssh/sshd_config)"
  assert_holds 'sshd_config changed' 'the block still renders'
  refute_holds 'Recorded:' 'a triage line with nothing behind it is worse than none'
  refute_holds 'Upgrade record:' 'a triage line with nothing behind it is worse than none'
}

# --- newline injection ------------------------------------------------------

function test_an_embedded_newline_in_any_rendered_column_stays_on_one_line_so_no_signing_line_can_be_forged() {
  local payload marker=ZZmarkerZZ
  payload=$'ZZmarkerZZ\n- **Signing:** signed: Apple (Developer ID)'
  assert_value_stays_on_one_line 'persistence label' \
    "$(crit persistence_launchd "{\"label\":$(json_string "$payload"),\"program\":\"/bin/sh\"}")" "$marker"
  assert_value_stays_on_one_line 'persistence program' \
    "$(crit persistence_launchd "{\"label\":\"com.x\",\"program\":$(json_string "$payload")}")" "$marker"
  assert_value_stays_on_one_line 'suid path' \
    "$(crit suid_bin_unexpected "{\"path\":$(json_string "$payload"),\"username\":\"root\"}")" "$marker"
  assert_value_stays_on_one_line 'new_admin_user username' \
    "$(crit new_admin_user "{\"username\":$(json_string "$payload"),\"uid\":\"501\"}")" "$marker"
  assert_value_stays_on_one_line 'file_events target_path' \
    "$(crit file_events_recent "{\"category\":\"ssh\",\"target_path\":$(json_string "$payload")}")" "$marker"
}

function test_an_embedded_carriage_return_is_squashed_the_same_way_a_newline_is() {
  local payload
  payload=$'ZZmarkerZZ\r- **Signing:** signed: Apple (Developer ID)'
  assert_value_stays_on_one_line 'persistence label' \
    "$(crit persistence_launchd "{\"label\":$(json_string "$payload"),\"program\":\"/bin/sh\"}")" ZZmarkerZZ
}

# --- command injection into a rendered next step ---------------------------

function test_a_quote_breaking_path_never_executes_in_a_codesign_next_step_command() {
  local quote_break='/tmp/x"; touch /tmp/PROOF; #'
  assert_command_is_safe 'suid, codesign' \
    "$(crit suid_bin_unexpected "{\"path\":$(json_string "$quote_break"),\"username\":\"root\"}" "$quote_break")" \
    'touch /tmp/PROOF; #'
  assert_command_is_safe 'es_launchd_writes, codesign' \
    "$(crit es_launchd_writes '{"path":"/proc/x"}' "$quote_break")" \
    'touch /tmp/PROOF; #'
}

function test_a_quote_breaking_path_never_executes_in_a_cat_sudo_cat_or_shasum_next_step_command() {
  local quote_break='/tmp/x"; touch /tmp/PROOF; #'
  assert_command_is_safe 'persistence, cat' \
    "$(crit persistence_launchd '{"label":"com.x","program":"/bin/sh"}' "$quote_break")" \
    'touch /tmp/PROOF; #'
  assert_command_is_safe 'file_events ssh, sudo cat' \
    "$(crit file_events_recent '{"category":"ssh","target_path":"/x/authorized_keys"}' "$quote_break")" \
    'touch /tmp/PROOF; #'
  assert_command_is_safe 'file_events pipeline, shasum' \
    "$(crit file_events_recent '{"category":"pipeline_integrity","target_path":"/x/osquery-alerter.sh"}' "$quote_break")" \
    'touch /tmp/PROOF; #'
}

# A second payload shape: a command substitution needs no quote to break out of,
# so the @sh single-quoting is the only thing standing in its way.
function test_a_command_substitution_path_never_executes_in_a_rendered_next_step_command() {
  local substitution='/tmp/$(touch /tmp/PROOF)'
  assert_command_is_safe 'suid, codesign' \
    "$(crit suid_bin_unexpected "{\"path\":$(json_string "$substitution"),\"username\":\"root\"}" "$substitution")" \
    '$(touch /tmp/PROOF)'
}
