#!/usr/bin/env bash
# ssh-hardening-tokenizer-differential.sh -- the scanner is a MODEL of sshd's
# configuration parser, and every place the model diverges from the real parser
# is a silent bypass. Five such divergences have been found on this one script,
# each by an expensive reading of the code, each patched, each followed by
# another. Enumerating known-bad forms has failed five times, so this test
# asserts the PROPERTY instead:
#
#   for every configuration line the real sshd ACCEPTS, if the resulting tree
#   resolves a protected directive to an unsafe value for an off-loopback
#   client, then `ssh-hardening.sh --verify` MUST exit nonzero.
#
# The expectation for each form comes from the BINARY, never from a
# hand-written table: the test asks `sshd -G` whether the tree is accepted and
# `sshd -G -T -C` what it resolves to, and only then asks whether --verify
# agrees. So a form nobody anticipated, or an OpenSSH release that starts
# accepting a form it used to reject, is caught here rather than rediscovered
# in review.
#
# Three outcomes per corpus form, each reported by name:
#
#   rejected  sshd refuses the tree. The scan may read the line any way it
#             likes; --verify must still exit nonzero, because a tree sshd
#             refuses fails `sshd -G` and that is check_global's whole job.
#             Asserted.
#   safe      sshd accepts the tree and every protected directive still
#             resolves to its required value (the line is inert, e.g. one sshd
#             ignores). Recorded, not asserted: a scan that flags an inert line
#             is over-strict, which is a false FAIL and not a bypass.
#   unsafe    sshd accepts the tree AND a protected directive resolves wrong.
#             --verify must exit nonzero. This is the security property, and
#             the only outcome whose failure is a hole.
#
# The corpus is BOUNDED, not exhaustive: it is a list of forms, and no list of
# forms proves the absence of a divergence over all possible inputs. What it
# does buy is that the next divergence in this class is found by running the
# suite instead of by reading the tokenizer closely enough. The seeds are the
# empty-first-token family, the four divergences found before it as regression
# fixtures, the argument-side quoting forms, and the forms believed to be
# REJECTED -- so an OpenSSH that starts accepting one of those fails this test
# instead of quietly opening a hole.
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite runs from the pre-push hook.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

# shellcheck source=../fixtures/ssh-hardening-lib.bash
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../fixtures" && pwd)/ssh-hardening-lib.bash"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -x /usr/sbin/sshd ]] || {
  printf 'SKIP: /usr/sbin/sshd not present; the differential needs the real parser\n'
  exit 0
}

ssh_sandbox_setup
trap 'ssh_sandbox_teardown' EXIT

write_hardened_dropin
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "positive control: --verify must PASS on the clean hardened tree (stderr: $SSH_RUN_ERR)"

hostile_file="$SSHD_CONFIG_D/500-hostile.conf"

# An address in TEST-NET-2 (RFC 5737): routable-looking, reserved for
# documentation, and deliberately NOT one of the loopback samples --verify
# probes, so every form below is judged where only the scan can see it.
off_sample_specification='user=offsample,host=elsewhere.example,addr=198.51.100.23'

MATCH_OFF_LOOPBACK='Match Address *,!127.0.0.1'
# Named, because these bytes are invisible in a diff and in a terminal. The
# carriage return comes from the shared library so both suites exercise the
# same byte.
CARRIAGE_RETURN="$SSH_CARRIAGE_RETURN"
HORIZONTAL_TAB=$'\t'
VERTICAL_TAB=$'\v'
FORM_FEED=$'\f'

# Pulled in by an Include from inside a Match block. It lives OUTSIDE the
# drop-in directory so the main config's own Include glob does not read it: the
# only path to it is the Include under test.
include_payload="$SSH_SANDBOX/payload.conf"
printf 'PasswordAuthentication yes\n' >"$include_payload"
include_directory="${include_payload%payload.conf}"

