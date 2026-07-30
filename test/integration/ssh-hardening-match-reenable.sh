#!/usr/bin/env bash
# ssh-hardening-match-reenable.sh -- --verify catches every Match-shaped
# bypass (slice 7, acceptance criterion 5). `sshd -G` without a connection
# spec dumps only the pre-Match configuration, so a Match block re-enabling a
# protected directive passes it while the machine is wide open; that is the
# most idiomatic sshd bypass and each form below must fail the verify, loudly
# and nonzero.
#
# EVERY case proves itself against real sshd first: the test resolves the
# hostile tree with its own `sshd -G -T -C` and requires the unsafe value to
# come back before it asks whether --verify catches it. A fixture that sshd
# quietly ignores would pin nothing, and three of the forms below were
# originally believed to be non-bypasses until the resolution was run.
#
# The cases, each on an otherwise fully hardened tree:
#   a. Match Address 0.0.0.0/0 re-enable        (raw scan names the file)
#   b. Match User * re-enable                   (raw scan names the file)
#   c. Match all re-enable, PermitRootLogin     (raw scan names the file)
#   d. Match User <invoking user> re-enable     (the CONNECTION-SPEC check
#      must catch it: asserted on the connection check's own attribution)
#   e. sibling that sorts FIRST (00-evil.conf) globally re-enabling UsePAM
#      and PubkeyAuthentication (the GLOBAL check catches first-value-wins)
#   f. Match Address on a subnet NO connection sample hits (only the raw scan
#      can catch this one)
#   g. '=' separator form: Match=all with PasswordAuthentication=yes (sshd
#      parses it, verified on OpenSSH 10.0p2, so the scan must too)
#   h. ChallengeResponseAuthentication alias re-enable inside Match (sshd
#      still honors the deprecated alias, verified on OpenSSH 10.0p2)
#   i. a QUOTED KEYWORD: `"PasswordAuthentication" yes`. sshd strips the
#      quotes around the keyword just as it does around a value, so this
#      re-enables password authentication while a scanner that only strips
#      quotes from the VALUE never recognizes the keyword at all.
#   j. a QUOTED VALUE: `PasswordAuthentication "yes"`. The mirror of i, and
#      the case that makes the value-side quote stripping non-removable.
#   k. a CARRIAGE RETURN inside the Match line: `Match<CR>Address ...`. CR is
#      whitespace to sshd, so the line opens a Match block, but a scanner
#      that only knows spaces and tabs never sees a Match at all and treats
#      every directive under it as global.
#   r-w. MIXED-QUOTE keywords: `Ma"tch"`, `Pubkey"Authentication"`,
#      `Inc"lude"`, and friends. sshd CONCATENATES an unquoted prefix with
#      one double-quoted segment and ends the keyword at the closing quote
#      (measured; the previous scanner believed a mid-keyword quote was
#      rejected, and that belief certified a tree with no usable off-loopback
#      authentication method at all). Each protected directive that sshd
#      allows inside a Match block gets its own mixed-quote case; UsePAM is
#      global-only in sshd, so its mixed-quote case is asserted through the
#      global check instead.
#
# Cases i, j and k assert on the SCAN's own attribution string, not merely on
# the file name, so each pins the scan rather than any other layer that
# happens to also fire.
#
# Between cases the hostile file is removed and --verify must PASS again, so
# a pass after a fail proves no state leaks and every failure is the hostile
# file's own doing. The tree starts clean and verifies clean (positive
# control), so "fails" is asserted on the right dimension.
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite can still inherit one from its caller.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

# shellcheck source=../fixtures/ssh-hardening-lib.bash
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../fixtures" && pwd)/ssh-hardening-lib.bash"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -x /usr/sbin/sshd ]] || {
  printf 'SKIP: /usr/sbin/sshd not present; cannot resolve effective configuration\n'
  exit 0
}

ssh_sandbox_setup
trap 'ssh_sandbox_teardown' EXIT

# Harden the sandbox tree through the script's own install, then the positive
# control: a clean tree must PASS.
run_ssh_hardening
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "install on a clean sandbox must succeed (stderr: $SSH_RUN_ERR)"
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "positive control: --verify must PASS on the clean hardened tree (stderr: $SSH_RUN_ERR)"

