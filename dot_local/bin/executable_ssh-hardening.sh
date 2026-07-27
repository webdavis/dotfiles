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
#   (no argument)   install: write the drop-in, pin mode 0644, migrate the
#                   legacy 50-no-password-auth.conf away, then run the verify
#                   and refuse to claim success unless it passes
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
#                       explicit test seam: when set AND $SSHD_BIN cannot run,
#                       --verify skips (exit 0) WITHOUT a verified claim.
#                       Never set in the default path; absent it, an
#                       unrunnable verifier fails closed.
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

# The five protected directives and their required values, lowercase exactly
# as `sshd -G` prints them. Parallel arrays because the deployed interpreter
# is the system bash 3.2, which has no associative arrays; every test drives
# this script through /bin/bash so a newer-bash-ism fails there.
PROTECTED_KEYS=(passwordauthentication kbdinteractiveauthentication usepam
  pubkeyauthentication permitrootlogin)
PROTECTED_VALUES=(no no yes yes no)

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
PasswordAuthentication no
KbdInteractiveAuthentication no
UsePAM yes
PubkeyAuthentication yes
PermitRootLogin no
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
VERIFY_SKIPPED=0

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
# each named: correct, wrong value, absent. All five are asserted
# individually; completeness beats counting.
assert_output_hardened() {
  local label="$1" output="$2" i key want got
  for i in "${!PROTECTED_KEYS[@]}"; do
    key="${PROTECTED_KEYS[$i]}"
    want="${PROTECTED_VALUES[$i]}"
    got="$(printf '%s\n' "$output" | awk -v k="$key" '$1 == k { print $2; exit }')"
    if [[ -z $got ]]; then
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
# OpenSSH 10.0p2 splits a configuration line the way strdelim() does: space,
# tab and CARRIAGE RETURN all separate tokens; a token that opens with a
# double quote runs to its closing quote; and a single '=' may stand in for
# the whitespace after the keyword. Every one of those forms was confirmed to
# parse AND to resolve the unsafe value against the real binary, including the
# two that defeated the previous scanner:
#
#   "PasswordAuthentication" yes      quotes around the KEYWORD, not the value
#   Match<CR>Address *,!127.0.0.1     a carriage return inside the Match line
#
# Vertical tab, form feed, a second '=', a quote in the middle of a keyword
# and a whole-line quote are all REJECTED by sshd (also verified), so they
# need no handling here: a file carrying them fails `sshd -G`, and reporting
# that is check_global's job.
#
# The line is tokenized rather than bulk-normalized. Bulk normalization is
# what let the quoted keyword through: quotes were stripped from the value
# only, and a `tr` that folded case and separators could not tell a keyword
# from a value in the first place.

CONFIG_TAB=$'\t'
CONFIG_CR=$'\r'
TOKEN=''
REST=''

# next_token <break-on-equals: 0|1>: pull the next token off REST into TOKEN.
# Returns 1 at end of line and 2 for an unterminated quote (a form sshd
# rejects outright, so the file fails check_global).
next_token() {
  local break_equals="$1" char start_length="${#REST}"
  while [[ -n $REST ]]; do
    case $REST in
      ' '* | "$CONFIG_TAB"* | "$CONFIG_CR"*) REST="${REST:1}" ;;
      *) break ;;
    esac
  done
  if [[ -z $REST ]]; then
    return 1
  fi
  TOKEN=''
  if [[ $REST == '"'* ]]; then
    REST="${REST:1}"
    case $REST in
      *'"'*) ;;
      *) return 2 ;;
    esac
    TOKEN="${REST%%\"*}"
    REST="${REST#*\"}"
    return 0
  fi
  while [[ -n $REST ]]; do
    char="${REST:0:1}"
    case $char in
      ' ' | "$CONFIG_TAB" | "$CONFIG_CR" | '"') break ;;
    esac
    if [[ $break_equals -eq 1 && $char == '=' ]]; then
      break
    fi
    TOKEN="$TOKEN$char"
    REST="${REST:1}"
  done
  # A successful token must have consumed input. Reporting success without
  # advancing REST would spin the caller's loop forever, and a verifier that
  # HANGS never reports -- strictly worse than one that misreads a line. The
  # quoted branch above means no input reaches here without advancing, so this
  # is a guard against a future edit to that branch, not a live condition.
  if [[ ${#REST} -eq $start_length ]]; then
    return 1
  fi
  return 0
}

# parse_config_line <raw line>: fill PARSED_KEYWORD (quotes stripped) and
# PARSED_ARGS. Returns 1 for a blank line, a comment, or a line sshd itself
# would reject.
PARSED_KEYWORD=''
PARSED_ARGS=()
parse_config_line() {
  REST="$1"
  PARSED_KEYWORD=''
  PARSED_ARGS=()
  if ! next_token 1; then
    return 1
  fi
  PARSED_KEYWORD="$TOKEN"
  # Only a '#' at the start of the first token opens a comment.
  case $PARSED_KEYWORD in
    '#'*) return 1 ;;
  esac
  # A single '=' may replace the whitespace after the keyword.
  while [[ -n $REST ]]; do
    case $REST in
      ' '* | "$CONFIG_TAB"* | "$CONFIG_CR"*) REST="${REST:1}" ;;
      *) break ;;
    esac
  done
  if [[ $REST == '='* ]]; then
    REST="${REST:1}"
  fi
  while next_token 0; do
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
  local file
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
  while IFS= read -r file; do
    [[ -n $file && -f $file ]] || continue
    scan_config_file "$file" 0 0 '|'
  done < <(printf '%s\n' "$SSHD_MAIN_CONFIG" "$SSHD_CONFIG_D"/* | LC_ALL=C sort -u)
}

check_connection_specs() {
  local invoking_user spec output status
  invoking_user="$(id -un)"
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
  VERIFY_SKIPPED=0
  if [[ ! -x $SSHD_BIN ]]; then
    if [[ -n ${SSH_HARDENING_ALLOW_MISSING_SSHD:-} ]]; then
      VERIFY_SKIPPED=1
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
  printf '[ssh-hardening] verify: PASS: all five directives hold globally, no Match block re-enables any of them, and both sampled connections resolve hardened.\n'
}

# --- install -----------------------------------------------------------------

install_dropin() {
  local target legacy
  target="$(dropin_path)"
  legacy="$SSHD_CONFIG_D/$LEGACY_DROPIN_NAME"
  [[ -d $SSHD_CONFIG_D ]] ||
    die "drop-in directory '$SSHD_CONFIG_D' does not exist"
  if ! print_config | run_privileged tee -- "$target" >/dev/null; then
    die "could not write '$target'"
  fi
  # Explicit mode, never the ambient umask: under e.g. umask 0077 tee lands
  # the file 0600, and a root-owned 0600 drop-in makes UNPRIVILEGED `sshd -G`
  # fail outright, so the whole verification would need elevation. 0644 is
  # safe: the file holds no credential and sshd must be able to read it.
  # `--` BEFORE the mode: BSD chmod treats a `--` after the mode as a file
  # operand and fails.
  if ! run_privileged chmod -- 0644 "$target"; then
    die "could not set mode 0644 on '$target'"
  fi
  printf '[ssh-hardening] wrote %s (mode 0644)\n' "$target"
  # Migrate the legacy drop-in away. Two reasons: one lock in one file (two
  # files declaring the same policy is drift waiting to happen), and the
  # legacy file was created 0600 under the umask, which breaks unprivileged
  # verification for the entire tree (see the chmod comment above).
  if [[ -e $legacy || -L $legacy ]]; then
    if ! run_privileged rm -f -- "$legacy"; then
      die "could not remove the legacy drop-in '$legacy'"
    fi
    printf '[ssh-hardening] removed legacy drop-in %s\n' "$legacy"
  fi
  if ! verify; then
    die "wrote '$target' but the effective configuration did NOT verify as fully hardened; refusing to claim success"
  fi
  if [[ $VERIFY_SKIPPED -eq 1 ]]; then
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
