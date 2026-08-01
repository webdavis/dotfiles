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
# sensitive data. Four mechanisms are recognized, each a named constant:
#   * a chezmoi secret template function called from a live `{{ }}` action, in
#     the file itself or in any partial it reaches, through `includeTemplate`
#     or through the `template` action (chezmoi registers every
#     `.chezmoitemplates` entry as a NAMED TEMPLATE, so both spellings reach the
#     same partial; measured against v2.71.1);
#   * a chezmoi template function that EXECUTES a command line (`output`,
#     `outputList`) whose action names a known vault CLI;
#   * the `encrypted_` source-state attribute;
#   * a vault command line run by a `modify_` entry that chezmoi EXECUTES
#     (a `modify_` entry without the modify-template directive is a script, so
#     its output, not its text, becomes the target).
# A hand-written home address, an inline literal, or a value read from the
# environment is invisible to it. So is a command line this scanner cannot read
# as a vault CLI: `{{ output "cat" "/path/to/a/secret" }}` names no vault, and
# `{{ output "sh" "-c" $computed }}` hides the name in a variable. Deciding
# whether an arbitrary command yields a secret is not decidable from the text,
# and refusing every `output` would refuse the three this repo already writes
# (a version string and two cache reads; all three sit outside this walk today,
# but the next one need not). The mechanism rule is what can be enforced without
# judgement.
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
#                          literal, or in ordinary prose does not, UNLESS the
#                          action hands that literal to a function that runs it
#                          (see mechanism 2, where the string IS the call). That
#                          exemption is the guard's brake against its own mirror
#                          defect (a check so strict it demands `private_` on an
#                          ordinary 0644 template). The scanner is quote-aware
#                          in both directions: a `}}` inside a string literal
#                          does not end an action, so a call written after one
#                          cannot hide. Go's three literal forms all count as
#                          literals, including the rune literal `'"'`, whose
#                          inner quote desynchronizes a scanner that knows only
#                          `"` and a backtick.
#   S3 the grammar       - S1 encodes chezmoi's parsing rules, so chezmoi is
#                          asked to confirm every row of them on every run
#                          rather than once by hand. Without this the rules rot
#                          silently on the next chezmoi upgrade, in the
#                          fail-open direction.
#   S4 the tree          - the actual guard, over chezmoi's own list of source
#                          files that become target FILES, plus the three things
#                          that list cannot tell you: that the universe did not
#                          SHRINK under it, that a rename did not MOVE a target,
#                          and that every source the walk finds secret-bearing
#                          is pinned (so the pin table cannot silently lose a
#                          row). The WHOLE enforcement step, not just its
#                          predicates, is exercised over a synthetic tree
#                          engineered to trip every check, so no check can be
#                          disabled while the fast gate stays green.
#   S5 the allowlist     - each exemption names a specific CALL, not a file, and
#                          must still be live. A file allowlisted for fetching
#                          one public value cannot quietly grow a second call.
#
# RUNTIME. Measured 0.77 to 0.83 s on the authoring machine, over the unit
# suite's 200ms WARN threshold. Five chezmoi invocations are most of it and they
# are the ground truth this guard is built on, not incidental work: one applies
# a probe tree to confirm which basenames render 0600, which OS chezmoi targets,
# that every listed vault function name is a real chezmoi function, and that
# both include spellings reach a `.chezmoitemplates` partial; one resolves a
# probe tree's target NAMES to confirm the whole attribute-sequence grammar; one
# asks which source files become target files; and two resolve target paths, one
# for the synthetic self-test tree and one for the protected sources. The
# warning is advisory and never fails a run.
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
  keyring
  lastpass lastpassRaw
  onepassword onepasswordDetailsFields onepasswordDocument
  onepasswordItemFields onepasswordRead
  pass passFields passRaw passhole
  protonPass protonPassJSON
  rbw rbwFields
  secret secretJSON vault
)

# MECHANISM 2: chezmoi template functions that RUN a command line and render its
# output. They turn any vault CLI into a template call, and the command name
# then sits inside a string literal where the vault-FUNCTION search cannot see
# it. Measured present in chezmoi v2.71.1.
COMMAND_EXECUTING_TEMPLATE_FUNCTIONS=(output outputList)

# MECHANISM 2 and 4: vault command lines. Consulted for an action that calls one
# of the command-executing functions above, and for a `modify_` entry that
# chezmoi EXECUTES rather than renders, where the secret arrives through a
# subprocess instead of a template function. Listed by the same argument as the
# function list: a guard that only knows the vault of the day fails open the day
# someone adopts another one. Every name here is a CLI whose purpose is reading
# a secret; deliberately absent are general-purpose tools that merely CAN carry
# one (`aws`, `curl`, `ssh`), because in command position they would refuse
# ordinary scripts.
SECRET_VAULT_COMMANDS=(
  age bw dashlane doppler ejson gopass keeper keepassxc-cli lpass op pass
  passhole rbw secret-tool security sops vault
)

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

# Go's own way to execute a named template. chezmoi registers every
# `.chezmoitemplates` entry under its relative path, so
# `{{ template "p.tmpl" . }}` and `{{ includeTemplate "p.tmpl" . }}` render the
# same partial (measured, v2.71.1, including a nested `dir/p.tmpl` name). Unlike
# `includeTemplate` it ALSO resolves a name defined in the same file, which is
# how this repo already writes `{{ template "shellSingleQuoted" . }}`; such a
# name reaches no new file, because the defining body is scanned as part of the
# file it sits in.
NAMED_TEMPLATE_ACTION=template
TEMPLATE_DEFINITION_ACTIONS=(define block)

# chezmoi's `.chezmoiignore` excludes `Library` on every OS but darwin, so the
# two protected sources under it are legitimately unmanaged elsewhere and their
# absence from the walk is not a shrunken universe.
DARWIN_ONLY_SOURCE_PREFIX='Library/'
CHEZMOI_DARWIN_OPERATING_SYSTEM=darwin

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

