# shellcheck shell=bash
# wait-for-log-line.sh -- poll a log file for a line matching an extended regex,
# with a bounded timeout. Sourced by the dispatch suites; no main.
#
# The dispatch library fires its loud local notification through a backgrounded
# process (alerter ... &), so a single immediate grep races the write under a
# loaded parallel CI host. This polls up to <tries> times at 50 ms each and
# returns 0 as soon as the pattern lands, nonzero after the timeout, so a
# genuinely absent line still fails the test. The match path returns on the first
# iteration, so a passing assertion is not slowed.

# wait_for_log_line <extended-regex> <file> [tries]
wait_for_log_line() {
  local pattern="$1" file="$2" tries="${3:-40}" attempt
  for ((attempt = 0; attempt < tries; attempt++)); do
    if grep -qiE "$pattern" "$file" 2>/dev/null; then
      return 0
    fi
    sleep 0.05
  done
  printf 'wait_for_log_line: /%s/ did not appear in %s within %s tries; contents:\n' "$pattern" "$file" "$tries" >&2
  cat "$file" >&2 2>/dev/null || true
  return 1
}
