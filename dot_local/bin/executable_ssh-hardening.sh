#!/bin/bash
# ssh-hardening.sh -- lock sshd to public-key authentication via a drop-in,
# closing the PAM password channel and denying root login.
#
# The drop-in file IS the lock: leave it in place permanently. Without it, sshd
# reverts to its default of allowing password auth.
#
# The drop-in is named 000-ssh-hardening.conf so it sorts FIRST under sshd's
# `Include /etc/ssh/sshd_config.d/*`. That Include is lexical and FIRST-VALUE-WINS,
# so a file that sorts before ours (e.g. Apple's 100-macos.conf, or any future
# sibling) would SHADOW the hardening. Sorting first (000- before any 0*/1* file)
# guarantees our values win. The old name (50-no-password-auth.conf) sorted AFTER
# 100-macos.conf and was defeated; install migrates it away.
#
# Modes:
#   (default)         write the drop-in if missing or stale (idempotent; needs
#                     sudo), migrate the old 50- name, then VERIFY the full
#                     effective config is hardened. Does NOT reload sshd (--reload).
#   --print-config    print the drop-in content to stdout and exit. No sudo, no
#                     writes, no sshd: the pure inspection and test seam.
#   --print-path      print the managed drop-in's absolute path and exit.
#   --verify          parse the FULL effective sshd config (main config + every
#                     drop-in) with `sshd -G` and assert all five hardening values
#                     are in force. Read-only: no reload, no writes. Nonzero if the
#                     hardening is shadowed by a sibling drop-in.
#   --reload          reload sshd so a running daemon picks up the drop-in. This
#                     is the disruptive, operator-controlled step: it validates the
#                     complete config first and fails closed, but a reload can drop
#                     the current SSH session, so run it deliberately from a local
#                     console (or with a second session open) and prove key auth
#                     works in a NEW session before closing the old one.
#
# On a fresh Mac the drop-in applies the moment Remote Login is first enabled
# (sshd starts and reads it), so no reload is needed there.
set -euo pipefail

# Overridable for tests, which target a sandbox tree and drop the sudo prefix.
SSHD_CONFIG_D="${SSHD_CONFIG_D:-/etc/ssh/sshd_config.d}"
DROPIN="$SSHD_CONFIG_D/000-ssh-hardening.conf"
# The superseded name (sorted AFTER 100-macos.conf, so it was shadowed). Migrated
# away on install.
LEGACY_DROPIN="$SSHD_CONFIG_D/50-no-password-auth.conf"

SSHD_BIN="${SSHD_BIN:-sshd}"
LAUNCHCTL_BIN="${LAUNCHCTL_BIN:-launchctl}"
SSHD_MAIN_CONFIG="${SSHD_MAIN_CONFIG:-/etc/ssh/sshd_config}"
# Privilege prefix for live-system reads/writes/reloads. Default sudo; tests set
# SSH_HARDENING_SUDO="" to operate unprivileged against a sandbox tree.
SUDO="${SSH_HARDENING_SUDO-sudo}"

# The accepted effective values (the hardening ruling), exactly as `sshd -G` /
# `sshd -T` print them (lowercased key, single space).
ACCEPTED_EFFECTIVE=(
  'passwordauthentication no'
  'kbdinteractiveauthentication no'
  'usepam yes'
  'pubkeyauthentication yes'
  'permitrootlogin no'
)

# The four auth directives that MAY appear inside a `Match` block, each with its
# hardened value. A Match block that sets any of these to a different value re-enables
# it for the matching connections (the criteria-based bypass `sshd -G` alone is blind
# to). UsePAM is intentionally absent: it is not a Match-scoped keyword. Keys are
# lowercased (sshd keywords are case-insensitive).
declare -A PROTECTED_MATCH_HARDENED=(
  [passwordauthentication]=no
  [kbdinteractiveauthentication]=no
  [pubkeyauthentication]=yes
  [permitrootlogin]=no
)

# Representative connection specs for the authoritative per-connection resolution
# (`sshd -G -T -C`). A privileged (root) login and an ordinary user, from a sample
# address/host, cover the wildcard/address/root Match criteria; the raw file scan
# below covers a Match keyed to a specific non-sampled user.
VERIFY_SPECS=(
  'user=root,addr=203.0.113.1,host=localhost'
  'user=nobody,addr=203.0.113.1,host=localhost'
)

# priv <cmd...> -- run a command with the configured privilege prefix (sudo by
# default; nothing when SSH_HARDENING_SUDO is empty, e.g. under test).
priv() {
  if [[ -n $SUDO ]]; then
    "$SUDO" "$@"
  else
    "$@"
  fi
}

