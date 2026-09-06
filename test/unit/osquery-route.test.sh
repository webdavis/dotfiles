#!/usr/bin/env bash
# route.sh: the base severity matrix (route_severity) and the three-outcome
# page/digest/log-only gate (route_findings).
#
# Everything is SOURCED and called in this process. The gate's collaborators are
# doubled here rather than stubbed at the process boundary: digest_append records
# to a spool file, and the signing enricher is a fixture script that answers
# UNTRUSTED for a path holding UNTRUSTED and trusted for anything else. No
# alerter, no osqueryd, no codesign, no clock.
#
# The gate passes run ONCE in set_up_before_script and every routing test reads
# the cached result, because route_findings forks jq per finding: one 27-finding
# pass costs a fraction of what 28 single-finding passes would. Each test still
# owns one behavior and fails on its own.
#
# Outcome vocabulary, used throughout: a finding's tag surfacing on stdout means
# PAGE, in the digest spool means DIGEST, in neither means LOG-ONLY.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit; test/validate-tests.sh pins that shape. A test body runs
# WITHOUT errexit, so every helper below ends in a real bashunit assertion: a
# mid-test `return 1` would report nothing. route.sh itself is sourced ONCE at
# file scope, because bashunit already runs each body in its own subshell.
#
# Three shellcheck codes are disabled for the whole file, all for one reason:
# each gate pass runs inside a `( ... )` subshell on purpose, so every export in
# it is MEANT to reach that pass and nothing else (SC2030, SC2031), and the
# collaborator doubles defined beside those exports are called by route_findings
# rather than by name (SC2329).
# shellcheck disable=SC2030,SC2031,SC2329

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ALERTER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter"

# shellcheck source=dot_local/libexec/osquery/results-alerter/route.sh
source "$ALERTER/route.sh"

# --- fixtures and the cached gate passes -----------------------------------

# finding <tag> <q> <act> [extra-cols-json] [ep]: one normalized finding, with no
# trailing newline so it can be collected into an array. Every finding carries its
# tag in a column, so a tag found in a channel identifies which finding landed
# there.
finding() {
  printf '{"q":"%s","act":"%s","cols":{"tag":"%s"%s},"ep":"%s"}' \
    "$2" "$3" "$1" "${4:+,$4}" "${5:-}"
}

# sha256_of <path>: the file's content digest, in the form the allowlist and the
# manifest store. openssl rather than shasum because shasum is a perl script.
sha256_of() {
  local line
  line=$(openssl dgst -sha256 -r "$1")
  printf '%s' "${line%% *}"
}

set_up_before_script() {
  FILE_FIXTURE="$(mktemp -d)"
  _build_fixtures
  _run_vouched_pass
  _run_failsafe_pass
  _run_severity_shortfall_passes
  _run_emit_passes
}

tear_down_after_script() { discard_fixture "$FILE_FIXTURE"; }

# discard_fixture <path>: remove one mktemp -d this file created, and nothing
# else. Plain rm -rf, the convention every other test in this repo uses; the
# suite also runs on a CI host with no Trash.
discard_fixture() {
  [[ -n ${1:-} && -d $1 ]] || return 0
  rm -rf "$1"
}

