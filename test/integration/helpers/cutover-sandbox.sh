# shellcheck shell=bash
# cutover-sandbox.sh: sourced helper for the cutover gate-runner tests.
#
# Builds a sandbox $HOME containing a real git repository at the runner's
# absolute repo handle ($HOME/workspaces/Ivy/webdavis/dotfiles) with a local
# bare `origin` carrying `main` and `integration/modernization`, plus PATH stubs
# for every external boundary the runner touches (launchctl, chezmoi, just, gh,
# tailscale, hermes). Nothing here reads or writes live state.
#
# Sourced, never executed. Callers set the case-level env the stubs read.

# cutover_git: git with a fixed identity and no signing, so a sandbox commit
# never depends on the operator's config.
cutover_git() {
  git -c user.email=cutover@example.invalid -c user.name='Cutover Test' \
    -c commit.gpgsign=false "$@"
}

# cutover_build_sandbox <home> : sets SANDBOX_HOME, SANDBOX_REPO, SANDBOX_BASE
# (the commit the tests pass as the recorded Phase A base).
#
# The repository carries, on main: two tracked LaunchAgent plists (one
# KeepAlive=true, one StartInterval), a script-rendered system daemon, a
# deleted historical out-of-prefix label, a renamed-away label, and a committed
# scripts/live-reconcile.sh recording stub. The integration branch carries the
# combined delta (a.txt landed unchanged, b.txt improved on main, c.txt omitted).
cutover_build_sandbox() {
  SANDBOX_HOME="$1"
  SANDBOX_REPO="$SANDBOX_HOME/workspaces/Ivy/webdavis/dotfiles"
  local origin="$SANDBOX_HOME/origin.git"
  mkdir -p "$SANDBOX_REPO" "$SANDBOX_HOME/.hermes/profiles/concerned"
  printf 'enabled: true\n' >"$SANDBOX_HOME/.hermes/profiles/concerned/config.yaml"
  cutover_git init --quiet --bare --initial-branch=main "$origin"
  cutover_git init --quiet --initial-branch=main "$SANDBOX_REPO"
  cutover_git -C "$SANDBOX_REPO" remote add origin "$origin"

  printf 'seed\n' >"$SANDBOX_REPO/seed.txt"
  # Mirrors the real repository: the committed map is TRACKED and everything
  # else under graphify-out/ is ignored rebuild output. A pristine checkout
  # therefore always HAS a graphify-out directory.
  {
    printf 'graphify-out/*\n'
    printf '!graphify-out/graph.json\n'
  } >"$SANDBOX_REPO/.gitignore"
  mkdir -p "$SANDBOX_REPO/graphify-out"
  printf '{"nodes":[]}\n' >"$SANDBOX_REPO/graphify-out/graph.json"
  # A base-commit file a test can RENAME on the integration branch, so the
  # manifest has a real rename to hide the source side of.
  mkdir -p "$SANDBOX_REPO/dot_aws"
  printf 'aws credentials template\n' >"$SANDBOX_REPO/dot_aws/credentials.tmpl"
  cutover_git -C "$SANDBOX_REPO" add -A
  cutover_git -C "$SANDBOX_REPO" commit --quiet -m 'base'
  # shellcheck disable=SC2034  # read by the sourcing test as the Phase A base
  SANDBOX_BASE="$(git -C "$SANDBOX_REPO" rev-parse HEAD)"

  cutover_git -C "$SANDBOX_REPO" checkout --quiet -b integration/modernization
  printf 'landed\n' >"$SANDBOX_REPO/a.txt"
  printf 'improved-on-integration\n' >"$SANDBOX_REPO/b.txt"
  printf 'omitted\n' >"$SANDBOX_REPO/c.txt"
  cutover_git -C "$SANDBOX_REPO" add -A
  cutover_git -C "$SANDBOX_REPO" commit --quiet -m 'integration delta'

  cutover_git -C "$SANDBOX_REPO" checkout --quiet main
  mkdir -p "$SANDBOX_REPO/Library/LaunchAgents" "$SANDBOX_REPO/dot_local/bin" \
    "$SANDBOX_REPO/scripts"
  printf '<plist><dict><key>Label</key><string>com.github.openclaw-setup.watchdog</string></dict></plist>\n' \
    >"$SANDBOX_REPO/Library/LaunchAgents/com.github.openclaw-setup.watchdog.plist.tmpl"
  printf '<plist><dict><key>Label</key><string>com.webdavis.osquery-fim-notify</string></dict></plist>\n' \
    >"$SANDBOX_REPO/Library/LaunchAgents/com.webdavis.osquery-fim-notify.plist.tmpl"
  cutover_git -C "$SANDBOX_REPO" add -A
  cutover_git -C "$SANDBOX_REPO" commit --quiet -m 'historical services'
  cutover_git -C "$SANDBOX_REPO" rm --quiet \
    "$SANDBOX_REPO/Library/LaunchAgents/com.github.openclaw-setup.watchdog.plist.tmpl"
  cutover_git -C "$SANDBOX_REPO" mv \
    "$SANDBOX_REPO/Library/LaunchAgents/com.webdavis.osquery-fim-notify.plist.tmpl" \
    "$SANDBOX_REPO/Library/LaunchAgents/com.webdavis.osquery-results-alerter.plist.tmpl"
  cutover_git -C "$SANDBOX_REPO" commit --quiet -m 'retire and rename'

  printf '<plist><dict><key>Label</key><string>com.webdavis.atuin-daemon</string><key>KeepAlive</key><true/></dict></plist>\n' \
    >"$SANDBOX_REPO/Library/LaunchAgents/com.webdavis.atuin-daemon.plist.tmpl"
  printf '<plist><dict><key>Label</key><string>com.webdavis.osquery-uptime-watchdog</string><key>RunAtLoad</key><true/><key>StartInterval</key><integer>900</integer></dict></plist>\n' \
    >"$SANDBOX_REPO/Library/LaunchAgents/com.webdavis.osquery-uptime-watchdog.plist.tmpl"
  {
    printf '#!/usr/bin/env bash\n'
    printf 'PLIST_PATH=/Library/LaunchDaemons/systems.nixos.nix-installer.nix-hook.plist\n'
    printf '\t<key>Label</key>\n'
    printf '\t<string>systems.nixos.nix-installer.nix-hook</string>\n'
    printf '\t<key>KeepAlive</key>\n'
    printf '\t<dict><key>SuccessfulExit</key><false/></dict>\n'
    # shellcheck disable=SC2016  # fixture text, not an expansion
    printf 'launchctl bootstrap system "$PLIST_PATH"\n'
  } >"$SANDBOX_REPO/dot_local/bin/executable_install-nix-repair-hook.sh"
  # The reconcile tool gate 3 runs by absolute path: records its argv, and fails
  # when RECONCILE_FAIL matches its mode ("dry-run" or "live").
  # shellcheck disable=SC2016  # literal stub body; $vars resolve when it runs
  {
    printf '#!/usr/bin/env bash\n'
    printf 'printf "%%s\\n" "$*" >>"$RECONCILE_LOG"\n'
    printf 'mode=live\n'
    printf '[[ "${1:-}" == "--dry-run" ]] && mode=dry-run\n'
    # RECONCILE_SELF_EDIT rewrites this file during the DRY RUN, so the live
    # invocation would execute code the dry run never proved.
    printf '[[ -n "${RECONCILE_SELF_EDIT:-}" && "$mode" == "dry-run" ]] && printf "# edited between invocations\\n" >>"$0"\n'
    printf '[[ "${RECONCILE_FAIL:-}" == "$mode" ]] && exit 1\n'
    printf 'exit 0\n'
  } >"$SANDBOX_REPO/scripts/live-reconcile.sh"
  chmod +x "$SANDBOX_REPO/scripts/live-reconcile.sh"
  # A .chezmoidata file for the render-versus-source check. chezmoi's data-only
  # reads walk nested worktrees ignoring .chezmoiignore (twpayne/chezmoi#4940),
  # and .chezmoidata merges last-one-wins, so a stale copy under one of them
  # silently replaces this declaration in every rendered view.
  mkdir -p "$SANDBOX_REPO/.chezmoidata"
  {
    printf 'packages:\n'
    printf '  macos:\n'
    printf '    homebrew:\n'
    printf '      casks:\n'
    printf '        - lulu\n'
    printf '        - oversight\n'
    printf '        - paseo\n'
    printf '      formulae:\n'
    printf '        - jq\n'
  } >"$SANDBOX_REPO/.chezmoidata/system_packages_autoinstall.yaml"
  printf 'landed\n' >"$SANDBOX_REPO/a.txt"
  printf 'improved-on-main\n' >"$SANDBOX_REPO/b.txt"
  cutover_git -C "$SANDBOX_REPO" add -A
  cutover_git -C "$SANDBOX_REPO" commit --quiet -m 'main state'

  cutover_git -C "$SANDBOX_REPO" push --quiet origin main integration/modernization
  cutover_git -C "$SANDBOX_REPO" fetch --quiet origin

  # The default rendered view AGREES with the data file just committed. A test
  # that wants the disagreement overwrites this afterwards.
  [[ -n ${CHEZMOI_DATA_FILE:-} ]] && cutover_write_render_view "$SANDBOX_HOME" "$CHEZMOI_DATA_FILE"
  return 0
}