# A second copy under a directory whose name contains a SPACE, so the corpus
# can exercise the two ways sshd lets an argument carry one: a backslash escape
# and a quoted segment. Both are accepted, and both really do pull the payload
# in (measured), so a scanner that splits the path on the space walks past an
# Include sshd follows.
spaced_include_directory="$SSH_SANDBOX/with space"
mkdir -p "$spaced_include_directory"
printf 'PasswordAuthentication yes\n' >"$spaced_include_directory/payload.conf"

# A third copy whose FILE NAME carries a carriage return. sshd separates
# arguments on space and tab only, so this path arrives at the Include intact
# and the payload really is pulled in (measured); a scanner that reuses the
# keyword's separator set splits the name in half and walks past it.
carriage_return_include="$SSH_SANDBOX/payload${CARRIAGE_RETURN}cr.conf"
printf 'PasswordAuthentication yes\n' >"$carriage_return_include"

# A fourth copy whose FILE NAME carries a horizontal tab, for the backslash
# asymmetry: `\<space>` is an escape to argv_split but `\<TAB>` is NOT -- the
# backslash stays literal and the tab still separates (both measured). Only a
# QUOTED segment carries a tab into an Include path.
tab_include="$SSH_SANDBOX/pay${HORIZONTAL_TAB}load.conf"
printf 'PasswordAuthentication yes\n' >"$tab_include"

# An inert file whose name begins with '#', for the mirror-image rule: a '#'
# opening an argument is a COMMENT to sshd, so a scan that reads it as another
# Include pattern would resolve this file and raise an alarm about a directive
# the daemon never reads.
printf '%s\nPasswordAuthentication yes\n' "$MATCH_OFF_LOOPBACK" \
  >"$SSH_SANDBOX/#hash-named.conf"
harmless_include="$SSH_SANDBOX/harmless.conf"
printf 'PubkeyAuthentication yes\n' >"$harmless_include"

rejected_count=0
safe_count=0
unsafe_count=0
bypass_forms=()
false_alarm_forms=()
control_outcome=''

# resolve_unsafe_directives: print every protected directive that resolves to
# something other than its required value for the off-sample connection, as
# 'key=value' pairs. Empty output means the tree resolves fully hardened.
resolve_unsafe_directives() {
  local resolution pair key want got unsafe=''
  resolution="$(/usr/sbin/sshd -G -T -C "$off_sample_specification" \
    -f "$SSHD_MAIN_CONFIG" 2>&1)" || return 1
  for pair in "${SSH_HARDENED_PAIRS[@]}"; do
    key="${pair%% *}"
    want="${pair##* }"
    got="$(printf '%s\n' "$resolution" | awk -v k="$key" '$1 == k { print $2; exit }')"
    if [[ $got != "$want" ]]; then
      unsafe="$unsafe $key=$got"
    fi
  done
  printf '%s' "$unsafe"
}

# differential_case <label> <line>...: write the lines as a drop-in over the
# hardened tree, ask the real sshd what it makes of them, then require
# --verify to agree. Sets CASE_OUTCOME to rejected/safe/unsafe.
CASE_OUTCOME=''
differential_case() {
  local label="$1"
  shift
  printf '%s\n' "$@" >"$hostile_file"

  local accepted=1 unsafe=''
  /usr/sbin/sshd -G -f "$SSHD_MAIN_CONFIG" >/dev/null 2>&1 || accepted=0
  if [[ $accepted -eq 1 ]]; then
    unsafe="$(resolve_unsafe_directives)" ||
      fail "$label: the tree passed 'sshd -G' but 'sshd -G -T -C' could not resolve it"
  fi

  run_ssh_hardening --verify
  local verify_status="$SSH_RUN_STATUS"

  if [[ $accepted -eq 0 ]]; then
    CASE_OUTCOME='rejected'
    rejected_count=$((rejected_count + 1))
    printf '  [rejected] %s\n' "$label"
    if [[ $verify_status -eq 0 ]]; then
      bypass_forms+=("$label: sshd REJECTS this tree, so 'sshd -G' fails, yet --verify exited 0")
    fi
  elif [[ -z $unsafe ]]; then
    CASE_OUTCOME='safe'
    safe_count=$((safe_count + 1))
    if [[ $verify_status -eq 0 ]]; then
      printf '  [safe    ] %s\n' "$label"
    else
      printf '  [ALARM   ] %s -> --verify refused a tree sshd resolves hardened\n' "$label"
      false_alarm_forms+=("$label: sshd accepts this tree and resolves every protected directive correctly, yet --verify exited $verify_status")
    fi
  else
    CASE_OUTCOME='unsafe'
    unsafe_count=$((unsafe_count + 1))
    if [[ $verify_status -ne 0 ]]; then
      printf '  [unsafe  ] %s -> refused (%s )\n' "$label" "$unsafe"
    else
      printf '  [BYPASS  ] %s -> --verify PASSED (%s )\n' "$label" "$unsafe"
      bypass_forms+=("$label: sshd resolves$unsafe for $off_sample_specification, yet --verify exited 0")
    fi
  fi
  rm -f "$hostile_file"
}

