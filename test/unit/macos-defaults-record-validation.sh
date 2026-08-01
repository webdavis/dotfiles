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

# ---- the field-TYPE rule, also asked of the file -------------------------------

# Beside the value rule and for the same reason the value rule exists: the joined
# record line cannot answer this question, because join already turned every
# field into a string by the time the line exists.
#
# What the rule is about is the pair of READERS. This library renders a scalar as
# the TEXT the file spells it with; the runner template renders the same scalar as
# Go formats the value chezmoi's loader parsed. Wherever those differ, one file
# produces two different writes. Measured, and each row below is one of the
# measurements: `scope:` was read here as `user` and PERFORMED while the template
# refused the file; `host: 0` sent this library to -currentHost and the template
# to the ordinary domain, so both readers accepted and wrote to different stores;
# `value: 010` wrote `010` here and `8` there.
#
# write_field_type_fixture / field_type_status mirror the value-rule helpers next
# door, so both rules are exercised through the same shape.
write_field_type_fixture() { # (fixture on stdin)
  cat >"$work/field-types.yaml"
}

field_type_status() {
  local status=0
  defaults_records_declare_agreeing_field_types "$work/field-types.yaml" >"$work/out" 2>"$work/err" || status=$?
  printf '%s' "$status"
}

# refute_field_spelling <record-body> <expected-named-field> <description>, one
# record written the way the row is about; the rule must refuse it AND name the
# field, so an operator is not left to guess which of eight fields to quote.
refute_field_spelling() { # <record-body> <expected-named-field> <description>
  local record_body="$1" named_field="$2" description="$3" status
  printf 'macos:\n  defaults:\n    - {%s}\n  killall: []\n' "$record_body" >"$work/field-types.yaml"
  status="$(field_type_status)"
  [[ $status -eq 2 ]] ||
    fail "$description must be refused with status 2 (got $status, stderr: $(cat "$work/err"))"
  grep -qF "$named_field" "$work/err" ||
    fail "$description was refused without naming the $named_field field (stderr: $(cat "$work/err"))"
  [[ ! -s $work/out ]] ||
    fail "$description printed to stdout; this rule is a predicate and must emit nothing"
}

# require_field_spelling <record-body> <description>, the false-positive
# direction. Every legitimate spelling the tracked file and `just
# defaults-capture` can produce has to keep streaming, or this rule takes the
# machine's settings out rather than keeping the two readers honest.
require_field_spelling() { # <record-body> <description>
  local record_body="$1" description="$2" status
  printf 'macos:\n  defaults:\n    - {%s}\n  killall: []\n' "$record_body" >"$work/field-types.yaml"
  status="$(field_type_status)"
  [[ $status -eq 0 ]] ||
    fail "$description must be accepted (got $status, stderr: $(cat "$work/err"))"
}

VALID_ENFORCE_FIELDS='domain: com.example.zebra, key: ZKey, type: bool, value: "1", tier: enforce'

# The five STRING fields. Anything but a plain string is refused, whichever way
# the two readers happen to disagree about that particular type.
refute_field_spelling "$VALID_ENFORCE_FIELDS, scope: " scope "a scope key typed with its value deleted"
refute_field_spelling "$VALID_ENFORCE_FIELDS, scope: false" scope "a scope written as YAML's own false"
refute_field_spelling "$VALID_ENFORCE_FIELDS, host: 0" host "a host written as an unquoted zero"
refute_field_spelling "$VALID_ENFORCE_FIELDS, host: [a, b]" host "a host written as a sequence"
refute_field_spelling "$VALID_ENFORCE_FIELDS, scope: system, plist_path: {a: 1}" plist_path "a plist_path written as a mapping"
refute_field_spelling 'domain: 0.10, key: ZKey, type: bool, value: "1", tier: enforce' domain "a domain written as an unquoted decimal"
refute_field_spelling 'domain: com.example.zebra, key: 0, type: bool, value: "1", tier: enforce' key "a key written as an unquoted zero"

# The VALUE field, which genuinely holds non-strings. A container never agrees,
# and a scalar agrees only in the canonical spelling Go renders back unchanged.
refute_field_spelling 'domain: com.example.zebra, key: ZKey, type: array, value: [a, b], tier: enforce' value "a value written as a sequence"
refute_field_spelling 'domain: com.example.zebra, key: ZKey, type: dict, value: {a: 1}, tier: enforce' value "a value written as a mapping"
for non_canonical_value in 010 0x1f 1_000 +1 True 1.0 0.10 1.5e10; do
  refute_field_spelling "domain: com.example.zebra, key: ZKey, type: string, value: $non_canonical_value, tier: enforce" \
    value "a value spelled $non_canonical_value, which the two readers render differently"
done

# The accepted half, which is the half that decides whether the rule is
# shippable. A rule that refuses everything satisfies every row above.
require_field_spelling "$VALID_ENFORCE_FIELDS" "a record declaring none of the optional fields"
require_field_spelling "$VALID_ENFORCE_FIELDS, host: \"mac-one\"" "a host written as a quoted string"
require_field_spelling "$VALID_ENFORCE_FIELDS, host: \"0\"" "a host written as a QUOTED zero, which is a string in both readers"
require_field_spelling "$VALID_ENFORCE_FIELDS, scope: user" "a scope written as a plain string"
require_field_spelling 'domain: com.example.zebra, key: ZKey, type: bool, value: "1", tier: enforce, scope: system, plist_path: /Library/Preferences/x.plist' \
  "a plist_path written as an unquoted path, which YAML reads as a string"
