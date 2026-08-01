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
# payload, declare 0600? It keys on the MECHANISM, not on whether a file holds
# sensitive data. Three mechanisms are recognized, each a named constant:
#   * a chezmoi secret template function called from a live `{{ }}` action, in
#     the file itself or in any `includeTemplate` partial it reaches;
#   * the `encrypted_` source-state attribute;
#   * a vault command line run by a `modify_` entry that chezmoi EXECUTES
#     (a `modify_` entry without the modify-template directive is a script, so
#     its output, not its text, becomes the target).
# A hand-written home address, an inline literal, or a value read from the
# environment is invisible to it. The mechanism rule is what can be enforced
# without judgement.
#
# Behaviors pinned:
#   S1 attribute chain   - which source basenames DECLARE private, parsed in
#                          chezmoi's order. Order is load-bearing and measured:
#                          `modify_private_dot_x` renders 0600 while
#                          `encrypted_modify_private_dot_x` does not, because
#                          chezmoi stops parsing after `encrypted` and the rest
#                          becomes literal name text. So the rule is an exact
#                          SEQUENCE whitelist, not a per-token membership test:
#                          a membership test vouches for the second.
#   S2 vault calls       - a call inside a live `{{ }}` action counts; the same
#                          name inside a Go-template comment, inside a string
#                          literal, or in ordinary prose does not. That
#                          exemption is the guard's brake against its own mirror
#                          defect (a check so strict it demands `private_` on an
#                          ordinary 0644 template). The scanner is quote-aware
#                          in both directions: a `}}` inside a string literal
#                          does not end an action, so a call written after one
#                          cannot hide.
#   S3 the grammar       - S1 encodes chezmoi's parsing rules, so chezmoi is
#                          asked to confirm every row of them on every run
#                          rather than once by hand. Without this the rules rot
#                          silently on the next chezmoi upgrade, in the
#                          fail-open direction.
#   S4 the tree          - the actual guard, over chezmoi's own list of source
#                          files that become target FILES, plus the two things
#                          that list cannot tell you: that the universe did not
#                          SHRINK under it, and that a rename did not MOVE a
#                          target. Its classify step and its report step are
#                          exercised over a synthetic tree first, so neither can
#                          be disabled while the fast gate stays green.
#   S5 the allowlist     - each exemption names a specific CALL, not a file, and
#                          must still be live. A file allowlisted for fetching
#                          one public value cannot quietly grow a second call.
#
# RUNTIME. Measured 0.51 to 0.56 s on the authoring machine, over the unit
# suite's 200ms WARN threshold. Four chezmoi invocations are most of it and they
# are the ground truth this guard is built on, not incidental work: one applies
# a probe tree to confirm which basenames render 0600, one resolves a probe
# tree's target NAMES to confirm the whole attribute-sequence grammar, one asks
# which source files become target files, and one asks where the protected
# sources land. The warning is advisory and never fails a run.
#
# RELATIONSHIP TO scripts/render-coverage-classifier.nix. That classifier also
# detects keepassxc calls to decide which templates the rendered-shellcheck
# formatter may render. The two are NOT mirrors and are not asserted to agree:
# its universe is `.chezmoiscripts/*.sh.tmpl` plus root shell `dot_*.tmpl`, and
# chezmoi scripts are never target files, so they carry no mode to widen. This
# guard's universe is target files only. Measured at authoring time, the FLAGGED
# sets are disjoint: no `.chezmoiscripts/` entry is a managed file.
set -euo pipefail

# This guard uses associative arrays. Under bash 3.2, still the system bash on
# macOS, `declare -A` is a syntax error mid-script and the shell would exit 0
# having asserted nothing, which is the worst possible failure for a security
# check. Refuse loudly instead.
readonly MINIMUM_BASH_MAJOR_VERSION=4
if [[ ${BASH_VERSINFO[0]:-0} -lt $MINIMUM_BASH_MAJOR_VERSION ]]; then
  printf 'secret-source-files-declare-private-mode: FAIL -- needs bash %d or newer, got %s\n' \
    "$MINIMUM_BASH_MAJOR_VERSION" "${BASH_VERSION:-unknown}" >&2
  exit 1
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# ---------- named constants --------------------------------------------------

# MECHANISM 1: chezmoi template functions that read a secret manager at apply
# time. Every name here was probed against the installed chezmoi (v2.71.1) with
# `printf '{{ NAME }}' | chezmoi execute-template`, which answers
# `function "NAME" not defined` for a name chezmoi does not provide. Two names
# from chezmoi's documentation, `hcpVaultSecret` and `hcpVaultSecretJson`, are
# absent from that build and are therefore absent here. Repo policy makes
# KeePassXC the only vault actually in use; the rest are listed because a guard
# that only knows the vault of the day fails open the day someone adopts
# another one.
SECRET_VAULT_TEMPLATE_FUNCTIONS=(
  awsSecretsManager awsSecretsManagerRaw azureKeyVault
  bitwarden bitwardenAttachment bitwardenAttachmentByRef bitwardenFields
  bitwardenSecrets
  dashlaneNote dashlanePassword doppler dopplerProjectJson
  ejsonDecrypt ejsonDecryptWithKey
  gopass gopassRaw
  keepassxc keepassxcAttachment keepassxcAttribute
  keeper keeperDataFields keeperFindPassword
  lastpass lastpassRaw
  onepassword onepasswordDetailsFields onepasswordDocument
  onepasswordItemFields onepasswordRead
  pass passFields passRaw passhole
  rbw rbwFields
  secret secretJSON vault
)

# MECHANISM 3: vault command lines. Only ever consulted for a `modify_` entry
# that chezmoi EXECUTES rather than renders, where the secret arrives through a
# subprocess instead of a template function.
SECRET_VAULT_COMMANDS=(keepassxc-cli)

# chezmoi source-state attribute tokens, from `chezmoi help chattr` (v2.71.1)
# plus the script attributes that command does not list. Used only to decide
# where a basename's leading attribute chain ENDS: `dot_` is a name
# substitution, not an attribute, so a chain stops there.
CHEZMOI_SOURCE_ATTRIBUTES=(
  after before create empty encrypted exact executable external literal
  modify once onchange private readonly remove run symlink
)