# The vouched regime: a HOME holding two allowlisted LaunchAgents whose plists are
# pinned by hash, and a manifest that vouches for the allowlist file itself.
# Without that manifest entry allowlist_verdict refuses to suppress anything, and
# every suppression assertion below would pass for the wrong reason.
_build_fixtures() {
  local home="$FILE_FIXTURE/home"
  mkdir -p "$home/Library/LaunchAgents" "$home/bin" "$home/.config/osquery"

  UNTRUSTED_PLIST="$home/Library/LaunchAgents/com.evilUNTRUSTED.plist"
  TRUSTED_PLIST="$home/Library/LaunchAgents/com.good.plist"
  printf 'PLIST evil\n' >"$UNTRUSTED_PLIST"
  printf 'PLIST good\n' >"$TRUSTED_PLIST"

  ALLOWLIST="$home/.config/osquery/page-launchd-allowlist.txt"
  {
    printf '{"label":"com.evil","path":"~/Library/LaunchAgents/com.evilUNTRUSTED.plist","program":"~/bin/evil","sha256":"%s"}\n' \
      "$(sha256_of "$UNTRUSTED_PLIST")"
    printf '{"label":"com.good","path":"~/Library/LaunchAgents/com.good.plist","program":"~/bin/good","sha256":"%s"}\n' \
      "$(sha256_of "$TRUSTED_PLIST")"
  } >"$ALLOWLIST"
  chmod 600 "$ALLOWLIST"

  MANIFEST="$FILE_FIXTURE/pipeline-known-good.sha256"
  printf '%s 0600 %s %s\n' "$(sha256_of "$ALLOWLIST")" "$(id -u)" "$ALLOWLIST" >"$MANIFEST"

  # The signing enricher: UNTRUSTED (exit 10) for a path holding UNTRUSTED, a
  # trusted authority otherwise. Deterministic, and never a real codesign call.
  ENRICHER="$FILE_FIXTURE/enrich-stub.sh"
  cat >"$ENRICHER" <<'STUB'
#!/bin/sh
case "$1" in
  *UNTRUSTED*) printf 'UNSIGNED'; exit 10 ;;
  *) printf 'signed: Apple'; exit 0 ;;
esac
STUB
  chmod +x "$ENRICHER"
  HOME_FIXTURE="$home"
}

# The tier table, under a regime where the allowlist and the manifest both exist
# and vouch: every detector whose outcome does not turn on a missing manifest.
_run_vouched_pass() {
  local home="$HOME_FIXTURE"
  local -a findings=()
  findings+=("$(finding T01 new_admin_user added '"username":"eve","uid":"501"')")
  findings+=("$(finding T02 filevault_off added '"note":"off"')")
  findings+=("$(finding T03 filevault_off removed '"note":"restored"')")
  findings+=("$(finding T04 agent_secretfile_changed added '"path":"/Users/x/.config/pns/webhook-secret"')")
  findings+=("$(finding T05 agent_exposure_changed added '"name":"nc","address":"0.0.0.0","port":"4444"')")
  findings+=("$(finding T06 agent_exposure_changed removed '"name":"nc","address":"0.0.0.0","port":"4444"')")
  findings+=("$(finding T07 suid_bin_unexpected added '"path":"/tmp/suid07"' /tmp/suid07)")
  findings+=("$(finding T08 suid_bin_unexpected added '"path":"/tmp/UNTRUSTED08"' /tmp/UNTRUSTED08)")
  findings+=("$(finding T09 file_events_recent added '"category":"sshd_config","target_path":"/etc/ssh/sshd_config"')")
  findings+=("$(finding T10 file_events_recent added '"category":"ssh","target_path":"/Users/x/.ssh/authorized_keys"')")
  findings+=("$(finding T11 file_events_recent added '"category":"ssh","target_path":"/Users/x/.ssh/id_rsa"')")
  findings+=("$(finding T12 file_events_recent added '"category":"sudoers","target_path":"/etc/sudoers"')")
  findings+=("$(finding T13 agent_authfile_changed added '"path":"/Users/x/.codex/config.toml"')")
  findings+=("$(finding T14 listening_ports_non_loopback added '"name":"proc","address":"0.0.0.0","port":"9999"')")
  findings+=("$(finding T15 firewall_state added '"global_state":"0"')")
  findings+=("$(finding T17 kernel_extensions_new added '"name":"com.evil"' /x/UNTRUSTED.kext)")
  findings+=("$(finding T18 kernel_extensions_new added '"name":"com.good"' /x/good.kext)")
  findings+=("$(finding T19 system_extensions_new added '"identifier":"com.evil"' /x/UNTRUSTED.app)")
  findings+=("$(finding T20 system_extensions_new added '"identifier":"com.good"' /x/good.app)")
  findings+=("$(finding T21 es_launchd_writes added '"path":"/x/UNTRUSTED_es"' /x/UNTRUSTED_es)")
  findings+=("$(finding T22 persistence_startup_items_crontab added '"name":"UNTRUSTED_cron"' /x/UNTRUSTED_cron)")
  findings+=("$(finding T23 persistence_launchd added \
    "\"label\":\"com.evil\",\"path\":\"$UNTRUSTED_PLIST\",\"program\":\"$home/bin/evil\"" "$UNTRUSTED_PLIST")")
  findings+=("$(finding T24 persistence_launchd added \
    "\"label\":\"com.good\",\"path\":\"$TRUSTED_PLIST\",\"program\":\"$home/bin/good\"" "$TRUSTED_PLIST")")
  findings+=("$(finding T25 persistence_launchd added \
    "\"label\":\"com.good\",\"path\":\"$TRUSTED_PLIST\",\"program\":\"$home/bin/EVIL\"")")
  findings+=("$(finding T26 persistence_launchd added \
    "\"label\":\"com.unknown\",\"path\":\"$home/Library/LaunchAgents/com.unknown.plist\",\"program\":\"$home/bin/unknown\"")")
  findings+=("$(finding T27 persistence_launchd added \
    '"label":"com.daemon","path":"/Library/LaunchDaemons/com.daemon.plist","program":"/usr/bin/daemon"')")
  findings+=("$(finding T28 persistence_launchd added \
    '"label":"com.apple.x","path":"/System/Library/LaunchAgents/com.apple.x.plist","program":"/usr/bin/x"')")

  : >"$FILE_FIXTURE/main.digest"
  (
    export HOME="$home"
    export OSQUERY_LAUNCHD_ALLOWLIST="$ALLOWLIST"
    export OSQUERY_PIPELINE_MANIFEST="$MANIFEST"
    export OSQUERY_PIPELINE_SETTLE_SECONDS=0
    export OSQUERY_ENRICH_SCRIPT="$ENRICHER"
    source "$ALERTER/route.sh"
    source "$ALERTER/allowlist-verdict.sh"
    source "$ALERTER/pipeline-verdict.sh"
    digest_append() { printf '%s\n' "$1" >>"$FILE_FIXTURE/main.digest"; }
    printf '%s\n' "${findings[@]}" | route_findings
  ) >"$FILE_FIXTURE/main.page" 2>"$FILE_FIXTURE/main.err"
}