# cutover_write_render_view <home> <outfile> : the JSON `chezmoi data` output
# the stub replays, built FROM the sandbox's own data file so it agrees with it.
# A test that wants the twpayne/chezmoi#4940 disagreement edits the result.
cutover_write_render_view() {
  local data_file="$1/workspaces/Ivy/webdavis/dotfiles/.chezmoidata/system_packages_autoinstall.yaml"
  yq -o=json '{"packages": .packages}' "$data_file" >"$2"
}

# cutover_entry_pair <repo> <int-rev> <main-rev> <path> : the "<int>|<main>"
# tree-entry pair the runner binds a classification row to.
cutover_entry_pair() {
  local repo="$1" int_rev="$2" main_rev="$3" path="$4" int_entry main_entry
  int_entry="$(git -C "$repo" ls-tree "$int_rev" -- "$path" 2>/dev/null |
    awk 'NR == 1 { printf "%s %s", $1, $3 }')"
  main_entry="$(git -C "$repo" ls-tree "$main_rev" -- "$path" 2>/dev/null |
    awk 'NR == 1 { printf "%s %s", $1, $3 }')"
  printf '%s|%s' "$int_entry" "$main_entry"
}

# cutover_write_classification <home> : the operator's delta classification for
# the two hunks the sandbox cannot auto-classify. Each row carries the exact
# tree-entry pair it was written for, so it stops applying the moment either
# side changes.
cutover_write_classification() {
  local ledger="$1/.local/state/cutover"
  local repo="$1/workspaces/Ivy/webdavis/dotfiles"
  local int_rev main_rev
  int_rev="$(git -C "$repo" rev-parse origin/integration/modernization)"
  main_rev="$(git -C "$repo" rev-parse origin/main)"
  mkdir -p "$ledger"
  {
    printf 'intentionally-improved\tb.txt\t%s\tmain carries the reviewed rewrite\n' \
      "$(cutover_entry_pair "$repo" "$int_rev" "$main_rev" b.txt)"
    printf 'deliberately-omitted-with-reason\tc.txt\t%s\tsuperseded by the S9 split\n' \
      "$(cutover_entry_pair "$repo" "$int_rev" "$main_rev" c.txt)"
  } >"$ledger/delta-classification.tsv"
}

