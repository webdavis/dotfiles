#!/usr/bin/env bash
#
# Every chezmoi source file that pulls from the secret vault must DECLARE target
# mode 0600.
#
# WHY THIS FILE EXISTS. chezmoi decides a target file's mode from its SOURCE
# BASENAME: no attribute gives 0644, a `private_` attribute gives 0600. It
# ENFORCES that decision on every apply instead of preserving what is on disk.
# Measured against chezmoi v2.71.1 in a scratch source/destination pair: a
# target hand-tightened to 0600 whose source carries no `private_` reports
# `old mode 100600 / new mode 100644` under `chezmoi diff`, `MM` under
# `chezmoi status`, and comes back 0644 after `chezmoi apply`.
#
# So a secret-bearing source without `private_` is a pending WIDENING, not a
# missing hardening, and `stat` on the live target says NOTHING about it: two of
# the four files this guard was written for were already 0600 on the authoring
# machine, tightened by hand or by their application, and the next apply would
# have re-opened them. That is the whole reason the check keys on the declared
# mode rather than the observed one.
#
# WHAT IT DOES AND DOES NOT PROMISE. It answers one question honestly: does
# every source file that reads the secret vault, or that ships an encrypted
# payload, declare 0600? It keys on the MECHANISM (a vault call, an `encrypted_`
# attribute), not on whether a file holds sensitive data. A hand-written home
# address, an inline literal, or a value read from the environment is invisible
# to it. The mechanism rule is what can be enforced without judgement; the
# mechanism list is a named constant so adding one is a one-line change.
#
# Behaviors pinned:
#   S1 attribute chain   - which source basenames DECLARE private, parsed in
#                          chezmoi's order. Order is load-bearing and measured:
#                          `private_executable_dot_x` renders 0700, while
#                          `executable_private_dot_x` is not parsed at all and
#                          lands as a literal file named `private_dot_x` at
#                          0755. A membership test that merely looked for
#                          `private_` anywhere would vouch for the second.
#   S2 vault calls       - a call inside a live `{{ }}` action counts; the same
#                          name inside a Go-template comment or in ordinary
#                          prose does not. That exemption is the guard's brake
#                          against its own mirror defect (a check so strict it
#                          demands `private_` on an ordinary 0644 template).
#   S3 the grammar       - S1 encodes chezmoi's parsing rules, so chezmoi is
#                          asked to confirm them on every run rather than once
#                          by hand. Without this the rules rot silently on the
#                          next chezmoi upgrade, in the fail-open direction.
#   S4 the tree          - the actual guard, over chezmoi's own list of source
#                          files that become target FILES.
#   S5 the allowlist     - each exemption names a specific CALL, not a file, and
#                          must still be live. A file allowlisted for fetching
#                          one public value cannot quietly grow a second call.
#
# RUNTIME. Three chezmoi invocations (roughly 250ms of the total) put this over
# the unit suite's 200ms WARN threshold. They are the ground truth this guard is
# built on, not incidental work: two ask chezmoi to confirm the naming grammar
# S1 encodes, one asks it which source files become target files. Everything
# cheaper was already removed (the walk forks no subprocess per file). The
# warning is advisory and never fails a run.
#
# RELATIONSHIP TO scripts/render-coverage-classifier.nix. That classifier also
# detects keepassxc calls, quote-aware and resolving `includeTemplate` partials,
# to decide which templates the rendered-shellcheck formatter may render. The
# two are NOT mirrors and are not asserted to agree: its universe is
# `.chezmoiscripts/*.sh.tmpl` plus root shell `dot_*.tmpl` (34 scripts plus
# `dot_bashrc.tmpl` as of this writing), and chezmoi scripts are never target
# files, so they carry no mode to widen. This guard's universe is target files
# only. Measured at authoring time, the FLAGGED sets are disjoint: no
# `.chezmoiscripts/` entry is a managed file, and `dot_bashrc.tmpl` makes no
# vault call. Two divergences in this file's cheaper scanner, stated rather than
# claimed away: it does not resolve `includeTemplate` partials, and it is not
# quote-aware, so a vault name inside a string literal in a live action reads as
# a call. The second fails toward demanding `private_`; the first would not, so
# S4 re-checks both conditions that make it harmless here.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# ---------- named constants --------------------------------------------------