# The fail-safe regime: no allowlist file and no manifest, so nothing can vouch
# for anything. A tracked pipeline file event and an unknown user LaunchAgent both
# have to page here, and an untracked neighbour in the same watched directory
# still has to stay quiet.
_run_failsafe_pass() {
  local -a findings=()
  findings+=("$(finding S01 file_events_recent added \
    '"category":"pipeline_integrity","target_path":"/Users/x/.local/libexec/osquery/results-alerter.sh","sha256":"abc","action":"UPDATED"' \
    /Users/x/.local/libexec/osquery/results-alerter.sh)")
  findings+=("$(finding S02 file_events_recent added \
    '"category":"allowlist_file","target_path":"/Users/x/.config/osquery/page-launchd-allowlist.txt","sha256":"abc","action":"UPDATED"')")
  findings+=("$(finding S03 file_events_recent added \
    '"category":"allowlist_file","target_path":"/Users/x/.config/osquery/webhook-secret","sha256":"abc","action":"UPDATED"')")
  findings+=("$(finding S04 persistence_launchd added \
    '"label":"com.s04","path":"/Users/x/Library/LaunchAgents/com.s04.plist","program":"/Users/x/bin/s04"')")

  : >"$FILE_FIXTURE/safe.digest"
  (
    export HOME=/Users/x
    export OSQUERY_LAUNCHD_ALLOWLIST="$FILE_FIXTURE/absent-allowlist.txt"
    export OSQUERY_PIPELINE_MANIFEST="$FILE_FIXTURE/absent-manifest.sha256"
    export OSQUERY_PIPELINE_SETTLE_SECONDS=0
    export OSQUERY_ENRICH_SCRIPT="$FILE_FIXTURE/absent-enricher.sh"
    source "$ALERTER/route.sh"
    source "$ALERTER/allowlist-verdict.sh"
    source "$ALERTER/pipeline-verdict.sh"
    digest_append() { printf '%s\n' "$1" >>"$FILE_FIXTURE/safe.digest"; }
    printf '%s\n' "${findings[@]}" | route_findings
  ) >"$FILE_FIXTURE/safe.page" 2>"$FILE_FIXTURE/safe.err"
}