printf 'differential corpus (expectations taken from /usr/sbin/sshd, not from a table):\n'

# --- the vacuity control -----------------------------------------------------
# If the plainest possible Match re-enable stops being accepted-and-unsafe, the
# fixture no longer measures anything and every result below is meaningless.
differential_case 'control: plain PasswordAuthentication yes inside Match' \
  "$MATCH_OFF_LOOPBACK" 'PasswordAuthentication yes'
control_outcome="$CASE_OUTCOME"

# --- the empty-first-token family --------------------------------------------
# sshd discards ONE empty keyword token and reads the NEXT token as the
# keyword, so each of these is a real directive to the daemon.
differential_case 'empty token: =PasswordAuthentication yes' \
  "$MATCH_OFF_LOOPBACK" '=PasswordAuthentication yes'
differential_case 'empty token: = PasswordAuthentication yes' \
  "$MATCH_OFF_LOOPBACK" '= PasswordAuthentication yes'
differential_case 'empty token: ""PasswordAuthentication yes' \
  "$MATCH_OFF_LOOPBACK" '""PasswordAuthentication yes'
differential_case 'empty token: "" PasswordAuthentication yes' \
  "$MATCH_OFF_LOOPBACK" '"" PasswordAuthentication yes'
differential_case 'empty token: ="PasswordAuthentication" yes' \
  "$MATCH_OFF_LOOPBACK" '="PasswordAuthentication" yes'
differential_case 'empty token: leading space then =PasswordAuthentication' \
  "$MATCH_OFF_LOOPBACK" ' =PasswordAuthentication yes'
differential_case 'empty token: leading tab then =PasswordAuthentication' \
  "$MATCH_OFF_LOOPBACK" "${HORIZONTAL_TAB}=PasswordAuthentication yes"
differential_case 'empty token: leading CR then ""PasswordAuthentication' \
  "$MATCH_OFF_LOOPBACK" "${CARRIAGE_RETURN}\"\"PasswordAuthentication yes"
differential_case 'empty token: leading space then ""PasswordAuthentication' \
  "$MATCH_OFF_LOOPBACK" ' ""PasswordAuthentication yes'
differential_case 'empty token: =PasswordAuthentication=yes' \
  "$MATCH_OFF_LOOPBACK" '=PasswordAuthentication=yes'
differential_case 'empty token: ""PasswordAuthentication=yes' \
  "$MATCH_OFF_LOOPBACK" '""PasswordAuthentication=yes'
differential_case 'empty token: =PasswordAuthentication = yes' \
  "$MATCH_OFF_LOOPBACK" '=PasswordAuthentication = yes'

# The same discard on the other protected directives and on an alias, so the
# fix cannot be a special case wired to one keyword.
differential_case 'empty token: =KbdInteractiveAuthentication yes' \
  "$MATCH_OFF_LOOPBACK" '=KbdInteractiveAuthentication yes'