# The attribute SEQUENCES chezmoi accepts before `private_` while still parsing
# `private` as an attribute, space separated, empty string meaning `private_`
# leads the name. This is an exact-sequence whitelist because chezmoi's parser
# is positional, not a set: it takes at most one source-file-type prefix
# (`create_` or `modify_`), then `encrypted_`, then `private_`. Swap any two and
# parsing stops at the first token out of place, leaving `private_` as literal
# name text on a 0644 target. An unlisted sequence answers FALSE, which is the
# safe direction: a loud demand for a rename, never a silent pass.
CHEZMOI_ATTRIBUTE_PREFIXES_DECLARING_PRIVATE=(
  ""
  "create"
  "modify"
  "encrypted"
  "create encrypted"
  "modify encrypted"
)

# Sequences that carry the literal string `private_` and do NOT declare 0600.
# Kept as data so S3 can ask chezmoi to re-confirm the negative half of the
# grammar too: a whitelist that quietly became a superset would otherwise look
# healthy from inside.
CHEZMOI_ATTRIBUTE_PREFIXES_NOT_DECLARING_PRIVATE=(
  "encrypted create"
  "encrypted modify"
  "create modify"
  "modify create"
  "executable"
  "readonly"
  "empty"
  "literal"
  "external"
)

PRIVATE_ATTRIBUTE=private
ENCRYPTED_ATTRIBUTE=encrypted
MODIFY_ATTRIBUTE=modify

CHEZMOI_TEMPLATE_SUFFIX=.tmpl
CHEZMOI_MODIFY_TEMPLATE_DIRECTIVE='chezmoi:modify-template'
CHEZMOI_TEMPLATES_DIRECTORY=.chezmoitemplates
INCLUDE_TEMPLATE_FUNCTION=includeTemplate

# Source files whose declared mode this guard exists to protect, each mapped to
# the target path it must keep. The pins answer the two questions chezmoi's file
# list cannot answer about itself:
#   * PRESENCE. That list is the guard's universe and it SHRINKS whenever
#     `.chezmoiignore` grows a pattern. Narrowing it to a subtree hides every
#     violation inside that subtree while the run still reports OK on a smaller
#     count, so every key here must be reached by the walk.
#   * STABILITY. This guard's own fix advice is that renaming a file to
#     `private_<name>` does not move its target. A rename that DID move one
#     (`dot_aws/private_dot_credentials.tmpl` lands at `.aws/.credentials`)
#     would leave the old 0644 file behind unmanaged, with the secret now
#     written somewhere else.
# Targets are as chezmoi resolves them under the EMPTY config this guard uses,
# which is why the age blobs keep their `.age` suffix here: stripping it is a
# function of the machine's `encryption` setting, and these pins exist to detect
# MOVEMENT, not to state the deployed name.
declare -A SECRET_SOURCE_TARGET_PINS=(
  ["dot_aws/private_credentials.tmpl"]=".aws/credentials"
  ["dot_composio/private_user_data.json.tmpl"]=".composio/user_data.json"
  ["dot_config/atuin/private_config.toml.tmpl"]=".config/atuin/config.toml"
  ["dot_config/himalaya/private_config.toml.tmpl"]=".config/himalaya/config.toml"
  ["dot_config/private_gogcli/private_credentials.json.tmpl"]=".config/gogcli/credentials.json"
  ["dot_config/relay/private_auth.json.tmpl"]=".config/relay/auth.json"
  ["dot_gitconfig.tmpl"]=".gitconfig"
  ["dot_hermes/encrypted_private_config.yaml.age"]=".hermes/config.yaml.age"
  ["dot_hermes/profiles/private_butters/encrypted_private_config.yaml.age"]=".hermes/profiles/butters/config.yaml.age"
  ["dot_hermes/profiles/private_concerned/encrypted_private_config.yaml.age"]=".hermes/profiles/concerned/config.yaml.age"
  ["dot_hermes/profiles/private_elaine/encrypted_private_config.yaml.age"]=".hermes/profiles/elaine/config.yaml.age"
  ["dot_hermes/profiles/private_nicodemus/encrypted_private_config.yaml.age"]=".hermes/profiles/nicodemus/config.yaml.age"
  ["Library/Application Support/Claude/modify_private_claude_desktop_config.json"]="Library/Application Support/Claude/claude_desktop_config.json"
  ["Library/Application Support/espanso/match/private_identity.yml.tmpl"]="Library/Application Support/espanso/match/identity.yml"
  ["modify_private_dot_claude.json"]=".claude.json"
)

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
# the tree walk asks these questions several times for each managed file, and a
# subshell per question dominated this guard's runtime.
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
# the chain AND the exact sequence of tokens before it is one chezmoi parses
# there. Anything else answers FALSE, which is the safe direction: an
# unrecognized ordering produces a loud demand for a rename, never a silent pass
# on a file chezmoi renders 0644.
source_basename_declares_private_mode() { # <basename>
  local token accepted prefix=
  read_chezmoi_source_attribute_chain "$1"
  for token in ${CHEZMOI_SOURCE_ATTRIBUTE_CHAIN[@]+"${CHEZMOI_SOURCE_ATTRIBUTE_CHAIN[@]}"}; do
    if [[ $token == "$PRIVATE_ATTRIBUTE" ]]; then
      for accepted in "${CHEZMOI_ATTRIBUTE_PREFIXES_DECLARING_PRIVATE[@]}"; do
        [[ $prefix == "$accepted" ]] && return 0
      done
      return 1
    fi
    prefix=${prefix:+$prefix }$token
  done
  return 1
}

# Does chezmoi expand `{{ }}` in this entry? Two shapes: the `.tmpl` suffix, and
# a `modify_` entry carrying chezmoi's modify-template directive ANYWHERE in its
# contents. Measured against chezmoi v2.71.1: a `modify_` entry with the
# directive on line 3 renders as a template, so a first-line-only test would let
# a vault call ride in on a 0644 target. That directive is not optional
# decoration either: a `modify_` entry without it is EXECUTED as a script and
# its `{{ }}` are never expanded.
source_file_is_chezmoi_template() { # <absolute-path> <basename>
  local contents
  [[ $2 == *"$CHEZMOI_TEMPLATE_SUFFIX" ]] && return 0
  source_basename_has_attribute "$2" "$MODIFY_ATTRIBUTE" || return 1
  contents=$(<"$1") || return 1
  [[ $contents == *"$CHEZMOI_MODIFY_TEMPLATE_DIRECTIVE"* ]]
}

