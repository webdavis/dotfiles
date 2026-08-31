# shellcheck shell=bash
# extract-shell-notifier.sh -- render dot_bashrc.tmpl and cut the long-running
# notifier region out of it. Sourced by the two suites that drive those
# functions directly; no main.
#
# THE REGION AND NOT ONE FUNCTION BODY. The marker paths are globals assigned
# beside the timer and the skip list is a helper defined between the two
# functions, so a snippet of one body would source a file that cannot say where
# it writes or what it skips.
#
# ONE COPY of the recipe, because there are two callers: a sed range hand-synced
# across two files is a range that drifts the day the region moves, and the half
# that is not updated then extracts something else and keeps passing.

# extract_shell_notifier <repo-root> <destination>
extract_shell_notifier() {
  local repo_root="$1" destination="$2" rendered="$2.bashrc"
  if ! CI=1 chezmoi --source "$repo_root" execute-template --no-tty \
    <"$repo_root/dot_bashrc.tmpl" >"$rendered" 2>/dev/null; then
    printf 'extract_shell_notifier: rendering %s/dot_bashrc.tmpl failed\n' "$repo_root" >&2
    return 1
  fi
  sed -n '/^  __cmd_notify_start=""$/,/^  precmd_functions+=(__cmd_notify_precmd)$/p' \
    "$rendered" | sed 's/^  //' >"$destination"
  # THE END ANCHOR IS ASSERTED, NOT TRUSTED. sed prints to the end of the file
  # when a range's closing address never matches, and that result is still
  # non-empty, so an emptiness guard passes on an extraction that swallowed the
  # rest of the bashrc. What fails then is whatever sourcing the runaway
  # produces, which on today's file is a bare `syntax error near unexpected
  # token fi` naming nothing. The anchor is the range's own last line, so its
  # absence is exactly the runaway (and an unmatched opening address leaves an
  # empty file, which this catches too).
  if ! grep -qxF 'precmd_functions+=(__cmd_notify_precmd)' "$destination"; then
    printf 'extract_shell_notifier: the notifier region was not found in %s; the sed range anchors have moved\n' "$rendered" >&2
    return 1
  fi
}
