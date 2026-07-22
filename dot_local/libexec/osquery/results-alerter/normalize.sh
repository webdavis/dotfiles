#!/usr/bin/env bash
#
# normalize.sh - a sourced helper for results-alerter.sh. Functions only, no
# main; nothing here delivers, checkpoints, or exits. The entry script sources
# it and pipes the raw results-log tail through normalize_findings, which is the
# first stage of the newline-delimited-JSON pipeline.
#
# normalize_findings reads raw osquery results-log lines on stdin (one JSON row
# per line) and writes ONE normalized finding per surviving row to stdout as
# newline-delimited JSON. The normalized shape this stage guarantees:
#   {q, act, cols}
#     q    - the query name with any pack_<pack>_ prefix stripped, so a packed
#            query and a top-level query reach the routing stage under the same
#            bare name it matches on.
#     act  - the row's action, defaulted to "changed" when the row omits it, so
#            no later stage has to special-case a null action.
#     cols - the row's columns object (an empty object when absent).
#
# Why no snapshot explosion: c69baab made the absolute-state *_off queries
# DIFFERENTIAL (filevault_off, remote_access_sharing_state, agent_exposure_changed),
# so an unsafe state now arrives as an ordinary "added" differential row and is
# logged to results.log, which this alerter reads. There is no longer a snapshot
# array to fan out, so normalize emits exactly one finding per row; a snapshot-
# action row is a single finding, carried through unexploded.

# normalize_findings: raw results-log lines on stdin -> normalized finding NDJSON
# on stdout. jq -rR reads each line as a raw string; the per-line try/fromjson
# means one malformed line yields nothing for that line instead of aborting the
# whole batch, and a `2>/dev/null || true` keeps a jq-level hiccup from killing
# the pipeline (a swallowed batch is caught by the cursor-retry logic in main).
normalize_findings() {
  jq -rR '
    # Known-query allowlist (the security select): the prefix-stripped names of
    # every scheduled detector the config-base slice ships, EXCEPT heartbeat_canary.
    # A row whose stripped name is not in this set is dropped before it can become
    # an alert, so an unknown, renamed, or rogue query name never surfaces. This is
    # a STRICT allowlist, not a blanket admit of anything under pack_: a made-up
    # pack_<x> is dropped exactly like an unknown top-level name. heartbeat_canary
    # is a liveness snapshot that lands only in osqueryd.snapshots.log (which this
    # alerter does not read); it is excluded defensively so a stray canary row can
    # never generate noise. Adding a new detector to the config means adding its
    # name here too (the config and this alerter are co-maintained in one repo).
    [
      "new_admin_user", "file_events_recent", "es_launchd_writes",
      "agent_authfile_changed", "agent_binary_changed", "agent_exposure_changed", "agent_secretfile_changed",
      "chrome_extensions", "firefox_addons", "homebrew_packages", "installed_apps", "safari_extensions",
      "kernel_extensions_new", "listening_ports_non_loopback", "persistence_launchd", "persistence_launchd_overrides",
      "persistence_startup_items_crontab", "recent_logins", "suid_bin_unexpected", "system_extensions_new",
      "filevault_off", "filevault_state", "firewall_state", "gatekeeper_state", "remote_access_sharing_state", "sip_state"
    ] as $known
    | . as $line | (try ($line | fromjson) catch empty)
    | select(.name != null)
    # Strip the pack_<pack>_ prefix. [^_]+ matches the pack name (pack names use
    # hyphens, not underscores) up to the first underscore, so only the pack
    # segment is removed and the query keeps its own underscores.
    | (.name | sub("^pack_[^_]+_"; "")) as $q
    | select($q | IN($known[]))
    # renameio exclusion: chezmoi/renameio writes files via an atomic rename
    # through a .renameio-TempDir-* scratch dir, so a file_events_recent row whose
    # target_path is inside one is write churn, not a real change - drop it. Only
    # file_events_recent carries a target_path, so this is a no-op for other rows.
    | select((.columns.target_path // "") | test("/\\.renameio-TempDir") | not)
    # Baseline (counter==0) discard: osquery emits a differential query first full
    # result set with counter 0 (the seeded baseline). Discard those first-run rows
    # so pre-existing state does not page on its first observation - EXCEPT the
    # three absolute-state queries, whose very presence IS an unsafe state (no
    # volume encrypted / a sharing service enabled / an agent port off-loopback),
    # so a counter==0 row there is a first-run PAGE that must be kept, not seeded.
    # An absent counter defaults to 1 (non-baseline), so a row without one is kept.
    | select((.counter // 1) != 0
        or $q == "filevault_off"
        or $q == "remote_access_sharing_state"
        or $q == "agent_exposure_changed")
    | (.action // "changed") as $act
    | {q: $q, act: $act, cols: (.columns // {})} | @json
  ' 2>/dev/null || true
}