differential_case 'empty token: =PubkeyAuthentication no' \
  "$MATCH_OFF_LOOPBACK" '=PubkeyAuthentication no'
differential_case 'empty token: =PermitRootLogin yes' \
  "$MATCH_OFF_LOOPBACK" '=PermitRootLogin yes'
differential_case 'empty token: =GSSAPIAuthentication yes' \
  "$MATCH_OFF_LOOPBACK" '=GSSAPIAuthentication yes'
differential_case 'empty token: ""HostbasedAuthentication yes' \
  "$MATCH_OFF_LOOPBACK" '""HostbasedAuthentication yes'
differential_case 'empty token: =SkeyAuthentication yes (undocumented alias)' \
  "$MATCH_OFF_LOOPBACK" '=SkeyAuthentication yes'

# The discard applied to the Match line itself hides the whole block.
differential_case 'empty token: =Match opens the block' \
  '=Match Address *,!127.0.0.1' 'PasswordAuthentication yes'
differential_case 'empty token: ""Match opens the block' \
  '""Match Address *,!127.0.0.1' 'PasswordAuthentication yes'
differential_case 'empty token: =Include pulls the payload in' \
  "$MATCH_OFF_LOOPBACK" "=Include $include_payload"

# --- regression cover OUTSIDE the empty-token property ------------------------
# These two carry '=' forms but do NOT exercise the discard: `Match all`
# applies to the GLOBAL resolution (measured: plain `sshd -G` with no -C
# resolves a Match-all block's directives), so even a scan with no discard at
# all still refuses both through check_global. They were filed under the
# empty-token family and read as evidence for a property they never tested;
# they stay as regression cover for the global path.
differential_case 'global path (discard not exercised): =DSAAuthentication no under Match all' \
  'Match all' '=DSAAuthentication no'
differential_case 'global path (discard not exercised): =Match=all opens the block' \
  '=Match=all' 'PasswordAuthentication yes'

# --- the four divergences found before this one, as regression fixtures ------
differential_case 'regression 1: Include followed from inside a Match block' \
  "$MATCH_OFF_LOOPBACK" "Include $include_payload"
differential_case 'regression 2: quoted keyword "PasswordAuthentication" yes' \
  "$MATCH_OFF_LOOPBACK" '"PasswordAuthentication" yes'
differential_case 'regression 3: carriage return inside the Match line' \
  "Match${CARRIAGE_RETURN}Address *,!127.0.0.1" 'PasswordAuthentication yes'
differential_case 'regression 4: mixed-quote keyword Ma"tch"' \
  'Ma"tch" Address *,!127.0.0.1' 'PasswordAuthentication yes'
differential_case 'regression 4b: mixed-quote keyword Ma"tch"Address, no space' \
  'Ma"tch"Address *,!127.0.0.1' 'PasswordAuthentication yes'
differential_case 'regression 4c: mixed-quote keyword PasswordAuth"entication"' \
  "$MATCH_OFF_LOOPBACK" 'PasswordAuth"entication" yes'
differential_case 'regression 4d: trailing empty quote PasswordAuthentication""' \
  "$MATCH_OFF_LOOPBACK" 'PasswordAuthentication"" yes'
differential_case 'regression 4e: mixed-quote keyword Inc"lude"' \
  "$MATCH_OFF_LOOPBACK" "Inc\"lude\" $include_payload"

# --- argument-side quoting ----------------------------------------------------
# sshd reads ARGUMENTS with argv_split, not with the keyword's tokenizer: any
# number of single- OR double-quoted segments concatenate with unquoted text
# into one argument. An Include path is an argument, so this side of the
# tokenizer decides which files the scan walks at all.
differential_case 'argument: PasswordAuthentication y"es"' \
  "$MATCH_OFF_LOOPBACK" 'PasswordAuthentication y"es"'