# route_findings classifies the whole batch in ONE route_severity pass and reads
# back one severity per finding by index. These two passes are the shortfall (the
# classifier dies partway, so the batch comes back short) and its healthy control.
#
# Both run under `set -euo pipefail`, the entry script's options, because the
# shortfall must not be answered by the caller's shell aborting the pass: an abort
# loses the findings already classified as well as the unclassified ones.
_run_severity_shortfall_passes() {
  local name status
  local -a findings=()
  findings+=("$(finding X_A new_admin_user added '"username":"eve"')")
  findings+=("$(finding X_B filevault_off added '"note":"off"')")
  findings+=("$(finding X_C suid_bin_unexpected added '"path":"/tmp/suid"')")
  findings+=("$(finding X_D homebrew_packages added '"name":"pkg"')")

  for name in short healthy; do
    status=0
    (
      set -euo pipefail
      export OSQUERY_ENRICH_SCRIPT="$FILE_FIXTURE/absent-enricher.sh"
      export OSQUERY_LAUNCHD_ALLOWLIST="$FILE_FIXTURE/absent-allowlist.txt"
      export OSQUERY_PIPELINE_MANIFEST="$FILE_FIXTURE/absent-manifest.sha256"
      source "$ALERTER/route.sh"
      source "$ALERTER/allowlist-verdict.sh"
      source "$ALERTER/pipeline-verdict.sh"
      digest_append() { :; }
      # The shortfall shape: one severity arrives, then the classifier dies with a
      # status of its own, leaving three findings with no severity at all.
      if [[ $name == short ]]; then
        route_severity() {
          printf 'CRIT\n'
          return 5
        }
      fi
      printf '%s\n' "${findings[@]}" | route_findings
    ) >"$FILE_FIXTURE/$name.page" 2>"$FILE_FIXTURE/$name.err" || status=$?
    printf '%s' "$status" >"$FILE_FIXTURE/$name.status"
  done
}

# The page emit is the LAST point at which a confirmed page can be lost: the entry
# checkpoints its cursor past the batch once the page is delivered, so an emit
# that died while route_findings reported success takes those findings with it.
#
# jq is doubled by a shell FUNCTION rather than a PATH shim, which shadows it for
# route.sh without adding a process to every call. The double fails only the emit,
# identified by a program text no other call in route.sh uses, so the same double
# serves the failure case and its controls.
_run_emit_passes() {
  local page_finding logonly_finding name arm finding_json status
  page_finding=$(finding EMIT_PAGE new_admin_user added '"username":"eve"')
  logonly_finding=$(finding EMIT_DROP homebrew_packages added '"name":"pkg"')

  for name in emit-armed emit-healthy emit-nothing; do
    case "$name" in
      emit-armed) arm=1 finding_json="$page_finding" ;;
      emit-healthy) arm=0 finding_json="$page_finding" ;;
      emit-nothing) arm=1 finding_json="$logonly_finding" ;;
    esac
    status=0
    # Deliberately NOT under `set -e`: a library must not owe its correctness to
    # the caller's shell options, and every other pass in this file sources
    # route.sh the same way, so a swallowed status would green them all.
    (
      export OSQUERY_ENRICH_SCRIPT="$FILE_FIXTURE/absent-enricher.sh"
      source "$ALERTER/route.sh"
      digest_append() { :; }
      jq() {
        local argument
        if [[ $arm == 1 ]]; then
          for argument in "$@"; do
            if [[ $argument == '.sev = "CRIT"' ]]; then
              printf 'jq double: simulated page-emit failure\n' >&2
              return 5
            fi
          done
        fi
        command jq "$@"
      }
      printf '%s\n' "$finding_json" | route_findings
    ) >"$FILE_FIXTURE/$name.page" 2>"$FILE_FIXTURE/$name.err" || status=$?
    printf '%s' "$status" >"$FILE_FIXTURE/$name.status"
  done
}

# --- assertions -------------------------------------------------------------

