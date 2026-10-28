#!/bin/bash
# The drop-in file holds the hardening. Removing it returns sshd to its defaults at the next restart.
# Only --reload restarts sshd. Install and --rollback change files and never restart it.
set -euo pipefail

# sshd matches keywords and yes/no values case-insensitively, so every comparison here does too.
shopt -s nocasematch

SSHD_CONFIG_D="${SSHD_CONFIG_D:-/etc/ssh/sshd_config.d}"
SSHD_MAIN_CONFIG="${SSHD_MAIN_CONFIG:-/etc/ssh/sshd_config}"
SSHD_BIN="${SSHD_BIN:-/usr/sbin/sshd}"
# An empty value runs commands directly without sudo, as tests do against a sandbox.
SSH_HARDENING_SUDO="${SSH_HARDENING_SUDO-sudo}"
LAUNCHCTL_BIN="${LAUNCHCTL_BIN:-/bin/launchctl}"
KEYSCAN_BIN="${KEYSCAN_BIN:-/usr/bin/ssh-keyscan}"
# An absolute path, because a missing sleep would stop the readiness loop after the restart
# without printing the recovery steps.
SLEEP_BIN="${SLEEP_BIN:-/bin/sleep}"
# An absolute path, because `stat -f` is BSD syntax
# and a GNU stat earlier on PATH reads it differently.
STAT_BIN="${STAT_BIN:-/usr/bin/stat}"
CKSUM_BIN="${CKSUM_BIN:-/usr/bin/cksum}"
# Set-but-empty is refused by validate_readiness_knobs instead of falling back to the default.
SSH_HARDENING_READY_ATTEMPTS="${SSH_HARDENING_READY_ATTEMPTS-30}"
SSH_HARDENING_READY_INTERVAL="${SSH_HARDENING_READY_INTERVAL-1}"
SSH_HARDENING_PROBE_TIMEOUT="${SSH_HARDENING_PROBE_TIMEOUT-5}"

# launchctl print exits 0 when the service is loaded and 113 when Remote Login is off.
# Any other status is a probe error, never "the daemon is stopped".
SSHD_LAUNCHD_SERVICE='system/com.openssh.sshd'
LAUNCHCTL_STATUS_SERVICE_ABSENT=113

# Uses BASH_SOURCE so a caller that rewrites $0 cannot redirect the child verify.
SSH_HARDENING_SELF="${BASH_SOURCE[0]}"

# sshd resolves a relative Include against its main config's directory, not the working directory.
SSHD_CONFIG_DIR="${SSHD_MAIN_CONFIG%/*}"
if [[ $SSHD_CONFIG_DIR == "$SSHD_MAIN_CONFIG" ]]; then
  SSHD_CONFIG_DIR='.'
fi

DROPIN_NAME="000-ssh-hardening.conf"
LEGACY_DROPIN_NAME="50-no-password-auth.conf"

# sshd refuses Include nesting past this depth. The scan fails at the same depth, so an include
# bomb cannot shrink the scanned set into a pass.
MAX_INCLUDE_DEPTH=15

# Bounds on the tree --reload observes. Its last pass runs after the restart, so an oversized tree
# is refused rather than walked slowly. The file bound counts visits, so a drop-in reached from both
# roots counts twice. The byte bound caps the per-character tokenizer's work.
MAX_CONFIG_TREE_FILES=512
MAX_CONFIG_TREE_BYTES=262144

# Separates record fields. No mode, id or checksum contains it, and a path that does is refused.
CONFIG_TREE_FIELD_SEPARATOR=$'\037'

# What stat's %HT prints for a regular file. Any other type means the tree changed under the walk.
CONFIG_TREE_REGULAR_FILE_TYPE='Regular File'

# Values as `sshd -G` prints them. Parallel arrays because the system bash has no associative arrays.
# Together they close every userauth method sshd offers except publickey. GSSAPI and hostbased
# default to no, but a Match block can enable them, so they are pinned too.
PROTECTED_KEYS=(passwordauthentication kbdinteractiveauthentication usepam
  pubkeyauthentication permitrootlogin gssapiauthentication
  hostbasedauthentication)
PROTECTED_VALUES=(no no yes yes no no no)

# Keywords sshd accepts as aliases for a protected directive, which a Match block can use to move
# its value. SkeyAuthentication is undocumented and still enables kbdinteractiveauthentication.
# DSAAuthentication is here for `Match all`, which sshd applies globally.
DIRECTIVE_ALIASES=(challengeresponseauthentication skeyauthentication
  dsaauthentication)
DIRECTIVE_ALIAS_TARGETS=(kbdinteractiveauthentication
  kbdinteractiveauthentication pubkeyauthentication)

# Arrival addresses and the refuseconnection value the drop-in's Match block must give each: one
# allowed sample per negated term, and one refused sample per address family. fd00::1 sits outside
# the tailnet prefix, which proves the IPv6 negation is prefix-precise.
# refuseconnection stays out of PROTECTED_KEYS because it is correctly `no` outside the Match block.
LOCAL_ADDRESS_SAMPLES=(100.64.0.1 fd7a:115c:a1e0::1 127.0.0.1 ::1
  192.168.0.1 fd00::1)
LOCAL_ADDRESS_REFUSALS=(no no no no yes yes)
REFUSAL_KEY='refuseconnection'

die() {
  printf '[ssh-hardening] ERROR: %s\n' "$*" >&2
  exit 1
}

warn() {
  printf '[ssh-hardening] WARNING: %s\n' "$*" >&2
}

run_privileged() {
  if [[ -n $SSH_HARDENING_SUDO ]]; then
    "$SSH_HARDENING_SUDO" "$@"
  else
    "$@"
  fi
}

dropin_path() {
  printf '%s/%s\n' "$SSHD_CONFIG_D" "$DROPIN_NAME"
}

print_config() {
  cat <<'EOF'
# Public-key-only sshd policy written by ssh-hardening.sh. Removing this file
# returns sshd to its defaults at the next restart.
#
# The 000- prefix sorts before Apple's 100-macos.conf. sshd keeps the first
# value it reads for a directive, so these values win over any that a later
# macOS release adds to that file.
#
# PasswordAuthentication and KbdInteractiveAuthentication together close both
# password channels, and either one alone leaves the other open. macOS needs
# UsePAM yes for account and session management. It is safe because no
# password path remains for PAM to check. PermitRootLogin no also blocks root
# login by key, which the default allows.
#
# GSSAPIAuthentication and HostbasedAuthentication are the other two login
# methods sshd offers. Both default to no, but a Match block can turn them on,
# so this file sets them too.
PasswordAuthentication no
KbdInteractiveAuthentication no
UsePAM yes
PubkeyAuthentication yes
PermitRootLogin no
GSSAPIAuthentication no
HostbasedAuthentication no
#
# The Match block refuses any connection that did not arrive on loopback or on
# a tailnet address. RefuseConnection is useful only inside a Match block, so
# this one block is the whole network restriction.
#
# LocalAddress is the address the connection arrived on, so a client cannot
# fake it. Address is the client's source address, which a local-network
# client can set to a tailnet address to slip through. LocalAddress accepts
# CIDR ranges, even though the sshd_config manual documents them only for
# Address.
#
# 100.64.0.0/10 and fd7a:115c:a1e0::/48 are the ranges Tailscale uses for every
# tailnet, so the block keeps working when a machine's address changes.
# Loopback stays allowed because the reload readiness probe connects to it.
#
# A single negated list keeps a bare `sshd -G`, which resolves no Match block,
# reporting refuseconnection no. An allow block followed by `Match all` would
# refuse the same connections, but that report would then read as refusing
# everything. The trailing '*' makes the list fail closed and must never be
# dropped. A typo in a negated term leaves that address to '*', which refuses
# it, and a list without '*' matches nothing and refuses nobody.
#
# This limits who can log in, not what can connect. launchd still answers on
# port 22 on every interface, and a refused connection still gets a banner and
# a host key.
Match LocalAddress "!127.0.0.0/8,!::1,!100.64.0.0/10,!fd7a:115c:a1e0::/48,*"
  RefuseConnection yes
EOF
}

# verify runs four read-only checks and reports every failure, so one broken layer cannot mask
# another. `sshd -G` sees no Match block, which is why the Match scan and the per-connection
# resolutions exist. A check that cannot run fails the verify.

VERIFY_FAILURES=()

# A child verify that never returns would skip the install rollback, so it is stopped at this
# deadline and counted as a failure. sshd blocks forever on a named pipe in an Include glob.
# Values outside 1..86400 fall back to 120, since a larger one overflows the tick arithmetic.
VERIFY_DEADLINE_MAX_SECONDS=86400
VERIFY_DEADLINE_SECONDS="${SSH_HARDENING_VERIFY_DEADLINE:-120}"
if [[ ! $VERIFY_DEADLINE_SECONDS =~ ^[1-9][0-9]{0,4}$ ]] ||
  [[ $VERIFY_DEADLINE_SECONDS -gt $VERIFY_DEADLINE_MAX_SECONDS ]]; then
  VERIFY_DEADLINE_SECONDS=120
fi
VERIFY_STOP_GRACE_SECONDS=2
# bash has no wait with a timeout, so the wait polls. Deadlines count ticks of this interval.
VERIFY_POLL_INTERVAL='0.25'
VERIFY_POLL_TICKS_PER_SECOND=4
# Separate from SLEEP_BIN, which tests replace with an instant spy that would make this poll spin.
VERIFY_POLL_SLEEP='/bin/sleep'

# stop_bounded_group <pid>: sends TERM, then KILL, to the whole process group, since the sshd under
# the child is what blocks. The child is reaped only after both, so its pid cannot be recycled.
stop_bounded_group() {
  local child_pid="$1"
  kill -TERM -"$child_pid" 2>/dev/null || :
  "$VERIFY_POLL_SLEEP" "$VERIFY_STOP_GRACE_SECONDS" || :
  kill -KILL -"$child_pid" 2>/dev/null || :
}

# Status for a command stopped at its deadline, matching timeout(1).
BOUNDED_TIMEOUT_STATUS=124

# Global so the install interrupt handler can stop the group run_bounded is waiting on.
BOUNDED_CHILD_PID=''

