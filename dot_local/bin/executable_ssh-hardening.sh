#!/bin/bash
# ssh-hardening.sh -- generate, install, and verify a public-key-only sshd
# drop-in. Everything here is inert for the RUNNING daemon: sshd re-reads its
# configuration only on restart, so writing the drop-in changes nothing until
# Remote Login (re)starts sshd. The reload is a deliberately separate,
# disruptive step and is NOT provided by this script.
#
# Modes:
#   --print-config  print the drop-in content (pure: no privilege, no writes)
#   --print-path    print the drop-in target path (pure)
#   --verify        read-only three-way check that the EFFECTIVE sshd
#                   configuration is fully hardened (see the verify section)
#   (no argument)   install: stage the drop-in, publish it with one rename,
#                   move the legacy 50-no-password-auth.conf aside, verify, and
#                   roll the whole tree back to what it found if that fails
#
# The drop-in file IS the lock; leave it in place permanently. Without it,
# sshd reverts to its defaults at the next restart.
#
# Seams (environment; defaults are the live values):
#   SSHD_CONFIG_D       drop-in directory      (default /etc/ssh/sshd_config.d)
#   SSHD_MAIN_CONFIG    main sshd config       (default /etc/ssh/sshd_config)
#   SSHD_BIN            sshd binary, ABSOLUTE  (default /usr/sbin/sshd) so a
#                       stripped PATH cannot turn the verifier into a no-op
#   SSH_HARDENING_SUDO  privilege wrapper for writes; set EMPTY to run
#                       unprivileged against a sandbox tree (default sudo)
#   SSH_HARDENING_ALLOW_MISSING_SSHD
#                       explicit test seam: when set to a TRUE-ish value AND
#                       $SSHD_BIN cannot run, --verify skips (exit 0) WITHOUT a
#                       verified claim. '0', 'false', 'no', 'off' and the empty
#                       string all read as OFF. Never set in the default path;
#                       absent it, an unrunnable verifier fails closed.
set -euo pipefail

# sshd matches configuration keywords AND their yes/no arguments
# case-insensitively: `PASSWORDauthentication YES` inside a Match block
# resolves to yes on OpenSSH 10.0p2. nocasematch lets every [[ ]] and case
# comparison below mirror that. The raw line is NEVER case-folded, because
# Include arguments are filenames and must keep their case.
shopt -s nocasematch

SSHD_CONFIG_D="${SSHD_CONFIG_D:-/etc/ssh/sshd_config.d}"
SSHD_MAIN_CONFIG="${SSHD_MAIN_CONFIG:-/etc/ssh/sshd_config}"
SSHD_BIN="${SSHD_BIN:-/usr/sbin/sshd}"
# `-` not `:-`: set-but-empty means "no wrapper, run the commands directly",
# which is how tests write into a user-owned sandbox without privilege.
SSH_HARDENING_SUDO="${SSH_HARDENING_SUDO-sudo}"

# This file, for the install path's strict child verify. BASH_SOURCE and not
# $0, so a caller that rewrites $0 cannot redirect the re-invocation.
SSH_HARDENING_SELF="${BASH_SOURCE[0]}"

# sshd resolves a RELATIVE Include argument against its compiled-in
# configuration directory, not the working directory (verified two ways: a
# matching file in the working directory is ignored, and `Include
# sshd_config.d/*` from a sandbox main config reproduces the live /etc/ssh
# resolution exactly). That directory is the one holding the default main
# config, so deriving it from SSHD_MAIN_CONFIG is faithful in production and
# follows the seam in a sandbox.
SSHD_CONFIG_DIR="${SSHD_MAIN_CONFIG%/*}"
if [[ $SSHD_CONFIG_DIR == "$SSHD_MAIN_CONFIG" ]]; then
  SSHD_CONFIG_DIR='.'
fi

DROPIN_NAME="000-ssh-hardening.conf"
LEGACY_DROPIN_NAME="50-no-password-auth.conf"

# sshd refuses a configuration nested deeper than this many Include levels
# ("Too many recursive configuration includes"; measured: a chain of 16 files
# below the main config is refused, 15 is accepted). The scan stops at the
# same depth and FAILS rather than returning quietly, so an include bomb
# cannot shrink the scanned set into a pass. Drop-in roots are entered at
# depth 0 where sshd reaches them at depth 1, which makes the scan tolerate
# one level more than sshd from that root -- harmless, because a tree sshd
# refuses fails check_global anyway.
MAX_INCLUDE_DEPTH=15

