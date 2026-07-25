#!/usr/bin/env bash
#
# digest_append (results-alerter/digest-store.sh) records a non-paging finding as
# one NDJSON line in the digest spool, the private local file that accumulates
# suspicious-but-ambiguous findings between daily digest sends. It is best-effort:
# a write failure is swallowed so it never aborts the detection path, and the
# read side (the digest-tier slice) is a later slice - B8 only appends.
#
# Privacy posture (F5-A lesson): the line carries only derived triage fields -
# timestamp, detector, category, identity, action, summary. It NEVER copies the
# whole columns object, so no raw sha256 and no secret column reaches the spool.
# Full filesystem paths are stored (this is a single-user private spool, dir 700
# / file 600), but a secret VALUE or a content hash is not.
#
# Unit test: append to a temp spool, then assert the line shape, the accumulation,
# the 700/600 modes, and the absence of any secret/sha256 leak.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/digest-store.sh"

fail() {
  printf 'osquery-digest-store: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $HELPER ]] || fail "missing helper: $HELPER"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
store="$work/state/osquery-digest-spool/digest.ndjson"
dir="$(dirname "$store")"

# perms_of <path> -> octal permission bits. GNU stat -c first (Linux CI), BSD
# stat -f as the fallback (macOS), per the test-suite's stat-portability guard.
perms_of() { stat -c '%a' "$1" 2>/dev/null || stat -f '%Lp' "$1"; }

# Three findings appended in ONE sourcing subshell (accumulation): a system
# extension (identity from .identifier), a listening port (the special-case
# "name address:port" identity), and an agent authfile change whose columns carry
# a sha256 AND a secret value that must NOT reach the spool.
OSQUERY_DIGEST_STORE="$store" bash -c '
  source "$1"
  digest_append '\''{"q":"system_extensions_new","act":"added","cols":{"identifier":"com.example.ext","team":"TEAMID"},"ep":""}'\''
  digest_append '\''{"q":"listening_ports_non_loopback","act":"added","cols":{"name":"nc","address":"0.0.0.0","port":"4444"},"ep":""}'\''
  digest_append '\''{"q":"agent_authfile_changed","act":"added","cols":{"path":"/Users/x/.codex/config.toml","sha256":"deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef","secret_value":"SUPERSECRETTOKEN"},"ep":""}'\''
' _ "$HELPER"

# -- accumulation: three appends -> three lines --
[[ -f $store ]] || fail "the spool file was not created"
line_count="$(grep -c . "$store" || true)"
[[ $line_count -eq 3 ]] || fail "expected 3 accumulated NDJSON lines, got $line_count"

# -- private modes: dir 700, file 600 --
dir_perms="$(perms_of "$dir")"
file_perms="$(perms_of "$store")"
[[ $dir_perms == 700 ]] || fail "the spool dir must be 700 (private), got $dir_perms"
[[ $file_perms == 600 ]] || fail "the spool file must be 600 (private), got $file_perms"

mapfile -t lines <"$store"

# -- line 1: system extension, derived fields --
l1="${lines[0]}"
[[ "$(jq -r '.detector' <<<"$l1")" == system_extensions_new ]] || fail "line 1 detector wrong: $l1"
[[ "$(jq -r '.identity' <<<"$l1")" == com.example.ext ]] || fail "line 1 identity should be the extension identifier: $l1"
[[ "$(jq -r '.action' <<<"$l1")" == added ]] || fail "line 1 action wrong: $l1"
[[ "$(jq -r '.summary' <<<"$l1")" == "system_extensions_new com.example.ext" ]] || fail "line 1 summary wrong: $l1"
[[ "$(jq -r '.timestamp' <<<"$l1")" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]] || fail "line 1 timestamp is not ISO 8601 UTC: $l1"

# -- line 2: listening port special-case identity "name address:port" --
l2="${lines[1]}"
[[ "$(jq -r '.identity' <<<"$l2")" == "nc 0.0.0.0:4444" ]] || fail "line 2 identity should be 'name address:port': $l2"

# -- line 3: privacy posture. The secret value and the raw sha256 must NOT appear,
#    and the line must carry no sha256 key. The full path IS allowed. --
l3="${lines[2]}"
[[ "$(jq -r '.identity' <<<"$l3")" == "/Users/x/.codex/config.toml" ]] || fail "line 3 identity should be the path: $l3"
jq -e 'has("sha256")' <<<"$l3" >/dev/null 2>&1 && fail "the digest line must not carry a sha256 field: $l3"
[[ $l3 != *deadbeef* ]] || fail "a raw sha256 leaked into the digest line: $l3"
[[ $l3 != *SUPERSECRETTOKEN* ]] || fail "a secret value leaked into the digest line: $l3"

# -- whole-file sweep: no secret/hash anywhere in the spool --
grep -q 'deadbeef' "$store" && fail "a raw sha256 leaked into the spool file"
grep -q 'SUPERSECRETTOKEN' "$store" && fail "a secret value leaked into the spool file"

# -- A FAILED append stays best-effort but must not be silent. The append is
#    deliberately swallowed so a spool problem can never abort the detection path,
#    which means the finding is recorded nowhere and is never retried. That loss
#    has to leave a trace in the alerter's launchd log, or a digest-tier finding
#    disappears with no signal at all. jq is shimmed to fail, which is the silent
#    shape: the helper's own `2>/dev/null` hides jq's complaint, so the ONLY thing
#    that can reach the log is the helper's own diagnostic. --
fail_work="$work/append-failure"
mkdir -p "$fail_work/bin"
fail_store="$fail_work/spool/digest.ndjson"
cat >"$fail_work/bin/jq" <<'SHIM'
#!/usr/bin/env bash
printf 'jq shim: simulated digest append failure\n' >&2
exit 5
SHIM
chmod +x "$fail_work/bin/jq"

fail_status=0
fail_err="$(
  OSQUERY_DIGEST_STORE="$fail_store" PATH="$fail_work/bin:$PATH" bash -c '
    source "$1"
    digest_append "{\"q\":\"system_extensions_new\",\"act\":\"added\",\"cols\":{\"identifier\":\"com.example.ext\"},\"ep\":\"\"}"
    printf "APPEND_RC=%s\n" "$?" >&2
  ' _ "$HELPER" 2>&1 >/dev/null
)" || fail_status=$?

[[ $fail_status -eq 0 ]] ||
  fail "a failed append must never abort the caller, the pass exited $fail_status ($fail_err)"
[[ $fail_err == *"APPEND_RC=0"* ]] ||
  fail "digest_append must still return 0 on a failed append (best-effort), got: $fail_err"
[[ $fail_err == *digest-store* ]] ||
  fail "a failed append must announce itself on stderr; got: ${fail_err:-<nothing>}"
[[ $fail_err == *"$fail_store"* ]] ||
  fail "the diagnostic must name the spool it could not write, got: $fail_err"
if [[ -s $fail_store ]]; then
  fail "the failed append must not have written a line, got: $(cat "$fail_store")"
fi

printf 'osquery-digest-store: OK (one NDJSON line per append, accumulation, dir 700 / file 600, listening-port identity, no secret/sha256 leak, failed append stays best-effort but is logged)\n'