hostile_file="$SSHD_CONFIG_D/500-hostile.conf"

# An address in TEST-NET-2 (RFC 5737): routable-looking, reserved for
# documentation, and deliberately NOT one of the loopback samples --verify
# probes, so a case scoped away from loopback is genuinely off-sample.
off_sample_spec='user=offsample,host=elsewhere.example,addr=198.51.100.23'

# run_reenable_case <label> <needle> <spec> <key> <unsafe-value> <line>...
# Writes the hostile file, PROVES with the test's own sshd that <key> really
# resolves to <unsafe-value> for <spec>, requires --verify to fail naming
# <needle>, then removes it and requires --verify to pass again.
run_reenable_case() {
  local label="$1" needle="$2" spec="$3" key="$4" unsafe="$5" resolved
  shift 5
  printf '%s\n' "$@" >"$hostile_file"

  resolved="$(effective_spec_value "$spec" "$key")" ||
    fail "$label: the test's own sshd could not resolve '$key' for '$spec'"
  [[ $resolved == "$unsafe" ]] ||
    fail "$label: real sshd must resolve '$key' as '$unsafe' for '$spec', got '$resolved'; this fixture is not a bypass and would pin nothing"

  run_ssh_hardening --verify
  [[ $SSH_RUN_STATUS -ne 0 ]] ||
    fail "$label: --verify must exit nonzero (stdout: $SSH_RUN_OUT)"
  grep -qF -- "$needle" <<<"$SSH_RUN_ERR" ||
    fail "$label: stderr must contain '$needle' (stderr: $SSH_RUN_ERR)"
  rm -f "$hostile_file"
  run_ssh_hardening --verify
  [[ $SSH_RUN_STATUS -eq 0 ]] ||
    fail "$label: --verify must PASS again once the hostile file is removed; state leaked (stderr: $SSH_RUN_ERR)"
}

# a. The idiomatic bypass. The raw scan must name the offending file.
run_reenable_case 'a: Match Address 0.0.0.0/0' "$hostile_file" \
  "$off_sample_spec" passwordauthentication yes \
  'Match Address 0.0.0.0/0' 'PasswordAuthentication yes'

# b. Match User * catches every user; scan names the file.
run_reenable_case 'b: Match User *' "$hostile_file" \
  "$off_sample_spec" kbdinteractiveauthentication yes \
  'Match User *' 'KbdInteractiveAuthentication yes'

# c. Match all; also pins permitrootlogin as a protected directive.
run_reenable_case 'c: Match all PermitRootLogin' "$hostile_file" \
  "$off_sample_spec" permitrootlogin yes \
  'Match all' 'PermitRootLogin yes'

# d. Match User <invoking user>: the connection-spec resolution must catch it.
# The raw scan fires too; the assertion pins the CONNECTION check's own
# attribution so removing that check fails here even with the scan intact.
invoking_user="$(id -un)"
run_reenable_case 'd: Match User (invoking user)' \
  "connection check (user=$invoking_user" \
  "user=$invoking_user,host=localhost,addr=127.0.0.1" passwordauthentication yes \
  "Match User $invoking_user" 'PasswordAuthentication yes'

# e. A sibling sorting BEFORE 000- with GLOBAL re-enables: first-value-wins
# hands these two directives to the sibling, and only the global check sees
# it (no Match block anywhere). Pins usepam and pubkeyauthentication.
sibling="$SSHD_CONFIG_D/00-evil.conf"
printf 'UsePAM no\nPubkeyAuthentication no\n' >"$sibling"
for pair in 'usepam no' 'pubkeyauthentication no'; do
  key="${pair%% *}"
  want="${pair##* }"
  got="$(effective_global_value "$key")" || fail 'e: sshd -G failed'
  [[ $got == "$want" ]] ||
    fail "e: the first-sorting sibling must win '$key' globally (got '$got'); the fixture is not a bypass"
done
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 'e: a first-sorting sibling with global re-enables must fail --verify'
grep -qF 'usepam' <<<"$SSH_RUN_ERR" ||
  fail "e: stderr must name usepam (stderr: $SSH_RUN_ERR)"
grep -qF 'pubkeyauthentication' <<<"$SSH_RUN_ERR" ||
  fail "e: stderr must name pubkeyauthentication (stderr: $SSH_RUN_ERR)"
