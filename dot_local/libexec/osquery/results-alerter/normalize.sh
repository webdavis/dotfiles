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
    . as $line | (try ($line | fromjson) catch empty)
    | select(.name != null)
    # Strip the pack_<pack>_ prefix. [^_]+ matches the pack name (pack names use
    # hyphens, not underscores) up to the first underscore, so only the pack
    # segment is removed and the query keeps its own underscores.
    | (.name | sub("^pack_[^_]+_"; "")) as $q
    | (.action // "changed") as $act
    | {q: $q, act: $act, cols: (.columns // {})} | @json
  ' 2>/dev/null || true
}