# Every live `{{ }}` action of a template, classified. Prints one tagged line
# per finding, tag and payload separated by a single space:
#   C <action>   the action names a secret vault function
#   I <name>     the action includes the named `.chezmoitemplates` partial
#   U <action>   the action includes a partial this scanner cannot name
#
# Two things make the classification honest rather than approximate. Comment
# actions are skipped whole, including their contents, so a `}}` written inside
# a comment cannot end an action early and hide what follows it. And string
# literals are transparent to the closing-delimiter search but opaque to the
# name search, so `{{ printf "}}" (keepassxc "E") }}` is one action holding a
# real call, while `{{ "keepassxc" | quote }}` is no call at all. A name
# preceded by `$` or `.` is a variable or a field, not a call.
#
# Unterminated constructs fail CLOSED: the remainder of the file is searched for
# vault names with no quote awareness at all, so a malformed template cannot
# swallow a call.
# shellcheck disable=SC2016  # the awk program is a literal, not a shell expression
LIVE_ACTION_SCANNER_PROGRAM='
function is_boundary_before(character) {
  return character !~ /^[A-Za-z0-9_$.]$/
}
function is_boundary_after(character) {
  return character !~ /^[A-Za-z0-9_]$/
}
function next_function_position(haystack, function_name, from,   position, before, after) {
  while (1) {
    position = index(substr(haystack, from), function_name)
    if (position == 0) return 0
    position = from + position - 1
    before = (position == 1) ? "" : substr(haystack, position - 1, 1)
    after = substr(haystack, position + length(function_name), 1)
    from = position + length(function_name)
    if (is_boundary_before(before) && is_boundary_after(after)) return from
  }
}
function names_any_vault_function(haystack,   i) {
  for (i in vault_functions) {
    if (vault_functions[i] == "") continue
    if (next_function_position(haystack, vault_functions[i], 1) > 0) return 1
  }
  return 0
}
function first_string_literal_after(body, from,   i, character, quote, literal) {
  i = from
  while (i <= length(body)) {
    character = substr(body, i, 1)
    if (character == "\"" || character == "`") {
      quote = character
      i++
      literal = ""
      while (i <= length(body)) {
        character = substr(body, i, 1)
        if (quote == "\"" && character == "\\") {
          literal = literal substr(body, i + 1, 1)
          i += 2
          continue
        }
        if (character == quote) return literal
        literal = literal character
        i++
      }
      return ""
    }
    if (character ~ /^[ \t\r\n]$/) { i++; continue }
    return ""
  }
  return ""
}
function normalize(body,   normalized) {
  normalized = body
  gsub(/^[- \t\r\n]+|[- \t\r\n]+$/, "", normalized)
  gsub(/[ \t\r\n]+/, " ", normalized)
  return normalized
}
function scan_action(start,   i, character, quote) {
  ACTION_BODY = ""
  ACTION_CODE = ""
  ACTION_END = 0
  i = start
  quote = ""
  while (i <= total) {
    character = substr(text, i, 1)
    if (quote != "") {
      if (quote == "\"" && character == "\\") {
        ACTION_BODY = ACTION_BODY character substr(text, i + 1, 1)
        ACTION_CODE = ACTION_CODE "  "
        i += 2
        continue
      }
      ACTION_BODY = ACTION_BODY character
      ACTION_CODE = ACTION_CODE " "
      if (character == quote) quote = ""
      i++
      continue
    }
    if (character == "\"" || character == "`") {
      quote = character
      ACTION_BODY = ACTION_BODY character
      ACTION_CODE = ACTION_CODE " "
      i++
      continue
    }
    if (character == "}" && substr(text, i, 2) == "}}") {
      ACTION_END = i
      return
    }
    ACTION_BODY = ACTION_BODY character
    ACTION_CODE = ACTION_CODE character
    i++
  }
}
function emit_unterminated(raw) {
  if (names_any_vault_function(raw)) print "C " normalize(raw)
}
function emit(body, code,   normalized, after_name, literal) {
  normalized = normalize(body)
  if (names_any_vault_function(code)) print "C " normalized
  after_name = 1
  while (1) {
    after_name = next_function_position(code, include_function, after_name)
    if (after_name == 0) break
    literal = first_string_literal_after(body, after_name)
    if (literal == "") print "U " normalized
    else print "I " literal
  }
}
BEGIN { split(vault_function_names, vault_functions, " ") }
{ text = text $0 "\n" }
END {
  total = length(text)
  pos = 1
  while (pos <= total) {
    opening = index(substr(text, pos), "{{")
    if (opening == 0) break
    opening = pos + opening - 1
    body_start = opening + 2

    probe = body_start
    if (substr(text, probe, 1) == "-") probe++
    while (probe <= total && substr(text, probe, 1) ~ /^[ \t\r\n]$/) probe++

    if (substr(text, probe, 2) == "/*") {
      comment_end = index(substr(text, probe + 2), "*/")
      if (comment_end == 0) {
        emit_unterminated(substr(text, body_start))
        break
      }
      after_comment = probe + 2 + comment_end + 1
      closing = index(substr(text, after_comment), "}}")
      if (closing == 0) {
        emit_unterminated(substr(text, after_comment))
        break
      }
      pos = after_comment + closing + 1
      continue
    }

    scan_action(body_start)
    if (ACTION_END == 0) {
      emit_unterminated(substr(text, body_start))
      break
    }
    emit(ACTION_BODY, ACTION_CODE)
    pos = ACTION_END + 2
  }
}
'

scan_live_actions() { # <path>
  awk -v vault_function_names="${SECRET_VAULT_TEMPLATE_FUNCTIONS[*]}" \
    -v include_function="$INCLUDE_TEMPLATE_FUNCTION" \
    "$LIVE_ACTION_SCANNER_PROGRAM" "$1"
}

# The vault calls written in THIS file, one per line. Used by the S2 fixtures
# and, through the transitive reader below, by the tree walk.
template_vault_calls() { # <path>
  local line
  while IFS= read -r line; do
    if [[ ${line:0:1} == C ]]; then
      printf '%s\n' "${line:2}"
    fi
  done < <(scan_live_actions "$1")
}