rm -f "$sibling"
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail 'e: --verify must PASS again once the sibling is removed'

# f. A subnet neither connection sample hits: the raw scan is the ONLY net.
run_reenable_case 'f: Match Address off-sample subnet' "$hostile_file" \
  "$off_sample_spec" permitrootlogin yes \
  'Match Address 198.51.100.0/24' 'PermitRootLogin yes'

# g. The '=' separator form sshd accepts must not slip past the scan.
run_reenable_case "g: '=' separator form" "$hostile_file" \
  "$off_sample_spec" passwordauthentication yes \
  'Match=all' 'PasswordAuthentication=yes'

# h. The deprecated alias still flips kbdinteractiveauthentication in sshd.
run_reenable_case 'h: ChallengeResponseAuthentication alias' "$hostile_file" \
  "$off_sample_spec" kbdinteractiveauthentication yes \
  'Match Address 198.51.100.0/24' 'ChallengeResponseAuthentication yes'

# i. A quoted KEYWORD. sshd strips the quotes and honours the directive; a
# scanner that carries the quotes into its keyword comparison never matches.
# Asserted on the scan's own attribution, so it pins the scan.
run_reenable_case 'i: quoted keyword' \
  "match scan: '$hostile_file' sets 'passwordauthentication yes'" \
  "$off_sample_spec" passwordauthentication yes \
  'Match Address *,!127.0.0.1' '"PasswordAuthentication" yes'

# j. A quoted VALUE, the mirror of i: without value-side quote stripping the
# scan reads the value as '"yes"' and compares it against 'no' -- which does
# fail, but for the wrong reason. Pinned on the attribution, which names the
# STRIPPED value, so a scan that stops stripping quotes fails this case.
run_reenable_case 'j: quoted value' \
  "match scan: '$hostile_file' sets 'passwordauthentication yes'" \
  "$off_sample_spec" passwordauthentication yes \
  'Match Address *,!127.0.0.1' 'PasswordAuthentication "yes"'

# k. A carriage return between Match and its criteria. CR is whitespace to
# sshd (misc.c WHITESPACE is " \t\r\n"), so the block opens normally; a
# scanner that splits on spaces and tabs only sees one token, never matches
# 'match', and reads every directive below it as global configuration.
run_reenable_case 'k: carriage return inside the Match line' \
  "match scan: '$hostile_file' sets 'passwordauthentication yes'" \
  "$off_sample_spec" passwordauthentication yes \
  "Match${SSH_CARRIAGE_RETURN}Address *,!127.0.0.1" 'PasswordAuthentication yes'

# l. SkeyAuthentication: an alias sshd_config(5) does not document at all,
# still wired to kbdinteractiveauthentication in the parser. The needle names
# the CANONICAL directive, so a scan that does not fold the alias cannot
# produce it.
run_reenable_case 'l: SkeyAuthentication alias' \
  "match scan: '$hostile_file' sets 'kbdinteractiveauthentication yes'" \
  "$off_sample_spec" kbdinteractiveauthentication yes \
  'Match Address *,!127.0.0.1' 'SkeyAuthentication yes'

# m. DSAAuthentication, the third alias, aimed at pubkeyauthentication. Unlike
# the other two it is GLOBAL-only: put it under `Match Address ...` and sshd
# refuses the whole configuration ("Directive 'DSAAuthentication' is not
# allowed within a Match block"). Under `Match all` sshd accepts it and
# applies it globally, which is the form used here. The needle is the SCAN's
# attribution naming pubkeyauthentication, so the case pins the alias fold
# even though the global check independently sees the same value move.
run_reenable_case 'm: DSAAuthentication alias' \
  "match scan: '$hostile_file' sets 'pubkeyauthentication no'" \
  "$off_sample_spec" pubkeyauthentication no \
  'Match all' 'DSAAuthentication no'