# The protected directives and their required values, lowercase exactly as
# `sshd -G` prints them. Parallel arrays because the deployed interpreter is
# the system bash 3.2, which has no associative arrays; every test drives this
# script through /bin/bash so a newer-bash-ism fails there.
#
# The set is derived from the userauth methods this sshd actually offers, not
# from a list of familiar directives. `strings /usr/sbin/sshd` names six:
# none, password, keyboard-interactive, publickey, gssapi-with-mic, hostbased.
#
#   password              closed by passwordauthentication no
#   keyboard-interactive  closed by kbdinteractiveauthentication no
#   none                  succeeds only when PasswordAuthentication and
#                         PermitEmptyPasswords are BOTH on, so the first
#                         directive closes it
#   publickey             the one method deliberately left open
#   gssapi-with-mic       closed by nothing above
#   hostbased             closed by nothing above
#
# The last two are why this list is seven long and not five. Both default to
# no, and a default is not a policy: both are settable inside a Match block
# (verified by resolving `sshd -G -T -C` against a tree that sets them there),
# so with only the first five named, `Match Address *,!127.0.0.1` plus
# `GSSAPIAuthentication yes` gave a second authentication method on every
# off-loopback connection while the verify reported the tree fully hardened.
PROTECTED_KEYS=(passwordauthentication kbdinteractiveauthentication usepam
  pubkeyauthentication permitrootlogin gssapiauthentication
  hostbasedauthentication)
PROTECTED_VALUES=(no no yes yes no no no)

# Keywords sshd accepts as ALIASES for a protected directive. Enumerated, not
# recalled: every lowercase keyword string in the sshd binary was extracted and
# each one set to yes and to no inside a real `Match Address` block over a
# hardened tree, then resolved with `sshd -G -T -C`. These are the only
# keywords in the whole binary that move a protected value.
#
# Two of them work inside a genuine Match block, and SkeyAuthentication is the
# dangerous one: sshd_config(5) does not document it at all, and it still
# flips kbdinteractiveauthentication to yes.
#
# DSAAuthentication is GLOBAL-only -- inside a real Match block sshd refuses
# the entire configuration, which check_global reports as a failed `sshd -G`.
# It is folded here for the `Match all` form, which sshd does accept and apply
# globally, so that the scan names the directive it moves instead of passing
# over a keyword it does not recognize.
DIRECTIVE_ALIASES=(challengeresponseauthentication skeyauthentication
  dsaauthentication)
DIRECTIVE_ALIAS_TARGETS=(kbdinteractiveauthentication
  kbdinteractiveauthentication pubkeyauthentication)

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
# 000-ssh-hardening.conf - public-key-only sshd policy, written by
# ssh-hardening.sh. This file IS the lock: remove it and sshd reverts to its
# defaults at the next restart.
#
# The 000- prefix sorts (LC_ALL=C) before Apple's 100-macos.conf. sshd's
# Include is lexical and first-value-wins, so sorting first keeps these values
# authoritative even if a future macOS release adds a competing directive to
# 100-macos.conf. Today's Apple file sets none of these, so the prefix is
# insurance, not the repair of a live conflict.
#
# PasswordAuthentication and KbdInteractiveAuthentication together close BOTH
# interactive password channels; either alone leaves one open. UsePAM yes is
# required on macOS for account and session management, and is safe here
# precisely because no password path remains for PAM to authenticate.
# PermitRootLogin no is strictly tighter than the without-password default,
# which still allows root login BY KEY.
#
# GSSAPIAuthentication and HostbasedAuthentication are the other two userauth
# methods this sshd offers. Both default to no, but a default is not a policy:
# both can be turned on inside a Match block, and none of the directives above
# constrain them. Naming them is what makes "public-key-only" a statement
# about this file rather than a hope about the defaults.
PasswordAuthentication no
KbdInteractiveAuthentication no
UsePAM yes
PubkeyAuthentication yes
PermitRootLogin no
GSSAPIAuthentication no
HostbasedAuthentication no
EOF
}

