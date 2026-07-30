#!/usr/bin/env bash
# macos-defaults-record-validation.sh -- unit coverage for the per-record gate in
# macos-defaults-lib.sh: validate_defaults_record, the field predicates it
# composes (validate_record_identity, validate_record_type), and the file-level
# value rule beside it (defaults_records_declare_a_value).
#
# The gate exists so that per-record validation can run for the WHOLE file before
# any consumer acts on any record. It is therefore a PREDICATE, not a producer:
# it answers yes or no, prints its reason on stderr, and prints nothing at all on
# stdout. Every case below asserts the empty stdout as well as the status,
# because a caller that read a value off this function's stdout would be handed
# something derived from a record that may have just been refused.
#
# The properties pinned here:
#
#   - Identity is required on every record, whatever its tier. An empty domain or
#     an empty key is REFUSED, never skipped. apply and drift both used to
#     `continue` past an empty domain, which turned a malformed record into a
#     silent no-op and let the run exit 0 having applied only part of the file.
#   - enforce and verify records must declare a type from the closed set the
#     runner template constrains .type to. A record with none reached
#     `defaults write dom key - true` before this gate existed.
#   - manual records carry no write payload, so only identity applies to them.
#   - The VALUE rule is asked of the FILE, not of the record, because the record
#     stream renders an absent value and a legitimately empty one identically.
#     defaults_records_declare_a_value refuses an absent or null value on an
#     enforce or verify record while leaving "", false and 0 alone, which is what
#     keeps this reader refusing exactly what the runner template refuses.
#   - The scope, host and plist_path rules the tools already had are reached
#     THROUGH this gate, so a caller gets one answer for the whole record rather
#     than discovering the next problem one write later.
#   - An unrecognized tier is refused here too. The record stream gates the tier
#     before this is ever called; the arm keeps the gate from failing OPEN into
#     "no payload rules apply" if that ever changes.
#
# Mostly pure bash: every record-level function under test is a predicate over
# eight strings. The file-level value rule needs real yq, which is cheap enough
# for this camp (the count-guard unit suite uses it the same way). No chezmoi and
# no `defaults` anywhere.
set -euo pipefail

# Scrubbed at SCRIPT scope. A linked worktree exports GIT_DIR to the hooks it
# runs, and the library's own override may be exported on a developer machine.
unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/dot_local/bin/macos-defaults-lib.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -f $LIB ]] || fail "missing library: $LIB"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# shellcheck source=/dev/null
source "$LIB"

# The eight fields, in the order the record stream emits them. Named here once so
# every case below reads as a record rather than as eight anonymous arguments.
#   domain key type value host scope plist_path tier

# accept_record <label> <domain> <key> <type> <value> <host> <scope> <plist_path> <tier>
accept_record() {
  local label="$1"
  shift
  local status=0 output
  output="$(validate_defaults_record "$@" 2>"$work/err")" || status=$?
  [[ $status -eq 0 ]] ||
    fail "$label: a valid record must be accepted (got $status, stderr: $(cat "$work/err"))"
  [[ -z $output ]] ||
    fail "$label: the gate is a predicate and must print nothing on stdout (got '$output')"
}

# reject_record <label> <stderr-fragment> <domain> <key> <type> <value> <host> <scope> <plist_path> <tier>
reject_record() {
  local label="$1" expected_fragment="$2"
  shift 2
  local status=0 output
  output="$(validate_defaults_record "$@" 2>"$work/err")" || status=$?
  [[ $status -ne 0 ]] ||
    fail "$label: the record must be refused (got 0, stdout: '$output')"
  [[ -z $output ]] ||
    fail "$label: a refused record must print nothing on stdout (got '$output')"
  grep -qF -- "$expected_fragment" "$work/err" ||
    fail "$label: the refusal must say '$expected_fragment' (stderr: $(cat "$work/err"))"
}

# ---- controls: valid records of every tier are accepted -----------------------

# Without these rows every rejection below is satisfied by a gate that refuses
# unconditionally, which would refuse the tracked data file too.
accept_record 'user-scope enforce record' \
  com.example.user UserKey bool true '' user '' enforce
accept_record 'ByHost enforce record' \
  com.example.byhost ByHostKey int 3 current user '' enforce
accept_record 'system-scope enforce record, default path' \
  com.example.sys SysKey bool false '' system '' enforce