# n and o. The two userauth methods the password-and-key directives do not
# reach. `strings /usr/sbin/sshd` lists six methods this binary offers -- none,
# password, keyboard-interactive, publickey, gssapi-with-mic, hostbased -- and
# the first four are settled by PasswordAuthentication,
# KbdInteractiveAuthentication and PubkeyAuthentication. gssapi-with-mic and
# hostbased are settled by nothing, and both default to no. A default is not a
# policy: both are settable inside a Match block, which is exactly what these
# two cases do.
run_reenable_case 'n: GSSAPIAuthentication inside a Match block' \
  "match scan: '$hostile_file' sets 'gssapiauthentication yes'" \
  "$off_sample_spec" gssapiauthentication yes \
  'Match Address *,!127.0.0.1' 'GSSAPIAuthentication yes'

run_reenable_case 'o: HostbasedAuthentication inside a Match block' \
  "match scan: '$hostile_file' sets 'hostbasedauthentication yes'" \
  "$off_sample_spec" hostbasedauthentication yes \
  'Match Address *,!127.0.0.1' 'HostbasedAuthentication yes'

# p. The ROOT connection sample, pinned the way case d pins the invoking-user
# sample. Without this, deleting the root spec from the connection check broke
# nothing: every other case is caught by the scan or by the invoking-user
# sample. root is the account PermitRootLogin exists to keep out, so it is the
# one sample that must not quietly disappear.
run_reenable_case 'p: Match User root' \
  'connection check (user=root' \
  'user=root,host=localhost,addr=127.0.0.1' passwordauthentication yes \
  'Match User root' 'PasswordAuthentication yes'

# q. The re-enable in the MAIN CONFIG rather than a drop-in. Every other
# hostile fixture in this file writes into the drop-in directory, so dropping
# $SSHD_MAIN_CONFIG from the scan roots broke nothing at all -- and the main
# config is the one file sshd is guaranteed to read.
main_config_backup="$SSH_SANDBOX/sshd_config.backup"
cp "$SSHD_MAIN_CONFIG" "$main_config_backup"
printf 'Match Address 198.51.100.0/24\nPasswordAuthentication yes\n' >>"$SSHD_MAIN_CONFIG"

resolved="$(effective_spec_value "$off_sample_spec" passwordauthentication)" ||
  fail 'q: the test sshd could not resolve the main-config fixture'
[[ $resolved == 'yes' ]] ||
  fail "q: real sshd must resolve passwordauthentication yes off-loopback, got '$resolved'"
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail "q: a Match re-enable in the MAIN config must fail --verify (stdout: $SSH_RUN_OUT)"
grep -qF -- "match scan: '$SSHD_MAIN_CONFIG' sets 'passwordauthentication yes'" <<<"$SSH_RUN_ERR" ||
  fail "q: the scan must name the main config itself (stderr: $SSH_RUN_ERR)"
cp "$main_config_backup" "$SSHD_MAIN_CONFIG"
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "q: --verify must PASS again once the main config is restored (stderr: $SSH_RUN_ERR)"

# r. Match ITSELF with a mid-keyword quote. sshd reads `Ma"tch"` as `Match`
# (unquoted prefix + quoted segment concatenate, measured on OpenSSH 10.0p2),
# so the block opens and the directive under it applies off-loopback. A
# scanner that treats '"' as a token terminator reads `Ma`, never enters
# Match state, and certifies the tree.
run_reenable_case 'r: mid-quote Match keyword' \
  "match scan: '$hostile_file' sets 'passwordauthentication yes'" \
  "$off_sample_spec" passwordauthentication yes \
  'Ma"tch" Address *,!127.0.0.1' 'PasswordAuthentication yes'

# s. Every protected directive sshd allows inside a Match block, each with a
# mid-keyword quote closing at the keyword's end (the only mixed-quote shape
# sshd accepts: the token ENDS at the closing quote, so `Pass"word"Auth...`
# is the unknown keyword `Password` and is rejected, while `Password"Authentication"`
# is the full keyword). UsePAM is absent here because sshd refuses it inside
# any Match block; its mixed-quote form is case t below.
run_reenable_case 's: mid-quote PasswordAuthentication' \
  "match scan: '$hostile_file' sets 'passwordauthentication yes'" \
  "$off_sample_spec" passwordauthentication yes \
  'Match Address *,!127.0.0.1' 'Password"Authentication" yes'
run_reenable_case 's: mid-quote KbdInteractiveAuthentication' \
  "match scan: '$hostile_file' sets 'kbdinteractiveauthentication yes'" \
  "$off_sample_spec" kbdinteractiveauthentication yes \
  'Match Address *,!127.0.0.1' 'KbdInteractive"Authentication" yes'