# The secret MECHANISMS this guard knows about: template functions that read the
# KeePassXC vault at apply time. Add a function here when a new one is adopted.
SECRET_VAULT_TEMPLATE_FUNCTIONS=(keepassxc keepassxcAttribute)

# chezmoi source-state attribute tokens, from `chezmoi help chattr` (v2.71.1)
# plus the script attributes that command does not list. Used only to decide
# where a basename's leading attribute chain ENDS: `dot_` is a name
# substitution, not an attribute, so a chain stops there.
CHEZMOI_SOURCE_ATTRIBUTES=(
  after before create empty encrypted exact executable external literal
  modify once onchange private readonly remove run symlink
)

# The attributes chezmoi accepts BEFORE `private_` while still parsing it as an
# attribute. Measured, not assumed: `create_private_`, `encrypted_private_` and
# `modify_private_` all render 0600, while `executable_`, `readonly_`, `empty_`,
# `literal_` and `external_` before `private_` leave the name unparsed. S3 asks
# chezmoi to re-confirm this list on every run.
CHEZMOI_ATTRIBUTES_ALLOWED_BEFORE_PRIVATE=(create encrypted modify)

PRIVATE_ATTRIBUTE=private
ENCRYPTED_ATTRIBUTE=encrypted
MODIFY_ATTRIBUTE=modify

CHEZMOI_TEMPLATE_SUFFIX=.tmpl
CHEZMOI_MODIFY_TEMPLATE_DIRECTIVE='chezmoi:modify-template'
INCLUDE_TEMPLATE_FUNCTION=includeTemplate

# Exemptions, keyed by source-relative path and naming the exact vault call that
# is exempt (whitespace-normalized). Allowlisting a FILE would let a new secret
# ride in beside the public value; allowlisting a CALL does not.
#
# dot_gitconfig.tmpl: its one vault call fetches a GPG (GNU Privacy Guard)
# signing subkey ID, which is public by construction, published with the key and
# carried in every signed commit. 0644 is also correct for ~/.gitconfig, which
# tooling in other trust domains reads. Renaming it would additionally break
# test/unit/gitconfig-tool-and-url-hygiene.sh, which names the path directly.
declare -A PUBLIC_VALUE_VAULT_CALL_ALLOWLIST=(
  ["dot_gitconfig.tmpl"]='keepassxcAttribute "GitHub (Webdavis) :: GPG :: Signing key" "Public Signing Subkey ID"'
)

# ---------- harness ----------------------------------------------------------

failures=0

fail() {
  printf 'secret-source-files-declare-private-mode: FAIL -- %s\n' "$*" >&2
  failures=$((failures + 1))
}

assert_predicate() { # <expect-true|expect-false> <label> <function> <args...>
  local expectation=$1 label=$2
  shift 2
  if "$@"; then
    [[ $expectation == expect-true ]] ||
      fail "$label: expected FALSE, got true -- $*"
  else
    [[ $expectation == expect-false ]] ||
      fail "$label: expected TRUE, got false -- $*"
  fi
}

assert_equal() { # <expected> <actual> <label>
  [[ $1 == "$2" ]] ||
    fail "$3: expected $(printf '%q' "$1"), got $(printf '%q' "$2")"
}

work="$(mktemp -d)"
# `trash` is the operator's rule for interactive removals; a committed test has
# to run on a bare CI runner, where only coreutils exist.
trap 'rm -rf "$work"' EXIT

# ---------- pure predicates --------------------------------------------------

is_chezmoi_source_attribute() { # <token>
  local candidate
  for candidate in "${CHEZMOI_SOURCE_ATTRIBUTES[@]}"; do
    [[ $1 == "$candidate" ]] && return 0
  done
  return 1
}