# assert_routed <tag> <page|digest|logonly> <why>: where the vouched pass sent it.
assert_routed() {
  _load_pass main
  _assert_routed "$MAIN_PAGE" "$MAIN_DIGEST" "$@"
}

# assert_routed_failsafe <tag> <page|digest|logonly> <why>: the same, for the pass
# with nothing to vouch for anything.
assert_routed_failsafe() {
  _load_pass safe
  _assert_routed "$SAFE_PAGE" "$SAFE_DIGEST" "$@"
}

# _load_pass <main|safe>: read a cached pass once per test subshell.
_load_pass() {
  case "$1" in
    main)
      [[ -n ${MAIN_PAGE+set} ]] && return 0
      MAIN_PAGE=$(<"$FILE_FIXTURE/main.page")
      MAIN_DIGEST=$(<"$FILE_FIXTURE/main.digest")
      ;;
    safe)
      [[ -n ${SAFE_PAGE+set} ]] && return 0
      SAFE_PAGE=$(<"$FILE_FIXTURE/safe.page")
      SAFE_DIGEST=$(<"$FILE_FIXTURE/safe.digest")
      ;;
  esac
}

_assert_routed() {
  local page="$1" digest="$2" tag="\"tag\":\"$3\"" want="$4" why="$5" got=logonly
  if [[ $page == *"$tag"* ]]; then got=page; fi
  if [[ $digest == *"$tag"* ]]; then
    if [[ $got == page ]]; then got=both; else got=digest; fi
  fi
  [[ $got == "$want" ]] || printf '%s: %s\n' "$3" "$why" >&2
  assert_same "$want" "$got"
}

# assert_severities <expected-newline-list> <finding>...: one route_severity pass,
# compared positionally, which pins the order and the count as well as the tiers.
assert_severities() {
  local expected="$1" got
  shift
  got=$(printf '%s\n' "$@" | route_severity)
  assert_same "$expected" "$got"
}

# assert_holds <haystack> <needle> <why>
assert_holds() {
  [[ $1 == *"$2"* ]] || printf 'why this must be present: %s\n' "$3" >&2
  assert_contains "$2" "$1"
}

# --- the base severity matrix ----------------------------------------------

function test_a_protection_in_its_unsafe_state_a_new_admin_account_and_a_new_setuid_root_binary_are_crit() {
  assert_severities 'CRIT
CRIT
CRIT
CRIT
CRIT
CRIT' \
    "$(finding P1 filevault_off added)" \
    "$(finding P2 firewall_state added '"global_state":"0"')" \
    "$(finding P3 gatekeeper_state added '"assessments_enabled":"0"')" \
    "$(finding P4 sip_state added '"enabled":"0"')" \
    "$(finding P5 new_admin_user added '"username":"eve"')" \
    "$(finding P6 suid_bin_unexpected added '"path":"/tmp/x"')"
}

function test_a_security_policy_row_that_is_not_the_unsafe_state_falls_to_notice_never_to_info() {
  assert_severities 'NOTICE
NOTICE
NOTICE
NOTICE' \
    "$(finding P7 firewall_state added '"global_state":"1"')" \
    "$(finding P8 filevault_off removed)" \
    "$(finding P9 filevault_state removed)" \
    "$(finding P10 remote_access_sharing_state added)"
}

function test_persistence_extensions_watched_files_and_endpoint_security_writes_are_notice() {
  assert_severities 'NOTICE
NOTICE
NOTICE
NOTICE
NOTICE
NOTICE' \
    "$(finding P11 persistence_launchd added)" \
    "$(finding P12 persistence_startup_items_crontab added)" \
    "$(finding P13 kernel_extensions_new added)" \
    "$(finding P14 system_extensions_new added)" \
    "$(finding P15 file_events_recent added)" \
    "$(finding P16 es_launchd_writes added)"
}

function test_software_drift_listeners_logins_and_the_agent_queries_are_info() {
  assert_severities 'INFO
INFO
INFO
INFO
INFO' \
    "$(finding P17 homebrew_packages added)" \
    "$(finding P18 installed_apps added)" \
    "$(finding P19 listening_ports_non_loopback added)" \
    "$(finding P20 recent_logins added)" \
    "$(finding P21 agent_exposure_changed added)"
}