# --- verify ------------------------------------------------------------------
# Three independent, read-only, host-key-free checks. All of them run and
# EVERY failure is reported, so one broken layer cannot mask another:
#
#   1. check_global: the pre-Match effective configuration via `sshd -G`.
#      Catches a global re-enable anywhere in the include chain, including a
#      sibling that sorts before the drop-in (first-value-wins). It cannot
#      see Match blocks at all, which is why the next two exist.
#   2. check_match_scan: a text scan for a Match block that re-enables a
#      protected directive, walking the SAME include graph sshd walks.
#   3. check_connection_specs: per-connection resolution via
#      `sshd -G -T -C`. Proves Match blocks RESOLVE hardened for concrete
#      connections (root and the invoking user).
#
# What these three do and do NOT cover, stated plainly because the comment
# that used to sit here claimed a completeness the code did not have. It
# called the scan "the completeness net", and readers stopped checking:
#
#   - The connection specs are SAMPLES. Two loopback connections. A Match
#     block scoped to any other address or user is not resolved by them at
#     all, by construction, and no number of samples changes that.
#   - The scan is not a completeness proof either. It is a text scan whose
#     fidelity is bounded by how exactly the tokenizer matches sshd's own
#     parser and how exactly the Include walk matches sshd's. Both were
#     derived from the behaviour of the real binary rather than from the
#     manual page, and the forms sshd REJECTS are left to check_global: a
#     file sshd refuses fails `sshd -G`, which is check_global's whole job.
#   - Where the scan cannot see -- an unreadable file, a listing it could not
#     build, an Include cycle, a chain deeper than sshd accepts -- it FAILS
#     rather than scanning a subset and reporting clean.
#
# Any check that cannot run FAILS the verify. A skip exists only behind the
# SSH_HARDENING_ALLOW_MISSING_SSHD test seam and never claims verified.

VERIFY_FAILURES=()

# verify_skip_allowed: the SSH_HARDENING_ALLOW_MISSING_SSHD seam is ON only for
# an explicitly TRUE-ish value. It used to be tested for being NONEMPTY, so
# every value turned it on -- including `0`, the one value a reader would
# expect to turn it OFF. A seam that disables verification when set to "off" is
# a trap, and this is the whole list of values that arm it.
verify_skip_allowed() {
  case "${SSH_HARDENING_ALLOW_MISSING_SSHD:-}" in
    1 | true | yes | on) return 0 ;;
    *) return 1 ;;
  esac
}

# add_failure <message>: record a problem once. The include graph is walked
# from both roots, so a drop-in reached through the main config's Include and
# again as a root of its own would otherwise report every problem twice and
# inflate the count the summary prints.
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

# required_value <key>: for a protected directive, set MATCHED_PROTECTED_KEY to
# its canonical lowercase spelling and REQUIRED_VALUE to the value policy
# demands, then return 0. Return 1 for a key that is not protected.
#
# Globals rather than a printed result for two reasons: this runs once per
# in-Match directive line, and a command substitution per line is a fork per
# line; and the canonical spelling is what every failure message should name,
# so a file writing `PASSWORDauthentication` is reported against the same
# directive name as one writing it in lowercase.
MATCHED_PROTECTED_KEY=''
REQUIRED_VALUE=''
# canonical_key <keyword>: set CANONICAL_KEY to the protected directive this
# keyword reaches, folding sshd's aliases onto their target so an alias is
# reported, and compared, against the directive it actually moves.
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

# assert_output_hardened <check-label> <sshd -G output>: every protected
# directive must be present with its required value. Three outcomes per key,
# each named: correct, wrong value, absent. Every one is asserted
# individually; completeness beats counting.
assert_output_hardened() {
  local label="$1" output="$2" i key want got status
  for i in "${!PROTECTED_KEYS[@]}"; do
    key="${PROTECTED_KEYS[$i]}"
    want="${PROTECTED_VALUES[$i]}"
    status=0
    got="$(printf '%s\n' "$output" | awk -v k="$key" '$1 == k { print $2; exit }')" ||
      status=$?
    if [[ $status -ne 0 ]]; then
      add_failure "$label: could not read '$key' out of the sshd output (exit $status); failing closed rather than reading an unset value as absent"
    elif [[ -z $got ]]; then
      add_failure "$label: '$key' is absent from the effective configuration"
    elif [[ $got != "$want" ]]; then
      add_failure "$label: '$key' is '$got', want '$want'"
    fi
  done
}

check_global() {
  local output status=0
  # Capture first, inspect after: the exit status of a piped sshd would be
  # lost to the pipeline's last element.
  output="$("$SSHD_BIN" -G -f "$SSHD_MAIN_CONFIG" 2>&1)" || status=$?
  if [[ $status -ne 0 ]]; then
    add_failure "global check: '$SSHD_BIN -G' exited $status; failing closed rather than assuming the tree is safe (output: $output)"
    return 0
  fi
  assert_output_hardened 'global check' "$output"
}