for canonical_value in true false 0 42 -7 0.5 -0.5 100.25 '""'; do
  require_field_spelling "domain: com.example.zebra, key: ZKey, type: string, value: $canonical_value, tier: enforce" \
    "a value spelled $canonical_value, which both readers render identically"
done

# The float DIGIT BOUND, both sides of it, because the shape pattern alone lets a
# 17-digit decimal through and `defaults read` prints exactly that for a
# slider-set float control. The runner template renders the shortest decimal that
# reaches the same float64, so the long spelling is written two different ways by
# the two readers while the short one is written the same way by both.
#
# The bound is asserted by SPELLING rather than by digit count so it fails on the
# thing an operator would actually write. `0.34999999999999998` is the measured
# divergence itself; the rows at the bound are what keep it from being tightened
# into refusing an ordinary decimal unnoticed.
refute_field_spelling 'domain: com.example.zebra, key: ZKey, type: float, value: 0.34999999999999998, tier: enforce' \
  value "a float spelled with the 17 digits an IEEE double needs, which the template shortens to 0.35"
refute_field_spelling 'domain: com.example.zebra, key: ZKey, type: float, value: 1234567890123456.7, tier: enforce' \
  value "a float carrying 17 digits either side of the point"
require_field_spelling 'domain: com.example.zebra, key: ZKey, type: float, value: 0.12345678901234, tier: enforce' \
  "a float sitting exactly on the 15-digit bound"
require_field_spelling 'domain: com.example.zebra, key: ZKey, type: float, value: -9999999999999.9, tier: enforce' \
  "a negative float at the digit bound, whose sign is matched outside it"
# One digit past the bound, and pinned as REFUSED on purpose rather than left
# unmentioned. Both readers do render this one identically, so the refusal is the
# bound erring strict: it counts the leading zero of `0.` as a digit, which no
# regular expression can avoid while also counting across the decimal point. The
# cost is a pair of quotes on a 16-digit decimal; pinning it is what stops the
# next reader mistaking the over-strictness for a measurement.
refute_field_spelling 'domain: com.example.zebra, key: ZKey, type: float, value: 0.123456789012345, tier: enforce' \
  value "a float one digit past the bound, which both readers agree on and the bound refuses anyway"
# The INT half of the same question, measured rather than assumed symmetric: an
# integer is rendered back digit for digit past int64, so no width bound belongs
# on it and a rule that grew one would refuse a record both readers agree about.
require_field_spelling 'domain: com.example.zebra, key: ZKey, type: int, value: 9223372036854775808, tier: enforce' \
  "an integer one past int64, which both readers render digit for digit"

# Fields the rule deliberately does NOT judge. `type` and `tier` each sit in a
# closed set of string literals that both readers refuse every non-member of, so
# a type rule here would only take the message away from the check that names the
# set. A field the schema does not know decides nothing, because neither reader
# reads it.
require_field_spelling 'domain: com.example.zebra, key: ZKey, type: bool, value: "1", tier: enforce, notes: [a, b]' \
  "an unknown field carrying a sequence"
require_field_spelling 'domain: com.example.zebra, key: ZKey, tier: manual, runbook: 0' \
  "a runbook written as a number, which both readers read the same way"

# The real tracked data file, which declares a !!bool value on every one of its
# records. The control that says this rule admits the schema as it is actually
# written, rather than as a rule about strings alone would have it.
TRACKED_DATA_FILE="$REPO_ROOT/.chezmoidata/macos_defaults.yaml"
[[ -f $TRACKED_DATA_FILE ]] || fail "missing tracked data file: $TRACKED_DATA_FILE"
tracked_value_tags="$(yq eval -r '[.macos.defaults[].value | tag] | unique | join(" ")' "$TRACKED_DATA_FILE")"
[[ $tracked_value_tags == '!!bool' ]] ||
  fail "the tracked file's values are now tagged [$tracked_value_tags] rather than !!bool alone, so remeasure which spellings this control exercises"
tracked_field_type_status=0
defaults_records_declare_agreeing_field_types "$TRACKED_DATA_FILE" >/dev/null 2>"$work/err" || tracked_field_type_status=$?
[[ $tracked_field_type_status -eq 0 ]] ||
  fail "the tracked data file must pass the field-type rule, got status $tracked_field_type_status ($(cat "$work/err"))"

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

printf 'macos-defaults-record-validation: OK (identity is required on every tier; enforce and verify records need a supported type; the value rule is asked of the file and keeps "" false and 0 while refusing an absent value; the field-type rule refuses every MEASURED spelling the two readers render differently, including a float wider than the digit bound, while admitting the tracked file and every canonical one; scope, host and plist_path rules answer through the same gate; an unknown tier fails closed; the write-time allowlist stays outside it)\n'