# --- the gate: which channel each detector reaches -------------------------

function test_the_page_tier_reaches_stdout_including_the_two_remote_auth_file_events() {
  assert_routed T01 page 'a new administrator account is criterion 1'
  assert_routed T02 page 'a filevault_off differential row is the protection being off, criterion 2'
  assert_routed T04 page 'a change to one of the two real secrets is criterion 3'
  assert_routed T05 page 'an agent port newly reachable off-loopback is criterion 3'
  assert_routed T07 page 'a new setuid-root binary pages on its base tier alone'
  assert_routed T09 page 'sshd_config is remote-auth policy'
  assert_routed T10 page 'authorized_keys is a remote-auth entry point'
}

function test_the_safe_direction_rows_and_the_poller_owned_protection_reach_neither_channel() {
  assert_routed T03 logonly 'encryption coming back is not an incident'
  assert_routed T06 logonly 'a removed exposure row is the port being closed, which is good news'
  assert_routed T15 logonly 'the 60s firewall poller pages this already, so routing it here would double-page'
}

function test_the_ambiguous_tier_digests_a_credential_file_a_new_listener_a_private_key_sudoers() {
  assert_routed T13 digest 'agent_authfile_changed is the three non-secret configs, routine churn'
  assert_routed T14 digest 'a new non-loopback listener is suspicious but ambiguous'
  assert_routed T11 digest 'a ~/.ssh file that is not authorized_keys digests'
  assert_routed T12 digest 'a sudoers change digests'
}

function test_the_extension_arms_honor_the_untrusted_signing_promotion_and_the_log_only_arms_ignore_it() {
  assert_routed T17 page 'the kext arm honors the enrichment promotion (operator ruling 2026-07-22)'
  assert_routed T18 logonly 'a signed kernel extension stays at its base tier, log-only'
  assert_routed T19 page 'the sysext arm honors the same promotion'
  assert_routed T20 digest 'a signed system extension is usually an app upgrade, so it digests'
  # The ruling is scoped to those two detectors: these two stay log-only on an
  # UNTRUSTED path, which is the same input that promoted the extensions above.
  assert_routed T21 logonly 'the raw ES write is already covered by the persistence_launchd differential'
  assert_routed T22 logonly 'crontab persistence is a low-noise legacy vector kept log-only'
}

function test_an_untrusted_program_behind_a_fully_allowlisted_label_still_pages_and_the_trusted_one_is_suppressed() {
  assert_routed T23 page 'the security invariant: a promoted CRIT is never suppressed by the allowlist'
  assert_routed T24 logonly 'a fully allowlisted agent whose program is trusted is suppressed'
}

function test_default_deny_a_reused_label_an_unknown_agent_and_a_launchdaemon_page_an_apple_item_is_skipped() {
  assert_routed T25 page 'the label is allowlisted but the program diverges, so this is not the vouched identity'
  assert_routed T26 page 'an unallowlisted user LaunchAgent pages (operator ruling 2026-07-22)'
  assert_routed T27 page 'a root LaunchDaemon runs at boot, so the path decides without the allowlist'
  assert_routed T28 logonly "Apple's own launchd items are log-only"
}