# run_bounded <deadline-seconds> <command...>: run a command in its own process group and stop the
# group at the deadline, returning BOUNDED_TIMEOUT_STATUS. stdin is /dev/null and TTOU/TTIN are
# ignored so terminal I/O cannot stop the background group, as `stty tostop` would.
run_bounded() {
  local deadline_seconds="$1"
  shift
  local ticks=0 status=0
  local deadline_ticks=$((deadline_seconds * VERIFY_POLL_TICKS_PER_SECOND))
  set -m
  (
    trap '' TTOU TTIN
    "$@"
  ) </dev/null &
  BOUNDED_CHILD_PID=$!
  set +m
  while [[ $ticks -lt $deadline_ticks ]] &&
    kill -0 "$BOUNDED_CHILD_PID" 2>/dev/null; do
    "$VERIFY_POLL_SLEEP" "$VERIFY_POLL_INTERVAL"
    ticks=$((ticks + 1))
  done
  if kill -0 "$BOUNDED_CHILD_PID" 2>/dev/null; then
    # Reaps quietly, because bash would print "Terminated: 15", which reads like an internal error.
    {
      stop_bounded_group "$BOUNDED_CHILD_PID"
      wait "$BOUNDED_CHILD_PID"
    } 2>/dev/null || :
    BOUNDED_CHILD_PID=''
    return "$BOUNDED_TIMEOUT_STATUS"
  fi
  { wait "$BOUNDED_CHILD_PID"; } 2>/dev/null || status=$?
  BOUNDED_CHILD_PID=''
  return "$status"
}

# The verify runs in a child shell because `set -e` is off inside `if !` and `||` tests, even in
# called functions. The seams are passed explicitly: run_bounded takes an argv, not assignments.
verify_child_command() {
  SSHD_CONFIG_D="$SSHD_CONFIG_D" \
    SSHD_MAIN_CONFIG="$SSHD_MAIN_CONFIG" \
    SSHD_BIN="$SSHD_BIN" \
    SSH_HARDENING_SUDO="$SSH_HARDENING_SUDO" \
    SSH_HARDENING_ALLOW_MISSING_SSHD="${SSH_HARDENING_ALLOW_MISSING_SSHD:-}" \
    "${BASH:-/bin/bash}" "$SSH_HARDENING_SELF" --verify
}

run_verify_child() {
  local status=0
  run_bounded "$VERIFY_DEADLINE_SECONDS" verify_child_command || status=$?
  if [[ $status -eq $BOUNDED_TIMEOUT_STATUS ]]; then
    printf '[ssh-hardening] verify TIMED OUT: the check was still running after %ss and was stopped, its whole process group with it. A verify that never returns is treated as a FAILED verify, so exactly what a reported failure does happens now. One known cause: a named pipe reached through an Include glob, which blocks sshd forever on the open.\n' \
      "$VERIFY_DEADLINE_SECONDS" >&2
    return 1
  fi
  return "$status"
}

# The test seam is on only for an explicit true value, so "0" or "off" never disables verification.
verify_skip_allowed() {
  case "${SSH_HARDENING_ALLOW_MISSING_SSHD:-}" in
    1 | true | yes | on) return 0 ;;
    *) return 1 ;;
  esac
}

# add_failure <message>: record each problem once; files reached from both roots would repeat it.
add_failure() {
  local existing
  if [[ ${#VERIFY_FAILURES[@]} -gt 0 ]]; then
    for existing in "${VERIFY_FAILURES[@]}"; do
      if [[ $existing == "$1" ]]; then
        return 0
      fi
    done
  fi
  VERIFY_FAILURES+=("$1")
}

# required_value <key>: set MATCHED_PROTECTED_KEY and REQUIRED_VALUE for a protected key, else
# return 1. Globals avoid a fork per config line.
MATCHED_PROTECTED_KEY=''
REQUIRED_VALUE=''
# canonical_key <keyword>: fold sshd's aliases onto the directive they move, into CANONICAL_KEY.
CANONICAL_KEY=''
canonical_key() {
  local i
  CANONICAL_KEY="$1"
  for i in "${!DIRECTIVE_ALIASES[@]}"; do
    if [[ $1 == "${DIRECTIVE_ALIASES[$i]}" ]]; then
      CANONICAL_KEY="${DIRECTIVE_ALIAS_TARGETS[$i]}"
      return 0
    fi
  done
}

required_value() {
  local i
  for i in "${!PROTECTED_KEYS[@]}"; do
    if [[ $1 == "${PROTECTED_KEYS[$i]}" ]]; then
      MATCHED_PROTECTED_KEY="${PROTECTED_KEYS[$i]}"
      REQUIRED_VALUE="${PROTECTED_VALUES[$i]}"
      return 0
    fi
  done
  return 1
}

# assert_directive_value <label> <sshd output> <key> <want>: fail when the key is wrong or absent.
assert_directive_value() {
  local label="$1" output="$2" key="$3" want="$4" got status=0
  got="$(printf '%s\n' "$output" | awk -v k="$key" '$1 == k { print $2; exit }')" ||
    status=$?
  if [[ $status -ne 0 ]]; then
    add_failure "$label: could not read '$key' out of the sshd output (exit $status); failing closed rather than reading an unset value as absent"
  elif [[ -z $got ]]; then
    add_failure "$label: '$key' is absent from the effective configuration"
  elif [[ $got != "$want" ]]; then
    add_failure "$label: '$key' is '$got', want '$want'"
  fi
}

assert_output_hardened() {
  local label="$1" output="$2" i
  for i in "${!PROTECTED_KEYS[@]}"; do
    assert_directive_value "$label" "$output" \
      "${PROTECTED_KEYS[$i]}" "${PROTECTED_VALUES[$i]}"
  done
}

check_global() {
  local output status=0
  # Captured first, because a piped sshd's exit status would be lost.
  output="$("$SSHD_BIN" -G -f "$SSHD_MAIN_CONFIG" 2>&1)" || status=$?
  if [[ $status -ne 0 ]]; then
    add_failure "global check: '$SSHD_BIN -G' exited $status; failing closed rather than assuming the tree is safe (output: $output)"
    return 0
  fi
  assert_output_hardened 'global check' "$output"
}

# The scan mirrors sshd's two tokenizers as the binary behaves: strdelim for the keyword and
# argv_split for arguments. Both matter, since an Include path is an argument. Forms sshd rejects
# can be read any way here, because they fail `sshd -G` in check_global.

CONFIG_TAB=$'\t'
CONFIG_CR=$'\r'
CONFIG_FF=$'\f'
# ANSI-C quoting is the one spelling both shellcheck (SC1003) and shfmt accept.
CONFIG_BACKSLASH=$'\\'
TOKEN=''
REST=''

# trim_trailing_line_whitespace: sshd trims trailing space, tab, CR and form feed before tokenizing,
# so a CRLF file's CR stays out of Include paths and values.
trim_trailing_line_whitespace() {
  while [[ -n $REST ]]; do
    case $REST in
      *' ' | *"$CONFIG_TAB" | *"$CONFIG_CR" | *"$CONFIG_FF") REST="${REST%?}" ;;
      *) break ;;
    esac
  done
}

skip_keyword_separators() {
  while [[ -n $REST ]]; do
    case $REST in
      ' '* | "$CONFIG_TAB"* | "$CONFIG_CR"*) REST="${REST:1}" ;;
      *) break ;;
    esac
  done
}

# consume_keyword_separator: strdelim consumes whitespace, at most one '=', then whitespace again.
consume_keyword_separator() {
  skip_keyword_separators
  if [[ $REST == '='* ]]; then
    REST="${REST:1}"
    skip_keyword_separators
  fi
}

# read_keyword_token: keyword into TOKEN. Returns 1 at end of line, 2 on an unterminated quote.
read_keyword_token() {
  local character
  skip_keyword_separators
  if [[ -z $REST ]]; then
    return 1
  fi
  TOKEN=''
  while [[ -n $REST ]]; do
    character="${REST:0:1}"
    case $character in
      ' ' | "$CONFIG_TAB" | "$CONFIG_CR" | '=')
        consume_keyword_separator
        return 0
        ;;
      '"')
        # A quoted segment appends to the keyword and ends it, so `Ma"tch"` is Match.
        REST="${REST:1}"
        case $REST in
          *'"'*) ;;
          *) return 2 ;;
        esac
        TOKEN="$TOKEN${REST%%\"*}"
        REST="${REST#*\"}"
        skip_keyword_separators
        return 0
        ;;
    esac
    TOKEN="$TOKEN$character"
    REST="${REST:1}"
  done
  return 0
}

# skip_argument_separators: only space and tab separate arguments. A CR does not.
skip_argument_separators() {
  while [[ -n $REST ]]; do
    case $REST in
      ' '* | "$CONFIG_TAB"*) REST="${REST:1}" ;;
      *) break ;;
    esac
  done
}

# read_argument_token: argument into TOKEN. Returns 1 at end of line or at a comment, 2 on an
# unterminated quote.
read_argument_token() {
  local character escaped quote=''
  skip_argument_separators
  if [[ -z $REST ]]; then
    return 1
  fi
  # A '#' opening an argument starts a comment. A '#' inside one stays literal.
  if [[ $REST == '#'* ]]; then
    REST=''
    return 1
  fi
  TOKEN=''
  while [[ -n $REST ]]; do
    character="${REST:0:1}"
    if [[ $character == "$CONFIG_BACKSLASH" ]]; then
      # A backslash escapes a quote, a backslash, and outside quotes a space. A tab still separates.
      # Any other backslash stays literal.
      escaped="${REST:1:1}"
      case $escaped in
        '"' | "'" | "$CONFIG_BACKSLASH")
          TOKEN="$TOKEN$escaped"
          REST="${REST:2}"
          continue
          ;;
        ' ')
          if [[ -z $quote ]]; then
            TOKEN="$TOKEN$escaped"
            REST="${REST:2}"
            continue
          fi
          ;;
      esac
      TOKEN="$TOKEN$character"
      REST="${REST:1}"
      continue
    fi
    if [[ -z $quote ]]; then
      case $character in
        ' ' | "$CONFIG_TAB") break ;;
        '"' | "'")
          # Quoted segments concatenate with unquoted text into one argument.
          quote="$character"
          REST="${REST:1}"
          continue
          ;;
      esac
    elif [[ $character == "$quote" ]]; then
      quote=''
      REST="${REST:1}"
      continue
    fi
    TOKEN="$TOKEN$character"
    REST="${REST:1}"
  done
  if [[ -n $quote ]]; then
    return 2
  fi
  return 0
}