# The leading attribute chain of a source basename, in the order chezmoi reads
# it, published in CHEZMOI_SOURCE_ATTRIBUTE_CHAIN. The chain stops at the first
# token that is not an attribute, which is what keeps `dot_private_foo` (target
# `.private_foo`, mode 0644) from reading as private.
#
# It writes a global instead of printing, so the callers below stay fork-free:
# the tree walk asks these questions several times for each of 148 managed
# files, and a subshell per question dominated this guard's runtime.
declare -a CHEZMOI_SOURCE_ATTRIBUTE_CHAIN=()
read_chezmoi_source_attribute_chain() { # <basename>
  local rest=$1 token
  CHEZMOI_SOURCE_ATTRIBUTE_CHAIN=()
  while [[ $rest == *_* ]]; do
    token=${rest%%_*}
    is_chezmoi_source_attribute "$token" || break
    CHEZMOI_SOURCE_ATTRIBUTE_CHAIN+=("$token")
    rest=${rest#*_}
  done
}

source_basename_has_attribute() { # <basename> <attribute>
  local token
  read_chezmoi_source_attribute_chain "$1"
  for token in ${CHEZMOI_SOURCE_ATTRIBUTE_CHAIN[@]+"${CHEZMOI_SOURCE_ATTRIBUTE_CHAIN[@]}"}; do
    [[ $token == "$2" ]] && return 0
  done
  return 1
}

# Does this basename declare target mode 0600? True only when `private` is in
# the chain AND every token before it is one chezmoi accepts there. Anything
# else answers FALSE, which is the safe direction: an unrecognized ordering
# produces a loud demand for a rename, never a silent pass on a file chezmoi
# renders 0644.
source_basename_declares_private_mode() { # <basename>
  local token allowed is_allowed
  read_chezmoi_source_attribute_chain "$1"
  for token in ${CHEZMOI_SOURCE_ATTRIBUTE_CHAIN[@]+"${CHEZMOI_SOURCE_ATTRIBUTE_CHAIN[@]}"}; do
    [[ $token == "$PRIVATE_ATTRIBUTE" ]] && return 0
    is_allowed=0
    for allowed in "${CHEZMOI_ATTRIBUTES_ALLOWED_BEFORE_PRIVATE[@]}"; do
      [[ $token == "$allowed" ]] && {
        is_allowed=1
        break
      }
    done
    ((is_allowed == 1)) || return 1
  done
  return 1
}

# Does chezmoi expand `{{ }}` in this entry? Two shapes: the `.tmpl` suffix, and
# a `modify_` entry whose first line carries chezmoi's modify-template
# directive. That directive is not optional decoration: a `modify_` entry
# without it is EXECUTED as a script and its `{{ }}` are never expanded.
source_file_is_chezmoi_template() { # <absolute-path> <basename>
  local first_line
  [[ $2 == *"$CHEZMOI_TEMPLATE_SUFFIX" ]] && return 0
  source_basename_has_attribute "$2" "$MODIFY_ATTRIBUTE" || return 1
  IFS= read -r first_line <"$1" || return 1
  [[ $first_line == *"$CHEZMOI_MODIFY_TEMPLATE_DIRECTIVE"* ]]
}

# Every live `{{ }}` action naming a vault function, whitespace-normalized, one
# per line. Comment actions are skipped whole, including their contents, so a
# `}}` written inside a comment cannot end the action early and hide what
# follows it.
template_vault_calls() { # <path>
  awk -v wanted_names="${SECRET_VAULT_TEMPLATE_FUNCTIONS[*]}" '
    BEGIN { split(wanted_names, wanted, " ") }
    { text = text $0 "\n" }
    END {
      pos = 1
      while (1) {
        opening = index(substr(text, pos), "{{")
        if (opening == 0) break
        opening = pos + opening - 1
        body_start = opening + 2

        probe = body_start
        if (substr(text, probe, 1) == "-") probe++
        while (substr(text, probe, 1) ~ /^[ \t\n]$/) probe++

        if (substr(text, probe, 2) == "/*") {
          comment_end = index(substr(text, probe + 2), "*/")
          if (comment_end == 0) break
          after_comment = probe + 2 + comment_end + 1
          closing = index(substr(text, after_comment), "}}")
          if (closing == 0) break
          pos = after_comment + closing + 1
          continue
        }

        closing = index(substr(text, body_start), "}}")
        if (closing == 0) break
        body = substr(text, body_start, closing - 1)
        pos = body_start + closing + 1

        for (i in wanted) {
          if (wanted[i] == "") continue
          if (body ~ ("(^|[^A-Za-z0-9_])" wanted[i] "([^A-Za-z0-9_]|$)")) {
            gsub(/^[- \t\n]+|[- \t\n]+$/, "", body)
            gsub(/[ \t\n]+/, " ", body)
            print body
            break
          }
        }
      }
    }
  ' "$1"
}

source_file_pulls_secrets() { # <absolute-path> <basename>
  source_basename_has_attribute "$2" "$ENCRYPTED_ATTRIBUTE" && return 0
  source_file_is_chezmoi_template "$1" "$2" || return 1
  [[ -n $(template_vault_calls "$1") ]]
}

# ---------- S1: which basenames declare private mode -------------------------

for fixture_basename in \
  private_credentials.tmpl \
  private_config.toml.tmpl \
  private_identity.yml.tmpl \
  private_dot_foo \
  create_private_dot_foo \
  encrypted_private_config.yaml.age \
  modify_private_dot_claude.json \
  modify_private_claude_desktop_config.json; do
  assert_predicate expect-true "s1-declares-private-$fixture_basename" \
    source_basename_declares_private_mode "$fixture_basename"
done

# The four this guard was written for, as they were named before their rename,
# plus the shapes that must stay legitimately 0644.
for fixture_basename in \
  credentials.tmpl \
  config.toml.tmpl \
  identity.yml.tmpl \
  dot_gitconfig.tmpl \
  dot_bashrc.tmpl \
  modify_settings.json \
  _pqi.yml \
  base.yml; do
  assert_predicate expect-false "s1-declares-public-$fixture_basename" \
    source_basename_declares_private_mode "$fixture_basename"
done

# ORDER. Each of these carries the literal string `private_` and none of them
# renders 0600; a membership test would pass every one.
for fixture_basename in \
  executable_private_dot_foo \
  readonly_private_dot_foo \
  empty_private_dot_foo \
  literal_private_dot_foo \
  external_private_dot_foo; do
  assert_predicate expect-false "s1-misordered-$fixture_basename" \
    source_basename_declares_private_mode "$fixture_basename"
done

# `private` after `dot_` is part of the NAME, not an attribute: the target is
# literally `.private_foo`, at 0644.
assert_predicate expect-false s1-private-inside-the-name \
  source_basename_declares_private_mode dot_private_foo
# A word merely beginning with the attribute is not the attribute.
assert_predicate expect-false s1-attribute-prefix-is-not-the-attribute \
  source_basename_declares_private_mode notprivate_dot_foo

read_chezmoi_source_attribute_chain modify_private_dot_claude.json
assert_equal "modify private" "${CHEZMOI_SOURCE_ATTRIBUTE_CHAIN[*]}" s1-chain-order
read_chezmoi_source_attribute_chain dot_gitconfig.tmpl
assert_equal "" "${CHEZMOI_SOURCE_ATTRIBUTE_CHAIN[*]-}" s1-chain-stops-at-dot
assert_predicate expect-true s1-has-encrypted-attribute \
  source_basename_has_attribute encrypted_private_config.yaml.age "$ENCRYPTED_ATTRIBUTE"
assert_predicate expect-false s1-encrypted-inside-the-name \
  source_basename_has_attribute dot_encrypted_notes.txt "$ENCRYPTED_ATTRIBUTE"

# ---------- S2: a call, a comment and prose are three different things -------

vault_call_fixture() { # <label> <expected-count> <body>
  local fixture="$work/fixture-$1"
  printf '%s' "$3" >"$fixture"
  assert_equal "$2" "$(template_vault_calls "$fixture" | grep -c . || true)" "s2-$1"
}

vault_call_fixture live-call 1 '[x]
key = {{ (keepassxc "Some :: Entry").Password }}
'
vault_call_fixture live-attribute-call 1 'signingkey = {{ keepassxcAttribute "E" "A" }}
'
vault_call_fixture two-live-calls 2 'a = {{ (keepassxc "E").UserName }}
b = {{ (keepassxc "E").Password }}
'
# A multi-line action is one action.
vault_call_fixture multi-line-action 1 'a = {{ (keepassxc
  "Some :: Entry").Password }}