differential_case 'argument: PasswordAuthentication "y"es' \
  "$MATCH_OFF_LOOPBACK" 'PasswordAuthentication "y"es'
differential_case "argument: PasswordAuthentication 'yes' (single quotes)" \
  "$MATCH_OFF_LOOPBACK" "PasswordAuthentication 'yes'"
differential_case "argument: PasswordAuthentication y'es'" \
  "$MATCH_OFF_LOOPBACK" "PasswordAuthentication y'es'"
differential_case 'argument: PasswordAuthentication ""yes' \
  "$MATCH_OFF_LOOPBACK" 'PasswordAuthentication ""yes'
differential_case 'argument: PasswordAuthentication "yes"' \
  "$MATCH_OFF_LOOPBACK" 'PasswordAuthentication "yes"'
differential_case 'argument: Include path with a mid-path double quote' \
  "$MATCH_OFF_LOOPBACK" "Include ${include_directory}pay\"load\".conf"
differential_case 'argument: Include path with a mid-path single quote' \
  "$MATCH_OFF_LOOPBACK" "Include ${include_directory}pay'load'.conf"
differential_case 'argument: Include path fully double quoted' \
  "$MATCH_OFF_LOOPBACK" "Include \"$include_payload\""
differential_case 'argument: Include quoted directory prefix' \
  "$MATCH_OFF_LOOPBACK" "Include \"$include_directory\"payload.conf"
differential_case 'argument: Include path followed by a # comment' \
  "$MATCH_OFF_LOOPBACK" "Include $include_payload #note"
differential_case 'argument: Include path with a backslash-escaped space' \
  "$MATCH_OFF_LOOPBACK" "Include ${SSH_SANDBOX}/with\\ space/payload.conf"
differential_case 'argument: Include path with the space inside quotes' \
  "$MATCH_OFF_LOOPBACK" "Include \"$spaced_include_directory/payload.conf\""
differential_case 'argument: Include path whose file name contains a CR' \
  "$MATCH_OFF_LOOPBACK" "Include $carriage_return_include"
# The backslash-tab asymmetry, both directions. `\<TAB>` is a SEPARATOR:
# sshd keeps the backslash, ends the argument at the tab, and reads what
# follows as a FURTHER Include argument -- which it follows. A scanner that
# reads `\<TAB>` as an escaped tab merges the two into one bogus path: it
# walks a file sshd never reads (false alarm) and walks PAST the second
# argument sshd does read (bypass).
differential_case 'argument: backslash-tab merged path names a real file sshd never reads' \
  "$MATCH_OFF_LOOPBACK" "Include ${SSH_SANDBOX}/pay\\${HORIZONTAL_TAB}load.conf"
differential_case 'argument: backslash-tab separates; sshd follows the second path' \
  "$MATCH_OFF_LOOPBACK" "Include ${SSH_SANDBOX}/absent\\${HORIZONTAL_TAB}${include_payload}"
differential_case 'argument: Include path with the tab inside quotes' \
  "$MATCH_OFF_LOOPBACK" "Include \"$tab_include\""
differential_case 'argument: a # comment naming a file that exists' \
  "$MATCH_OFF_LOOPBACK" "Include $harmless_include #hash-named.conf"
differential_case 'argument: Match criteria with a quoted segment' \
  'Match Addr"ess" *,!127.0.0.1' 'PasswordAuthentication yes'

# --- the backslash-and-glob class, filed separately ---------------------------
# Recorded, not asserted: sshd unescapes `\\` to `\` and the following space
# then SEPARATES, so this Include resolves nothing on this binary and the tree
# stays hardened -- the case lands in the 'safe' bin and exercises no walk.
# The scanner's real divergence in this family (sshd unescapes an Include path
# TWICE, argv_split then glob(3), while the scan unescapes once) predates this
# branch, is live on main, and is filed as its own task. This entry only pins
# that the form is inert today, so an OpenSSH that starts following it flips
# the case to unsafe and the assertion fires on its own.
differential_case 'inert today (backslash-glob class, filed separately): with\\ space' \
  "$MATCH_OFF_LOOPBACK" "Include ${SSH_SANDBOX}/with\\\\ space/payload.conf"