function test_the_signing_verdict_is_attached_to_a_paged_finding_trusted_or_not() {
  local signings
  _load_pass main
  signings=$(jq -sr 'map(select(.cols.tag == "T07" or .cols.tag == "T08"))
    | sort_by(.cols.tag) | map(.signing // "absent") | join(",")' <<<"$MAIN_PAGE")
  assert_same 'signed: Apple,UNSIGNED' "$signings"
}

function test_every_finding_reaches_exactly_one_channel_and_every_page_is_stamped_crit() {
  local -a pages digests
  _load_pass main
  mapfile -t pages <<<"$MAIN_PAGE"
  mapfile -t digests <<<"$MAIN_DIGEST"
  assert_same 14 "${#pages[@]}"
  assert_same 5 "${#digests[@]}"
  assert_same true "$(jq -s 'all(.[]; .sev == "CRIT")' <<<"$MAIN_PAGE")"
}

# --- the gate with nothing to vouch for anything ---------------------------

function test_with_nothing_able_to_vouch_tracked_edits_and_an_unknown_agent_page_while_a_neighbour_stays_silent() {
  assert_routed_failsafe S01 page 'no manifest means the change cannot be confirmed, which is fail-safe, not benign'
  assert_routed_failsafe S02 page 'the allowlist decides whether an unknown agent pages, so an unvouched edit is a tamper of the deciding component'
  assert_routed_failsafe S04 page 'a missing allowlist is not an empty one: default-deny still applies'
  # The counterweight, and the reason the three above are not just "everything
  # pages": the watch is a DIRECTORY watch, so a secret file in the same directory
  # really does arrive here, and it must not become a CRIT nobody can confirm.
  assert_routed_failsafe S03 logonly 'an untracked neighbour stays silent'
}

# --- a severity batch that comes back short --------------------------------

function test_a_finding_whose_severity_never_arrived_pages_the_batch_completes_and_the_shortfall_is_announced() {
  local page status err
  page=$(<"$FILE_FIXTURE/short.page")
  status=$(<"$FILE_FIXTURE/short.status")
  err=$(<"$FILE_FIXTURE/short.err")
  assert_holds "$page" X_A 'the finding the truncated batch did classify must still page'
  assert_holds "$page" X_B 'filevault_off lost its severity and must page, not be dropped'
  assert_holds "$page" X_C 'suid_bin_unexpected lost its severity and must page, not be dropped'
  [[ $status -eq 0 ]] ||
    printf 'the pass must complete rather than abort mid-batch (%s)\n' "$err" >&2
  assert_same 0 "$status"
  assert_holds "$err" severity 'the degradation must be human-visible in the launchd log, not inferred from an over-page'
  assert_holds "$err" 'route_severity exit 5' \
    'reporting the real status proves it was captured, not thrown away by a process substitution'
}

function test_a_healthy_severity_batch_routes_normally_and_warns_about_nothing() {
  local page status err
  page=$(<"$FILE_FIXTURE/healthy.page")
  status=$(<"$FILE_FIXTURE/healthy.status")
  err=$(<"$FILE_FIXTURE/healthy.err")
  [[ $status -eq 0 ]] || printf 'the healthy pass must exit 0 (%s)\n' "$err" >&2
  assert_same 0 "$status"
  assert_holds "$page" X_A 'a new admin account pages on a healthy batch'
  assert_holds "$page" X_B 'filevault_off pages on a healthy batch'
  assert_holds "$page" X_C 'a new setuid binary pages on a healthy batch'
  [[ $page != *X_D* ]] ||
    printf 'homebrew drift is INFO and must stay log-only, so the fail-safe cannot be a blanket page\n' >&2
  assert_not_contains X_D "$page"
  [[ -z $err ]] || printf 'a healthy batch must not warn\n' >&2
  assert_empty "$err"
}

# --- the page emit ----------------------------------------------------------

function test_a_failed_page_emit_reports_nonzero_instead_of_a_clean_nothing_to_page() {
  local status
  status=$(<"$FILE_FIXTURE/emit-armed.status")
  [[ $status -ne 0 ]] ||
    printf 'a lost page emit reported success, so the entry would checkpoint past findings that were never written\n' >&2
  assert_not_same 0 "$status"
}

function test_a_healthy_emit_writes_the_candidate_and_a_batch_with_nothing_to_page_emits_nothing_both_exiting_zero() {
  local status page
  status=$(<"$FILE_FIXTURE/emit-healthy.status")
  page=$(<"$FILE_FIXTURE/emit-healthy.page")
  [[ $status -eq 0 ]] ||
    printf 'the fix cannot be a blanket nonzero return: a healthy emit exited %s\n' "$status" >&2
  assert_same 0 "$status"
  assert_holds "$page" EMIT_PAGE 'a healthy emit still writes the page-candidate'

  # The armed double is inert here, because a batch with no page-candidates never
  # reaches the emit at all.
  status=$(<"$FILE_FIXTURE/emit-nothing.status")
  page=$(<"$FILE_FIXTURE/emit-nothing.page")
  assert_same 0 "$status"
  assert_empty "$page"
}