# render_dropin -- print the desired drop-in content. Pure: no sudo, no writes.
# PasswordAuthentication no + KbdInteractiveAuthentication no together close BOTH
# interactive-password channels. UsePAM yes is required on macOS for account and
# session management, and is safe here precisely because neither password channel
# is open, so PAM has no password path to authenticate.
render_dropin() {
  cat <<'EOF'
# Managed by ssh-hardening.sh: lock sshd to public-key authentication only.
PasswordAuthentication no
KbdInteractiveAuthentication no
UsePAM yes
PubkeyAuthentication yes
PermitRootLogin no
EOF
}

# assert_effective_config <effective-config-text> [context-label] -- return 0 iff ALL
# five accepted values are present. On any mismatch, print a loud per-key WARNING to
# stderr (tagged with the context label so the operator knows WHICH view failed) and
# return 1. Pure: no sshd, no sudo, no writes -- the unit seam.
assert_effective_config() {
  local effective="$1" ctx="${2:-effective sshd config}" rc=0 pair key actual
  for pair in "${ACCEPTED_EFFECTIVE[@]}"; do
    if ! grep -qxiF "$pair" <<<"$effective"; then
      key="${pair%% *}"
      actual="$(grep -iE "^${key} " <<<"$effective" | head -n1 || true)"
      printf '[ssh-hardening] WARNING: %s has "%s" but hardening requires "%s"\n' \
        "$ctx" "${actual:-(no ${key} line)}" "$pair" >&2
      rc=1
    fi
  done
  return "$rc"
}