run_reenable_case 's: mid-quote PubkeyAuthentication' \
  "match scan: '$hostile_file' sets 'pubkeyauthentication no'" \
  "$off_sample_spec" pubkeyauthentication no \
  'Match Address *,!127.0.0.1' 'Pubkey"Authentication" no'
run_reenable_case 's: mid-quote PermitRootLogin' \
  "match scan: '$hostile_file' sets 'permitrootlogin yes'" \
  "$off_sample_spec" permitrootlogin yes \
  'Match Address *,!127.0.0.1' 'PermitRoot"Login" yes'
run_reenable_case 's: mid-quote GSSAPIAuthentication' \
  "match scan: '$hostile_file' sets 'gssapiauthentication yes'" \
  "$off_sample_spec" gssapiauthentication yes \
  'Match Address *,!127.0.0.1' 'GSSAPI"Authentication" yes'
run_reenable_case 's: mid-quote HostbasedAuthentication' \
  "match scan: '$hostile_file' sets 'hostbasedauthentication yes'" \
  "$off_sample_spec" hostbasedauthentication yes \
  'Match Address *,!127.0.0.1' 'Hostbased"Authentication" yes'

# t. UsePAM with a mid-keyword quote, GLOBAL (sshd refuses it in Match
# blocks). Real sshd resolves `Use"PAM" no` as usepam no, so the global
# check must flag it; this pins the mixed-quote form on the one protected
# directive the scan can never see inside a Match block.
printf '%s\n' 'Use"PAM" no' >"$SSHD_CONFIG_D/00-quoted-usepam.conf"
got="$(effective_global_value usepam)" || fail 't: sshd -G failed on the quoted-UsePAM fixture'
[[ $got == no ]] ||
  fail "t: real sshd must resolve 'Use\"PAM\" no' as usepam no globally, got '$got'; the fixture proves nothing"
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -ne 0 ]] ||
  fail 't: a mixed-quote global UsePAM re-enable must fail --verify'
grep -qF 'usepam' <<<"$SSH_RUN_ERR" ||
  fail "t: stderr must name usepam (stderr: $SSH_RUN_ERR)"
rm -f "$SSHD_CONFIG_D/00-quoted-usepam.conf"
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail 't: --verify must PASS again once the quoted-UsePAM fixture is removed'

# u. An ALIAS with a mid-keyword quote: sshd reads `Skey"Authentication"` as
# the undocumented SkeyAuthentication alias and flips
# kbdinteractiveauthentication (measured). The needle names the CANONICAL
# directive, so this pins the alias fold THROUGH the mixed-quote tokenizer.
run_reenable_case 'u: mid-quote SkeyAuthentication alias' \
  "match scan: '$hostile_file' sets 'kbdinteractiveauthentication yes'" \
  "$off_sample_spec" kbdinteractiveauthentication yes \
  'Match Address *,!127.0.0.1' 'Skey"Authentication" yes'

# v and w. Include through the mixed-quote tokenizer, with the hostile file
# OUTSIDE both scan roots so ONLY following the Include can reach it: v puts
# the quote in the Include keyword itself, w puts it inside the path argument
# (argument tokens concatenate any number of quoted segments, measured).
outside_dir="$SSH_SANDBOX/outside"
mkdir -p "$outside_dir"
printf 'Match Address *,!127.0.0.1\nPasswordAuthentication yes\n' >"$outside_dir/evil.conf"
run_reenable_case 'v: mid-quote Include keyword' \
  "match scan: '$outside_dir/evil.conf' sets 'passwordauthentication yes'" \
  "$off_sample_spec" passwordauthentication yes \
  "Inc\"lude\" $outside_dir/evil.conf"
run_reenable_case 'w: quoted fragment inside the Include path' \
  "match scan: '$outside_dir/evil.conf' sets 'passwordauthentication yes'" \
  "$off_sample_spec" passwordauthentication yes \
  "Include $SSH_SANDBOX/out\"side\"/evil.conf"

printf 'ssh-hardening-match-reenable: OK (every Match bypass form resolves unsafe under real sshd and fails --verify loudly; clean tree passes before and after every case)\n'
