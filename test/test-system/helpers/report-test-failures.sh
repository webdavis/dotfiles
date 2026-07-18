# shellcheck shell=bash
# Collect assertion failures and report them all at the end, so one run shows
# every problem instead of stopping at the first. Sourced by the test-system
# suite; no main.
#
# Call record_failure "<message>" for each failed assertion, then finish with
# report_failures "<suite-name>"; its exit status is the script's verdict.

recorded_failures=()

record_failure() {
  recorded_failures+=("$1")
  printf 'FAIL: %s\n' "$1" >&2
}

report_failures() { # <suite-name>
  if ((${#recorded_failures[@]} > 0)); then
    printf '%s: %d assertion(s) failed\n' "$1" "${#recorded_failures[@]}" >&2
    return 1
  fi
  printf '%s: OK\n' "$1"
  return 0
}