# --- sshd configuration tokenizer --------------------------------------------
# OpenSSH 10.0p2 reads a configuration line with TWO different tokenizers, and
# the scan mirrors both. Every rule below was derived from the behaviour of the
# real binary -- `sshd -G` for accept/reject and `sshd -G -T -C` for what the
# line resolves to -- and not from sshd_config(5), which documents almost none
# of it.
#
#   THE LINE. Leading whitespace is stripped before anything else, and a line
#   that is then empty or opens with '#' is dropped.
#
#   THE KEYWORD (strdelim semantics). Space, tab and CARRIAGE RETURN separate;
#   a single '=' may stand in for the separating whitespace and is consumed
#   with it; a double-quoted segment may sit ANYWHERE in the token, its content
#   appending to what came before, and the keyword ENDS at the closing quote.
#   The one rule with no counterpart in any other config format: an EMPTY first
#   token is DISCARDED and the next token becomes the keyword, which is why
#   `=PasswordAuthentication yes` and `""PasswordAuthentication yes` are live
#   directives. Exactly one empty token is discarded; a line still keyword-less
#   after that is ignored.
#
#   THE ARGUMENTS (argv_split semantics). Only space and tab separate -- a
#   carriage return does NOT. Any number of single- OR double-quoted segments
#   concatenate with unquoted text into one argument. A backslash escapes
#   either quote, another backslash, and (outside quotes) a space or tab; every
#   other backslash is kept literally. A '#' OPENING an argument comments out
#   the rest of the line, while a '#' inside one stays literal.
#
# Both sides matter to security, not just the keyword: an Include path is an
# argument, so the argument rules decide which files the scan walks at all.
#
# What this comment does NOT claim is completeness. Four earlier versions of it
# listed the forms sshd was believed to reject and each list turned out to be
# wrong about at least one form, so the guarantee now lives in a test rather
# than in a paragraph: test/integration/ssh-hardening-tokenizer-differential.sh
# runs a corpus of forms past the REAL binary and requires --verify to refuse
# every one that sshd accepts and resolves unsafe. That corpus is bounded, so
# it is evidence and not a proof; what it buys is that the next divergence is
# found by running the suite instead of by reading this comment closely enough.
#
# Forms sshd REJECTS may be read here any way at all: a file carrying one fails
# `sshd -G`, and reporting that is check_global's job.
#
# The line is tokenized rather than bulk-normalized. Bulk normalization is
# what let the quoted keyword through: quotes were stripped from the value
# only, and a `tr` that folded case and separators could not tell a keyword
# from a value in the first place.

CONFIG_TAB=$'\t'
CONFIG_CR=$'\r'
# Written in ANSI-C quoting like the two constants above, and not as a plain
# quoted backslash: shellcheck reads '\' as a botched attempt to escape a
# single quote (SC1003) and refuses it, while shfmt -s rewrites "\\" into
# exactly that '\'. This spelling is the one both tools accept.
CONFIG_BACKSLASH=$'\\'
TOKEN=''
REST=''

# skip_keyword_separators: drop sshd's keyword separators (space, tab, CR) off
# the front of REST.
skip_keyword_separators() {
  while [[ -n $REST ]]; do
    case $REST in
      ' '* | "$CONFIG_TAB"* | "$CONFIG_CR"*) REST="${REST:1}" ;;
      *) break ;;
    esac
  done
}

# consume_keyword_separator: strdelim ends the keyword at a separator and then
# consumes the WHOLE run: the whitespace, at most one '=' standing in for it,
# and the whitespace after that. Consuming it here rather than in the caller is
# what keeps `"Keyword"=value` faithful -- a keyword ended by a closing quote
# consumes no '=' at all (measured: sshd rejects that form as
# `unsupported option "=yes"`), and the quoted branch below reflects that by
# skipping only whitespace.
consume_keyword_separator() {
  skip_keyword_separators
  if [[ $REST == '='* ]]; then
    REST="${REST:1}"
    skip_keyword_separators
  fi
}

# read_keyword_token: pull the KEYWORD off REST into TOKEN with strdelim
# semantics. Returns 1 at end of line and 2 for an unterminated quote (sshd
# ignores such a line entirely, which is what the caller does with a nonzero
# status).
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
        # A double-quoted segment may sit ANYWHERE in the keyword: its content
        # appends to what came before it, and the keyword ENDS at the closing
        # quote. `Ma"tch"` is therefore the keyword Match, and
        # `Pass"word"Authentication` is the keyword Password followed by a
        # stray token (which is why sshd rejects that one).
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

# skip_argument_separators: only space and tab separate ARGUMENTS. A carriage
# return does NOT, unlike in the keyword (measured: `Match Address<CR>*` is
# refused as one run-together criterion, and `PasswordAuthentication yes<CR>x`
# as one run-together value).
skip_argument_separators() {
  while [[ -n $REST ]]; do
    case $REST in
      ' '* | "$CONFIG_TAB"*) REST="${REST:1}" ;;
      *) break ;;
    esac
  done
}