'
# The brake against the mirror defect: documentation is not a call.
vault_call_fixture go-template-comment 0 '{{/* keepassxc is described here */}}
plain = 1
'
vault_call_fixture trimmed-comment 0 '{{- /* keepassxc, described */ -}}
plain = 1
'
# A `}}` INSIDE a comment must not end the action early; if it did, the text
# after it would be rescanned and a later real call could be missed.
vault_call_fixture comment-holding-closing-delimiter 0 '{{/* see {{ keepassxc }} here */}}
plain = 1
'
vault_call_fixture comment-then-real-call 1 '{{/* see {{ keepassxc }} here */}}
key = {{ (keepassxc "E").Password }}
'
# Prose outside any action, which is how the atuin config documents where its
# regex lives. Both the exact spelling and the product spelling stay clear.
vault_call_fixture prose-outside-an-action 0 '## Regex lives in KeePassXC, same entry as before.
## The keepassxc entry name is recorded in the design spec.
plain = 1
'
vault_call_fixture word-boundary 0 'x = {{ mykeepassxcHelper }}
y = {{ keepassxcAttributes2 }}
'
printf 'k = {{ keepassxcAttribute "E" "A" }}\n' >"$work/fixture-call-text"
assert_equal 'keepassxcAttribute "E" "A"' \
  "$(template_vault_calls "$work/fixture-call-text")" s2-call-text-normalized