# cutover_make_launchctl_stub <dir> : writes a `launchctl` stub reading
# $LOADED_GUI / $LOADED_SYSTEM (one label per line) and logging argv to
# $LAUNCHCTL_LOG.
#
#   print <domain>          the real services-block shape
#   print <domain>/<label>  exit 0 when loaded, else 113 (the not-found status);
#                           the per-label body is driven by $PRINT_STATE_<n>
#                           entries seeded through $PRINT_DETAIL (label=body)
#   bootout <domain>/<label> drops the label (stateful), unless FAIL_BOOTOUT
#                           names it
cutover_make_launchctl_stub() {
  local dir="$1"
  mkdir -p "$dir"
  # shellcheck disable=SC2016  # stub body is literal; $vars resolve when it runs
  printf '%s\n' '#!/usr/bin/env bash
printf "%s\n" "$*" >>"$LAUNCHCTL_LOG"
domain_file() {
  case "$1" in
    system | system/*) printf "%s" "$LOADED_SYSTEM" ;;
    *) printf "%s" "$LOADED_GUI" ;;
  esac
}
case "${1:-}" in
  print)
    target="$2"
    file="$(domain_file "$target")"
    case "$target" in
      gui/*/* | system/*)
        label="${target##*/}"
        # 112 is an operational error (an unreachable GUI domain on this host),
        # NOT the 113 not-found status. A gate must never read it as absence.
        [[ -n "${FAIL_PRINT:-}" && "$label" == "${FAIL_PRINT}" ]] && exit 112
        grep -qxF -- "$label" "$file" || exit 113
        detail=""
        [[ -f "$PRINT_DETAIL_DIR/$label" ]] && detail="$(cat "$PRINT_DETAIL_DIR/$label")"
        printf "%s = {\n" "$target"
        printf "%s\n" "$detail"
        printf "}\n"
        exit 0
        ;;
    esac
    printf "%s = {\n" "$target"
    printf "\tservices = {\n"
    while IFS= read -r label; do
      [[ -n "$label" ]] || continue
      printf "\t\t       0      0 \t%s\n" "$label"
    done <"$file"
    printf "\t}\n"
    printf "}\n"
    exit 0
    ;;
  bootout)
    target="$2"
    label="${target##*/}"
    file="$(domain_file "$target")"
    [[ "${FAIL_BOOTOUT:-}" == "$label" ]] && exit 1
    grep -vxF -- "$label" "$file" >"$file.tmp" 2>/dev/null || true
    mv "$file.tmp" "$file"
    exit 0
    ;;