# Every vault call reachable from a template, following `includeTemplate` into
# `.chezmoitemplates`, published in REACHABLE_VAULT_CALLS. A reference this
# scanner cannot follow (a computed argument, or a name with no file behind it)
# is recorded in UNFOLLOWABLE_INCLUDE_REFERENCES rather than skipped, so the
# blind spot is reported instead of assumed harmless. Cycles terminate: a
# partial is scanned at most once per call.
declare -a REACHABLE_VAULT_CALLS=()
declare -a UNFOLLOWABLE_INCLUDE_REFERENCES=()
read_reachable_vault_calls() { # <source-root> <absolute-path>
  local root=$1 current line partial
  local -a queue=("$2")
  declare -A visited=()
  REACHABLE_VAULT_CALLS=()
  UNFOLLOWABLE_INCLUDE_REFERENCES=()
  while ((${#queue[@]} > 0)); do
    current=${queue[0]}
    queue=("${queue[@]:1}")
    [[ -n ${visited[$current]+set} ]] && continue
    visited[$current]=1
    while IFS= read -r line; do
      case ${line:0:1} in
        C) REACHABLE_VAULT_CALLS+=("${line:2}") ;;
        I)
          partial="$root/$CHEZMOI_TEMPLATES_DIRECTORY/${line:2}"
          if [[ -f $partial ]]; then
            queue+=("$partial")
          else
            UNFOLLOWABLE_INCLUDE_REFERENCES+=("${current#"$root/"} includes '${line:2}', which is not a file under $CHEZMOI_TEMPLATES_DIRECTORY/")
          fi
          ;;
        U)
          UNFOLLOWABLE_INCLUDE_REFERENCES+=("${current#"$root/"} computes its $INCLUDE_TEMPLATE_FUNCTION argument in '${line:2}', which this scanner cannot follow")
          ;;
      esac
    done < <(scan_live_actions "$current")
  done
}

# A `modify_` entry without the modify-template directive is executed, so its
# secret arrives through a command line rather than a template function. A read
# failure counts as a hit: an unreadable file is not an all-clear.
executed_entry_invokes_vault_command() { # <absolute-path>
  local command_name status
  for command_name in "${SECRET_VAULT_COMMANDS[@]}"; do
    status=0
    grep -qF -e "$command_name" -- "$1" || status=$?
    if ((status == 0)); then
      return 0
    elif ((status != 1)); then
      return 0
    fi
  done
  return 1
}