# ---------- S3: chezmoi confirms the grammar S1 encodes ----------------------
# S1's ordering rules came from measurement, so they are re-measured here rather
# than trusted. Without this they rot silently on the next chezmoi upgrade, and
# they rot toward vouching for a file chezmoi renders 0644. The empty config
# keeps both probes independent of the machine's own chezmoi configuration.

read_target_mode() { # <path>
  stat -c '%a' "$1" 2>/dev/null || stat -f '%Lp' "$1"
}

if ! command -v chezmoi >/dev/null 2>&1; then
  fail "chezmoi is not on PATH; this guard cannot confirm its own grammar"
else
  probe_config="$work/probe-config.toml"
  : >"$probe_config"
  mode_source="$work/probe-mode-source"
  mode_dest="$work/probe-mode-dest"
  mkdir -p "$mode_source" "$mode_dest"

  printf 'stub\n' >"$mode_source/private_dot_probe-private"
  printf 'stub\n' >"$mode_source/create_private_dot_probe-create"
  printf 'stub\n' >"$mode_source/dot_probe-plain"
  printf 'stub\n' >"$mode_source/dot_private_probe-inside-name"
  printf '{{- /* %s */ -}}\n{{ "stub" }}\n' "$CHEZMOI_MODIFY_TEMPLATE_DIRECTIVE" \
    >"$mode_source/modify_private_dot_probe-modify"

  if ! chezmoi --config "$probe_config" --source "$mode_source" \
    --destination "$mode_dest" --no-tty apply --force >"$work/probe-apply.log" 2>&1; then
    fail "the chezmoi mode probe could not apply: $(cat "$work/probe-apply.log")"
  else
    assert_equal 600 "$(read_target_mode "$mode_dest/.probe-private")" \
      s3-private-renders-0600
    assert_equal 600 "$(read_target_mode "$mode_dest/.probe-create")" \
      s3-create-private-renders-0600
    assert_equal 600 "$(read_target_mode "$mode_dest/.probe-modify")" \
      s3-modify-private-renders-0600
    # The control row. Without it every assertion above would also pass against
    # a chezmoi that had started making everything 0600.
    assert_equal 644 "$(read_target_mode "$mode_dest/.probe-plain")" \
      s3-no-attribute-renders-0644
    assert_equal 644 "$(read_target_mode "$mode_dest/.private_probe-inside-name")" \
      s3-private-after-dot-is-name-text
  fi

  # The two shapes the mode probe cannot apply: an `encrypted_` entry needs a
  # real age blob and an age identity, and a misordered chain produces a
  # literally-named file rather than a mode. Both are answered by the TARGET
  # NAME instead, which chezmoi resolves without decrypting or rendering
  # anything: a consumed attribute disappears from the name, an unparsed one
  # stays in it.
  name_source="$work/probe-name-source"
  mkdir -p "$name_source"
  printf 'not-a-real-age-blob\n' >"$name_source/encrypted_private_probe-encrypted.age"
  printf 'stub\n' >"$name_source/executable_private_dot_probe-misordered"

  if ! chezmoi --config "$probe_config" --source "$name_source" \
    --destination "$work/probe-name-dest" --no-tty managed --include=files \
    --path-style=all --format=json >"$work/probe-names.json" 2>"$work/probe-names.err"; then
    fail "the chezmoi name probe failed: $(cat "$work/probe-names.err")"
  else
    probe_target_for() { # <source-basename>
      jq -r --arg source "$1" \
        '[to_entries[] | select(.value.sourceRelative == $source) | .key] | first // ""' \
        "$work/probe-names.json"
    }
    encrypted_target="$(probe_target_for encrypted_private_probe-encrypted.age)"
    [[ -n $encrypted_target ]] ||
      fail "s3-encrypted-private-parsed: chezmoi listed no target for the encrypted probe"
    [[ $encrypted_target != *"$PRIVATE_ATTRIBUTE"* ]] ||
      fail "s3-encrypted-private-parsed: chezmoi left '$PRIVATE_ATTRIBUTE' in the target name '$encrypted_target', so encrypted_private_ no longer parses as an attribute chain"
    misordered_target="$(probe_target_for executable_private_dot_probe-misordered)"
    [[ $misordered_target == *"$PRIVATE_ATTRIBUTE"* ]] ||
      fail "s3-misordered-not-parsed: chezmoi consumed '$PRIVATE_ATTRIBUTE' out of '$misordered_target', so executable_private_ IS private now and CHEZMOI_ATTRIBUTES_ALLOWED_BEFORE_PRIVATE is missing an entry"
  fi
