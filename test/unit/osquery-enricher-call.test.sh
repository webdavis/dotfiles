#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

set_up() {
  CASE_ROOT="$(mktemp -d)"
  export HOME="$CASE_ROOT/home"
  mkdir -p "$HOME/.local/libexec/posture"
  export ARG_RECORD="$CASE_ROOT"
  cat >"$HOME/.local/libexec/posture/posture" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
printf '%s' "$#" >"$ARG_RECORD/count"
printf '%s' "${1:-}" >"$ARG_RECORD/first"
printf '%s' "${2:-}" >"$ARG_RECORD/second"
[[ ${1:-} == enrich ]] || exit 98
printf 'captured fact'
exit "${ENRICH_EXIT:-10}"
STUB
  chmod 755 "$HOME/.local/libexec/posture/posture"
}

route_case() {
  local status="$1"
  export ENRICH_EXIT="$status"
  local route_path="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/route.sh"
  # The child shell expands these variables after its private environment is set.
  # shellcheck disable=SC2016
  ROUTE_PATH="$route_path" "${BASH:-bash}" -c '
    set -euo pipefail
    source "$ROUTE_PATH"
    digest_append() { printf "%s\n" "$1" >"$ARG_RECORD/digest"; }
    printf "%s\n" '\''{"q":"system_extensions_new","act":"added","cols":{},"ep":"/a quoted path.app"}'\'' | route_findings
  ' >"$CASE_ROOT/page"
}

function test_default_executable_receives_enrich_and_the_unsplit_path() {
  unset OSQUERY_ENRICH_SCRIPT
  route_case 10
  assert_same 2 "$(cat "$CASE_ROOT/count")"
  assert_same enrich "$(cat "$CASE_ROOT/first")"
  assert_same '/a quoted path.app' "$(cat "$CASE_ROOT/second")"
  assert_same captured\ fact "$(jq -r .signing <"$CASE_ROOT/page")"
}

function test_override_is_one_executable_filename_including_spaces() {
  export OSQUERY_ENRICH_SCRIPT="$CASE_ROOT/quoted executable"
  cp "$HOME/.local/libexec/posture/posture" "$OSQUERY_ENRICH_SCRIPT"
  route_case 10
  assert_same 2 "$(cat "$CASE_ROOT/count")"
  assert_same enrich "$(cat "$CASE_ROOT/first")"
  assert_same '/a quoted path.app' "$(cat "$CASE_ROOT/second")"
  assert_same CRIT "$(jq -r .sev <"$CASE_ROOT/page")"
}

function test_exit_zero_preserves_notice_and_the_signing_fact() {
  unset OSQUERY_ENRICH_SCRIPT
  route_case 0
  assert_same '' "$(cat "$CASE_ROOT/page")"
  assert_same captured\ fact "$(jq -r .signing <"$CASE_ROOT/digest")"
}

function test_exit_five_keeps_stdout_without_promoting_notice() {
  unset OSQUERY_ENRICH_SCRIPT
  route_case 5
  assert_same '' "$(cat "$CASE_ROOT/page")"
  assert_same captured\ fact "$(jq -r .signing <"$CASE_ROOT/digest")"
}

function test_a_nonexecutable_override_still_skips_enrichment() {
  export OSQUERY_ENRICH_SCRIPT="$CASE_ROOT/not executable"
  cp "$HOME/.local/libexec/posture/posture" "$OSQUERY_ENRICH_SCRIPT"
  chmod 644 "$OSQUERY_ENRICH_SCRIPT"
  route_case 10
  assert_same '' "$(cat "$CASE_ROOT/page")"
  assert_same null "$(jq -r .signing <"$CASE_ROOT/digest")"
  assert_same false "$([[ -f $CASE_ROOT/count ]] && printf true || printf false)"
}