# read_argument_token: pull one ARGUMENT off REST into TOKEN with argv_split
# semantics. Returns 1 at end of line or at a comment, and 2 for an
# unterminated quote.
read_argument_token() {
  local character escaped quote=''
  skip_argument_separators
  if [[ -z $REST ]]; then
    return 1
  fi
  # An unquoted '#' OPENING an argument comments out the rest of the line. A
  # '#' inside an argument stays literal (measured both ways:
  # `PasswordAuthentication yes #note` resolves yes, while
  # `PasswordAuthentication yes#note` is refused as the unsupported option
  # `yes#note`).
  if [[ $REST == '#'* ]]; then
    REST=''
    return 1
  fi
  TOKEN=''
  while [[ -n $REST ]]; do
    character="${REST:0:1}"
    if [[ $character == "$CONFIG_BACKSLASH" ]]; then
      # A backslash escapes either quote character, another backslash, and --
      # outside a quoted segment -- a space or a tab. Any other backslash is
      # kept, along with the character after it.
      escaped="${REST:1:1}"
      case $escaped in
        '"' | "'" | "$CONFIG_BACKSLASH")
          TOKEN="$TOKEN$escaped"
          REST="${REST:2}"
          continue
          ;;
        ' ' | "$CONFIG_TAB")
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
          # Any number of single- OR double-quoted segments concatenate with
          # unquoted text into ONE argument, so `y"es"`, `"y"es`, `'yes'` and
          # `""yes` all read as yes (measured).
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

# next_keyword_token / next_argument_token: the two readers above, each behind
# a PROGRESS guard. A reader that reports a token without consuming input would
# spin the caller's loop forever, and a verifier that HANGS never reports --
# strictly worse than one that misreads a line. Every path through both readers
# does consume, so this guards a future edit rather than a live condition.
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

# parse_config_line <raw line>: fill PARSED_KEYWORD (quotes stripped) and
# PARSED_ARGS. Returns 1 for a blank line, a comment, or a line sshd itself
# ignores or rejects.
PARSED_KEYWORD=''
PARSED_ARGS=()
parse_config_line() {
  REST="$1"
  PARSED_KEYWORD=''
  PARSED_ARGS=()
  if ! next_keyword_token; then
    return 1
  fi
  # sshd discards ONE empty keyword token and reads the NEXT token as the
  # keyword, so `=PasswordAuthentication yes` and `""PasswordAuthentication
  # yes` are both real directives to the daemon. A second empty token is not
  # discarded: the line is ignored instead.
  if [[ -z $TOKEN ]] && ! next_keyword_token; then
    return 1
  fi
  PARSED_KEYWORD="$TOKEN"
  if [[ -z $PARSED_KEYWORD ]]; then
    return 1
  fi
  # Only a '#' at the start of the keyword opens a comment, and it is tested
  # AFTER the discard above because sshd tests it there too (`=#Keyword arg`
  # is a comment, measured).
  case $PARSED_KEYWORD in
    '#'*) return 1 ;;
  esac
  while next_argument_token; do
    PARSED_ARGS+=("$TOKEN")
  done
  return 0
}