# Does this source file pull a secret by any mechanism this guard knows? Sets
# REACHABLE_VAULT_CALLS and UNFOLLOWABLE_INCLUDE_REFERENCES as a side effect for
# templates, so the caller can report the exact calls without rescanning.
source_file_pulls_secrets() { # <source-root> <absolute-path> <basename>
  REACHABLE_VAULT_CALLS=()
  UNFOLLOWABLE_INCLUDE_REFERENCES=()
  if source_basename_has_attribute "$3" "$ENCRYPTED_ATTRIBUTE"; then
    return 0
  fi
  if source_file_is_chezmoi_template "$2" "$3"; then
    read_reachable_vault_calls "$1" "$2"
    if ((${#REACHABLE_VAULT_CALLS[@]} > 0)); then
      return 0
    fi
    return 1
  fi
  source_basename_has_attribute "$3" "$MODIFY_ATTRIBUTE" || return 1
  executed_entry_invokes_vault_command "$2"
}

# ---------- S1: which basenames declare private mode -------------------------

for fixture_basename in \
  private_credentials.tmpl \
  private_config.toml.tmpl \
  private_identity.yml.tmpl \
  private_dot_foo \
  create_private_dot_foo \
  encrypted_private_config.yaml.age \
  create_encrypted_private_dot_foo \
  modify_encrypted_private_dot_foo \
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
# renders 0600; a membership test would pass every one. The first four are the
# ones a per-token membership test passes even after it learns which tokens may
# precede `private`, because they use only accepted tokens in an order chezmoi
# does not parse.
for fixture_basename in \
  encrypted_create_private_dot_foo \
  encrypted_modify_private_dot_foo \
  create_modify_private_dot_foo \
  modify_create_private_dot_foo \
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
# A word merely beginning with the attribute is not the attribute, in either
# direction.
assert_predicate expect-false s1-attribute-prefix-is-not-the-attribute \
  source_basename_declares_private_mode notprivate_dot_foo
assert_predicate expect-false s1-attribute-is-not-a-word-prefix \
  source_basename_declares_private_mode privatestuff_dot_foo
# A chain token must EQUAL an attribute, not merely begin with one. If it only
# had to begin with one, `encryptedstuff` would be consumed as a chain token and
# the parse would run on into the `encrypted` behind it, so this name would read
# as an encrypted payload. chezmoi stops at `encryptedstuff` and calls the whole
# thing name text.
assert_predicate expect-false s1-chain-token-is-not-a-word-prefix \
  source_basename_has_attribute encryptedstuff_encrypted_notes.age "$ENCRYPTED_ATTRIBUTE"

read_chezmoi_source_attribute_chain modify_private_dot_claude.json
assert_equal "modify private" "${CHEZMOI_SOURCE_ATTRIBUTE_CHAIN[*]}" s1-chain-order
read_chezmoi_source_attribute_chain dot_gitconfig.tmpl
assert_equal "" "${CHEZMOI_SOURCE_ATTRIBUTE_CHAIN[*]-}" s1-chain-stops-at-dot
assert_predicate expect-true s1-has-encrypted-attribute \
  source_basename_has_attribute encrypted_private_config.yaml.age "$ENCRYPTED_ATTRIBUTE"
assert_predicate expect-false s1-encrypted-inside-the-name \
  source_basename_has_attribute dot_encrypted_notes.txt "$ENCRYPTED_ATTRIBUTE"

# ---------- S2: a call, a comment, a string and prose are four things --------

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
vault_call_fixture live-attachment-call 1 'blob = {{ keepassxcAttachment "E" "a.txt" }}
'
vault_call_fixture live-other-vault-call 1 'token = {{ onepasswordItemFields "item" }}
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
# QUOTE AWARENESS, the fail-open direction. Go template lexing is quote-aware,
# so a `}}` inside a string literal does not end the action: chezmoi renders
# `{{ printf "%s%s" "}}" (upper "x") }}` as `}}X`. A delimiter search that
# stopped at the first `}}` would end the action inside the literal and never
# see the call written after it.
vault_call_fixture closing-delimiter-inside-a-string 1 'api_key = {{ printf "%s%s" "}}" ((keepassxc "Fixture :: Secret").Password) | quote }}
'
# shellcheck disable=SC2016  # the Go raw string literal is the fixture
vault_call_fixture closing-delimiter-inside-a-raw-string 1 'api_key = {{ printf `}}` ((keepassxc "E").Password) }}
'
vault_call_fixture escaped-quote-inside-a-string 1 'api_key = {{ printf "a\"}}b" ((keepassxc "E").Password) }}
'
# QUOTE AWARENESS, the mirror direction. A vault name inside a string literal is
# data, not a call, and demanding `private_` for it would be a false build
# failure on an ordinary 0644 template.
vault_call_fixture name-inside-a-string 0 'label = {{ "keepassxc" | quote }}
'
vault_call_fixture command-name-inside-a-string 0 'x = {{ if lookPath "keepassxc-cli" }}y{{ end }}
'
# A name preceded by `$` or `.` is a variable or a field, not a call. This is
# what makes the generic function names in the mechanism list (`pass`, `secret`,
# `vault`) safe to include.
# shellcheck disable=SC2016  # Go-template variables and fields are the fixture
vault_call_fixture variable-and-field-names 0 'x = {{ $pass }}{{ $secret }}{{ .vault }}{{ .x.secret }}
'
vault_call_fixture generic-function-name-called 1 'x = {{ pass "aws/secret" }}
'
# Unterminated constructs fail CLOSED.
vault_call_fixture unterminated-action 1 'x = {{ (keepassxc "E").Password
'
vault_call_fixture unterminated-comment 1 'x = {{/* keepassxc
'
printf 'k = {{ keepassxcAttribute "E" "A" }}\n' >"$work/fixture-call-text"
assert_equal 'keepassxcAttribute "E" "A"' \
  "$(template_vault_calls "$work/fixture-call-text")" s2-call-text-normalized

# ---------- S2b: which entries chezmoi templates, and transitive reach -------

printf '{{ (keepassxc "E").Password }}\n' >"$work/probe-dot_plain.tmpl"
assert_predicate expect-true s2b-tmpl-suffix-is-a-template \
  source_file_is_chezmoi_template "$work/probe-dot_plain.tmpl" dot_plain.tmpl

printf '{{- /* %s */ -}}\n{{ (keepassxc "E").Password }}\n' "$CHEZMOI_MODIFY_TEMPLATE_DIRECTIVE" \
  >"$work/probe-modify_first"
assert_predicate expect-true s2b-modify-directive-on-line-one \
  source_file_is_chezmoi_template "$work/probe-modify_first" modify_first

# The fail-open shape: chezmoi accepts the directive anywhere in the contents
# (S3 re-measures that), so a first-line-only test would classify this as a
# script and never scan it.
printf '{\n  "a": 1,\n{{- /* %s */ -}}\n  "b": {{ (keepassxc "E").Password }}\n}\n' \
  "$CHEZMOI_MODIFY_TEMPLATE_DIRECTIVE" >"$work/probe-modify_late"
assert_predicate expect-true s2b-modify-directive-after-line-one \
  source_file_is_chezmoi_template "$work/probe-modify_late" modify_late

# The mirror shape: without the directive the entry is EXECUTED, its `{{ }}` are
# literal text, and demanding `private_` for them would be a false failure.
printf '#!/bin/sh\nprintf "%%s" "{{ (keepassxc \\"E\\").Password }}"\n' \
  >"$work/probe-modify_script"
assert_predicate expect-false s2b-modify-without-the-directive-is-a-script \
  source_file_is_chezmoi_template "$work/probe-modify_script" modify_script
assert_predicate expect-false s2b-plain-entry-is-not-a-template \
  source_file_is_chezmoi_template "$work/probe-modify_script" dot_plain

# MECHANISM 2, the encrypted attribute, which no other assertion reaches: an
# `.age` blob carries its secret in its bytes, so it counts with no vault call
# anywhere in it.
mkdir -p "$work/reach-root/$CHEZMOI_TEMPLATES_DIRECTORY"
printf 'age-ciphertext-not-a-template\n' >"$work/reach-root/encrypted_probe.yaml.age"
assert_predicate expect-true s2b-encrypted-attribute-pulls-secrets \
  source_file_pulls_secrets "$work/reach-root" "$work/reach-root/encrypted_probe.yaml.age" \
  encrypted_probe.yaml.age
printf 'plain-bytes\n' >"$work/reach-root/dot_probe-plain"
assert_predicate expect-false s2b-plain-file-pulls-nothing \
  source_file_pulls_secrets "$work/reach-root" "$work/reach-root/dot_probe-plain" dot_probe-plain

# MECHANISM 3, an executed `modify_` entry that shells out to the vault.
# shellcheck disable=SC2016  # the literal fixture script body is the point
printf '#!/bin/bash\nkeepassxc-cli show -a Password "$KDBX" AWS\n' \
  >"$work/reach-root/modify_dot_probe-script"
assert_predicate expect-true s2b-executed-entry-invokes-vault-command \
  source_file_pulls_secrets "$work/reach-root" "$work/reach-root/modify_dot_probe-script" \
  modify_dot_probe-script
printf '#!/bin/bash\nprintf "%%s" "no secret here"\n' >"$work/reach-root/modify_dot_probe-harmless"
assert_predicate expect-false s2b-executed-entry-without-a-vault-command \
  source_file_pulls_secrets "$work/reach-root" "$work/reach-root/modify_dot_probe-harmless" \
  modify_dot_probe-harmless

# TRANSITIVE REACH. A call inside an included partial belongs to its includer,
# which is the file that carries the mode.
printf '{{ (keepassxc "E").Password }}\n' \
  >"$work/reach-root/$CHEZMOI_TEMPLATES_DIRECTORY/secret-partial.tmpl"
printf 'plain text, no call\n' \
  >"$work/reach-root/$CHEZMOI_TEMPLATES_DIRECTORY/safe-partial.tmpl"
printf '{{ includeTemplate "secret-partial.tmpl" . }}\n' \
  >"$work/reach-root/dot_probe-includes-secret.tmpl"
assert_predicate expect-true s2b-include-reaches-a-vault-call \
  source_file_pulls_secrets "$work/reach-root" \
  "$work/reach-root/dot_probe-includes-secret.tmpl" dot_probe-includes-secret.tmpl
printf '{{ includeTemplate "safe-partial.tmpl" . }}\n' \
  >"$work/reach-root/dot_probe-includes-safe.tmpl"
assert_predicate expect-false s2b-include-of-a-safe-partial-is-not-a-call \
  source_file_pulls_secrets "$work/reach-root" \
  "$work/reach-root/dot_probe-includes-safe.tmpl" dot_probe-includes-safe.tmpl
assert_equal "" "${UNFOLLOWABLE_INCLUDE_REFERENCES[*]-}" s2b-safe-include-is-followable

# A cycle must terminate rather than hang the fast gate.
printf '{{ includeTemplate "cycle-b.tmpl" . }}\n' \
  >"$work/reach-root/$CHEZMOI_TEMPLATES_DIRECTORY/cycle-a.tmpl"
printf '{{ includeTemplate "cycle-a.tmpl" . }}{{ (keepassxc "E").Password }}\n' \
  >"$work/reach-root/$CHEZMOI_TEMPLATES_DIRECTORY/cycle-b.tmpl"
printf '{{ includeTemplate "cycle-a.tmpl" . }}\n' >"$work/reach-root/dot_probe-cyclic.tmpl"
assert_predicate expect-true s2b-cyclic-include-terminates-and-reaches \
  source_file_pulls_secrets "$work/reach-root" "$work/reach-root/dot_probe-cyclic.tmpl" \
  dot_probe-cyclic.tmpl

# References this scanner cannot follow are RECORDED, never assumed harmless.
# shellcheck disable=SC2016  # a Go-template variable reference, not a shell one
printf '{{ includeTemplate (printf "%%s.tmpl" $name) . }}\n' \
  >"$work/reach-root/dot_probe-computed-include.tmpl"
source_file_pulls_secrets "$work/reach-root" \
  "$work/reach-root/dot_probe-computed-include.tmpl" dot_probe-computed-include.tmpl || true
assert_equal 1 "${#UNFOLLOWABLE_INCLUDE_REFERENCES[@]}" s2b-computed-include-is-recorded
printf '{{ includeTemplate "no-such-partial.tmpl" . }}\n' \
  >"$work/reach-root/dot_probe-missing-include.tmpl"
source_file_pulls_secrets "$work/reach-root" \
  "$work/reach-root/dot_probe-missing-include.tmpl" dot_probe-missing-include.tmpl || true
assert_equal 1 "${#UNFOLLOWABLE_INCLUDE_REFERENCES[@]}" s2b-missing-include-is-recorded

# ---------- S3: chezmoi confirms the grammar S1 encodes ----------------------
# S1's ordering rules came from measurement, so they are re-measured here rather
# than trusted. Without this they rot silently on the next chezmoi upgrade, and
# they rot toward vouching for a file chezmoi renders 0644. The empty config
# keeps both probes independent of the machine's own chezmoi configuration.

read_target_mode() { # <path>
  stat -c '%a' "$1" 2>/dev/null || stat -f '%Lp' "$1"
}

# A source basename built from an attribute SEQUENCE, e.g. "modify encrypted"
# plus "dot_probe" gives "modify_encrypted_private_dot_probe".
private_probe_basename() { # <space-separated-prefix> <suffix>
  local token prefix=
  local -a tokens=()
  read -ra tokens <<<"$1"
  for token in ${tokens[@]+"${tokens[@]}"}; do
    prefix="$prefix${token}_"
  done
  printf '%s%s_%s' "$prefix" "$PRIVATE_ATTRIBUTE" "$2"
}

probe_config="$work/probe-config.toml"
: >"$probe_config"

if ! command -v chezmoi >/dev/null 2>&1; then
  fail "chezmoi is not on PATH; this guard cannot confirm its own grammar"
else
  mode_source="$work/probe-mode-source"
  mode_dest="$work/probe-mode-dest"
  mkdir -p "$mode_source" "$mode_dest"

  printf 'stub\n' >"$mode_source/private_dot_probe-private"
  printf 'stub\n' >"$mode_source/create_private_dot_probe-create"
  printf 'stub\n' >"$mode_source/dot_probe-plain"
  printf 'stub\n' >"$mode_source/dot_private_probe-inside-name"
  printf '{{- /* %s */ -}}\n{{ "stub" }}\n' "$CHEZMOI_MODIFY_TEMPLATE_DIRECTIVE" \
    >"$mode_source/modify_private_dot_probe-modify"
  # The late-directive shape, which is why source_file_is_chezmoi_template reads
  # the whole file. If chezmoi ever required the directive on line 1, this probe
  # would be EXECUTED and its rendered body would not match.
  printf '{\n{{- /* %s */ -}}\n"b": "{{ "rendered" }}"\n}\n' "$CHEZMOI_MODIFY_TEMPLATE_DIRECTIVE" \
    >"$mode_source/modify_dot_probe-late-directive"
  # The mirror: no directive at all, so chezmoi EXECUTES it.
  printf '#!/bin/sh\nprintf "executed\\n"\n' >"$mode_source/modify_dot_probe-no-directive"

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
    assert_equal '"b": "rendered"' \
      "$(sed -n '2p' "$mode_dest/.probe-late-directive")" \
      s3-modify-directive-is-not-line-one-only
    assert_equal executed "$(cat "$mode_dest/.probe-no-directive")" \
      s3-modify-without-the-directive-is-executed
  fi

  # The mode probe cannot answer for chains that need a real age blob, or for
  # chains whose point is that they produce a literally-named file rather than a
  # mode. Both are answered by the TARGET NAME instead, which chezmoi resolves
  # without decrypting or rendering anything: a consumed attribute disappears
  # from the name, an unparsed one stays in it. Every row of both sequence
  # constants is asked, so a constant that silently became a superset fails.
  name_source="$work/probe-name-source"
  mkdir -p "$name_source"
  declare -a name_probe_arguments=()
  declare -a name_probe_expect_private=()
  probe_index=0
  for probe_prefix in "${CHEZMOI_ATTRIBUTE_PREFIXES_DECLARING_PRIVATE[@]}"; do
    probe_index=$((probe_index + 1))
    probe_basename="$(private_probe_basename "$probe_prefix" "dot_probe$probe_index")"
    printf 'stub\n' >"$name_source/$probe_basename"
    name_probe_arguments+=("$name_source/$probe_basename")
    name_probe_expect_private+=(consumed)
  done
  for probe_prefix in "${CHEZMOI_ATTRIBUTE_PREFIXES_NOT_DECLARING_PRIVATE[@]}"; do
    probe_index=$((probe_index + 1))
    probe_basename="$(private_probe_basename "$probe_prefix" "dot_probe$probe_index")"
    printf 'stub\n' >"$name_source/$probe_basename"
    name_probe_arguments+=("$name_source/$probe_basename")
    name_probe_expect_private+=(retained)
  done

  if ! chezmoi --config "$probe_config" --source "$name_source" \
    --destination "$work/probe-name-dest" --no-tty target-path \
    "${name_probe_arguments[@]}" >"$work/probe-names.txt" 2>"$work/probe-names.err"; then
    fail "the chezmoi name probe failed: $(cat "$work/probe-names.err")"
  else
    probe_index=0
    while IFS= read -r probe_target; do
      probe_index=$((probe_index + 1))
      probe_source=${name_probe_arguments[$((probe_index - 1))]}
      case ${name_probe_expect_private[$((probe_index - 1))]} in
        consumed)
          [[ $probe_target != *"$PRIVATE_ATTRIBUTE"* ]] ||
            fail "s3-sequence-declares-private: chezmoi left '$PRIVATE_ATTRIBUTE' in the target of ${probe_source##*/} ('${probe_target##*/}'), so that sequence no longer declares 0600 and CHEZMOI_ATTRIBUTE_PREFIXES_DECLARING_PRIVATE has a stale row"
          ;;
        retained)
          [[ $probe_target == *"$PRIVATE_ATTRIBUTE"* ]] ||
            fail "s3-sequence-does-not-declare-private: chezmoi consumed '$PRIVATE_ATTRIBUTE' out of ${probe_source##*/}, so that sequence IS private now and CHEZMOI_ATTRIBUTE_PREFIXES_DECLARING_PRIVATE is missing a row"
          ;;
      esac
    done <"$work/probe-names.txt"
    assert_equal "${#name_probe_arguments[@]}" "$probe_index" s3-every-sequence-answered
  fi
fi

# ---------- S4: the guard ----------------------------------------------------
# The universe is chezmoi's own answer to "which source files become target
# FILES", so scripts, ignored dev files, the config template and the fixture
# tree are excluded by chezmoi rather than by a hand-maintained list here. NUL
# separation because one managed path contains a space
# (Library/Application Support/...), and a failed listing exits rather than
# passing on a short one.

declare -a WIDENED_SOURCE_FILES=()
declare -a REACHED_ALLOWLIST_ENTRIES=()
declare -a REACHED_TARGET_PIN_KEYS=()
declare -a CLASSIFIER_UNFOLLOWABLE_INCLUDES=()
MANAGED_FILE_COUNT=0

# Walk a NUL-separated list of source-relative paths under one root and sort
# every entry into: widens a secret, exempt by an allowlisted call, or clean.
# It classifies and never reports, so the same call can be made over a synthetic
# tree without printing a failure the caller did not ask for.
classify_managed_source_files() { # <source-root> <nul-separated-list-file>
  local root=$1 list=$2 source_relative absolute source_basename calls_text
  local pulls_secrets reference
  WIDENED_SOURCE_FILES=()
  REACHED_ALLOWLIST_ENTRIES=()
  REACHED_TARGET_PIN_KEYS=()
  CLASSIFIER_UNFOLLOWABLE_INCLUDES=()
  MANAGED_FILE_COUNT=0
  while IFS= read -r -d '' source_relative; do
    absolute="$root/$source_relative"
    source_basename=${source_relative##*/}
    MANAGED_FILE_COUNT=$((MANAGED_FILE_COUNT + 1))
    if [[ -n ${SECRET_SOURCE_TARGET_PINS[$source_relative]+set} ]]; then
      REACHED_TARGET_PIN_KEYS+=("$source_relative")
    fi
    [[ -f $absolute ]] || {
      fail "chezmoi lists $source_relative but it is not a readable file"
      continue
    }
    pulls_secrets=0
    if source_file_pulls_secrets "$root" "$absolute" "$source_basename"; then
      pulls_secrets=1
    fi
    # Recorded whatever the verdict was. A file whose ONLY vault reach is an
    # include this scanner cannot follow answers "no secret" above, so checking
    # this after a `continue` would drop exactly the case it exists for.
    for reference in ${UNFOLLOWABLE_INCLUDE_REFERENCES[@]+"${UNFOLLOWABLE_INCLUDE_REFERENCES[@]}"}; do
      CLASSIFIER_UNFOLLOWABLE_INCLUDES+=("$reference")
    done
    ((pulls_secrets == 1)) || continue

    if [[ -n ${PUBLIC_VALUE_VAULT_CALL_ALLOWLIST[$source_relative]+set} ]]; then
      REACHED_ALLOWLIST_ENTRIES+=("$source_relative")
      calls_text=""
      ((${#REACHABLE_VAULT_CALLS[@]} == 0)) ||
        calls_text="$(printf '%s\n' "${REACHABLE_VAULT_CALLS[@]}")"
      assert_equal "${PUBLIC_VALUE_VAULT_CALL_ALLOWLIST[$source_relative]}" \
        "$calls_text" "s5-allowlisted-calls-unchanged-$source_relative"
      source_basename_declares_private_mode "$source_basename" &&
        fail "s5-dead-exemption: $source_relative declares private now, so its allowlist entry is dead and must be deleted"
      continue
    fi

    source_basename_declares_private_mode "$source_basename" ||
      WIDENED_SOURCE_FILES+=("$source_relative")
  done <"$list"
}

# Report the classification. Non-zero exit when anything widens, which is the
# whole point of the file; a caller that ignored it would be the one bug this
# guard cannot see from inside.
report_widened_source_files() { # <source-relative>...
  (($# > 0)) || return 0
  printf 'secret-source-files-declare-private-mode: FAIL -- these source files pull from the secret vault but declare target mode 0644, so the next chezmoi apply renders them world-readable:\n' >&2
  printf '  %s\n' "$@" >&2
  printf '  fix: git mv <dir>/<name> <dir>/private_<name> (the target path does not move, and S4 pins that), or add a CALL-level entry to PUBLIC_VALUE_VAULT_CALL_ALLOWLIST saying why the value is public\n' >&2
  return 1
}

# ---------- S4a: the classifier and the report, over a synthetic tree --------
# Every predicate above is pinned individually, but their COMPOSITION was not:
# with only per-predicate tests, `if false` around the report, or a classify
# step that never appends, leaves the fast gate green forever because a healthy
# repo has no violation to notice. So run both steps over a tree that does.

self_test_root="$work/self-test-source"
mkdir -p "$self_test_root"
printf '{{/* keepassxc is only described here */}}\nplain = 1\n' \
  >"$self_test_root/dot_probe-clean.tmpl"
printf 'key = {{ (keepassxc "E").Password }}\n' >"$self_test_root/dot_probe-widened.tmpl"
printf 'key = {{ (keepassxc "E").Password }}\n' >"$self_test_root/private_dot_probe-declared.tmpl"
printf 'age-ciphertext\n' >"$self_test_root/encrypted_probe-widened.age"
printf 'plain\n' >"$self_test_root/private_dot_probe-quiet"
# A file whose only vault reach would be an include this scanner cannot name.
# It must be REPORTED even though it classifies as pulling no secret, which is
# the ordering the classifier gets wrong if the record is read after the skip.
# shellcheck disable=SC2016  # a Go-template variable reference, not a shell one
printf '{{ includeTemplate (printf "%%s.tmpl" $name) . }}\n' \
  >"$self_test_root/dot_probe-unfollowable.tmpl"
self_test_list="$work/self-test-list"
printf '%s\0' \
  dot_probe-clean.tmpl \
  dot_probe-widened.tmpl \
  private_dot_probe-declared.tmpl \
  encrypted_probe-widened.age \
  private_dot_probe-quiet \
  dot_probe-unfollowable.tmpl >"$self_test_list"

classify_managed_source_files "$self_test_root" "$self_test_list"
assert_equal 6 "$MANAGED_FILE_COUNT" s4a-classifier-walks-every-entry
assert_equal "dot_probe-widened.tmpl encrypted_probe-widened.age" \
  "${WIDENED_SOURCE_FILES[*]-}" s4a-classifier-names-exactly-the-violations
assert_equal 1 "${#CLASSIFIER_UNFOLLOWABLE_INCLUDES[@]}" \
  s4a-classifier-records-an-unfollowable-include
case ${CLASSIFIER_UNFOLLOWABLE_INCLUDES[0]-} in
  dot_probe-unfollowable.tmpl*) ;;
  *) fail "s4a-classifier-names-the-unfollowable-include: got '${CLASSIFIER_UNFOLLOWABLE_INCLUDES[0]-}'" ;;
esac

self_test_report="$work/self-test-report"
if report_widened_source_files ${WIDENED_SOURCE_FILES[@]+"${WIDENED_SOURCE_FILES[@]}"} \
  2>"$self_test_report"; then
  fail "s4a-report-fails-on-a-violation: report_widened_source_files returned 0 with a widened file"
fi
grep -q 'dot_probe-widened.tmpl' "$self_test_report" ||
  fail "s4a-report-names-the-violation: the report did not name dot_probe-widened.tmpl"
if ! report_widened_source_files 2>"$work/self-test-clean-report"; then
  fail "s4a-report-passes-a-clean-tree: report_widened_source_files returned non-zero with nothing widened"
fi

# ---------- S4b: the repository ----------------------------------------------

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

classify_managed_source_files "$REPO_ROOT" "$managed_list"
report_widened_source_files ${WIDENED_SOURCE_FILES[@]+"${WIDENED_SOURCE_FILES[@]}"} ||
  failures=$((failures + 1))

# A reach this scanner cannot follow is a hole in the walk, not a clean bill.
for unfollowable in ${CLASSIFIER_UNFOLLOWABLE_INCLUDES[@]+"${CLASSIFIER_UNFOLLOWABLE_INCLUDES[@]}"}; do
  fail "s4-unfollowable-include: $unfollowable"
done

# The universe must not shrink under the walk. A `.chezmoiignore` pattern that
# covers a protected subtree removes those files from the list, and the run then
# reports OK on a smaller count with the violation still on disk.
for pinned_source in "${!SECRET_SOURCE_TARGET_PINS[@]}"; do
  pin_reached=0
  for reached in ${REACHED_TARGET_PIN_KEYS[@]+"${REACHED_TARGET_PIN_KEYS[@]}"}; do
    [[ $reached == "$pinned_source" ]] && pin_reached=1
  done
  ((pin_reached == 1)) ||
    fail "s4-universe-shrank: $pinned_source holds a secret but the walk never reached it (deleted, renamed, or newly covered by a .chezmoiignore pattern); restore it or delete its SECRET_SOURCE_TARGET_PINS row deliberately"
done

# And a rename must not move a target. `chezmoi target-path` exits non-zero on a
# source path it cannot find, so a deleted or renamed pin is caught here too.
declare -a pin_keys=()
declare -a pin_sources=()
declare -a pin_expected_targets=()
for pinned_source in "${!SECRET_SOURCE_TARGET_PINS[@]}"; do
  pin_keys+=("$pinned_source")
  pin_sources+=("$REPO_ROOT/$pinned_source")
  pin_expected_targets+=("${SECRET_SOURCE_TARGET_PINS[$pinned_source]}")
done
pin_destination="$work/pin-destination"
if ! chezmoi --config "$work/managed-config.toml" --source "$REPO_ROOT" \
  --destination "$pin_destination" --no-tty target-path "${pin_sources[@]}" \
  >"$work/pin-targets.txt" 2>"$work/pin-targets.err"; then
  fail "s4-target-pins-unresolvable: chezmoi could not resolve a protected source path: $(cat "$work/pin-targets.err")"
else
  pin_index=0
  while IFS= read -r resolved_target; do
    pin_index=$((pin_index + 1))
    assert_equal "${pin_expected_targets[$((pin_index - 1))]}" \
      "${resolved_target#"$pin_destination/"}" \
      "s4-target-pinned-${pin_keys[$((pin_index - 1))]}"
  done <"$work/pin-targets.txt"
  assert_equal "${#pin_sources[@]}" "$pin_index" s4-every-target-pin-answered
fi

# ---------- S5: no unreachable exemptions ------------------------------------
for allowlisted in "${!PUBLIC_VALUE_VAULT_CALL_ALLOWLIST[@]}"; do
  reached=0
  for entry in ${REACHED_ALLOWLIST_ENTRIES[@]+"${REACHED_ALLOWLIST_ENTRIES[@]}"}; do
    [[ $entry == "$allowlisted" ]] && reached=1
  done
  ((reached == 1)) ||
    fail "s5-unreachable-exemption: $allowlisted is allowlisted but the walk never reached it (moved, no longer managed, or no longer calling the vault); delete the entry"
done

if ((failures > 0)); then
  printf 'secret-source-files-declare-private-mode: %d assertion(s) failed\n' "$failures" >&2
  exit 1
fi
printf 'secret-source-files-declare-private-mode: OK (attribute-sequence grammar re-measured, quote-aware live-action vault calls followed through includeTemplate, classifier and report self-tested, %d managed source files walked, %d protected sources pinned to their targets, %d call-level exemption(s))\n' \
  "$MANAGED_FILE_COUNT" "${#SECRET_SOURCE_TARGET_PINS[@]}" \
  "${#PUBLIC_VALUE_VAULT_CALL_ALLOWLIST[@]}"