# enumerate_config_files <main-config> -- print the main config path, then every file
# it Includes, expanded in sshd's own lexical order. Handles absolute and
# base-relative Include globs (one level -- the drop-in dir; the authoritative
# `sshd -G -T -C` check below is the catch-all for any deeper structure). Reads under
# the privilege prefix so a root-only main config is still enumerable.
enumerate_config_files() {
  local main="$1"
  printf '%s\n' "$main"
  local base
  base="$(dirname "$main")"
  local line first rest pat abspat match
  while IFS= read -r line; do
    line="${line#"${line%%[![:space:]]*}"}"
    [[ -z $line || ${line:0:1} == '#' ]] && continue
    read -r first rest <<<"$line"
    [[ "$(printf '%s' "$first" | tr '[:upper:]' '[:lower:]')" == include ]] || continue
    local -a pats=()
    read -r -a pats <<<"$rest"
    for pat in "${pats[@]}"; do
      [[ $pat == /* ]] && abspat="$pat" || abspat="$base/$pat"
      while IFS= read -r match; do
        printf '%s\n' "$match"
      done < <(compgen -G "$abspat" 2>/dev/null | LC_ALL=C sort)
    done
  done < <(priv cat "$main" 2>/dev/null || true)
}

# scan_one_file_match <file> -- scan a single sshd config file for a `Match` block
# that sets a protected directive to a non-hardened value. Return 1 (with a loud
# per-hit WARNING naming the file and the Match line) if any is found. Pure text scan:
# host-key-free, catches even a Match keyed to a specific user the -C sampling misses.
scan_one_file_match() {
  local file="$1" rc=0 in_match=0 match_ctx="" raw line kw val lc_kw lc_val hardened
  while IFS= read -r raw; do
    line="${raw#"${raw%%[![:space:]]*}"}"
    [[ -z $line || ${line:0:1} == '#' ]] && continue
    read -r kw val _rest <<<"$line"
    lc_kw="$(printf '%s' "$kw" | tr '[:upper:]' '[:lower:]')"
    if [[ $lc_kw == match ]]; then
      in_match=1
      match_ctx="$line"
      continue
    fi
    [[ $in_match -eq 1 ]] || continue
    hardened="${PROTECTED_MATCH_HARDENED[$lc_kw]:-}"
    [[ -n $hardened ]] || continue
    lc_val="$(printf '%s' "$val" | tr '[:upper:]' '[:lower:]')"
    if [[ $lc_val != "$hardened" ]]; then
      printf '[ssh-hardening] WARNING: %s re-enables a protected directive inside "%s": sets "%s %s" but hardening requires "%s %s" for ALL connections.\n' \
        "$file" "$match_ctx" "$kw" "$val" "$kw" "$hardened" >&2
      rc=1
    fi
  done < <(priv cat "$file" 2>/dev/null || true)
  return "$rc"
}

# scan_match_reenables -- scan the whole include tree (main + every included file) for
# a Match block that weakens a protected directive. Return 1 if any file does.
scan_match_reenables() {
  local rc=0 file
  while IFS= read -r file; do
    scan_one_file_match "$file" || rc=1
  done < <(enumerate_config_files "$SSHD_MAIN_CONFIG")
  return "$rc"
}

# verify_effective_specs -- authoritative per-connection resolution. For each
# representative spec, `sshd -G -T -C <spec>` resolves ALL Match blocks (host-key-free,
# root-free) and prints the effective config for that connection; assert the five
# values hold. Return 1 if any spec diverges or cannot be resolved.
verify_effective_specs() {
  local rc=0 spec eff
  for spec in "${VERIFY_SPECS[@]}"; do
    if ! eff="$(priv "$SSHD_BIN" -G -T -C "$spec" -f "$SSHD_MAIN_CONFIG" 2>/dev/null)"; then
      printf '[ssh-hardening] WARNING: could not resolve the effective config for connection spec %s (%s -G -T -C failed); NOT claiming hardened.\n' \
        "$spec" "$SSHD_BIN" >&2
      rc=1
      continue
    fi
    assert_effective_config "$eff" "connection spec [$spec]" || rc=1
  done
  return "$rc"
}

# verify_effective -- assert the hardening holds THREE independent ways, all
# read-only and host-key-free (never reload, never bind):
#   1. the global (pre-Match) effective config via `sshd -G`;
#   2. a raw scan of every included file for a Match block that re-enables a protected
#      directive (the criteria-based bypass `sshd -G` alone is blind to -- it dumps
#      only the pre-Match config);
#   3. an authoritative per-connection resolution via `sshd -G -T -C` for a root spec
#      and a normal-user spec (this DOES resolve Match blocks, without host keys).
# The parse runs under the privilege prefix because a sibling drop-in in the include
# tree may be root-only; running privileged lets the parse read the COMPLETE tree so a
# hostile root-only sibling cannot hide from the check. Each view is reported
# separately -- no blanket "all in force" claim unless every view passes.
verify_effective() {
  if ! command -v "$SSHD_BIN" >/dev/null 2>&1; then
    printf '[ssh-hardening] NOTE: %s not found; cannot verify the effective config here. On macOS verify with: sudo sshd -G -T -C user=root,addr=203.0.113.1,host=localhost | grep -iE "passwordauthentication|kbdinteractiveauthentication|usepam|pubkeyauthentication|permitrootlogin"\n' \
      "$SSHD_BIN" >&2
    return 0
  fi
  local rc=0 global_eff
  # 1. Global (pre-Match) effective config.
  if ! global_eff="$(priv "$SSHD_BIN" -G -f "$SSHD_MAIN_CONFIG" 2>/dev/null)"; then
    printf '[ssh-hardening] WARNING: could not parse the global effective sshd config (%s -G -f %s failed); NOT claiming this host is hardened.\n' \
      "$SSHD_BIN" "$SSHD_MAIN_CONFIG" >&2
    return 1
  fi
  if assert_effective_config "$global_eff" "global effective config (pre-Match)"; then
    printf '[ssh-hardening] global effective config verified (sshd -G, pre-Match): all five values accepted\n'
  else
    rc=1
  fi
  # 2. Raw Match-block scan (catches criteria-based re-enables; names the file).
  if scan_match_reenables; then
    printf '[ssh-hardening] Match-block scan: no sibling Match re-enables a protected directive\n'
  else
    rc=1
  fi
  # 3. Authoritative per-connection resolution (sshd -G -T -C; resolves Match blocks).
  if verify_effective_specs; then
    printf '[ssh-hardening] per-connection check (sshd -G -T -C): hardening holds for root and a normal user\n'
  else
    rc=1
  fi
  if [[ $rc -eq 0 ]]; then
    printf '[ssh-hardening] verified: hardening holds globally, under the Match-block scan, and per connection spec\n'
  fi
  return "$rc"
}

# install_dropin -- write the drop-in when missing or stale (idempotent), migrate
# the superseded 50- name, then verify the full effective config is hardened.
install_dropin() {
  local desired current
  desired="$(render_dropin)"
  current=""
  [[ -f $DROPIN ]] && current="$(priv cat "$DROPIN" 2>/dev/null || true)"
  if [[ $current != "$desired" ]]; then
    render_dropin | priv tee "$DROPIN" >/dev/null
    printf '[ssh-hardening] wrote %s\n' "$DROPIN"
  else
    printf '[ssh-hardening] %s already current\n' "$DROPIN"
  fi

  # Migrate the superseded 50- drop-in: it sorted AFTER 100-macos.conf and so was
  # shadowed. Remove it in the same privileged step so no orphan/duplicate lingers.
  if [[ -e $LEGACY_DROPIN ]]; then
    priv rm -f "$LEGACY_DROPIN"
    printf '[ssh-hardening] removed superseded drop-in %s\n' "$LEGACY_DROPIN"
  fi

  # Defense in depth: the drop-in must WIN over every sibling (e.g. a future hostile
  # 100-macos.conf). Refuse to claim success if any of the five is not accepted.
  if ! verify_effective; then
    printf '[ssh-hardening] ERROR: the drop-in is in place but the effective sshd config is NOT fully hardened -- a sibling drop-in is overriding it. Resolve before relying on this host.\n' >&2
    return 1
  fi

  printf '[ssh-hardening] drop-in in place; run ssh-hardening.sh --reload (or re-enable Remote Login) to activate it on a running sshd\n'
}

# prime_sudo -- refresh the sudo timestamp visibly, failing closed if privilege
# escalation is unavailable. No-op when the sudo prefix is empty (test mode).
prime_sudo() {
  [[ -n $SUDO ]] || return 0
  "$SUDO" -v
}

# do_reload -- reload a running sshd via the modern kickstart -k idiom (kill +
# restart in one call). This TERMINATES the current listener, so it fails CLOSED:
# it validates the complete config first, distinguishes a confirmed-absent service
# from an errored probe, returns nonzero on any failure, and confirms sshd came
# back. It never treats a sudo/launchctl error as proof the daemon is down.
do_reload() {
  # (a) Prime privilege escalation visibly. A sudo failure must NOT be mistaken for
  #     "sshd is down".
  if ! prime_sudo; then
    printf '[ssh-hardening] ERROR: could not acquire sudo; refusing to reload sshd.\n' >&2
    return 1
  fi

  # (b) Validate the COMPLETE live config BEFORE the disruptive kickstart. A
  #     kickstart -k terminates the listener, so never restart onto a config that
  #     fails syntax or has lost the hardening (a broken sibling drop-in must not be
  #     allowed to drop the daemon). Syntax first, then the five effective values.
  if ! priv "$SSHD_BIN" -t; then
    printf '[ssh-hardening] ERROR: sshd config failed syntax validation (sshd -t); refusing to reload.\n' >&2
    return 1
  fi
  if ! verify_effective; then
    printf '[ssh-hardening] ERROR: the live effective config is not fully hardened; refusing to reload.\n' >&2
    return 1
  fi

  # (c)/(d) Probe the service, distinguishing CONFIRMED-absent from a probe ERROR.
  #     `launchctl print` exits 0 when the service is loaded and 113 ("Could not
  #     find service") when it is genuinely absent; ANY other nonzero is an errored
  #     probe (e.g. a sudo/launchctl failure) and is NOT proof the daemon is down --
  #     propagate it, never proceed as if stopped.
  local probe_rc=0
  priv "$LAUNCHCTL_BIN" print system/com.openssh.sshd >/dev/null 2>&1 || probe_rc=$?
  if [[ $probe_rc -eq 0 ]]; then
    : # loaded; fall through to the kickstart
  elif [[ $probe_rc -eq 113 ]]; then
    printf '[ssh-hardening] sshd is not loaded (Remote Login off); the drop-in applies when it is next enabled.\n'
    return 0
  else
    printf '[ssh-hardening] ERROR: could not determine sshd state (launchctl print failed rc=%d); NOT proceeding as if it were stopped.\n' "$probe_rc" >&2
    return 1
  fi

  # Disruptive step: kill + restart in one call.
  if ! priv "$LAUNCHCTL_BIN" kickstart -k system/com.openssh.sshd; then
    printf '[ssh-hardening] ERROR: launchctl kickstart failed; sshd may not have restarted. Check: sudo launchctl print system/com.openssh.sshd\n' >&2
    return 1
  fi

  # (e) Verify the service came back after the kickstart.
  if ! priv "$LAUNCHCTL_BIN" print system/com.openssh.sshd >/dev/null 2>&1; then
    printf '[ssh-hardening] ERROR: sshd did NOT come back after kickstart. Investigate immediately: sudo launchctl print system/com.openssh.sshd\n' >&2
    return 1
  fi
  printf '[ssh-hardening] sshd reloaded and confirmed running.\n'
}

case "${1:-}" in
  --print-config)
    render_dropin
    ;;
  --print-path)
    printf '%s\n' "$DROPIN"
    ;;
  --verify)
    verify_effective
    ;;
  --reload)
    do_reload
    ;;
  "")
    install_dropin
    ;;
  *)
    printf 'usage: ssh-hardening.sh [--print-config | --print-path | --verify | --reload]\n' >&2
    exit 2
    ;;
esac