# A reader that returns a token without consuming input would spin the caller forever.
read_token_with_progress_guard() { # <reader function>
  local reader="$1" start_length="${#REST}" status=0
  "$reader" || status=$?
  if [[ $status -eq 0 && ${#REST} -eq $start_length ]]; then
    return 1
  fi
  return "$status"
}

next_keyword_token() {
  read_token_with_progress_guard read_keyword_token
}

next_argument_token() {
  read_token_with_progress_guard read_argument_token
}

# parse_config_line <raw line>: fill PARSED_KEYWORD and PARSED_ARGS. Returns 1 for a line sshd ignores.
PARSED_KEYWORD=''
PARSED_ARGS=()
parse_config_line() {
  REST="$1"
  trim_trailing_line_whitespace
  PARSED_KEYWORD=''
  PARSED_ARGS=()
  if ! next_keyword_token; then
    return 1
  fi
  # sshd discards one empty keyword token, so `=PasswordAuthentication yes` is a real directive.
  if [[ -z $TOKEN ]] && ! next_keyword_token; then
    return 1
  fi
  PARSED_KEYWORD="$TOKEN"
  if [[ -z $PARSED_KEYWORD ]]; then
    return 1
  fi
  # Tested after the discard, as sshd does: `=#Keyword arg` is a comment.
  case $PARSED_KEYWORD in
    '#'*) return 1 ;;
  esac
  while next_argument_token; do
    PARSED_ARGS+=("$TOKEN")
  done
  return 0
}

# sshd unescapes an Include path twice: argv_split, then glob(3), which turns every remaining `\X`
# into X. Bash unescapes only when it expands, so a token with no live metacharacter
# is resolved here.

# include_bracket_opens_a_set <text after '['>: glob(3) needs an unescaped ']' at least one
# character past the '[', after an optional '!'.
include_bracket_opens_a_set() {
  local rest="$1" character
  case $rest in
    '!'*) rest="${rest:1}" ;;
  esac
  # The first member may be ']', so the search starts after it.
  if [[ ${rest:0:1} == "$CONFIG_BACKSLASH" ]]; then
    rest="${rest:2}"
  else
    rest="${rest:1}"
  fi
  while [[ -n $rest ]]; do
    character="${rest:0:1}"
    if [[ $character == "$CONFIG_BACKSLASH" ]]; then
      rest="${rest:2}"
      continue
    fi
    if [[ $character == ']' ]]; then
      return 0
    fi
    rest="${rest:1}"
  done
  return 1
}

# unescape_include_pattern <token>: INCLUDE_UNESCAPED_PATH gets every `\X` reduced to X, and
# INCLUDE_HAS_METACHARACTER says whether the token is a pattern. INCLUDE_PATTERN_DIVERGES_FROM_GLOB
# flags a leading '^' in a bracket, which bash negates and glob(3) reads as a member.
# The case patterns are quoted so `*` and `?` match only themselves.
INCLUDE_UNESCAPED_PATH=''
INCLUDE_HAS_METACHARACTER=0
INCLUDE_PATTERN_DIVERGES_FROM_GLOB=0
unescape_include_pattern() {
  local rest="$1" character
  INCLUDE_UNESCAPED_PATH=''
  INCLUDE_HAS_METACHARACTER=0
  INCLUDE_PATTERN_DIVERGES_FROM_GLOB=0
  while [[ -n $rest ]]; do
    character="${rest:0:1}"
    if [[ $character == "$CONFIG_BACKSLASH" ]]; then
      if [[ ${#rest} -eq 1 ]]; then
        # A trailing backslash stays literal: glob(3) keeps it as part of the file name.
        INCLUDE_UNESCAPED_PATH="$INCLUDE_UNESCAPED_PATH$CONFIG_BACKSLASH"
        break
      fi
      INCLUDE_UNESCAPED_PATH="$INCLUDE_UNESCAPED_PATH${rest:1:1}"
      rest="${rest:2}"
      continue
    fi
    case $character in
      '*' | '?') INCLUDE_HAS_METACHARACTER=1 ;;
      '[')
        if include_bracket_opens_a_set "${rest:1}"; then
          INCLUDE_HAS_METACHARACTER=1
          case ${rest:1} in
            '^'*) INCLUDE_PATTERN_DIVERGES_FROM_GLOB=1 ;;
          esac
        fi
        ;;
    esac
    INCLUDE_UNESCAPED_PATH="$INCLUDE_UNESCAPED_PATH$character"
    rest="${rest:1}"
  done
}

# scan_included_files <file> <in-match> <depth> <chain>: follow the Include in PARSED_ARGS. The
# Match state where the Include appears applies inside the included file, so it is passed down.

# resolve_include_paths: the regular files the Include in PARSED_ARGS names, into
# INCLUDE_RESOLVED_PATHS. Both walks share it so they judge the tree sshd reads. Returns 1 with
# INCLUDE_RESOLVE_ERROR for an empty Include or a '^' bracket bash would expand differently.
# Callers copy the result before recursing, since recursion overwrites it.
INCLUDE_RESOLVED_PATHS=()
INCLUDE_RESOLVE_ERROR=''
resolve_include_paths() {
  local pattern resolved old_ifs
  local -a patterns matches
  INCLUDE_RESOLVED_PATHS=()
  INCLUDE_RESOLVE_ERROR=''
  patterns=()
  if [[ ${#PARSED_ARGS[@]} -gt 0 ]]; then
    patterns=("${PARSED_ARGS[@]}")
  fi
  if [[ ${#patterns[@]} -eq 0 ]]; then
    INCLUDE_RESOLVE_ERROR='has an Include with no path'
    return 1
  fi
  for pattern in "${patterns[@]}"; do
    # Relative is decided on the raw token, as sshd does: `\/etc/x.conf` is relative.
    case $pattern in
      /*) ;;
      *) pattern="$SSHD_CONFIG_DIR/$pattern" ;;
    esac
    unescape_include_pattern "$pattern"
    if [[ $INCLUDE_PATTERN_DIVERGES_FROM_GLOB -eq 1 ]]; then
      INCLUDE_RESOLVE_ERROR="has an Include pattern ('$pattern') whose bracket begins with '^', which bash expands to a different set of files than the glob(3) sshd uses"
      return 1
    fi
    matches=()
    if [[ $INCLUDE_HAS_METACHARACTER -eq 0 ]]; then
      # Without a live metacharacter, glob(3) only stats the unescaped path.
      matches=("$INCLUDE_UNESCAPED_PATH")
    else
      # IFS=newline keeps paths with spaces whole. A pattern matching nothing is dropped by -f below.
      old_ifs="$IFS"
      IFS=$'\n'
      # shellcheck disable=SC2206  # deliberate glob expansion of the Include pattern
      matches=($pattern)
      IFS="$old_ifs"
    fi
    if [[ ${#matches[@]} -eq 0 ]]; then
      continue
    fi
    for resolved in "${matches[@]}"; do
      # Keeps regular files only, because reading a named pipe blocks forever. sshd's own Include has
      # no such filter, which is why every sshd call runs under run_bounded.
      if [[ ! -f $resolved ]]; then
        continue
      fi
      INCLUDE_RESOLVED_PATHS+=("$resolved")
    done
  done
  return 0
}

# path_is_a_regular_file <path>: asked again right before each open, because the resolver's -f test
# can run long before it and reading a named pipe hangs. A -r test passes for a named pipe.
# No test reaches this guard, but it narrows a real window where the file type can change.
path_is_a_regular_file() {
  [[ -f $1 ]]
}

scan_included_files() {
  local from="$1" in_match="$2" depth="$3" chain="$4"
  local resolved
  local -a resolved_paths
  if ! resolve_include_paths; then
    add_failure "match scan: '$from' $INCLUDE_RESOLVE_ERROR; failing closed rather than guessing what it pulls in"
    return 0
  fi
  if [[ ${#INCLUDE_RESOLVED_PATHS[@]} -eq 0 ]]; then
    return 0
  fi
  # Copied before the recursive call below overwrites the global.
  resolved_paths=("${INCLUDE_RESOLVED_PATHS[@]}")
  for resolved in "${resolved_paths[@]}"; do
    scan_config_file "$resolved" "$in_match" "$((depth + 1))" "$chain"
  done
}

# scan_config_file <file> <in-match> <depth> <chain>: flag protected directives weakened inside a
# Match block. <chain> lists the files open on this path, so a cycle fails instead of looping.
scan_config_file() {
  local file="$1" in_match="$2" depth="$3" chain="$4"
  local key value status=0 line
  if [[ $depth -gt $MAX_INCLUDE_DEPTH ]]; then
    add_failure "match scan: '$file' sits more than $MAX_INCLUDE_DEPTH Include levels deep; sshd refuses a tree that deep and this scan refuses to report on part of one"
    return 0
  fi
  case $chain in
    *"|$file|"*)
      add_failure "match scan: Include cycle returning to '$file'; failing closed rather than scanning part of the tree"
      return 0
      ;;
  esac
  chain="$chain$file|"
  if [[ ! -r $file ]]; then
    add_failure "match scan: cannot read '$file'; failing closed rather than treating it as clean"
    return 0
  fi
  # Re-checked next to the open, for the reason path_is_a_regular_file gives.
  if ! path_is_a_regular_file "$file"; then
    add_failure "match scan: '$file' was a regular file when this scan resolved it and is not one now; failing closed rather than opening something that can block forever"
    return 0
  fi
  # Reads without a here-string, because a full TMPDIR would feed it zero lines and pass a hostile file.
  # shellcheck disable=SC2094  # $file is passed to the recursive call for the
  # message it prints; the scan never writes a config file.
  while IFS= read -r line || [[ -n $line ]]; do
    if ! parse_config_line "$line"; then
      continue
    fi
    key="$PARSED_KEYWORD"
    if [[ $key == match ]]; then
      in_match=1
      continue
    fi
    if [[ $key == include ]]; then
      scan_included_files "$file" "$in_match" "$depth" "$chain"
      continue
    fi
    if [[ $in_match -ne 1 ]]; then
      continue
    fi
    if [[ ${#PARSED_ARGS[@]} -gt 0 ]]; then
      value="${PARSED_ARGS[0]}"
    else
      value=''
    fi
    canonical_key "$key"
    if required_value "$CANONICAL_KEY" && [[ $value != "$REQUIRED_VALUE" ]]; then
      add_failure "match scan: '$file' sets '$MATCHED_PROTECTED_KEY $value' inside a Match block (want '$REQUIRED_VALUE'); a Match-scoped re-enable bypasses the global check"
    fi
  done <"$file" || status=$?
  if [[ $status -ne 0 ]]; then
    add_failure "match scan: reading '$file' failed (exit $status); failing closed rather than treating a partial read as clean"
  fi
}

# config_tree_roots: the main config plus every drop-in, in sshd's byte order, into CONFIG_TREE_ROOTS,
# so a drop-in stays covered if the main config's Include resolves differently. The listing is
# captured, not read from a process substitution, so a failing sort fails here.
CONFIG_TREE_ROOTS=()
CONFIG_TREE_ROOTS_ERROR=''
config_tree_roots() {
  local listing status=0 file old_ifs
  local -a candidates
  CONFIG_TREE_ROOTS=()
  CONFIG_TREE_ROOTS_ERROR=''
  listing="$(printf '%s\n' "$SSHD_MAIN_CONFIG" "$SSHD_CONFIG_D"/* | LC_ALL=C sort -u)" ||
    status=$?
  if [[ $status -ne 0 ]]; then
    CONFIG_TREE_ROOTS_ERROR="could not list the configuration files to scan (exit $status)"
    return 1
  fi
  # Newline split with globbing off, so a name holding a glob character is not expanded again.
  old_ifs="$IFS"
  IFS=$'\n'
  set -f
  # shellcheck disable=SC2206  # deliberate newline split of the captured listing
  candidates=($listing)
  set +f
  IFS="$old_ifs"
  if [[ ${#candidates[@]} -eq 0 ]]; then
    CONFIG_TREE_ROOTS_ERROR='the configuration file listing came back empty'
    return 1
  fi
  for file in "${candidates[@]}"; do
    # Keeps regular files only, because reading a named pipe blocks forever.
    if [[ -z $file || ! -f $file ]]; then
      continue
    fi
    CONFIG_TREE_ROOTS+=("$file")
  done
  return 0
}

check_match_scan() {
  local file
  local -a roots
  if ! config_tree_roots; then
    add_failure "match scan: $CONFIG_TREE_ROOTS_ERROR; failing closed rather than scanning none"
    return 0
  fi
  if [[ ${#CONFIG_TREE_ROOTS[@]} -eq 0 ]]; then
    return 0
  fi
  roots=("${CONFIG_TREE_ROOTS[@]}")
  for file in "${roots[@]}"; do
    scan_config_file "$file" 0 0 '|'
  done
}

check_connection_specs() {
  local invoking_user spec output status=0
  invoking_user="$(id -un)" || status=$?
  if [[ $status -ne 0 || -z $invoking_user ]]; then
    add_failure "connection check: could not determine the invoking user ('id -un' exited $status); failing closed rather than probing a spec built from an empty name"
    return 0
  fi
  # Resolves for root, which PermitRootLogin keeps out,
  # and for the invoking user, which a `Match User` would target.
  for spec in \
    'user=root,host=localhost,addr=127.0.0.1' \
    "user=$invoking_user,host=localhost,addr=127.0.0.1"; do
    status=0
    # -C requires -T, and -T alone demands host keys. Together they resolve Match blocks unprivileged.
    output="$("$SSHD_BIN" -G -T -C "$spec" -f "$SSHD_MAIN_CONFIG" 2>&1)" || status=$?
    if [[ $status -ne 0 ]]; then
      add_failure "connection check ($spec): '$SSHD_BIN -G -T -C' exited $status; failing closed (output: $output)"
      continue
    fi
    assert_output_hardened "connection check ($spec)" "$output"
  done
}

# check_local_address_specs: resolve one connection per arrival address and require its refusal
# verdict. Only a spec with an laddr reaches the drop-in's LocalAddress Match block.
check_local_address_specs() {
  local invoking_user spec output status=0 i address want
  invoking_user="$(id -un)" || status=$?
  if [[ $status -ne 0 || -z $invoking_user ]]; then
    add_failure "local address check: could not determine the invoking user ('id -un' exited $status); failing closed rather than probing a spec built from an empty name"
    return 0
  fi
  for i in "${!LOCAL_ADDRESS_SAMPLES[@]}"; do
    address="${LOCAL_ADDRESS_SAMPLES[$i]}"
    want="${LOCAL_ADDRESS_REFUSALS[$i]}"
    spec="user=$invoking_user,host=localhost,addr=203.0.113.1,laddr=$address"
    status=0
    output="$("$SSHD_BIN" -G -T -C "$spec" -f "$SSHD_MAIN_CONFIG" 2>&1)" || status=$?
    if [[ $status -ne 0 ]]; then
      add_failure "local address check ($spec): '$SSHD_BIN -G -T -C' exited $status; failing closed (output: $output)"
      continue
    fi
    assert_directive_value "local address check ($spec)" "$output" "$REFUSAL_KEY" "$want"
  done
}

verify() {
  VERIFY_FAILURES=()
  if [[ ! -x $SSHD_BIN ]]; then
    if verify_skip_allowed; then
      printf '[ssh-hardening] verify SKIPPED: %s is not executable and the SSH_HARDENING_ALLOW_MISSING_SSHD test seam is set. The configuration was NOT checked.\n' "$SSHD_BIN"
      return 0
    fi
    printf '[ssh-hardening] verify: FAILING CLOSED: %s is not executable, so the effective configuration cannot be checked. Refusing to guess.\n' "$SSHD_BIN" >&2
    return 1
  fi
  check_global
  check_match_scan
  check_connection_specs
  check_local_address_specs
  if [[ ${#VERIFY_FAILURES[@]} -gt 0 ]]; then
    printf '[ssh-hardening] verify FAILED, %d problem(s):\n' "${#VERIFY_FAILURES[@]}" >&2
    printf '  - %s\n' "${VERIFY_FAILURES[@]}" >&2
    return 1
  fi
  # Counts come from the arrays, so the success line names exactly what was checked.
  printf '[ssh-hardening] verify: PASS: all %d protected directives hold globally, no Match block in the include graph re-enables any of them, both sampled connections resolve hardened, and all %d sampled arrival addresses resolve the refusal verdict policy demands.\n' \
    "${#PROTECTED_KEYS[@]}" "${#LOCAL_ADDRESS_SAMPLES[@]}"
}

# Install is a transaction: stage, publish with one rename, move the legacy file aside, verify, and
# restore the tree as found if a step fails or an interrupt arrives. Working files are dot-prefixed,
# which sshd's Include glob skips, so they never take effect.

# Transaction state, so rollback undoes what this run did instead of inferring it from disk.
INSTALL_TARGET=''
INSTALL_LEGACY=''
INSTALL_STAGING=''
INSTALL_SAVED_TARGET=''
INSTALL_SAVED_LEGACY=''
INSTALL_PUBLISHED=0
INSTALL_ROLLED_BACK=0

# rollback_install: undo what install did and nothing else. Each step warns instead of aborting.
# It runs at most once, because a second pass would read the restored tree
# as "no drop-in before" and delete it.
rollback_install() {
  if [[ $INSTALL_ROLLED_BACK -eq 1 ]]; then
    return 0
  fi
  INSTALL_ROLLED_BACK=1
  if [[ -e $INSTALL_STAGING || -L $INSTALL_STAGING ]]; then
    run_privileged rm -f -- "$INSTALL_STAGING" ||
      warn "rollback could not remove the staging file '$INSTALL_STAGING'"
  fi
  # Undo the target only if this run replaced it.
  if [[ $INSTALL_PUBLISHED -eq 1 ]]; then
    if [[ -e $INSTALL_SAVED_TARGET || -L $INSTALL_SAVED_TARGET ]]; then
      run_privileged mv -f -- "$INSTALL_SAVED_TARGET" "$INSTALL_TARGET" ||
        warn "rollback could not restore the previous drop-in from '$INSTALL_SAVED_TARGET'"
    else
      # No drop-in existed before this run.
      run_privileged rm -f -- "$INSTALL_TARGET" ||
        warn "rollback could not remove the drop-in '$INSTALL_TARGET' this run created"
    fi
  fi
  if [[ -e $INSTALL_SAVED_LEGACY || -L $INSTALL_SAVED_LEGACY ]]; then
    run_privileged mv -f -- "$INSTALL_SAVED_LEGACY" "$INSTALL_LEGACY" ||
      warn "rollback could not restore the legacy drop-in from '$INSTALL_SAVED_LEGACY'"
  fi
}

# handle_install_interrupt <signal>: Ctrl-C misses the verify's own process group, so stop it, roll
# back, then re-raise the signal so the caller sees a signal death. The trap is reset first.
handle_install_interrupt() {
  local signal="$1"
  trap - INT TERM HUP
  if [[ -n $BOUNDED_CHILD_PID ]]; then
    # Reaps quietly, as run_bounded does, so the rollback starts after the verify lets go of the tree.
    {
      stop_bounded_group "$BOUNDED_CHILD_PID"
      wait "$BOUNDED_CHILD_PID"
    } 2>/dev/null || :
    BOUNDED_CHILD_PID=''
  fi
  rollback_install
  printf '[ssh-hardening] INTERRUPTED (SIG%s): the tree was rolled back to the state this install found it in, nothing the verify started is still running, and no success is claimed.\n' \
    "$signal" >&2
  kill -"$signal" "$$"
}

install_dropin() {
  local target legacy staging saved_target saved_legacy
  target="$(dropin_path)"
  legacy="$SSHD_CONFIG_D/$LEGACY_DROPIN_NAME"
  staging="$SSHD_CONFIG_D/.$DROPIN_NAME.staging"
  saved_target="$SSHD_CONFIG_D/.$DROPIN_NAME.saved"
  saved_legacy="$SSHD_CONFIG_D/.$LEGACY_DROPIN_NAME.saved"
  INSTALL_TARGET="$target"
  INSTALL_LEGACY="$legacy"
  INSTALL_STAGING="$staging"
  INSTALL_SAVED_TARGET="$saved_target"
  INSTALL_SAVED_LEGACY="$saved_legacy"
  INSTALL_PUBLISHED=0
  INSTALL_ROLLED_BACK=0

  # Armed before the first write, so a signal triggers a rollback only while there is one to do.
  trap 'handle_install_interrupt INT' INT
  trap 'handle_install_interrupt TERM' TERM
  trap 'handle_install_interrupt HUP' HUP

  [[ -d $SSHD_CONFIG_D ]] ||
    die "drop-in directory '$SSHD_CONFIG_D' does not exist"
  if ! run_privileged rm -f -- "$staging" "$saved_target" "$saved_legacy"; then
    die "could not clear the working files under '$SSHD_CONFIG_D'; refusing to begin an install that could not be rolled back"
  fi

  # Stages first, because tee truncates its target on open and a failed pipeline would empty the drop-in.
  if ! print_config | run_privileged tee -- "$staging" >/dev/null; then
    rollback_install
    die "could not stage the new drop-in at '$staging'; '$target' is untouched"
  fi
  # Sets 0644 explicitly, because a root-owned 0600 drop-in breaks unprivileged `sshd -G`.
  # BSD chmod reads a `--` after the mode as a file name.
  if ! run_privileged chmod -- 0644 "$staging"; then
    rollback_install
    die "could not set mode 0644 on '$staging'; '$target' is untouched"
  fi
  # Back up before replacing, so the target is never absent. -R recreates a symlink or named pipe as
  # itself. Without it, cp follows the link or blocks forever on the pipe.
  if [[ -e $target || -L $target ]]; then
    if ! run_privileged cp -Rp -- "$target" "$saved_target"; then
      rollback_install
      die "could not copy the existing '$target' aside; refusing to replace a file that could not then be restored"
    fi
  fi
  if ! run_privileged mv -f -- "$staging" "$target"; then
    rollback_install
    die "could not publish the staged drop-in as '$target'"
  fi
  INSTALL_PUBLISHED=1
  printf '[ssh-hardening] wrote %s (mode 0644)\n' "$target"

  # Moves the legacy drop-in aside until the new one verifies. One policy belongs in one file,
  # and the legacy file's 0600 mode breaks unprivileged verification.
  if [[ -e $legacy || -L $legacy ]]; then
    if ! run_privileged mv -f -- "$legacy" "$saved_legacy"; then
      rollback_install
      die "could not move the legacy drop-in '$legacy' aside"
    fi
    printf '[ssh-hardening] removed legacy drop-in %s\n' "$legacy"
  fi
  # Verified in a child shell, where errexit stays on, as verify_child_command explains.
  local verify_status=0
  run_verify_child || verify_status=$?
  if [[ $verify_status -ne 0 ]]; then
    rollback_install
    die "the effective configuration did NOT verify as fully hardened; the tree was rolled back to the state this install found it in, and no success is claimed"
  fi
  # Disarmed once the drop-in verifies, so an interrupt from here on keeps the hardening.
  trap - INT TERM HUP
  if ! run_privileged rm -f -- "$saved_target" "$saved_legacy"; then
    warn "could not remove the rollback copies under '$SSHD_CONFIG_D'; they are dot-prefixed and inert, but should be cleaned up"
  fi
  # The child's skip is invisible in its exit status, so the same predicate picks the wording.
  if [[ ! -x $SSHD_BIN ]] && verify_skip_allowed; then
    printf '[ssh-hardening] wrote %s, but verification was SKIPPED via the test seam; the effective configuration is NOT verified.\n' "$target"
    return 0
  fi
  printf '[ssh-hardening] install complete: %s is in place and the effective configuration verified fully hardened.\n' "$target"
}

# --reload compares observations of the tree: one record per file with path, mode, owner and
# checksum. mtime is left out because whole seconds hide a rewrite, and inode because an atomic
# republish changes it.
# Reads are unprivileged, so the post-restart pass cannot raise a sudo prompt.

CONFIG_TREE_FILES=()
CONFIG_TREE_ERROR=''
CONFIG_TREE_BYTES_READ=0

# config_tree_would_exceed_width <count>: 0 with CONFIG_TREE_ERROR set when that many more files
# would pass MAX_CONFIG_TREE_FILES. Asked before walking a match list, not once per file.
config_tree_would_exceed_width() {
  local incoming="$1"
  if [[ $((${#CONFIG_TREE_FILES[@]} + incoming)) -le $MAX_CONFIG_TREE_FILES ]]; then
    return 1
  fi
  CONFIG_TREE_ERROR="the configuration tree names more than $MAX_CONFIG_TREE_FILES files"
  return 0
}

# collect_config_tree_from_file <file> <depth> <chain>: append this file and its Includes to
# CONFIG_TREE_FILES. Fails closed through CONFIG_TREE_ERROR, never with a partial listing.
collect_config_tree_from_file() {
  local file="$1" depth="$2" chain="$3"
  local line status=0 resolved
  local -a resolved_paths
  if config_tree_would_exceed_width 1; then
    return 0
  fi
  if [[ $depth -gt $MAX_INCLUDE_DEPTH ]]; then
    CONFIG_TREE_ERROR="'$file' sits more than $MAX_INCLUDE_DEPTH Include levels deep"
    return 0
  fi
  case $chain in
    *"|$file|"*)
      CONFIG_TREE_ERROR="Include cycle returning to '$file'"
      return 0
      ;;
  esac
  chain="$chain$file|"
  if [[ ! -r $file ]]; then
    CONFIG_TREE_ERROR="cannot read '$file'"
    return 0
  fi
  # Re-checked next to the open, for the reason path_is_a_regular_file gives.
  if ! path_is_a_regular_file "$file"; then
    CONFIG_TREE_ERROR="'$file' was a regular file when this walk resolved it and is not one now, so reading it here could block"
    return 0
  fi
  CONFIG_TREE_FILES+=("$file")
  # No here-string, for the reason scan_config_file gives.
  # shellcheck disable=SC2094  # $file is passed to the recursive call for the
  # message it may print; this walk never opens a config file for writing.
  while IFS= read -r line || [[ -n $line ]]; do
    # Counted before parsing, where the walk's cost lies. +1 counts the newline that read strips.
    CONFIG_TREE_BYTES_READ=$((CONFIG_TREE_BYTES_READ + ${#line} + 1))
    if [[ $CONFIG_TREE_BYTES_READ -gt $MAX_CONFIG_TREE_BYTES ]]; then
      CONFIG_TREE_ERROR="the configuration tree holds more than $MAX_CONFIG_TREE_BYTES bytes to read (reached while reading '$file')"
      return 0
    fi
    if ! parse_config_line "$line"; then
      continue
    fi
    if [[ $PARSED_KEYWORD != include ]]; then
      continue
    fi
    if ! resolve_include_paths; then
      CONFIG_TREE_ERROR="'$file' $INCLUDE_RESOLVE_ERROR"
      return 0
    fi
    if [[ ${#INCLUDE_RESOLVED_PATHS[@]} -eq 0 ]]; then
      continue
    fi
    # Copied before the recursive call below overwrites the global.
    resolved_paths=("${INCLUDE_RESOLVED_PATHS[@]}")
    if config_tree_would_exceed_width "${#resolved_paths[@]}"; then
      return 0
    fi
    for resolved in "${resolved_paths[@]}"; do
      collect_config_tree_from_file "$resolved" "$((depth + 1))" "$chain"
      if [[ -n $CONFIG_TREE_ERROR ]]; then
        return 0
      fi
    done
  done <"$file" || status=$?
  if [[ $status -ne 0 ]]; then
    CONFIG_TREE_ERROR="reading '$file' failed (exit $status)"
  fi
}

# collect_config_tree_files: the whole include graph from both roots. Returns 1 with CONFIG_TREE_ERROR.
collect_config_tree_files() {
  local root
  local -a roots
  CONFIG_TREE_FILES=()
  CONFIG_TREE_ERROR=''
  CONFIG_TREE_BYTES_READ=0
  if ! config_tree_roots; then
    CONFIG_TREE_ERROR="$CONFIG_TREE_ROOTS_ERROR"
    return 1
  fi
  if [[ ${#CONFIG_TREE_ROOTS[@]} -eq 0 ]]; then
    CONFIG_TREE_ERROR='the configuration tree holds no regular file at all'
    return 1
  fi
  roots=("${CONFIG_TREE_ROOTS[@]}")
  if config_tree_would_exceed_width "${#roots[@]}"; then
    return 1
  fi
  for root in "${roots[@]}"; do
    collect_config_tree_from_file "$root" 0 '|'
    if [[ -n $CONFIG_TREE_ERROR ]]; then
      return 1
    fi
  done
  return 0
}

# config_tree_path_is_recordable <path>: 1 for a path holding a newline or the field separator,
# which would split its record wrongly and report a false change.
config_tree_path_is_recordable() {
  case $1 in
    *"$CONFIG_TREE_FIELD_SEPARATOR"* | *$'\n'*) return 1 ;;
  esac
  return 0
}

# observe_config_tree: one sorted, deduplicated observation into CONFIG_TREE_OBSERVATION, so walk
# order cannot matter. Returns 1 with CONFIG_TREE_ERROR, which callers treat as a refusal.
CONFIG_TREE_OBSERVATION=''
observe_config_tree() {
  local listing status=0 file attributes file_type checksum old_ifs
  local -a unique_files
  CONFIG_TREE_OBSERVATION=''
  if ! collect_config_tree_files; then
    return 1
  fi
  # The system bash aborts on an empty array under set -u.
  if [[ ${#CONFIG_TREE_FILES[@]} -eq 0 ]]; then
    CONFIG_TREE_ERROR='the configuration tree walk produced no file at all'
    return 1
  fi
  # Checked before the newline-framed listing, which would split such a path.
  for file in "${CONFIG_TREE_FILES[@]}"; do
    if ! config_tree_path_is_recordable "$file"; then
      CONFIG_TREE_ERROR="the name of '$file' holds a newline or an ASCII unit separator, so its record cannot be recorded unambiguously; rename it and re-run"
      return 1
    fi
  done
  listing="$(printf '%s\n' "${CONFIG_TREE_FILES[@]}" | LC_ALL=C sort -u)" || status=$?
  if [[ $status -ne 0 ]]; then
    CONFIG_TREE_ERROR="could not order the configuration file listing (exit $status)"
    return 1
  fi
  old_ifs="$IFS"
  IFS=$'\n'
  set -f
  # shellcheck disable=SC2206  # deliberate newline split of the captured listing
  unique_files=($listing)
  set +f
  IFS="$old_ifs"
  if [[ ${#unique_files[@]} -eq 0 ]]; then
    CONFIG_TREE_ERROR='the ordered configuration file listing came back empty'
    return 1
  fi
  for file in "${unique_files[@]}"; do
    status=0
    # -L describes the file actually opened, not a symlink's own mode and owner. The type is the last
    # check that the path is still a regular file, since reading a named pipe hangs.
    attributes="$("$STAT_BIN" -Lf "%HT$CONFIG_TREE_FIELD_SEPARATOR%Lp %u %g" -- "$file" 2>&1)" || status=$?
    if [[ $status -ne 0 ]]; then
      CONFIG_TREE_ERROR="could not read the mode and owner of '$file' ($STAT_BIN exited $status: $attributes)"
      return 1
    fi
    file_type="${attributes%%"$CONFIG_TREE_FIELD_SEPARATOR"*}"
    attributes="${attributes#*"$CONFIG_TREE_FIELD_SEPARATOR"}"
    if [[ $file_type != "$CONFIG_TREE_REGULAR_FILE_TYPE" ]]; then
      CONFIG_TREE_ERROR="'$file' was a regular file when the tree was walked and is a $file_type now, so the tree changed type under this observation and reading it could block"
      return 1
    fi
    status=0
    checksum="$("$CKSUM_BIN" <"$file")" || status=$?
    if [[ $status -ne 0 ]]; then
      CONFIG_TREE_ERROR="could not checksum '$file' ($CKSUM_BIN exited $status)"
      return 1
    fi
    CONFIG_TREE_OBSERVATION="$CONFIG_TREE_OBSERVATION$file$CONFIG_TREE_FIELD_SEPARATOR$attributes$CONFIG_TREE_FIELD_SEPARATOR$checksum"$'\n'
  done
  return 0
}

# config_tree_record_fields <record>: split a record on the field separator, never on whitespace.
CONFIG_TREE_RECORD_PATH=''
CONFIG_TREE_RECORD_ATTRIBUTES=''
CONFIG_TREE_RECORD_CHECKSUM=''
config_tree_record_fields() {
  local rest
  CONFIG_TREE_RECORD_PATH="${1%%"$CONFIG_TREE_FIELD_SEPARATOR"*}"
  rest="${1#*"$CONFIG_TREE_FIELD_SEPARATOR"}"
  CONFIG_TREE_RECORD_ATTRIBUTES="${rest%%"$CONFIG_TREE_FIELD_SEPARATOR"*}"
  CONFIG_TREE_RECORD_CHECKSUM="${rest#*"$CONFIG_TREE_FIELD_SEPARATOR"}"
}

# config_tree_find_record <path> [record...]: 0 with CONFIG_TREE_FOUND_* set when a record names
# the path. Callers turn nocasematch off so a case-only rename counts as a change.
CONFIG_TREE_FOUND_ATTRIBUTES=''
CONFIG_TREE_FOUND_CHECKSUM=''
config_tree_find_record() {
  local wanted="$1" record
  CONFIG_TREE_FOUND_ATTRIBUTES=''
  CONFIG_TREE_FOUND_CHECKSUM=''
  shift
  for record in "$@"; do
    config_tree_record_fields "$record"
    if [[ $CONFIG_TREE_RECORD_PATH != "$wanted" ]]; then
      continue
    fi
    CONFIG_TREE_FOUND_ATTRIBUTES="$CONFIG_TREE_RECORD_ATTRIBUTES"
    CONFIG_TREE_FOUND_CHECKSUM="$CONFIG_TREE_RECORD_CHECKSUM"
    return 0
  done
  return 1
}

# config_tree_is_unchanged <earlier> <later>: 1 with CONFIG_TREE_CHANGE_DESCRIPTION naming each file
# and whether its content or its mode moved, so the operator knows where to look.
CONFIG_TREE_CHANGE_DESCRIPTION=''
config_tree_is_unchanged() {
  local earlier="$1" later="$2" changed=0 separator=''
  local old_ifs record other path attributes checksum found
  local other_attributes other_checksum
  local -a earlier_records later_records
  CONFIG_TREE_CHANGE_DESCRIPTION=''
  # Case-sensitive, so a case-only rename counts as a change. It is restored at the single exit.
  shopt -u nocasematch
  old_ifs="$IFS"
  IFS=$'\n'
  set -f
  # shellcheck disable=SC2206  # deliberate newline split of the observations
  earlier_records=($earlier)
  # shellcheck disable=SC2206  # deliberate newline split of the observations
  later_records=($later)
  set +f
  IFS="$old_ifs"
  if [[ ${#earlier_records[@]} -gt 0 ]]; then
    for record in "${earlier_records[@]}"; do
      config_tree_record_fields "$record"
      path="$CONFIG_TREE_RECORD_PATH"
      attributes="$CONFIG_TREE_RECORD_ATTRIBUTES"
      checksum="$CONFIG_TREE_RECORD_CHECKSUM"
      found=0
      other_attributes=''
      other_checksum=''
      # Guarded, because the system bash aborts under `set -u` on an empty array expansion.
      if [[ ${#later_records[@]} -gt 0 ]] &&
        config_tree_find_record "$path" "${later_records[@]}"; then
        found=1
        other_attributes="$CONFIG_TREE_FOUND_ATTRIBUTES"
        other_checksum="$CONFIG_TREE_FOUND_CHECKSUM"
      fi
      if [[ $found -eq 0 ]]; then
        CONFIG_TREE_CHANGE_DESCRIPTION="$CONFIG_TREE_CHANGE_DESCRIPTION$separator'$path' disappeared"
        separator='; '
        changed=1
        continue
      fi
      if [[ $checksum != "$other_checksum" ]]; then
        CONFIG_TREE_CHANGE_DESCRIPTION="$CONFIG_TREE_CHANGE_DESCRIPTION$separator'$path' changed (content)"
        separator='; '
        changed=1
      fi
      if [[ $attributes != "$other_attributes" ]]; then
        CONFIG_TREE_CHANGE_DESCRIPTION="$CONFIG_TREE_CHANGE_DESCRIPTION$separator'$path' changed (mode or owner: $attributes became $other_attributes)"
        separator='; '
        changed=1
      fi
    done
  fi
  if [[ ${#later_records[@]} -gt 0 ]]; then
    for other in "${later_records[@]}"; do
      config_tree_record_fields "$other"
      path="$CONFIG_TREE_RECORD_PATH"
      if [[ ${#earlier_records[@]} -gt 0 ]] &&
        config_tree_find_record "$path" "${earlier_records[@]}"; then
        continue
      fi
      CONFIG_TREE_CHANGE_DESCRIPTION="$CONFIG_TREE_CHANGE_DESCRIPTION$separator'$path' appeared"
      separator='; '
      changed=1
    done
  fi
  shopt -s nocasematch
  return "$changed"
}

# report_absent_service_outcome <earlier observation>: Remote Login is off, so nothing restarts and
# a moved tree still exits 0 without a claim about it. A failed observation is fatal, since the run
# can no longer see whether the drop-in is there.
report_absent_service_outcome() {
  local earlier="$1" tree_changed=0
  printf '[ssh-hardening] reload: the sshd launchd service is confirmed absent, which is what Remote Login being off looks like, so there is no daemon to restart and nothing was restarted.\n'
  if ! observe_config_tree; then
    die "the sshd launchd service is confirmed absent and nothing was restarted, but the sshd configuration tree could not be re-read after the checks above ($CONFIG_TREE_ERROR), so nothing is claimed about what the tree on disk does when Remote Login is next enabled. sshd was not touched. Check the tree with 'ssh-hardening.sh --verify'."
  fi
  config_tree_is_unchanged "$earlier" "$CONFIG_TREE_OBSERVATION" || tree_changed=$?
  if [[ $tree_changed -ne 0 ]]; then
    warn "the sshd configuration tree CHANGED while this reload was validating it, so the checks above judged a tree that is no longer on disk and nothing is claimed about what the tree does when Remote Login is next enabled. What moved: $CONFIG_TREE_CHANGE_DESCRIPTION. Re-run 'ssh-hardening.sh --verify' once the tree has settled."
    return 0
  fi
  printf '[ssh-hardening] reload: the configuration tree these checks passed is still the one on disk, with every file reading back byte-for-byte and mode-for-mode identical each time this run read it. Whether it is still what sshd reads when Remote Login is next enabled is NOT checked here: that happens later, possibly much later, over whatever the tree holds at that moment, and nothing re-validates it then, so re-run ssh-hardening.sh --verify once Remote Login is on.\n'
}

# recovery_instructions: the way back in, printed after every failure past the restart. The
# fresh-machine runbook quotes this sentence; change both together.
recovery_instructions() {
  printf 'Recovery: keep any SSH session you still have OPEN until a new login succeeds. From the physical console, or Screen Sharing over the tailnet, run: ssh-hardening.sh --rollback (or: sudo rm %s), then turn Remote Login off and back on in System Settings > General > Sharing.' "$(dropin_path)"
}

# probe_sshd_service: run launchctl without sudo, so a wrapper failure cannot pose as status 113.
# The caller needs the exact status: 0 loaded, 113 absent, anything else a probe error.
SERVICE_PROBE_STATUS=0
SERVICE_PROBE_OUTPUT=''
probe_sshd_service() {
  SERVICE_PROBE_STATUS=0
  SERVICE_PROBE_OUTPUT="$("$LAUNCHCTL_BIN" print "$SSHD_LAUNCHD_SERVICE" 2>&1)" ||
    SERVICE_PROBE_STATUS=$?
}

# validate_readiness_knobs: refuse knob values that would not mean what they say. Integers take no
# leading zero, which bash reads as octal, and stay short enough for safe arithmetic.
validate_readiness_knobs() {
  case $SSH_HARDENING_READY_ATTEMPTS in
    '' | *[!0-9]* | 0*)
      die "SSH_HARDENING_READY_ATTEMPTS must be a positive base-10 integer with no leading zero, got '$SSH_HARDENING_READY_ATTEMPTS'; refusing to run with a readiness bound that does not mean what it says"
      ;;
  esac
  if [[ ${#SSH_HARDENING_READY_ATTEMPTS} -gt 9 ]]; then
    die "SSH_HARDENING_READY_ATTEMPTS ('$SSH_HARDENING_READY_ATTEMPTS') is too long to stay inside safe bash arithmetic; refusing to run with it"
  fi
  case $SSH_HARDENING_PROBE_TIMEOUT in
    '' | *[!0-9]* | 0*)
      die "SSH_HARDENING_PROBE_TIMEOUT must be a positive base-10 integer number of seconds with no leading zero, got '$SSH_HARDENING_PROBE_TIMEOUT'"
      ;;
  esac
  if [[ ${#SSH_HARDENING_PROBE_TIMEOUT} -gt 9 ]]; then
    die "SSH_HARDENING_PROBE_TIMEOUT ('$SSH_HARDENING_PROBE_TIMEOUT') is too long to stay inside safe bash arithmetic; refusing to run with it"
  fi
  case $SSH_HARDENING_READY_INTERVAL in
    '' | . | *.*.* | *[!0-9.]* | 0[0-9]*)
      die "SSH_HARDENING_READY_INTERVAL must be a canonical non-negative decimal number of seconds, got '$SSH_HARDENING_READY_INTERVAL'"
      ;;
  esac
}

# resolve_probe_ports: every Port the configuration declares, validated and deduplicated, into
# PROBE_PORTS. launchd owns the real listener, normally on port 22, so a nonstandard Port can be the
# wrong target. The lockout message says so.
PROBE_PORTS=()
# run_bounded takes an argv, not a redirection, so the redirection lives in this command.
# A command substitution would wait unbounded on its pipe.
PORT_RESOLUTION_OUTPUT=''
resolve_ports_command() {
  "$SSHD_BIN" -G -f "$SSHD_MAIN_CONFIG" >"$PORT_RESOLUTION_OUTPUT" 2>&1
}

resolve_probe_ports() {
  local status=0 output listing port existing duplicate
  PROBE_PORTS=()
  PORT_RESOLUTION_OUTPUT="$(mktemp "${TMPDIR:-/tmp}/ssh-hardening-ports.XXXXXX")" ||
    die "could not create a temporary file to resolve the effective sshd port into; failing closed before the disruptive step. sshd was not touched."
  # Bounded, because sshd -G blocks forever on a named pipe reached through an Include glob.
  run_bounded "$VERIFY_DEADLINE_SECONDS" resolve_ports_command || status=$?
  output="$(cat "$PORT_RESOLUTION_OUTPUT" 2>/dev/null || :)"
  rm -f "$PORT_RESOLUTION_OUTPUT"
  if [[ $status -eq $BOUNDED_TIMEOUT_STATUS ]]; then
    die "resolving the effective sshd port ('$SSHD_BIN -G') was still running after ${VERIFY_DEADLINE_SECONDS}s and was stopped, so there is no port to probe readiness on; refusing to restart blind. sshd was not touched. One known cause: a named pipe reached through an Include glob, which blocks sshd forever on the open."
  fi
  if [[ $status -ne 0 ]]; then
    die "could not resolve the effective sshd port ('$SSHD_BIN -G' exited $status); refusing to restart a daemon whose readiness could not then be probed. sshd was not touched. Output: $output"
  fi
  status=0
  listing="$(printf '%s\n' "$output" | awk '$1 == "port" { print $2 }')" ||
    status=$?
  if [[ $status -ne 0 ]]; then
    die "could not read the port out of the sshd -G output (exit $status); failing closed before the disruptive step. sshd was not touched."
  fi
  if [[ -z $listing ]]; then
    die "the effective configuration resolved NO port at all; refusing to probe readiness blind. sshd was not touched."
  fi
  # A failed here-string reads zero lines, and the empty-PROBE_PORTS check below refuses.
  while IFS= read -r port; do
    case $port in
      '' | *[!0-9]* | 0*)
        die "the effective sshd port resolved to '$port', which is not a canonical port number; refusing to probe readiness blind. sshd was not touched."
        ;;
    esac
    if [[ ${#port} -gt 5 || $port -gt 65535 ]]; then
      die "the effective sshd port resolved to '$port', which is outside 1-65535; refusing to probe readiness blind. sshd was not touched."
    fi
    duplicate=0
    if [[ ${#PROBE_PORTS[@]} -gt 0 ]]; then
      for existing in "${PROBE_PORTS[@]}"; do
        if [[ $existing == "$port" ]]; then
          duplicate=1
          break
        fi
      done
    fi
    if [[ $duplicate -eq 0 ]]; then
      PROBE_PORTS+=("$port")
    fi
  done <<<"$listing"
  if [[ ${#PROBE_PORTS[@]} -eq 0 ]]; then
    die "the effective configuration resolved no usable port; refusing to probe readiness blind. sshd was not touched."
  fi
}

# banner_output_names_host_key <output>: a stdout line shaped like a host-key record, so an
# overridden KEYSCAN_BIN exiting 0 with arbitrary text cannot prove readiness.
banner_output_names_host_key() {
  printf '%s\n' "$1" |
    awk 'NF >= 3 && $1 !~ /^#/ { found = 1 } END { exit found ? 0 : 1 }'
}

# wait_for_ssh_banner: probe every port once per attempt, up to SSH_HARDENING_READY_ATTEMPTS.
# Success needs exit 0 and a host-key record, and sets READY_PORT. Statuses are captured, since
# callers run it with errexit off.
READY_PORT=''
wait_for_ssh_banner() {
  local attempt=1 keyscan_status banner_output port status
  READY_PORT=''
  while [[ $attempt -le $SSH_HARDENING_READY_ATTEMPTS ]]; do
    for port in "${PROBE_PORTS[@]}"; do
      keyscan_status=0
      banner_output="$("$KEYSCAN_BIN" -T "$SSH_HARDENING_PROBE_TIMEOUT" -p "$port" 127.0.0.1 2>/dev/null)" ||
        keyscan_status=$?
      if [[ $keyscan_status -eq 0 ]] &&
        banner_output_names_host_key "$banner_output"; then
        READY_PORT="$port"
        return 0
      fi
    done
    if [[ $attempt -lt $SSH_HARDENING_READY_ATTEMPTS && $SSH_HARDENING_READY_INTERVAL != 0 ]]; then
      status=0
      "$SLEEP_BIN" "$SSH_HARDENING_READY_INTERVAL" || status=$?
      if [[ $status -ne 0 ]]; then
        die "the retry delay '$SLEEP_BIN $SSH_HARDENING_READY_INTERVAL' failed (exit $status) between readiness probes, so readiness cannot be awaited; the restart HAS already happened. $(recovery_instructions)"
      fi
    fi
    attempt=$((attempt + 1))
  done
  return 1
}

# reload_sshd restarts the daemon that may carry this session, so it validates everything first
# and claims success only after an SSH banner. It never rolls back: a machine that just failed a
# restart must not get a second unattended change.
reload_sshd() {
  local status output
  local tree_before_preflight tree_before_kickstart tree_changed

  validate_readiness_knobs

  # 1. Prime sudo visibly first, so a sudo failure is not mistaken for a service problem.
  if [[ -n $SSH_HARDENING_SUDO ]]; then
    status=0
    "$SSH_HARDENING_SUDO" -v || status=$?
    if [[ $status -ne 0 ]]; then
      die "privilege escalation is unavailable ('$SSH_HARDENING_SUDO -v' exited $status), and the reload needs it before it can do anything, so nothing was attempted. This is a sudo failure, not a statement about the sshd service."
    fi
  fi

  # 2. The prover and the delay tool run after the restart, so check they exist before it.
  if [[ ! -x $KEYSCAN_BIN ]]; then
    die "the readiness prover '$KEYSCAN_BIN' is not runnable, so there would be no way to prove sshd came back after a restart; refusing to kickstart blind. sshd was not touched."
  fi
  if [[ ! -x $SLEEP_BIN ]]; then
    die "the retry delay tool '$SLEEP_BIN' is not runnable, so the readiness loop could only abort after the restart; refusing to kickstart. sshd was not touched."
  fi

  # 3. Observe the tree before the first read, so every check below falls inside the observed window.
  if ! observe_config_tree; then
    die "could not read the sshd configuration tree to establish that it stays put during this reload ($CONFIG_TREE_ERROR); refusing to restart onto a tree whose stability cannot be established. sshd was not touched."
  fi
  tree_before_preflight="$CONFIG_TREE_OBSERVATION"

  # 4. Never restart onto a configuration sshd cannot parse. Privileged for the host keys, and bounded
  # because sshd -t blocks forever on a named pipe. Its message goes straight to stderr.
  status=0
  run_bounded "$VERIFY_DEADLINE_SECONDS" \
    run_privileged "$SSHD_BIN" -t -f "$SSHD_MAIN_CONFIG" || status=$?
  if [[ $status -eq $BOUNDED_TIMEOUT_STATUS ]]; then
    die "the configuration's syntax check ('$SSHD_BIN -t') was still running after ${VERIFY_DEADLINE_SECONDS}s and was stopped, so whether sshd can parse this tree is unknown; refusing to restart onto it. sshd was not touched. One known cause: a named pipe reached through an Include glob, which blocks sshd forever on the open."
  fi
  if [[ $status -ne 0 ]]; then
    die "the configuration failed sshd's syntax check ('$SSHD_BIN -t' exited $status); refusing to restart onto it. sshd was not touched. Its own message is above."
  fi

  # 5. The full verify: never restart onto a configuration that parses but has lost the hardening.
  if ! run_verify_child; then
    die "the effective configuration is not fully hardened (the verify failures are above); refusing to restart sshd onto it. sshd was not touched."
  fi

  # 6. Resolve ports now, so a missing probe target fails before the restart.
  resolve_probe_ports

  # 7. Three outcomes: loaded, confirmed absent, or probe error.
  probe_sshd_service
  if [[ $SERVICE_PROBE_STATUS -eq $LAUNCHCTL_STATUS_SERVICE_ABSENT ]]; then
    report_absent_service_outcome "$tree_before_preflight"
    return 0
  fi
  if [[ $SERVICE_PROBE_STATUS -ne 0 ]]; then
    die "could not determine the state of the sshd launchd service: '$LAUNCHCTL_BIN print $SSHD_LAUNCHD_SERVICE' exited $SERVICE_PROBE_STATUS, which is neither 0 (loaded) nor $LAUNCHCTL_STATUS_SERVICE_ABSENT (confirmed absent). A probe error is not evidence the daemon is stopped; refusing to guess. sshd was not touched. Output: $SERVICE_PROBE_OUTPUT"
  fi

  # 8. The last observation before the restart refuses if anything moved during the preflight. It comes
  # after the absent-service return, which is a documented exit-0 no-op.
  if ! observe_config_tree; then
    die "the sshd configuration tree could not be re-read before the restart ($CONFIG_TREE_ERROR); refusing to restart onto a tree whose stability cannot be established. sshd was not touched."
  fi
  tree_before_kickstart="$CONFIG_TREE_OBSERVATION"
  tree_changed=0
  config_tree_is_unchanged "$tree_before_preflight" "$tree_before_kickstart" ||
    tree_changed=$?
  if [[ $tree_changed -ne 0 ]]; then
    die "the sshd configuration tree CHANGED while this reload was validating it, so the checks above judged a tree that is no longer on disk; refusing to restart onto a configuration nothing has validated. sshd was not touched. What moved: $CONFIG_TREE_CHANGE_DESCRIPTION. Re-run --reload once the tree has settled."
  fi

  # The guard above catches a concurrent writer, not one aiming at it: a change after the last
  # observation, or one made and reverted between two, is invisible. Step 12 then refuses to claim
  # success.
  #
  # 9. Print the recovery command before the restart, which can drop the session carrying this output.
  printf '[ssh-hardening] reload: about to restart sshd; on a remote machine this can drop the SSH session carrying this output. %s\n' "$(recovery_instructions)"
  status=0
  output="$(run_privileged "$LAUNCHCTL_BIN" kickstart -k "$SSHD_LAUNCHD_SERVICE" 2>&1)" || status=$?
  if [[ $status -ne 0 ]]; then
    die "'launchctl kickstart -k $SSHD_LAUNCHD_SERVICE' failed (exit $status), so sshd may now be in any state between untouched and stopped. $(recovery_instructions) Output: $output"
  fi

  # 10. launchctl print returns 0 for a crashed service too, so this can only refute success.
  probe_sshd_service
  if [[ $SERVICE_PROBE_STATUS -ne 0 ]]; then
    die "the sshd launchd service did not reload: after the kickstart, '$LAUNCHCTL_BIN print' exited $SERVICE_PROBE_STATUS instead of confirming a loaded job. $(recovery_instructions)"
  fi

  # 11. A banner on loopback proves the daemon answers, not that remote clients reach it.
  if ! wait_for_ssh_banner; then
    die "POSSIBLE LOCKOUT: the launchd job reports loaded, but no SSH banner arrived on port(s) ${PROBE_PORTS[*]} after $SSH_HARDENING_READY_ATTEMPTS attempt(s). A loaded job with a silent listener is what a crashed sshd looks like, so treat this as a failure. One more possibility BEFORE assuming an outage: on macOS, launchd owns Remote Login's listening socket (ssh.plist inetdCompatibility) and sshd's Port directive does not move it, so if this configuration carries a nonstandard Port the daemon may be healthy on launchd's socket (normally 22) while every probe watched the wrong port; check with '$KEYSCAN_BIN -p 22 127.0.0.1' before treating this as a lockout. $(recovery_instructions)"
  fi
  # 12. This can only refute success, since a moved tree means the success line would be false.
  # It runs after the banner check, because a possible lockout matters more, and nothing is rolled back.
  if ! observe_config_tree; then
    die "sshd RESTARTED and answered on port $READY_PORT, but the configuration tree could not be re-read afterwards ($CONFIG_TREE_ERROR), so whether the daemon is serving what the preflight validated is unknown and no success is claimed. Nothing was rolled back. Check the tree with 'ssh-hardening.sh --verify'. $(recovery_instructions)"
  fi
  tree_changed=0
  config_tree_is_unchanged "$tree_before_kickstart" "$CONFIG_TREE_OBSERVATION" ||
    tree_changed=$?
  if [[ $tree_changed -ne 0 ]]; then
    die "sshd RESTARTED and answered on port $READY_PORT, but the configuration tree CHANGED after the last pre-restart check, so what the daemon actually read is unknown and no claim is made that it serves the hardened policy. Nothing was rolled back. What moved: $CONFIG_TREE_CHANGE_DESCRIPTION. Check the tree with 'ssh-hardening.sh --verify', and reload again once it has settled. $(recovery_instructions)"
  fi

  printf '[ssh-hardening] reload complete: sshd restarted and is accepting connections on port %s (SSH banner exchange completed), and every file in the configuration tree read back byte-for-byte and mode-for-mode identical each time this run read it, before the restart and after it. What the daemon read at its own instant is not observable from here, so that is a check that found nothing, not a guarantee.\n' "$READY_PORT"
}

# Three outcomes, because "still blocked" and "errored" must never read as access restored.
PASSWORD_CHANNEL_OPEN=0
PASSWORD_CHANNEL_BLOCKED=1
PASSWORD_CHANNEL_ERROR=2

# The recovery gate counts access as restored when a password or keyboard-interactive channel
# resolves yes for both a loopback and an off-loopback connection. Any anomaly is an error.
# run_bounded takes an argv, so the redirection lives in password_channel_command.
PASSWORD_CHANNEL_OUTPUT=''
PASSWORD_CHANNEL_SPEC=''
password_channel_command() {
  "$SSHD_BIN" -G -T -C "$PASSWORD_CHANNEL_SPEC" -f "$SSHD_MAIN_CONFIG" \
    >"$PASSWORD_CHANNEL_OUTPUT" 2>&1
}

# check_password_channel: owns resolve_password_channel's temp file, so every early return cleans up.
check_password_channel() {
  local status=0
  PASSWORD_CHANNEL_OUTPUT="$(mktemp "${TMPDIR:-/tmp}/ssh-hardening-recovery.XXXXXX")" ||
    return "$PASSWORD_CHANNEL_ERROR"
  resolve_password_channel || status=$?
  rm -f "$PASSWORD_CHANNEL_OUTPUT"
  return "$status"
}

resolve_password_channel() {
  local invoking_user status=0 output spec key value channel_open
  invoking_user="$(id -un)" || status=$?
  if [[ $status -ne 0 || -z $invoking_user ]]; then
    return "$PASSWORD_CHANNEL_ERROR"
  fi
  for spec in \
    "user=$invoking_user,host=localhost,addr=127.0.0.1" \
    "user=$invoking_user,host=recovery.invalid,addr=198.51.100.23"; do
    status=0
    # Bounded, because this runs after the removal, where a hang would hide that the way back in is open.
    PASSWORD_CHANNEL_SPEC="$spec"
    run_bounded "$VERIFY_DEADLINE_SECONDS" password_channel_command || status=$?
    output="$(cat "$PASSWORD_CHANNEL_OUTPUT" 2>/dev/null || :)"
    if [[ $status -ne 0 ]]; then
      return "$PASSWORD_CHANNEL_ERROR"
    fi
    channel_open=0
    for key in passwordauthentication kbdinteractiveauthentication; do
      status=0
      value="$(printf '%s\n' "$output" | awk -v k="$key" '$1 == k { print $2; exit }')" ||
        status=$?
      if [[ $status -ne 0 ]]; then
        return "$PASSWORD_CHANNEL_ERROR"
      fi
      case $value in
        yes) channel_open=1 ;;
        no) ;;
        *) return "$PASSWORD_CHANNEL_ERROR" ;;
      esac
    done
    if [[ $channel_open -ne 1 ]]; then
      return "$PASSWORD_CHANNEL_BLOCKED"
    fi
  done
  return "$PASSWORD_CHANNEL_OPEN"
}

# confirm_password_access_restored <target>: rollback's success gate, run even when the drop-in was
# already absent, since another file can still enforce the policy.
confirm_password_access_restored() {
  local target="$1" channel_status=0
  if [[ ! -x $SSHD_BIN ]]; then
    if verify_skip_allowed; then
      printf '[ssh-hardening] rollback: %s is absent, but verification was SKIPPED via the test seam; whether password access is restored was NOT checked.\n' "$target"
      return 0
    fi
    die "'$target' is absent, but '$SSHD_BIN' cannot run, so whether password access is really restored cannot be checked; failing closed rather than claiming the way back in is open"
  fi
  check_password_channel || channel_status=$?
  case $channel_status in
    "$PASSWORD_CHANNEL_OPEN")
      # --reload refuses an unhardened tree, so the restart advice names only the System Settings toggle.
      printf '[ssh-hardening] rollback complete: %s is absent and an interactive password channel (PasswordAuthentication or KbdInteractiveAuthentication) resolves ON for the sampled loopback and off-loopback connections, so password access is restored at the next sshd start. The running daemon keeps its current configuration until sshd restarts: toggle Remote Login off and back on in System Settings > General > Sharing (or reboot). --reload cannot perform this restart, because it refuses to restart onto a tree that is no longer hardened; reinstall first if the hardened policy should return.\n' "$target"
      ;;
    "$PASSWORD_CHANNEL_BLOCKED")
      die "'$target' is absent, but the interactive password channels (PasswordAuthentication and KbdInteractiveAuthentication) still resolve OFF for a sampled connection, so something else under '$SSHD_CONFIG_D' (or the main config) is still enforcing the policy and password access is NOT restored. Inspect the remaining files there."
      ;;
    *)
      die "could not verify that password access is restored: the recovery check errored instead of answering (an sshd resolution failed, was stopped at its ${VERIFY_DEADLINE_SECONDS}s bound, or could not be parsed); refusing to guess either way. The removal itself already happened, so re-run --rollback once the tree can be resolved to get the answer."
      ;;
  esac
}

# rollback_dropin: remove the drop-in first, then prove password access is back. It never restarts
# sshd, so the emergency path stays non-disruptive.
rollback_dropin() {
  local target
  target="$(dropin_path)"
  if [[ ! -e $target && ! -L $target ]]; then
    printf '[ssh-hardening] rollback: %s is already absent; nothing to remove.\n' "$target"
    confirm_password_access_restored "$target"
    return 0
  fi
  if ! run_privileged rm -f -- "$target"; then
    die "could not remove '$target'; the hardening is still in place. Remove it by hand (sudo rm $target) and re-run --rollback to confirm."
  fi
  if [[ -e $target || -L $target ]]; then
    die "'$target' still exists after the removal command reported success; refusing to claim the hardening is gone"
  fi
  printf '[ssh-hardening] rollback: removed %s\n' "$target"
  confirm_password_access_restored "$target"
}

usage() {
  cat <<'EOF'
usage: ssh-hardening.sh [--print-config | --print-path | --verify | --reload | --rollback]

  --print-config  print the generated drop-in content and exit
  --print-path    print the drop-in target path and exit
  --verify        read-only check that the effective sshd configuration is
                  fully hardened; never writes, never escalates
  --reload        validate the complete configuration, then restart the sshd
                  launchd service and prove it answers with an SSH banner
                  before claiming success; DISRUPTIVE, and fails closed at
                  every step
  --rollback      remove the managed drop-in and confirm the hardening is
                  gone from the effective configuration (the way back in);
                  never restarts sshd
  (no argument)   install the drop-in and verify

--reload is the only mode that restarts sshd, and it never writes; install
writes and never restarts. The disruptive step only ever happens because an
operator typed it.
EOF
}

main() {
  if [[ $# -gt 1 ]]; then
    usage >&2
    exit 2
  fi
  # Dispatch is case-sensitive, so a mistyped `--RELOAD` cannot run the disruptive mode.
  shopt -u nocasematch
  case "${1-}" in
    --print-config | --print-path | --verify | --reload | --rollback | '' | --help | -h) ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
  shopt -s nocasematch
  case "${1-}" in
    --print-config) print_config ;;
    --print-path) dropin_path ;;
    --verify) verify ;;
    --reload) reload_sshd ;;
    --rollback) rollback_dropin ;;
    '') install_dropin ;;
    --help | -h) usage ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
}

main "$@"