# --- line-trailing whitespace -------------------------------------------------
# sshd trims trailing space, tab, carriage return and FORM FEED off the whole
# line before either tokenizer runs; vertical tab, BEL and BS are NOT in the
# set (all seven measured against the binary). The CRLF line ending is the
# everyday carrier: every line of a file with Windows line endings ends in
# <CR>. A scanner without the trim carries the CR into the Include pattern it
# globs (walking past a file sshd reads) and into the value it compares
# (raising a false alarm on a hardened restatement, which makes install roll
# its own hardening back).
differential_case 'line trim: Include payload<CR> (CRLF line ending)' \
  "$MATCH_OFF_LOOPBACK" "Include ${include_payload}${CARRIAGE_RETURN}"
differential_case 'line trim: Include payload<FF>' \
  "$MATCH_OFF_LOOPBACK" "Include ${include_payload}${FORM_FEED}"
differential_case 'line trim: hardened restatement PasswordAuthentication no<CR>' \
  "$MATCH_OFF_LOOPBACK" "PasswordAuthentication no${CARRIAGE_RETURN}"
differential_case 'line trim: PasswordAuthentication yes<CR>' \
  "$MATCH_OFF_LOOPBACK" "PasswordAuthentication yes${CARRIAGE_RETURN}"
differential_case 'line trim boundary: Include payload<VT>, NOT trimmed by sshd' \
  "$MATCH_OFF_LOOPBACK" "Include ${include_payload}${VERTICAL_TAB}"

# --- accepted but INERT: sshd discards only ONE empty token -------------------
# Each of these leaves the keyword still empty after the single retry, so sshd
# ignores the line. They are here because the day a future OpenSSH starts
# honouring one, the resolution above turns unsafe and the assertion fires on
# its own -- no reader has to notice.
differential_case 'inert: ==PasswordAuthentication yes' \
  "$MATCH_OFF_LOOPBACK" '==PasswordAuthentication yes'
differential_case 'inert: = =PasswordAuthentication yes' \
  "$MATCH_OFF_LOOPBACK" '= =PasswordAuthentication yes'
differential_case 'inert: ""=PasswordAuthentication yes' \
  "$MATCH_OFF_LOOPBACK" '""=PasswordAuthentication yes'
differential_case 'inert: = ""PasswordAuthentication yes' \
  "$MATCH_OFF_LOOPBACK" '= ""PasswordAuthentication yes'
differential_case 'inert: "" "" PasswordAuthentication yes' \
  "$MATCH_OFF_LOOPBACK" '"" "" PasswordAuthentication yes'
differential_case 'inert: =#PasswordAuthentication yes (comment after discard)' \
  "$MATCH_OFF_LOOPBACK" '=#PasswordAuthentication yes'
differential_case 'inert: unterminated quote opening the keyword' \
  "$MATCH_OFF_LOOPBACK" '"PasswordAuthentication yes'

# --- forms believed to be REJECTED -------------------------------------------
# The tokenizer comment leaves these to check_global. They are asserted here so
# that an OpenSSH which starts ACCEPTING one fails this test rather than
# silently opening a hole: the moment sshd accepts and resolves one unsafe, the
# unsafe branch above takes over and demands that --verify refuse it.
differential_case 'rejected form: vertical tab inside the keyword' \
  "$MATCH_OFF_LOOPBACK" "PasswordAuthentication${VERTICAL_TAB}yes"
differential_case 'rejected form: form feed inside the keyword' \
  "$MATCH_OFF_LOOPBACK" "PasswordAuthentication${FORM_FEED}yes"
differential_case 'rejected form: a second = separator' \
  "$MATCH_OFF_LOOPBACK" 'PasswordAuthentication == yes'