# An EMPTY chezmoi config, so every chezmoi question this guard asks is answered
# independently of the authoring machine's own chezmoi configuration.
empty_chezmoi_config="$work/empty-chezmoi-config.toml"
: >"$empty_chezmoi_config"

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
#   C <action>   the action names a secret vault function, or executes a
#                command line naming a vault CLI
#   I <name>     the action reaches the named `.chezmoitemplates` partial
#   U <action>   the action reaches a partial this scanner cannot name
#
# Three things make the classification honest rather than approximate. Comment
# actions are skipped whole, including their contents, so a `}}` written inside
# a comment cannot end an action early and hide what follows it, and a comment
# NOT closed by its own delimiter (Go requires `*/}}` or `*/ -}}` with nothing
# between) is malformed rather than a licence to swallow the next action. String
# and rune literals are transparent to the closing-delimiter search but opaque
# to the name search, so `{{ printf "}}" (keepassxc "E") }}` is one action
# holding a real call, while `{{ "keepassxc" | quote }}` is no call at all. A
# name preceded by `$` or `.` is a variable or a field, not a call.
#
# Unterminated constructs fail CLOSED: the remainder of the file is searched for
# vault names and vault command names with no quote awareness at all, so a
# malformed template cannot swallow a call.
# shellcheck disable=SC2016  # the awk program is a literal, not a shell expression
LIVE_ACTION_SCANNER_PROGRAM='
function is_boundary_before(character) {
  return character !~ /^[A-Za-z0-9_$.]$/
}
function is_boundary_after(character) {
  return character !~ /^[A-Za-z0-9_]$/
}
# A command name may contain the characters a function name may not, so its
# boundaries are wider: `keepassxc-cli` inside `my-keepassxc-cli-wrapper` is a
# different command.
function is_command_boundary(character) {
  return character !~ /^[A-Za-z0-9_.-]$/
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
function next_command_position(haystack, command_name, from,   position, before, after) {
  while (1) {
    position = index(substr(haystack, from), command_name)
    if (position == 0) return 0
    position = from + position - 1
    before = (position == 1) ? "" : substr(haystack, position - 1, 1)
    after = substr(haystack, position + length(command_name), 1)
    from = position + length(command_name)
    if (is_command_boundary(before) && is_command_boundary(after)) return from
  }
}
function names_any_function_from(names, haystack,   i) {
  for (i in names) {
    if (names[i] == "") continue
    if (next_function_position(haystack, names[i], 1) > 0) return 1
  }
  return 0
}
function names_any_command_from(names, haystack,   i) {
  for (i in names) {
    if (names[i] == "") continue
    if (next_command_position(haystack, names[i], 1) > 0) return 1
  }
  return 0
}
function runs_a_vault_command(body, code) {
  return names_any_function_from(command_executing_functions, code) &&
    names_any_command_from(vault_commands, body)
}
function is_literal_delimiter(character) {
  return character == "\"" || character == "`" || character == RUNE_DELIMITER
}
# The value of the first string or rune literal after `from`, skipping over
# whitespace and grouping parentheses so `f ("x")` reads like `f "x"`. Anything
# else in between (a variable, a nested call) means the argument is computed and
# this scanner cannot name it, which is reported rather than assumed harmless.
function first_string_literal_after(body, from,   i, character, quote, literal) {
  i = from
  while (i <= length(body)) {
    character = substr(body, i, 1)
    if (is_literal_delimiter(character)) {
      quote = character
      i++
      literal = ""
      while (i <= length(body)) {
        character = substr(body, i, 1)
        if (quote != "`" && character == "\\") {
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
    if (character ~ /^[ \t\r\n(]$/) { i++; continue }
    return ""
  }
  return ""
}
# The action text as one line, whitespace collapsed OUTSIDE literals only. The
# text is compared against an allowlist entry, so collapsing runs inside a
# literal would make two different vault entry names normalize to one string,
# and a second entry would inherit the exemption written for the first.
function normalize(body,   i, character, quote, out, pending_space, trimmed) {
  trimmed = body
  sub(/^-/, "", trimmed)
  sub(/-[ \t\r\n]*$/, "", trimmed)
  out = ""
  quote = ""
  pending_space = 0
  i = 1
  while (i <= length(trimmed)) {
    character = substr(trimmed, i, 1)
    if (quote != "") {
      out = out character
      if (quote != "`" && character == "\\") {
        out = out substr(trimmed, i + 1, 1)
        i += 2
        continue
      }
      if (character == quote) quote = ""
      i++
      continue
    }
    if (character ~ /^[ \t\r\n]$/) {
      if (out != "") pending_space = 1
      i++
      continue
    }
    if (pending_space) {
      out = out " "
      pending_space = 0
    }
    if (is_literal_delimiter(character)) quote = character
    out = out character
    i++
  }
  return out
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
      if (quote != "`" && character == "\\") {
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
    if (is_literal_delimiter(character)) {
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
  if (names_any_function_from(vault_functions, raw) ||
      names_any_command_from(vault_commands, raw)) print "C " normalize(raw)
}
# Names this action defines. A defined name is reachable by the `template`
# action from this file only (measured: a name defined in another source file is
# not in scope and chezmoi refuses the render), and its body is ordinary text of
# this same file, so it is already scanned.
function record_definitions(body, code,   i, after_name, literal) {
  for (i in definition_actions) {
    if (definition_actions[i] == "") continue
    after_name = next_function_position(code, definition_actions[i], 1)
    if (after_name == 0) continue
    literal = first_string_literal_after(body, after_name)
    if (literal != "") defined_template_names[literal] = 1
  }
}
function emit_partial_references(body, code, reference_name, honour_definitions,
                                 after_name, literal) {
  if (reference_name == "") return
  after_name = 1
  while (1) {
    after_name = next_function_position(code, reference_name, after_name)
    if (after_name == 0) return
    literal = first_string_literal_after(body, after_name)
    if (literal == "") print "U " normalize(body)
    else if (!(honour_definitions && (literal in defined_template_names))) print "I " literal
  }
}
function emit(body, code,   normalized) {
  normalized = normalize(body)
  if (names_any_function_from(vault_functions, code) || runs_a_vault_command(body, code))
    print "C " normalized
  emit_partial_references(body, code, include_function, 0)
  emit_partial_references(body, code, named_template_action, 1)
}
# One pass over every live action. `collect_only` gathers the names this file
# defines, which the emitting pass needs BEFORE it reads the first `template`
# reference, because Go resolves a name defined later in the same file.
function walk_actions(collect_only,   pos, opening, body_start, probe, comment_end, after_comment) {
  pos = 1
  while (pos <= total) {
    opening = index(substr(text, pos), "{{")
    if (opening == 0) return
    opening = pos + opening - 1
    body_start = opening + 2

    probe = body_start
    if (substr(text, probe, 1) == "-") probe++
    while (probe <= total && substr(text, probe, 1) ~ /^[ \t\r\n]$/) probe++

    if (substr(text, probe, 2) == "/*") {
      comment_end = index(substr(text, probe + 2), "*/")
      if (comment_end == 0) {
        if (!collect_only) emit_unterminated(substr(text, body_start))
        return
      }
      after_comment = probe + 2 + comment_end + 1
      if (substr(text, after_comment, 2) == "}}") {
        pos = after_comment + 2
        continue
      }
      if (substr(text, after_comment, 4) == " -}}") {
        pos = after_comment + 4
        continue
      }
      if (!collect_only) emit_unterminated(substr(text, after_comment))
      return
    }

    scan_action(body_start)
    if (ACTION_END == 0) {
      if (!collect_only) emit_unterminated(substr(text, body_start))
      return
    }
    if (collect_only) record_definitions(ACTION_BODY, ACTION_CODE)
    else emit(ACTION_BODY, ACTION_CODE)
    pos = ACTION_END + 2
  }
}
BEGIN {
  # Written as a code point so this program stays one shell-single-quoted
  # literal: a rune literal delimiter is an apostrophe.
  RUNE_DELIMITER = sprintf("%c", 39)
  split(vault_function_names, vault_functions, " ")
  split(vault_command_names, vault_commands, " ")
  split(command_executing_function_names, command_executing_functions, " ")
  split(definition_action_names, definition_actions, " ")
}
{ text = text $0 "\n" }
END {
  total = length(text)
  walk_actions(1)
  walk_actions(0)
}
'

scan_live_actions() { # <path>
  awk -v vault_function_names="${SECRET_VAULT_TEMPLATE_FUNCTIONS[*]}" \
    -v vault_command_names="${SECRET_VAULT_COMMANDS[*]}" \
    -v command_executing_function_names="${COMMAND_EXECUTING_TEMPLATE_FUNCTIONS[*]}" \
    -v definition_action_names="${TEMPLATE_DEFINITION_ACTIONS[*]}" \
    -v include_function="$INCLUDE_TEMPLATE_FUNCTION" \
    -v named_template_action="$NAMED_TEMPLATE_ACTION" \
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

# Every vault call reachable from a template, following both include spellings
# into `.chezmoitemplates`, published in REACHABLE_VAULT_CALLS. A reference this
# scanner cannot follow (a computed argument, or a name with no file behind it)
# is recorded in UNFOLLOWABLE_INCLUDE_REFERENCES rather than skipped, so the
# blind spot is reported instead of assumed harmless. Cycles terminate: a
# partial is scanned at most once per call.
#
# A file the scanner could not READ is neither a call nor a blind spot but a
# failed scan, and a failed scan is not an all-clear: it is recorded as a
# reachable call in its own right, so the file classifies as secret-bearing and
# has to declare 0600 (or be made readable). The scanner runs in a command
# substitution rather than a process substitution for exactly this reason: a
# process substitution discards awk's exit status and the loop then reads zero
# lines, which is indistinguishable from a clean file.
UNSCANNABLE_SOURCE_MARKER='<unscannable source file>'
declare -a REACHABLE_VAULT_CALLS=()
declare -a UNFOLLOWABLE_INCLUDE_REFERENCES=()
read_reachable_vault_calls() { # <source-root> <absolute-path>
  local root=$1 current line partial scan_output
  local -a queue=("$2")
  declare -A visited=()
  REACHABLE_VAULT_CALLS=()
  UNFOLLOWABLE_INCLUDE_REFERENCES=()
  while ((${#queue[@]} > 0)); do
    current=${queue[0]}
    queue=("${queue[@]:1}")
    [[ -n ${visited[$current]+set} ]] && continue
    visited[$current]=1
    if ! scan_output=$(scan_live_actions "$current" 2>&1); then
      REACHABLE_VAULT_CALLS+=("$UNSCANNABLE_SOURCE_MARKER ${current#"$root/"}")
      continue
    fi
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
          UNFOLLOWABLE_INCLUDE_REFERENCES+=("${current#"$root/"} computes a partial name in '${line:2}', which this scanner cannot follow")
          ;;
      esac
    done <<<"$scan_output"
  done
}

# Does this text RUN one of the vault CLIs? Backslash, double-quote and
# apostrophe are removed first, because the shell removes them too:
# `keepassxc\-cli` and `"keepassxc-cli"` both execute keepassxc-cli, and a
# fixed-string search for the plain spelling sees neither. The name then has to
# stand in COMMAND POSITION, at the start of the text or after a shell
# separator, which is what keeps a name inside a comment or inside an argument
# from counting: `# see keepassxc-cli` and `printf keepassxc-cli` are not
# invocations. It is deliberately a heuristic over shell text, not a shell
# parser, and it errs toward reporting.
SHELL_COMMAND_POSITION_PREFIX='(^|[;&|(){}`]|\$\()[[:space:]]*'
SHELL_COMMAND_POSITION_SUFFIX='([[:space:]]|[;&|)]|$)'
text_invokes_vault_command() { # <text>
  local unquoted=$1 alternation
  unquoted=${unquoted//\\/}
  unquoted=${unquoted//\"/}
  unquoted=${unquoted//\'/}
  alternation=$(
    IFS='|'
    printf '%s' "${SECRET_VAULT_COMMANDS[*]}"
  )
  grep -qE -- \
    "$SHELL_COMMAND_POSITION_PREFIX($alternation)$SHELL_COMMAND_POSITION_SUFFIX" \
    <<<"$unquoted"
}

# A `modify_` entry without the modify-template directive is executed, so its
# secret arrives through a command line rather than a template function. A read
# failure counts as a hit: an unreadable file is not an all-clear.
executed_entry_invokes_vault_command() { # <absolute-path>
  local contents
  contents=$(<"$1") || return 0
  text_invokes_vault_command "$contents"
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
# The three names this list was missing. All three are real functions of the
# installed chezmoi (S3 re-measures that on every run), so a template using one
# renders a secret into whatever mode its basename declares.
vault_call_fixture keyring-call 1 'token = {{ keyring "service" "user" }}
'
vault_call_fixture proton-pass-call 1 'token = {{ protonPass "item" }}
'
vault_call_fixture proton-pass-json-call 1 'token = {{ protonPassJSON "item" }}
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
# RUNE LITERALS. Go has three literal forms and the third one is a quote
# delimiter that carries a quote: `{{ printf "%c" (a rune holding a double
# quote) }}` renders one `"` (measured). A scanner that knows only `"` and the
# backtick reads that inner quote as the START of a string, and the NEXT rune
# literal ends it, so everything between two of them is blanked out of the code
# view. One occurrence alone runs to end of file and fails closed; the pair is
# what hides a call, so the pair is the fixture.
vault_call_fixture rune-literal-pair-around-a-call 1 "x = {{ printf \"%c\" '\"' }}
key = {{ (keepassxc \"E\").Password }}
z = {{ printf \"%c\" '\"' }}
"
# Unterminated constructs fail CLOSED.
vault_call_fixture unterminated-action 1 'x = {{ (keepassxc "E").Password
'
vault_call_fixture unterminated-comment 1 'x = {{/* keepassxc
'
# A Go comment must be closed by its OWN delimiter: chezmoi refuses
# `{{/* c */ }}` with "comment ends before closing delimiter" (measured), so a
# `*/` followed by anything other than `}}` or ` -}}` is malformed, and the next
# `}}` in the file belongs to a LATER action. A scanner that consumes it anyway
# swallows that action whole.
vault_call_fixture comment-not-closed-by-its-own-delimiter 1 '{{/* benign */
key = {{ keepassxc "E" }}
'
# COMMAND-EXECUTING FUNCTIONS. `output` runs its argument and renders the
# result, so the vault CLI name sits in a string literal where the vault
# FUNCTION search cannot see it. Measured: with a stub `keepassxc-cli` on PATH,
# chezmoi renders the fixture below into a 0644 target holding its output.
vault_call_fixture output-runs-a-vault-command 1 'aws_secret_access_key = {{ output "keepassxc-cli" "show" "-a" "Password" "/db.kdbx" "AWS" | trim }}
'
vault_call_fixture output-runs-a-vault-command-through-a-shell 1 'x = {{ output "sh" "-c" "op read op://vault/item/field" }}
'
vault_call_fixture output-list-runs-a-vault-command 1 'x = {{ outputList "keepassxc-cli" "show" }}
'
# The mirror: naming a command without running one is not a call, which is why
# the existing `lookPath` fixture above stays at zero, and running a command
# that is not a vault CLI is not a call either.
vault_call_fixture output-runs-an-ordinary-command 0 'stamp = {{ output "date" "+%s" | trim }}
'
vault_call_fixture vault-command-inside-a-word 0 'x = {{ output "my-keepassxc-cli-wrapper" }}
'
printf 'k = {{ keepassxcAttribute "E" "A" }}\n' >"$work/fixture-call-text"
assert_equal 'keepassxcAttribute "E" "A"' \
  "$(template_vault_calls "$work/fixture-call-text")" s2-call-text-normalized
# Whitespace INSIDE a literal is part of the value. The call text is what the
# allowlist matches on, so collapsing runs inside a literal would make a
# different vault entry normalize onto an existing exemption and inherit it.
printf 'k = {{ keepassxcAttribute "E  n" "A  t" }}\n' >"$work/fixture-inner-spacing"
assert_equal 'keepassxcAttribute "E  n" "A  t"' \
  "$(template_vault_calls "$work/fixture-inner-spacing")" s2-inner-literal-spacing-kept

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

# MECHANISM 4, an executed `modify_` entry that shells out to the vault.
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

# The shell removes quoting before it resolves a command name, so a search for
# the plain spelling has to remove it too. Each of these executes the same
# binary; a fixed-string search for `keepassxc-cli` finds none of them.
assert_predicate expect-true s2c-backslash-escaped-command-name \
  text_invokes_vault_command 'keepassxc\-cli show -a Password db.kdbx AWS'
assert_predicate expect-true s2c-quoted-command-name \
  text_invokes_vault_command '"keepassxc-cli" show'
# shellcheck disable=SC2016  # the shell substitution is the fixture text
assert_predicate expect-true s2c-command-name-in-a-substitution \
  text_invokes_vault_command 'secret=$(keepassxc-cli show)'
assert_predicate expect-true s2c-command-name-after-a-pipe \
  text_invokes_vault_command 'printf x | op read op://vault/item/field'
assert_predicate expect-true s2c-indented-command-name \
  text_invokes_vault_command '  security find-generic-password -w'
# And the mirror. Naming a vault CLI is not running one, which is the whole
# difference between a comment or an argument and an invocation.
assert_predicate expect-false s2c-command-name-in-a-comment \
  text_invokes_vault_command '# see keepassxc-cli show for the manual step'
assert_predicate expect-false s2c-command-name-as-an-argument \
  text_invokes_vault_command 'printf "%s" keepassxc-cli'
assert_predicate expect-false s2c-command-name-inside-a-word \
  text_invokes_vault_command 'my-keepassxc-cli-wrapper show'
assert_predicate expect-false s2c-no-vault-command-at-all \
  text_invokes_vault_command 'printf "%s" "no secret here"'

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

# The OTHER include spelling. chezmoi registers every `.chezmoitemplates` entry
# as a NAMED template, so Go's own `template` action reaches the same partial as
# `includeTemplate` (measured, and S3 re-measures it on every run). Following
# only one spelling means one word between a caught secret and a missed one.
printf '{{ template "secret-partial.tmpl" . }}\n' \
  >"$work/reach-root/dot_probe-named-template.tmpl"
assert_predicate expect-true s2b-named-template-action-reaches-a-vault-call \
  source_file_pulls_secrets "$work/reach-root" \
  "$work/reach-root/dot_probe-named-template.tmpl" dot_probe-named-template.tmpl
# A name the file DEFINES itself reaches no other file: the defining body is
# ordinary text of this same file and is already scanned. Treating it as a
# partial reference would report an unfollowable include for every one of the
# eight `{{ template "shellSingleQuoted" . }}` calls this repo already writes.
printf '{{- define "local-helper" }}{{ upper . }}{{ end -}}\nx = {{ template "local-helper" .name }}\n' \
  >"$work/reach-root/dot_probe-local-define.tmpl"
assert_predicate expect-false s2b-locally-defined-name-is-not-a-partial \
  source_file_pulls_secrets "$work/reach-root" \
  "$work/reach-root/dot_probe-local-define.tmpl" dot_probe-local-define.tmpl
assert_equal "" "${UNFOLLOWABLE_INCLUDE_REFERENCES[*]-}" s2b-local-define-is-not-unfollowable
# ... and a local definition does not hide a call written inside it.
printf '{{- define "local-secret" }}{{ (keepassxc "E").Password }}{{ end -}}\nx = {{ template "local-secret" . }}\n' \
  >"$work/reach-root/dot_probe-local-define-secret.tmpl"
assert_predicate expect-true s2b-call-inside-a-local-define-still-counts \
  source_file_pulls_secrets "$work/reach-root" \
  "$work/reach-root/dot_probe-local-define-secret.tmpl" dot_probe-local-define-secret.tmpl

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
# A parenthesized literal is still a literal: `includeTemplate ("p.tmpl")`
# renders exactly like `includeTemplate "p.tmpl"`, so reporting it as
# unfollowable is a false refusal of a legitimate template.
printf '{{ includeTemplate ("secret-partial.tmpl") . }}\n' \
  >"$work/reach-root/dot_probe-parenthesized-include.tmpl"
assert_predicate expect-true s2b-parenthesized-include-literal-is-followed \
  source_file_pulls_secrets "$work/reach-root" \
  "$work/reach-root/dot_probe-parenthesized-include.tmpl" dot_probe-parenthesized-include.tmpl
assert_equal "" "${UNFOLLOWABLE_INCLUDE_REFERENCES[*]-}" \
  s2b-parenthesized-include-is-not-unfollowable

# A file the scanner cannot READ is not a clean file. awk reports the failure on
# stderr and exits non-zero; read through a process substitution that status is
# discarded and the loop reads zero lines, which looks exactly like a template
# with no calls in it.
unreadable_root="$work/unreadable-source"
mkdir -p "$unreadable_root"
printf 'key = {{ (keepassxc "E").Password }}\n' >"$unreadable_root/dot_probe-unreadable.tmpl"
chmod 000 "$unreadable_root/dot_probe-unreadable.tmpl"
# Running as a user that ignores the mode bits (root, or a filesystem without
# them) would make the assertions pass for the wrong reason, so record whether
# the fixture is meaningful instead of asserting on no evidence. S4a asks the
# same question of the WALK, and reads this same flag.
unreadable_probe_is_meaningful=1
if [[ -r "$unreadable_root/dot_probe-unreadable.tmpl" ]]; then
  unreadable_probe_is_meaningful=0
  printf 'secret-source-files-declare-private-mode: NOTE -- skipping the unreadable-source assertions, this user can read a 000 file\n' >&2
else
  assert_predicate expect-true s2b-unreadable-source-is-not-an-all-clear \
    source_file_pulls_secrets "$unreadable_root" \
    "$unreadable_root/dot_probe-unreadable.tmpl" dot_probe-unreadable.tmpl
fi

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

PROBE_PARTIAL_RENDERED_TEXT='partial-body-reached'
# Which OS chezmoi targets. Defaulted to darwin, the value under which EVERY
# protected source is expected in the walk, so a probe that could not answer
# leaves the strictest reading in place rather than excusing an absence.
chezmoi_operating_system=$CHEZMOI_DARWIN_OPERATING_SYSTEM

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
  # Which OS chezmoi is targeting, asked of chezmoi rather than of `uname`,
  # because it is chezmoi's answer that decides which sources are managed.
  printf '{{ .chezmoi.os }}' >"$mode_source/dot_probe-operating-system.tmpl"
  # Both include spellings, against a partial in the directory the constants
  # NAME, so CHEZMOI_TEMPLATES_DIRECTORY, INCLUDE_TEMPLATE_FUNCTION and
  # NAMED_TEMPLATE_ACTION stop being their own oracle. chezmoi is what resolves
  # these: point any of the three at a name chezmoi does not use and the apply
  # below fails, where before the tree walk would simply stop following
  # partials and report nothing.
  mkdir -p "$mode_source/$CHEZMOI_TEMPLATES_DIRECTORY"
  printf '%s' "$PROBE_PARTIAL_RENDERED_TEXT" \
    >"$mode_source/$CHEZMOI_TEMPLATES_DIRECTORY/probe-partial.tmpl"
  printf '{{ %s "probe-partial.tmpl" . }}' "$INCLUDE_TEMPLATE_FUNCTION" \
    >"$mode_source/dot_probe-include-spelling.tmpl"
  printf '{{ %s "probe-partial.tmpl" . }}' "$NAMED_TEMPLATE_ACTION" \
    >"$mode_source/dot_probe-named-template-spelling.tmpl"
  # Every vault function name, referenced but never executed. Go resolves
  # function names at PARSE time, so an apply that succeeds proves each name is
  # a real chezmoi function; one that has been retired upstream names itself in
  # the failure. (This direction only: no probe can prove the list is
  # COMPLETE.)
  printf '{{ if false }}%s{{ end }}' \
    "$(printf '{{ %s }}' "${SECRET_VAULT_TEMPLATE_FUNCTIONS[@]}" "${COMMAND_EXECUTING_TEMPLATE_FUNCTIONS[@]}")" \
    >"$mode_source/dot_probe-vault-function-names.tmpl"

  if ! chezmoi --config "$empty_chezmoi_config" --source "$mode_source" \
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
    assert_equal "$PROBE_PARTIAL_RENDERED_TEXT" \
      "$(cat "$mode_dest/.probe-include-spelling")" \
      s3-includeTemplate-reaches-the-templates-directory
    assert_equal "$PROBE_PARTIAL_RENDERED_TEXT" \
      "$(cat "$mode_dest/.probe-named-template-spelling")" \
      s3-template-action-reaches-the-templates-directory
    chezmoi_operating_system="$(cat "$mode_dest/.probe-operating-system")"
    [[ -n $chezmoi_operating_system ]] ||
      fail "s3-operating-system-answered: chezmoi rendered an empty .chezmoi.os"
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

  # A destination whose PATH contains the word `private`, deliberately. Every
  # row below asks whether chezmoi consumed the `private` attribute, and the
  # answer is only in the target BASENAME; a check that reads the whole path
  # answers "retained" for all 15 rows, which is six false failures and nine
  # silent passes. A real TMPDIR of `/private/tmp` does exactly this, so the
  # hazard is built into the fixture rather than left to the environment.
  name_probe_destination="$work/probe-name-dest/private-parent"
  if ! chezmoi --config "$empty_chezmoi_config" --source "$name_source" \
    --destination "$name_probe_destination" --no-tty target-path \
    "${name_probe_arguments[@]}" >"$work/probe-names.txt" 2>"$work/probe-names.err"; then
    fail "the chezmoi name probe failed: $(cat "$work/probe-names.err")"
  else
    probe_index=0
    # The BASENAME, never the whole path: the probe tree sits under a temporary
    # directory whose name this guard does not choose, and a TMPDIR of
    # `/private/tmp` (the default under the flake's coreutils mktemp when TMPDIR
    # is set to it) puts the word `private` in every path. That reads as
    # "chezmoi kept the attribute" for the consumed rows, six false failures,
    # and as "chezmoi kept the attribute" for the retained rows too, where it is
    # the silent direction: every negative row would pass without measuring
    # anything.
    while IFS= read -r probe_target; do
      probe_index=$((probe_index + 1))
      probe_source=${name_probe_arguments[$((probe_index - 1))]}
      probe_target_basename=${probe_target##*/}
      case ${name_probe_expect_private[$((probe_index - 1))]} in
        consumed)
          [[ $probe_target_basename != *"$PRIVATE_ATTRIBUTE"* ]] ||
            fail "s3-sequence-declares-private: chezmoi left '$PRIVATE_ATTRIBUTE' in the target of ${probe_source##*/} ('$probe_target_basename'), so that sequence no longer declares 0600 and CHEZMOI_ATTRIBUTE_PREFIXES_DECLARING_PRIVATE has a stale row"
          ;;
        retained)
          [[ $probe_target_basename == *"$PRIVATE_ATTRIBUTE"* ]] ||
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
declare -a SECRET_BEARING_SOURCE_FILES=()
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
  SECRET_BEARING_SOURCE_FILES=()
  CLASSIFIER_UNFOLLOWABLE_INCLUDES=()
  MANAGED_FILE_COUNT=0
  while IFS= read -r -d '' source_relative; do
    absolute="$root/$source_relative"
    source_basename=${source_relative##*/}
    MANAGED_FILE_COUNT=$((MANAGED_FILE_COUNT + 1))
    if [[ -n ${SECRET_SOURCE_TARGET_PINS[$source_relative]+set} ]]; then
      REACHED_TARGET_PIN_KEYS+=("$source_relative")
    fi
    # READABLE, not merely present: a file the walk cannot open is one the walk
    # cannot clear, and every classification below would answer "no secret" for
    # it on no evidence.
    if [[ ! -f $absolute || ! -r $absolute ]]; then
      fail "chezmoi lists $source_relative but it is not a readable file"
      continue
    fi
    pulls_secrets=0
    if source_file_pulls_secrets "$root" "$absolute" "$source_basename"; then
      pulls_secrets=1
      SECRET_BEARING_SOURCE_FILES+=("$source_relative")
    fi
    # Recorded whatever the verdict was. A file whose ONLY vault reach is an
    # include this scanner cannot follow answers "no secret" above, so checking
    # this after a `continue` would drop exactly the case it exists for.
    #
    # A source that already declares 0600 is exempt: whatever the partial holds,
    # its target is not world-readable, so an unnameable reach out of it is not
    # a widening and refusing it would be a false demand for a rename that has
    # already happened.
    if ! source_basename_declares_private_mode "$source_basename"; then
      for reference in ${UNFOLLOWABLE_INCLUDE_REFERENCES[@]+"${UNFOLLOWABLE_INCLUDE_REFERENCES[@]}"}; do
        CLASSIFIER_UNFOLLOWABLE_INCLUDES+=("$reference")
      done
    fi
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

# Is a pinned source expected in the walk at all on this operating system?
# `.chezmoiignore` drops `Library` everywhere but darwin, so on linux those
# sources are legitimately unmanaged and their absence is not a shrunken
# universe. Every other source, and every source on darwin, must be reached.
secret_source_is_managed_on_operating_system() { # <operating-system> <source-relative>
  [[ $1 == "$CHEZMOI_DARWIN_OPERATING_SYSTEM" ]] && return 0
  [[ $2 != "$DARWIN_ONLY_SOURCE_PREFIX"* ]]
}

# The whole enforcement step over one source tree: classify, then ask the four
# questions the classification alone cannot answer. It reports through `fail`,
# so a caller cannot drop its status, and it takes its tree as arguments, so the
# S4a self-test runs THIS function rather than a copy of its loops. Without
# that, disabling any one of these loops leaves a healthy repo green forever.
enforce_managed_source_tree() { # <source-root> <list-file> <operating-system> <target-path-destination>
  local root=$1 list=$2 operating_system=$3 destination=$4
  local unfollowable secret_bearing pinned_source reached pin_reached allowlisted
  local entry resolved_target pin_index
  local -a pin_keys=() pin_sources=() pin_expected_targets=()

  classify_managed_source_files "$root" "$list"
  report_widened_source_files ${WIDENED_SOURCE_FILES[@]+"${WIDENED_SOURCE_FILES[@]}"} ||
    failures=$((failures + 1))

  # A reach this scanner cannot follow is a hole in the walk, not a clean bill.
  for unfollowable in ${CLASSIFIER_UNFOLLOWABLE_INCLUDES[@]+"${CLASSIFIER_UNFOLLOWABLE_INCLUDES[@]}"}; do
    fail "s4-unfollowable-include: $unfollowable"
  done

  # Every source the walk finds secret-bearing must be pinned. The pins are the
  # only thing that notices a shrunken universe or a moved target, so a table
  # that quietly lost a row would leave that source unwatched while the run
  # still says OK. Derived from the walk, not from the table, so the table
  # cannot be its own oracle.
  for secret_bearing in ${SECRET_BEARING_SOURCE_FILES[@]+"${SECRET_BEARING_SOURCE_FILES[@]}"}; do
    [[ -n ${SECRET_SOURCE_TARGET_PINS[$secret_bearing]+set} ]] ||
      fail "s4-unpinned-secret-source: $secret_bearing pulls from the secret vault but has no SECRET_SOURCE_TARGET_PINS row, so nothing would notice it leaving the walk or its target moving; add one naming its target path"
  done

  # The universe must not shrink under the walk. A `.chezmoiignore` pattern that
  # covers a protected subtree removes those files from the list, and the run
  # then reports OK on a smaller count with the violation still on disk.
  for pinned_source in "${!SECRET_SOURCE_TARGET_PINS[@]}"; do
    secret_source_is_managed_on_operating_system "$operating_system" "$pinned_source" ||
      continue
    pin_reached=0
    for reached in ${REACHED_TARGET_PIN_KEYS[@]+"${REACHED_TARGET_PIN_KEYS[@]}"}; do
      [[ $reached == "$pinned_source" ]] && pin_reached=1
    done
    ((pin_reached == 1)) ||
      fail "s4-universe-shrank: $pinned_source holds a secret but the walk never reached it (deleted, renamed, or newly covered by a .chezmoiignore pattern); restore it or delete its SECRET_SOURCE_TARGET_PINS row deliberately"
  done

  # And a rename must not move a target. `chezmoi target-path` exits non-zero on
  # a source path it cannot find, so a deleted or renamed pin is caught here
  # too, on every OS: it reads the source tree and never the ignore rules.
  for pinned_source in "${!SECRET_SOURCE_TARGET_PINS[@]}"; do
    pin_keys+=("$pinned_source")
    pin_sources+=("$root/$pinned_source")
    pin_expected_targets+=("${SECRET_SOURCE_TARGET_PINS[$pinned_source]}")
  done
  if ((${#pin_sources[@]} > 0)); then
    if ! chezmoi --config "$empty_chezmoi_config" --source "$root" \
      --destination "$destination" --no-tty target-path "${pin_sources[@]}" \
      >"$work/pin-targets.txt" 2>"$work/pin-targets.err"; then
      fail "s4-target-pins-unresolvable: chezmoi could not resolve a protected source path: $(cat "$work/pin-targets.err")"
    else
      pin_index=0
      while IFS= read -r resolved_target; do
        pin_index=$((pin_index + 1))
        assert_equal "${pin_expected_targets[$((pin_index - 1))]}" \
          "${resolved_target#"$destination/"}" \
          "s4-target-pinned-${pin_keys[$((pin_index - 1))]}"
      done <"$work/pin-targets.txt"
      assert_equal "${#pin_sources[@]}" "$pin_index" s4-every-target-pin-answered
    fi
  fi

  # S5: no unreachable exemptions. An exemption whose file the walk never
  # reaches has stopped being reviewed, so it must be deleted rather than left
  # to vouch for a path that no longer exists.
  for allowlisted in "${!PUBLIC_VALUE_VAULT_CALL_ALLOWLIST[@]}"; do
    reached=0
    for entry in ${REACHED_ALLOWLIST_ENTRIES[@]+"${REACHED_ALLOWLIST_ENTRIES[@]}"}; do
      [[ $entry == "$allowlisted" ]] && reached=1
    done
    ((reached == 1)) ||
      fail "s5-unreachable-exemption: $allowlisted is allowlisted but the walk never reached it (moved, no longer managed, or no longer calling the vault); delete the entry"
  done
}

# ---------- S4a: the whole enforcement step, over a synthetic tree -----------
# Every predicate above is pinned individually, but their COMPOSITION was not:
# with only per-predicate tests, `if false` around the report, a classify step
# that never appends, or any one of the four enforcement loops deleted, leaves
# the fast gate green forever because a healthy repo has no violation to notice.
# So run the REAL enforcement function over a tree engineered to trip every
# check at once, and count the reports.

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
# The mirror of that file: the same unnameable reach out of a source that
# ALREADY declares 0600, where there is no widening to report and a report
# would be a demand to rename a file that is correctly named.
# shellcheck disable=SC2016  # a Go-template variable reference, not a shell one
printf '{{ includeTemplate (printf "%%s.tmpl" $name) . }}\n' \
  >"$self_test_root/private_dot_probe-unfollowable-but-private.tmpl"
# On disk and pinned, but absent from the list: the shape a `.chezmoiignore`
# pattern produces, and the one thing the list cannot report about itself.
printf 'key = {{ (keepassxc "E").Password }}\n' >"$self_test_root/private_dot_probe-unlisted.tmpl"
self_test_list="$work/self-test-list"
printf '%s\0' \
  dot_probe-clean.tmpl \
  dot_probe-widened.tmpl \
  private_dot_probe-declared.tmpl \
  encrypted_probe-widened.age \
  private_dot_probe-quiet \
  dot_probe-unfollowable.tmpl \
  private_dot_probe-unfollowable-but-private.tmpl >"$self_test_list"

classify_managed_source_files "$self_test_root" "$self_test_list"
assert_equal 7 "$MANAGED_FILE_COUNT" s4a-classifier-walks-every-entry
assert_equal "dot_probe-widened.tmpl encrypted_probe-widened.age" \
  "${WIDENED_SOURCE_FILES[*]-}" s4a-classifier-names-exactly-the-violations
assert_equal "dot_probe-widened.tmpl private_dot_probe-declared.tmpl encrypted_probe-widened.age" \
  "${SECRET_BEARING_SOURCE_FILES[*]-}" s4a-classifier-names-every-secret-bearing-source
assert_equal 1 "${#CLASSIFIER_UNFOLLOWABLE_INCLUDES[@]}" \
  s4a-classifier-records-an-unfollowable-include-of-a-widened-source
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

# The walk REFUSES an entry it cannot read rather than skipping it. Presence is
# not readability: a 000 file is a regular file, so a `-f` test clears it and
# every classification then answers "no secret" without having read a byte.
if ((unreadable_probe_is_meaningful == 1)); then
  printf '%s\0' dot_probe-unreadable.tmpl >"$work/unreadable-list"
  failures_before_unreadable_walk=$failures
  classify_managed_source_files "$unreadable_root" "$work/unreadable-list" \
    2>"$work/unreadable-report"
  unreadable_walk_findings=$((failures - failures_before_unreadable_walk))
  failures=$failures_before_unreadable_walk
  assert_equal 1 "$unreadable_walk_findings" s4a-walk-refuses-an-unreadable-entry
  grep -q 'not a readable file' "$work/unreadable-report" ||
    fail "s4a-walk-names-the-unreadable-entry: got '$(tr '\n' '|' <"$work/unreadable-report")'"
fi
chmod 600 "$unreadable_root/dot_probe-unreadable.tmpl"

assert_predicate expect-true s4a-darwin-expects-every-pinned-source \
  secret_source_is_managed_on_operating_system "$CHEZMOI_DARWIN_OPERATING_SYSTEM" \
  "${DARWIN_ONLY_SOURCE_PREFIX}Application Support/probe"
assert_predicate expect-false s4a-linux-does-not-expect-a-darwin-only-source \
  secret_source_is_managed_on_operating_system linux \
  "${DARWIN_ONLY_SOURCE_PREFIX}Application Support/probe"
assert_predicate expect-true s4a-linux-still-expects-every-other-source \
  secret_source_is_managed_on_operating_system linux dot_aws/private_credentials.tmpl

# Now the enforcement step itself, over the same tree, with the policy tables
# swapped for synthetic ones so each of the four loops has exactly one thing to
# report. `failures` is restored afterwards: these findings are the fixtures,
# not the repository's.
saved_policy_tables="$(declare -p SECRET_SOURCE_TARGET_PINS PUBLIC_VALUE_VAULT_CALL_ALLOWLIST)"
SECRET_SOURCE_TARGET_PINS=(
  ["dot_probe-widened.tmpl"]=".probe-widened"
  ["private_dot_probe-declared.tmpl"]=".probe-declared-MOVED"
  ["private_dot_probe-unlisted.tmpl"]=".probe-unlisted"
)
PUBLIC_VALUE_VAULT_CALL_ALLOWLIST=(
  ["dot_probe-never-walked.tmpl"]='keepassxc "Nowhere"'
)
failures_before_self_test=$failures
enforce_managed_source_tree "$self_test_root" "$self_test_list" \
  "$CHEZMOI_DARWIN_OPERATING_SYSTEM" "$work/self-test-pin-destination" \
  2>"$work/self-test-enforcement-report"
self_test_findings=$((failures - failures_before_self_test))
failures=$failures_before_self_test
eval "$saved_policy_tables"

# One report per check: widened, unfollowable, unpinned secret-bearing source,
# shrunken universe, moved target, unreachable exemption.
assert_equal 6 "$self_test_findings" s4a-enforcement-reports-every-defect
for expected_report in \
  'declare target mode 0644' \
  's4-unfollowable-include: dot_probe-unfollowable.tmpl' \
  's4-unpinned-secret-source: encrypted_probe-widened.age' \
  's4-universe-shrank: private_dot_probe-unlisted.tmpl' \
  's4-target-pinned-private_dot_probe-declared.tmpl' \
  's5-unreachable-exemption: dot_probe-never-walked.tmpl'; do
  grep -qF -e "$expected_report" "$work/self-test-enforcement-report" ||
    fail "s4a-enforcement-names-its-finding: no report matching '$expected_report' in $(tr '\n' '|' <"$work/self-test-enforcement-report")"
done

# The false-positive control. Without it every assertion above would also pass
# against an enforcement step that reports on everything it is handed.
clean_test_root="$work/clean-test-source"
mkdir -p "$clean_test_root"
printf 'key = {{ (keepassxc "E").Password }}\n' >"$clean_test_root/private_dot_probe-clean.tmpl"
printf 'plain = 1\n' >"$clean_test_root/dot_probe-plain"
clean_test_list="$work/clean-test-list"
printf '%s\0' private_dot_probe-clean.tmpl dot_probe-plain >"$clean_test_list"
saved_policy_tables="$(declare -p SECRET_SOURCE_TARGET_PINS PUBLIC_VALUE_VAULT_CALL_ALLOWLIST)"
SECRET_SOURCE_TARGET_PINS=(["private_dot_probe-clean.tmpl"]=".probe-clean")
PUBLIC_VALUE_VAULT_CALL_ALLOWLIST=()
failures_before_clean_test=$failures
enforce_managed_source_tree "$clean_test_root" "$clean_test_list" \
  "$CHEZMOI_DARWIN_OPERATING_SYSTEM" "$work/clean-test-pin-destination" \
  2>"$work/clean-test-enforcement-report"
clean_test_findings=$((failures - failures_before_clean_test))
failures=$failures_before_clean_test
eval "$saved_policy_tables"
assert_equal 0 "$clean_test_findings" s4a-enforcement-passes-a-clean-tree
assert_equal 2 "$MANAGED_FILE_COUNT" s4a-enforcement-walked-the-clean-tree

# ---------- S4b: the repository ----------------------------------------------

managed_list="$work/managed-source-files"
if ! chezmoi --config "$empty_chezmoi_config" --source "$REPO_ROOT" \
  --destination "$work/unused-destination" --no-tty managed --include=files \
  --nul-path-separator --path-style=source-relative >"$managed_list" \
  2>"$work/managed.err"; then
  printf 'secret-source-files-declare-private-mode: FAIL -- chezmoi managed failed; refusing to pass on a partial list: %s\n' \
    "$(cat "$work/managed.err")" >&2
  exit 1
fi
[[ -s $managed_list ]] ||
  fail "chezmoi managed listed no target files at all, so the guard would have nothing to check"

enforce_managed_source_tree "$REPO_ROOT" "$managed_list" \
  "$chezmoi_operating_system" "$work/pin-destination"

# The walk must have consumed the repository's list and not some earlier one.
# Deleting the call above is the only mutation the self-test cannot see, and
# this is what sees it: the count would still be the synthetic tree's.
managed_list_entry_count="$(tr -cd '\0' <"$managed_list" | wc -c | tr -d ' ')"
assert_equal "$managed_list_entry_count" "$MANAGED_FILE_COUNT" \
  s4-walk-consumed-the-whole-managed-list

if ((failures > 0)); then
  printf 'secret-source-files-declare-private-mode: %d assertion(s) failed\n' "$failures" >&2
  exit 1
fi
printf 'secret-source-files-declare-private-mode: OK (attribute-sequence grammar re-measured, quote-aware live-action vault calls followed through both include spellings, whole enforcement step self-tested against a tripping tree and a clean one, %d managed source files walked on %s, %d protected sources pinned to their targets, %d call-level exemption(s))\n' \
  "$MANAGED_FILE_COUNT" "$chezmoi_operating_system" "${#SECRET_SOURCE_TARGET_PINS[@]}" \
  "${#PUBLIC_VALUE_VAULT_CALL_ALLOWLIST[@]}"