esac
exit 0' >"$dir/launchctl"
  chmod +x "$dir/launchctl"
}

# cutover_make_command_stubs <dir> : chezmoi, just, gh, tailscale and hermes
# stubs. Each logs its argv to $CMD_LOG and fails when $FAIL_<NAME> is set.
cutover_make_command_stubs() {
  local dir="$1" name
  mkdir -p "$dir"
  for name in chezmoi just gh tailscale hermes; do
    # shellcheck disable=SC2016  # stub body is literal; $vars resolve when it runs
    printf '%s\n' '#!/usr/bin/env bash
self="$(basename "$0")"
printf "%s %s\n" "$self" "$*" >>"$CMD_LOG"
case "$self" in
  chezmoi)
    for arg in "$@"; do
      if [[ "$arg" == "data" ]]; then
        cat "${CHEZMOI_DATA_FILE:-/dev/null}"
        exit 0
      fi
      if [[ "$arg" == "managed" ]]; then
        [[ -n "${FAIL_MANAGED:-}" ]] && exit 1
        cat "${CHEZMOI_MANAGED_FILE:-/dev/null}" 2>/dev/null || true
        exit 0
      fi
    done
    [[ -n "${FAIL_CHEZMOI:-}" ]] && { printf " M %s\n" "${FAIL_CHEZMOI}"; exit 0; }
    exit 0
    ;;
  just) [[ -n "${FAIL_JUST:-}" ]] && exit 1 ;;
  gh)
    printf "gh-env GH_REPO=%s GH_HOST=%s\n" "${GH_REPO:-}" "${GH_HOST:-}" >>"$CMD_LOG"
    [[ -n "${FAIL_GH:-}" ]] && exit 1
    ;;
  tailscale) [[ -n "${FAIL_TAILSCALE:-}" ]] && exit 1 ;;
  hermes) [[ -n "${FAIL_HERMES:-}" ]] && exit 1 ;;
esac
exit 0' >"$dir/$name"
    chmod +x "$dir/$name"
  done
}

# cutover_make_home_scripts <home> : the smoke-check executables the runner
# calls by absolute path under $HOME.
cutover_make_home_scripts() {
  local home="$1"
  mkdir -p "$home/.local/bin" "$home/.local/libexec/osquery"
  # The shared canary-freshness seam the heartbeat and the uptime watchdog both
  # source. CANARY_AGE (default 0) drives how stale the sandbox canary looks;
  # CANARY_MISSING makes it unreadable.
  # shellcheck disable=SC2016  # literal stub body; $vars resolve when it runs
  printf '%s\n' 'newest_canary_timestamp() {
  [[ -n "${CANARY_MISSING:-}" ]] && return 1
  printf "%s" "$(($(date +%s) - ${CANARY_AGE:-0}))"
}' >"$home/.local/libexec/osquery/canary-freshness.sh"
  # shellcheck disable=SC2016  # stub body is literal; $vars resolve when it runs
  printf '%s\n' '#!/usr/bin/env bash
printf "relay %s\n" "$*" >>"$CMD_LOG"
[[ -n "${FAIL_RELAY:-}" ]] && exit 1
exit 0' >"$home/.local/bin/relay.sh"
  # shellcheck disable=SC2016  # stub body is literal; $vars resolve when it runs
  printf '%s\n' '#!/usr/bin/env bash
printf "heartbeat %s\n" "$*" >>"$CMD_LOG"
[[ -n "${FAIL_HEARTBEAT:-}" ]] && exit 1
exit 0' >"$home/.local/libexec/osquery/heartbeat.sh"
  chmod +x "$home/.local/bin/relay.sh" "$home/.local/libexec/osquery/heartbeat.sh"
}