differential_case 'rejected form: = attached to the value' \
  "$MATCH_OFF_LOOPBACK" 'PasswordAuthentication==yes'
differential_case 'rejected form: the whole line quoted' \
  "$MATCH_OFF_LOOPBACK" '"PasswordAuthentication yes"'
differential_case 'rejected form: single-quoted segment inside the Match keyword' \
  "Ma'tch' Address *,!127.0.0.1" 'PasswordAuthentication yes'
differential_case 'rejected form: quote-ended keyword followed by =value' \
  "$MATCH_OFF_LOOPBACK" '"PasswordAuthentication"=yes'
differential_case 'rejected form: quoted segment mid-keyword, Pass"word"Auth' \
  "$MATCH_OFF_LOOPBACK" 'Pass"word"Authentication yes'
differential_case 'rejected form: unterminated quote in the value' \
  "$MATCH_OFF_LOOPBACK" 'PasswordAuthentication "yes'
differential_case 'rejected form: an extra argument at end of line' \
  "$MATCH_OFF_LOOPBACK" 'PasswordAuthentication yes extra'
differential_case 'rejected form: # attached to the value' \
  "$MATCH_OFF_LOOPBACK" 'PasswordAuthentication yes#note'
differential_case 'rejected form: empty first argument' \
  "$MATCH_OFF_LOOPBACK" 'PasswordAuthentication "" yes'

# The hostile file is removed by every case, so one final check is enough to
# prove no case leaked state into the tree: the clean tree must verify clean
# again. Doing it after every case would double the number of --verify runs to
# re-prove a property whose only writer is the line above.
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "--verify must PASS again on the clean tree once the corpus is done; state leaked (stderr: $SSH_RUN_ERR)"

printf '\ncorpus outcomes: %d accepted-and-unsafe, %d accepted-and-inert, %d rejected by sshd\n' \
  "$unsafe_count" "$safe_count" "$rejected_count"

# Vacuity guards. A corpus whose forms sshd all rejects, or whose control stops
# being a bypass, measures nothing while still reporting green.
[[ $control_outcome == 'unsafe' ]] ||
  fail "the control form (plain 'PasswordAuthentication yes' inside a Match block) came back '$control_outcome', not 'unsafe'; the fixture no longer measures anything"
# The floor is TODAY'S exact accepted-and-unsafe count, not a round number a
# shrunken corpus could still clear: at 20 a change that silently halved the
# corpus would have passed. Adding forms raises the count and should raise the
# floor with it; a count BELOW the floor means forms stopped being exercised
# (or this OpenSSH stopped resolving one unsafe), and either way a human looks.
[[ $unsafe_count -ge 52 ]] ||
  fail "only $unsafe_count corpus forms resolved unsafe, below the pinned floor of 52; forms have dropped out of the corpus or the parser changed, and neither may pass silently"

status=0
if [[ ${#bypass_forms[@]} -gt 0 ]]; then
  printf '\nFAIL: %d form(s) sshd accepts and resolves UNSAFE that --verify let through:\n' \
    "${#bypass_forms[@]}" >&2
  printf '  - %s\n' "${bypass_forms[@]}" >&2
  status=1
fi
# Reported separately and after the bypasses, because a false alarm is a
# different defect: it blocks an install rather than certifying a hole. It is
# still a disagreement with the real parser, which is the class this suite
# exists to catch, so it still fails.
if [[ ${#false_alarm_forms[@]} -gt 0 ]]; then
  printf '\nFAIL: %d form(s) sshd accepts and resolves HARDENED that --verify refused:\n' \
    "${#false_alarm_forms[@]}" >&2
  printf '  - %s\n' "${false_alarm_forms[@]}" >&2
  status=1
fi
if [[ $status -ne 0 ]]; then
  exit "$status"
fi

printf 'ssh-hardening-tokenizer-differential: OK (every form the real sshd accepts and resolves unsafe is refused by --verify; every tree sshd rejects fails it too)\n'