# scan_included_files <including file> <in-match> <depth> <chain>: follow the
# Include whose arguments are sitting in PARSED_ARGS.
#
# Include semantics, every one measured against OpenSSH 10.0p2 rather than
# assumed:
#
#   - The Match state in force where the Include appears APPLIES INSIDE the
#     included file. An `Include` under `Match Address *,!127.0.0.1` really
#     does re-enable password authentication for off-loopback addresses. The
#     comment that used to sit here claimed the opposite, and it was wrong:
#     the probe behind it tested a Match not reaching the NEXT included file,
#     which is a different question with a different answer.
#   - A Match opened inside the included file does NOT persist back into the
#     including file once the Include returns. The state is therefore passed
#     down by value, and the caller's copy is deliberately left alone.
#   - One Include line may carry several paths, applied left to right.
#   - A relative path resolves against sshd's configuration directory, not the
#     working directory.
#   - A path matching nothing is ignored, exactly as sshd ignores it. A file
#     that exists but cannot be read is fatal to sshd, and scan_config_file
#     treats it as fatal too.
scan_included_files() {
  local from="$1" in_match="$2" depth="$3" chain="$4"
  local pattern resolved old_ifs
  local -a patterns matches
  patterns=()
  if [[ ${#PARSED_ARGS[@]} -gt 0 ]]; then
    patterns=("${PARSED_ARGS[@]}")
  fi
  if [[ ${#patterns[@]} -eq 0 ]]; then
    add_failure "match scan: '$from' has an Include with no path; failing closed rather than guessing what it pulls in"
    return 0
  fi
  for pattern in "${patterns[@]}"; do
    case $pattern in
      /*) ;;
      *) pattern="$SSHD_CONFIG_DIR/$pattern" ;;
    esac
    # IFS=newline so a path containing spaces survives the split while the
    # glob still expands: pathname expansion produces its own fields whatever
    # IFS holds. A pattern matching nothing stays literal and is dropped by
    # the -f test below, which is what sshd does with it.
    old_ifs="$IFS"
    IFS=$'\n'
    # shellcheck disable=SC2206  # deliberate glob expansion of the Include pattern
    matches=($pattern)
    IFS="$old_ifs"
    if [[ ${#matches[@]} -eq 0 ]]; then
      continue
    fi
    for resolved in "${matches[@]}"; do
      if [[ ! -f $resolved ]]; then
        continue
      fi
      scan_config_file "$resolved" "$in_match" "$((depth + 1))" "$chain"
    done
  done
}

# scan_config_file <file> <in-match> <depth> <chain>: flag every protected
# directive set to a non-required value inside a Match block, following Include
# as it goes. <chain> is the '|'-delimited list of files already open on this
# include path, so a cycle is reported rather than walked.
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
  # Read the file directly. No here-string: bash materializes one in a
  # temporary file, so a full or unwritable TMPDIR would feed the loop zero
  # lines and the scan would report a hostile file clean.
  #
  # shellcheck disable=SC2094  # $file is passed to the recursive call for the
  # message it prints; nothing in this verifier ever opens a config file for
  # writing, and the scan is read-only by design.
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

check_match_scan() {
  local listing status=0 file old_ifs
  local -a files
  # Two roots, each walked with Include followed from it. The main config is
  # where sshd starts, so everything sshd reads is reachable from it. The
  # drop-in directory is kept as a second root so a drop-in stays covered even
  # if the main config is unreadable or its Include pattern resolves
  # differently than this scan computes; a file reached from both roots
  # reports once (see add_failure).
  #
  # The include order is lexical byte order; LC_ALL=C sort mirrors it. A
  # config file name containing a newline would break this listing; sshd's
  # own glob handling shares the no-newline assumption.
  #
  # The listing is CAPTURED, not streamed from a process substitution. A
  # process substitution discards its exit status, so a failing sort produced
  # zero files, the loop ran over nothing, and a scan of nothing reported the
  # tree clean.
  listing="$(printf '%s\n' "$SSHD_MAIN_CONFIG" "$SSHD_CONFIG_D"/* | LC_ALL=C sort -u)" ||
    status=$?
  if [[ $status -ne 0 ]]; then
    add_failure "match scan: could not list the configuration files to scan (exit $status); failing closed rather than scanning none"
    return 0
  fi
  # Split on newlines only, with globbing off so a file name containing a glob
  # character is not expanded a second time.
  old_ifs="$IFS"
  IFS=$'\n'
  set -f
  # shellcheck disable=SC2206  # deliberate newline split of the captured listing
  files=($listing)
  set +f
  IFS="$old_ifs"
  if [[ ${#files[@]} -eq 0 ]]; then
    add_failure 'match scan: the configuration file listing came back empty; failing closed rather than scanning none'
    return 0
  fi
  for file in "${files[@]}"; do
    if [[ -z $file || ! -f $file ]]; then
      continue
    fi
    scan_config_file "$file" 0 0 '|'
  done
}

check_connection_specs() {
  local invoking_user spec output status=0
  # Guarded, not assumed: the status of this substitution used to be visible
  # only through errexit, which the install path switched off.
  invoking_user="$(id -un)" || status=$?
  if [[ $status -ne 0 || -z $invoking_user ]]; then
    add_failure "connection check: could not determine the invoking user ('id -un' exited $status); failing closed rather than probing a spec built from an empty name"
    return 0
  fi
  # Two samples: root (the account PermitRootLogin exists to keep out) and
  # the invoking user (so a 'Match User <name>' aimed at the operator's own
  # account fails RESOLUTION, not only the raw scan). Samples cannot be
  # exhaustive; check_match_scan is the completeness net behind them.
  for spec in \
    'user=root,host=localhost,addr=127.0.0.1' \
    "user=$invoking_user,host=localhost,addr=127.0.0.1"; do
    status=0
    # -G -T -C: on OpenSSH 10.0p2, -C without -T is rejected and -T alone
    # demands host keys; the three together resolve Match blocks for the spec
    # with no privilege and no host keys (verified empirically).
    output="$("$SSHD_BIN" -G -T -C "$spec" -f "$SSHD_MAIN_CONFIG" 2>&1)" || status=$?
    if [[ $status -ne 0 ]]; then
      add_failure "connection check ($spec): '$SSHD_BIN -G -T -C' exited $status; failing closed (output: $output)"
      continue
    fi
    assert_output_hardened "connection check ($spec)" "$output"
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
  if [[ ${#VERIFY_FAILURES[@]} -gt 0 ]]; then
    printf '[ssh-hardening] verify FAILED, %d problem(s):\n' "${#VERIFY_FAILURES[@]}" >&2
    printf '  - %s\n' "${VERIFY_FAILURES[@]}" >&2
    return 1
  fi
  # The count comes from the array, so a directive added to policy cannot
  # leave the success line claiming a number nobody checked.
  printf '[ssh-hardening] verify: PASS: all %d protected directives hold globally, no Match block in the include graph re-enables any of them, and both sampled connections resolve hardened.\n' "${#PROTECTED_KEYS[@]}"
}

# --- install -----------------------------------------------------------------

# Install is a TRANSACTION. It stages the new drop-in beside the target,
# publishes it with one rename, moves the legacy file aside rather than
# deleting it, and only then verifies. If any step fails the tree goes back
# exactly as it was found, because refusing to CLAIM success is not the same
# as refusing to CAUSE harm and the previous version did only the first.
#
# The working files are DOT-PREFIXED deliberately: sshd's Include glob does not
# match a leading dot (glob(3) semantics, verified), so a half-written staging
# file is never part of the effective configuration and neither is a rollback
# copy. This scan skips them for the same reason.

# The transaction's state, so rollback undoes what THIS run actually did
# rather than inferring it from what happens to be on disk. Inferring is how a
# rollback deletes a drop-in the run never published: the backup copy is
# absent both before the publish step and when there was nothing to back up.
INSTALL_TARGET=''
INSTALL_LEGACY=''
INSTALL_STAGING=''
INSTALL_SAVED_TARGET=''
INSTALL_SAVED_LEGACY=''
INSTALL_PUBLISHED=0

# rollback_install: undo everything install did, and nothing it did not. Every
# step reports on its own rather than aborting the rollback, because a rollback
# that stops half way leaves exactly the state it exists to prevent.
rollback_install() {
  if [[ -e $INSTALL_STAGING || -L $INSTALL_STAGING ]]; then
    run_privileged rm -f -- "$INSTALL_STAGING" ||
      warn "rollback could not remove the staging file '$INSTALL_STAGING'"
  fi
  # Only if this run replaced the target is the target this run's to undo.
  if [[ $INSTALL_PUBLISHED -eq 1 ]]; then
    if [[ -e $INSTALL_SAVED_TARGET || -L $INSTALL_SAVED_TARGET ]]; then
      run_privileged mv -f -- "$INSTALL_SAVED_TARGET" "$INSTALL_TARGET" ||
        warn "rollback could not restore the previous drop-in from '$INSTALL_SAVED_TARGET'"
    else
      # No drop-in existed before this run, so removing the one it created is
      # what "as it was found" means.
      run_privileged rm -f -- "$INSTALL_TARGET" ||
        warn "rollback could not remove the drop-in '$INSTALL_TARGET' this run created"
    fi
  fi
  if [[ -e $INSTALL_SAVED_LEGACY || -L $INSTALL_SAVED_LEGACY ]]; then
    run_privileged mv -f -- "$INSTALL_SAVED_LEGACY" "$INSTALL_LEGACY" ||
      warn "rollback could not restore the legacy drop-in from '$INSTALL_SAVED_LEGACY'"
  fi
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

  [[ -d $SSHD_CONFIG_D ]] ||
    die "drop-in directory '$SSHD_CONFIG_D' does not exist"
  if ! run_privileged rm -f -- "$staging" "$saved_target" "$saved_legacy"; then
    die "could not clear the working files under '$SSHD_CONFIG_D'; refusing to begin an install that could not be rolled back"
  fi

  # Stage first, publish second. `tee` truncates its target the instant it
  # opens it, so writing the drop-in directly EMPTIED a perfectly good file
  # whenever the pipeline feeding it failed afterwards -- which a PATH without
  # `cat` does, and did.
  if ! print_config | run_privileged tee -- "$staging" >/dev/null; then
    rollback_install
    die "could not stage the new drop-in at '$staging'; '$target' is untouched"
  fi
  # Explicit mode, never the ambient umask: under e.g. umask 0077 tee lands
  # the file 0600, and a root-owned 0600 drop-in makes UNPRIVILEGED `sshd -G`
  # fail outright, so the whole verification would need elevation. 0644 is
  # safe: the file holds no credential and sshd must be able to read it.
  # `--` BEFORE the mode: BSD chmod treats a `--` after the mode as a file
  # operand and fails.
  if ! run_privileged chmod -- 0644 "$staging"; then
    rollback_install
    die "could not set mode 0644 on '$staging'; '$target' is untouched"
  fi
  # Copy the existing drop-in aside BEFORE replacing it, so the target itself
  # is never absent: the copy is the backup, and the rename below is atomic.
  if [[ -e $target || -L $target ]]; then
    if ! run_privileged cp -p -- "$target" "$saved_target"; then
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

  # Retire the legacy drop-in, reversibly. Two reasons it has to go: one lock
  # in one file (two files declaring the same policy is drift waiting to
  # happen), and the legacy file was created 0600 under the umask, which breaks
  # unprivileged verification for the entire tree (see the chmod comment
  # above). It is MOVED, not deleted, until the replacement is proven good --
  # deleting the only effective policy and then failing verification is exactly
  # how an install leaves a machine worse than it found it.
  if [[ -e $legacy || -L $legacy ]]; then
    if ! run_privileged mv -f -- "$legacy" "$saved_legacy"; then
      rollback_install
      die "could not move the legacy drop-in '$legacy' aside"
    fi
    printf '[ssh-hardening] removed legacy drop-in %s\n' "$legacy"
  fi
  # Verify in a CHILD shell. bash suppresses `set -e` for everything inside an
  # `if !` or `||` test, and that suppression reaches into called functions and
  # even into subshells (confirmed on bash 3.2), so calling verify in-process
  # here ran every check with errexit switched off: a failure mid-flight was
  # stepped over and the success line still printed. A separate process gets
  # its own `set -euo pipefail`, so install judges the tree by exactly the
  # rules --verify applies on its own. The seams are passed explicitly so the
  # child inspects the tree this install just wrote whether or not the caller
  # exported them.
  local verify_status=0
  SSHD_CONFIG_D="$SSHD_CONFIG_D" \
    SSHD_MAIN_CONFIG="$SSHD_MAIN_CONFIG" \
    SSHD_BIN="$SSHD_BIN" \
    SSH_HARDENING_SUDO="$SSH_HARDENING_SUDO" \
    SSH_HARDENING_ALLOW_MISSING_SSHD="${SSH_HARDENING_ALLOW_MISSING_SSHD:-}" \
    "${BASH:-/bin/bash}" "$SSH_HARDENING_SELF" --verify || verify_status=$?
  if [[ $verify_status -ne 0 ]]; then
    rollback_install
    die "the effective configuration did NOT verify as fully hardened; the tree was rolled back to the state this install found it in, and no success is claimed"
  fi
  if ! run_privileged rm -f -- "$saved_target" "$saved_legacy"; then
    warn "could not remove the rollback copies under '$SSHD_CONFIG_D'; they are dot-prefixed and inert, but should be cleaned up"
  fi
  # The child's skip cannot be read out of its exit status, so the same
  # predicate decides the wording here. One function, two callers, no second
  # copy of the rule to drift.
  if [[ ! -x $SSHD_BIN ]] && verify_skip_allowed; then
    printf '[ssh-hardening] wrote %s, but verification was SKIPPED via the test seam; the effective configuration is NOT verified.\n' "$target"
    return 0
  fi
  printf '[ssh-hardening] install complete: %s is in place and the effective configuration verified fully hardened.\n' "$target"
}

usage() {
  cat <<'EOF'
usage: ssh-hardening.sh [--print-config | --print-path | --verify]

  --print-config  print the generated drop-in content and exit
  --print-path    print the drop-in target path and exit
  --verify        read-only check that the effective sshd configuration is
                  fully hardened; never writes, never escalates
  (no argument)   install the drop-in and verify

Reloading a running sshd is deliberately not provided here; the drop-in
takes effect when sshd next starts.
EOF
}

main() {
  if [[ $# -gt 1 ]]; then
    usage >&2
    exit 2
  fi
  case "${1-}" in
    --print-config) print_config ;;
    --print-path) dropin_path ;;
    --verify) verify ;;
    '') install_dropin ;;
    --help | -h) usage ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
}

main "$@"