accept_record 'system-scope enforce record, explicit path' \
  com.example.lulu LuLuKey string block '' system /Library/Objective-See/LuLu/preferences.plist enforce
accept_record 'verify record' \
  com.example.verify VerifyKey bool true '' user '' verify
# A manual record streams with an empty type, value, host and plist_path: it
# carries a runbook pointer and nothing else. Only identity applies.
accept_record 'manual record carries no payload' \
  com.example.manual ManualKey '' '' '' user '' manual

# Every member of the supported-type set is accepted, asserted by enumeration
# rather than by a count: a count matches just as happily when one member has
# been swapped for a duplicate.
for supported_type in "${MACOS_DEFAULTS_SUPPORTED_TYPES[@]}"; do
  accept_record "supported type $supported_type" \
    com.example.types TypeKey "$supported_type" somevalue '' user '' enforce
done
[[ ${#MACOS_DEFAULTS_SUPPORTED_TYPES[@]} -gt 0 ]] ||
  fail 'the supported-type set must not be empty; an empty loop above would assert nothing'

# ---- identity is required on every record, whatever its tier ------------------

reject_record 'blank domain, enforce' 'blank domain' \
  '' OrphanKey bool true '' user '' enforce
reject_record 'blank key, enforce' 'blank key' \
  com.example.nokey '' bool true '' user '' enforce
# The tier does not exempt a record from having a name: a manual record with no
# domain names no control either.
reject_record 'blank domain, manual' 'blank domain' \
  '' ManualKey '' '' '' user '' manual
reject_record 'blank domain, verify' 'blank domain' \
  '' VerifyKey bool true '' user '' verify

# ---- enforce and verify records must declare a usable type ---------------------

reject_record 'blank type' 'unsupported type' \
  com.example.notype NoTypeKey '' true '' user '' enforce
reject_record 'unsupported type' 'unsupported type' \
  com.example.badtype BadTypeKey bogus true '' user '' enforce
# The type is rendered as a bare option word by both readers, so a value that
# smuggles shell syntax through it must be refused rather than quoted.
reject_record 'injecting type' 'unsupported type' \
  com.example.injecttype InjectTypeKey 'bool true; touch /tmp/pwned #' true '' user '' enforce
# A verify record is compared, not written, but it is compared against the value
# it declares, so the same rule applies to it.
reject_record 'blank type on a verify record' 'unsupported type' \
  com.example.noverifytype NoVerifyTypeKey '' true '' user '' verify

# The VALUE rule is deliberately absent from this gate, and this row says so out
# loud rather than leaving its absence to look like an oversight. The record
# stream renders an absent value and a legitimately empty one identically, so the
# gate cannot tell them apart and must not guess; the question is asked of the
# FILE instead, by defaults_records_declare_a_value, exercised below.
accept_record 'an empty value is not this gate to judge' \
  com.example.emptyvalue EmptyValueKey string '' '' user '' enforce

# ---- the value rule, asked of the file -----------------------------------------

# write_value_fixture -- store the fixture on stdin as the file under test.
# Kept separate from the call below so no heredoc ever sits inside a command
# substitution: bash accepts that shape but warns about an unterminated
# here-document on every one, and a suite that prints warnings trains its reader
# to skim them.
write_value_fixture() { # (fixture on stdin)
  cat >"$work/values.yaml"
}

# declared_value_status -- run defaults_records_declare_a_value over the stored
# fixture and print its status. stdout and stderr are captured, so a rule that
# started printing records would be visible rather than mixed into the run.
declared_value_status() {
  local status=0
  defaults_records_declare_a_value "$work/values.yaml" >"$work/out" 2>"$work/err" || status=$?
  printf '%s' "$status"
}

# An absent value and an explicit null are the same record error and both are
# refused, naming the record so it can be found in the file.
write_value_fixture <<'EOF'
macos:
  defaults:
    - {domain: com.example.ok, key: OkKey, type: bool, value: true, tier: enforce}
    - {domain: com.example.novalue, key: NoValueKey, type: bool, tier: enforce}
EOF
value_status="$(declared_value_status)"
[[ $value_status -eq 2 ]] ||
  fail "an absent value must be refused with status 2 (got $value_status, stderr: $(cat "$work/err"))"
grep -qF 'com.example.novalue' "$work/err" ||
  fail "the refusal must name the record with no value (stderr: $(cat "$work/err"))"
grep -qF 'blank value' "$work/err" ||
  fail "the refusal must say the value is the problem (stderr: $(cat "$work/err"))"

write_value_fixture <<'EOF'
macos:
  defaults:
    - {domain: com.example.nullvalue, key: NullValueKey, type: bool, value: null, tier: verify}
EOF
value_status="$(declared_value_status)"
[[ $value_status -eq 2 ]] ||
  fail "an explicitly null value must be refused with status 2 (got $value_status)"

# The distinctions this rule must NOT lose. An empty STRING is a legitimate
# value, which the runner template renders; refusing it here would make one YAML
# render cleanly and then make every tool refuse it. A `false` value is the most
# common one in the tracked file, and it is exactly what a truthiness test eats.
write_value_fixture <<'EOF'
macos:
  defaults:
    - {domain: com.example.emptystring, key: EmptyStringKey, type: string, value: "", tier: enforce}
    - {domain: com.example.false, key: FalseKey, type: bool, value: false, tier: enforce}
    - {domain: com.example.zero, key: ZeroKey, type: int, value: 0, tier: verify}
EOF
value_status="$(declared_value_status)"
[[ $value_status -eq 0 ]] ||
  fail "an empty string, a false and a zero are values, not missing ones (got $value_status, stderr: $(cat "$work/err"))"

# A manual record carries a runbook pointer and no payload, so it is not asked.
write_value_fixture <<'EOF'
macos:
  defaults:
    - {domain: com.example.manual, key: ManualKey, tier: manual, runbook: Some section}
EOF
value_status="$(declared_value_status)"
[[ $value_status -eq 0 ]] ||
  fail "a manual record must not be asked for a value (got $value_status, stderr: $(cat "$work/err"))"

write_value_fixture <<'EOF'
macos:
  defaults: []
EOF
value_status="$(declared_value_status)"
[[ $value_status -eq 0 ]] ||
  fail "an empty record list must pass the value rule (got $value_status, stderr: $(cat "$work/err"))"

# ---- the scope, host and plist_path rules are reached through the gate --------

reject_record 'unknown scope' 'unknown scope' \
  com.example.bogus BogusKey bool true '' bogus '' enforce
reject_record 'set-but-empty scope' 'unknown scope' \
  com.example.emptyscope EmptyScopeKey bool true '' '' '' enforce
reject_record 'system scope with a host' 'ByHost storage is per-user' \
  com.example.syshost SysHostKey bool true current system '' enforce
reject_record 'user scope with a plist_path' 'only honored on scope system' \
  com.example.userpath UserPathKey bool true '' user /Library/Preferences/x.plist enforce
reject_record 'system-scope traversal domain' 'contains a slash' \
  '../../tmp/owned' OwnedKey bool true '' system '' enforce
reject_record 'system-scope relative plist_path' 'absolute path is required' \
  com.example.rel RelKey bool true '' system 'Library/Preferences/rel.plist' enforce
reject_record 'system-scope parent-directory plist_path' 'parent-directory component' \
  com.example.parent ParentKey bool true '' system '/Library/Preferences/../../etc/x.plist' enforce

# ---- an unrecognized tier fails closed ----------------------------------------

# The record stream refuses an unknown tier before this is ever called. The arm
# is here so that if that ever changes, the gate refuses rather than falling
# through to "no payload rules apply to this tier".
reject_record 'unrecognized tier' 'tier' \
  com.example.mystery MysteryKey bool true '' user '' mystery

# ---- the write-time allowlist is deliberately NOT part of this gate -----------

# drift reads records it must never write, and refusing an odd path in the shared
# gate would hide the row from the drift report instead of reporting it. The
# allowlist stays apply's own check; assert the split so a later "tidy-up" that
# folds it in has to turn this red first.
accept_record 'out-of-allowlist path passes the shared gate' \
  com.example.evil EvilKey bool true '' system /etc/example.evil.plist enforce
allowlist_status=0
require_system_plist_path_permitted /etc/example.evil.plist 2>"$work/err" || allowlist_status=$?
[[ $allowlist_status -ne 0 ]] ||
  fail 'the write-time allowlist must still refuse /etc/example.evil.plist'

printf 'macos-defaults-record-validation: OK (identity is required on every tier; enforce and verify records need a supported type; the value rule is asked of the file and keeps "" false and 0 while refusing an absent value; scope, host and plist_path rules answer through the same gate; an unknown tier fails closed; the write-time allowlist stays outside it)\n'