fi

# ---------- S4: the guard ----------------------------------------------------
# The universe is chezmoi's own answer to "which source files become target
# FILES", so scripts, ignored dev files, the config template and the fixture
# tree are excluded by chezmoi rather than by a hand-maintained list here. NUL
# separation because one managed path contains a space
# (Library/Application Support/...), and a failed listing exits rather than
# passing on a short one.

managed_list="$work/managed-source-files"
: >"$work/managed-config.toml"
if ! chezmoi --config "$work/managed-config.toml" --source "$REPO_ROOT" \
  --destination "$work/unused-destination" --no-tty managed --include=files \
  --nul-path-separator --path-style=source-relative >"$managed_list" \
  2>"$work/managed.err"; then
  printf 'secret-source-files-declare-private-mode: FAIL -- chezmoi managed failed; refusing to pass on a partial list: %s\n' \
    "$(cat "$work/managed.err")" >&2
  exit 1
fi
[[ -s $managed_list ]] ||
  fail "chezmoi managed listed no target files at all, so the guard would have nothing to check"

declare -a widened=()
declare -a managed_templates=()
declare -a reached_allowlist_entries=()
managed_file_count=0

while IFS= read -r -d '' source_relative; do
  absolute="$REPO_ROOT/$source_relative"
  source_basename=${source_relative##*/}
  managed_file_count=$((managed_file_count + 1))
  [[ -f $absolute ]] || {
    fail "chezmoi lists $source_relative but it is not a readable file"
    continue
  }
  if source_file_is_chezmoi_template "$absolute" "$source_basename"; then
    managed_templates+=("$absolute")
  fi
  source_file_pulls_secrets "$absolute" "$source_basename" || continue

  if [[ -n ${PUBLIC_VALUE_VAULT_CALL_ALLOWLIST[$source_relative]+set} ]]; then
    reached_allowlist_entries+=("$source_relative")
    assert_equal "${PUBLIC_VALUE_VAULT_CALL_ALLOWLIST[$source_relative]}" \
      "$(template_vault_calls "$absolute")" \
      "s5-allowlisted-calls-unchanged-$source_relative"
    source_basename_declares_private_mode "$source_basename" &&
      fail "s5-dead-exemption: $source_relative declares private now, so its allowlist entry is dead and must be deleted"
    continue
  fi

  source_basename_declares_private_mode "$source_basename" || widened+=("$source_relative")
done <"$managed_list"

if [[ ${#widened[@]} -gt 0 ]]; then
  printf 'secret-source-files-declare-private-mode: FAIL -- these source files pull from the secret vault but declare target mode 0644, so the next chezmoi apply renders them world-readable:\n' >&2
  printf '  %s\n' "${widened[@]}" >&2
  printf '  fix: git mv <dir>/<name> <dir>/private_<name> (the target path does not move), or add a CALL-level entry to PUBLIC_VALUE_VAULT_CALL_ALLOWLIST saying why the value is public\n' >&2
  failures=$((failures + 1))
fi

# The transitive blind spot, kept honest rather than assumed away: this scanner
# reads one file at a time, so a vault call reached through `includeTemplate`
# would be invisible. Neither condition below holds today, and this fails if
# either changes.
if [[ ${#managed_templates[@]} -gt 0 ]] &&
  grep -l "$INCLUDE_TEMPLATE_FUNCTION" "${managed_templates[@]}" >"$work/include-template-users" 2>/dev/null; then
  fail "s4-transitive: managed template(s) use $INCLUDE_TEMPLATE_FUNCTION, which this scanner does not follow: $(tr '\n' ' ' <"$work/include-template-users")"
fi
if [[ -d $REPO_ROOT/.chezmoitemplates ]]; then
  while IFS= read -r -d '' partial; do
    [[ -z $(template_vault_calls "$partial") ]] ||
      fail "s4-partial-calls-vault: ${partial#"$REPO_ROOT/"} calls the secret vault, and this scanner does not follow $INCLUDE_TEMPLATE_FUNCTION, so its includers go unchecked"
  done < <(find "$REPO_ROOT/.chezmoitemplates" -type f -print0)
fi

# ---------- S5: no unreachable exemptions ------------------------------------
for allowlisted in "${!PUBLIC_VALUE_VAULT_CALL_ALLOWLIST[@]}"; do
  reached=0
  for entry in ${reached_allowlist_entries[@]+"${reached_allowlist_entries[@]}"}; do
    [[ $entry == "$allowlisted" ]] && reached=1
  done
  ((reached == 1)) ||
    fail "s5-unreachable-exemption: $allowlisted is allowlisted but the walk never reached it (moved, no longer managed, or no longer calling the vault); delete the entry"
done

if ((failures > 0)); then
  printf 'secret-source-files-declare-private-mode: %d assertion(s) failed\n' "$failures" >&2
  exit 1
fi
printf 'secret-source-files-declare-private-mode: OK (attribute-chain ordering, live-action vault calls vs comments and prose, chezmoi-confirmed grammar with a 0644 control, %d managed source files walked, %d call-level exemption(s))\n' \
  "$managed_file_count" "${#PUBLIC_VALUE_VAULT_CALL_ALLOWLIST[@]}"
