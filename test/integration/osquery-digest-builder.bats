#!/usr/bin/env bats
# The daily digest builder (digest.sh): drains the digest spool (NDJSON, written
# by the alerter's digest_append) into ONE grouped, silent, non-paging message,
# then rotates the live store aside. This suite exercises the builder as a black
# box against a stubbed dispatch: a message-recording spy replaces the real
# send_alert, so a test asserts whether (and how) the builder dispatched without
# touching the network or the real SQLite store.
#
# B1 (this commit): empty-suppression. An absent, zero-byte, or whitespace-only
# store produces no message and no error. Grouping, rotation, and the send land
# in later behaviors.

setup() { setup_digest_harness; }
teardown() { teardown_digest_harness; }

# setup_digest_harness (makeSUT factory) - stand up a throwaway HOME whose only
# dispatch library is a recording spy, point the builder at a temp spool path,
# and export the inputs the builder reads. Sets nothing at file-load time; every
# export happens here, called from setup().
setup_digest_harness() {
  HARNESS_HOME="$(mktemp -d)"
  # Record ownership only after our own mktemp, so teardown removes this path and
  # never a pre-set or inherited HARNESS_HOME.
  _DIGEST_HARNESS_OWNED_DIR="$HARNESS_HOME"
  export HOME="$HARNESS_HOME"

  # The recording spy for send_alert, at the exact libexec path the builder
  # sources. It appends each call's argv to $SEND_ALERT_LOG, so "no dispatch" is
  # an empty log and a later behavior can grep the log for the rendered body.
  local dispatch_dir="$HARNESS_HOME/.local/libexec/osquery"
  mkdir -p "$dispatch_dir"
  export SEND_ALERT_LOG="$HARNESS_HOME/send-alert.log"
  : >"$SEND_ALERT_LOG"
  cat >"$dispatch_dir/alert-dispatch.sh" <<'SPY'
# Recording spy for alert-dispatch.sh: capture each send_alert call so a test can
# assert whether the builder dispatched, and with what, without a real send.
send_alert() {
  printf '%s\n' "$*" >>"$SEND_ALERT_LOG"
}
SPY

  # A temp spool path the builder resolves via OSQUERY_DIGEST_STORE. Left ABSENT
  # by default so a test opts in to a zero-byte or whitespace-only variant.
  export OSQUERY_DIGEST_STORE="$HARNESS_HOME/.local/state/osquery-digest-spool/digest.ndjson"

  DIGEST_BUILDER="${BATS_TEST_DIRNAME}/../../dot_local/libexec/osquery/executable_digest.sh"
}

# teardown_digest_harness - remove ONLY a temp dir this harness created. The
# ownership marker is set after our own mktemp, so a pre-set HARNESS_HOME (marker
# unset) is left untouched.
teardown_digest_harness() {
  [[ -n ${_DIGEST_HARNESS_OWNED_DIR:-} ]] || return 0
  rm -rf "$_DIGEST_HARNESS_OWNED_DIR"
  unset _DIGEST_HARNESS_OWNED_DIR
}

# run_digest - invoke the builder as a child process under the harness env.
run_digest() { bash "$DIGEST_BUILDER"; }

# given_absent_store - the spool file does not exist (the default, made explicit).
given_absent_store() { rm -f "$OSQUERY_DIGEST_STORE"; }

# given_empty_store - a zero-byte spool file.
given_empty_store() {
  mkdir -p "$(dirname "$OSQUERY_DIGEST_STORE")"
  : >"$OSQUERY_DIGEST_STORE"
}

# given_whitespace_only_store - a spool with bytes but no non-whitespace content.
given_whitespace_only_store() {
  mkdir -p "$(dirname "$OSQUERY_DIGEST_STORE")"
  printf ' \t\n  \n' >"$OSQUERY_DIGEST_STORE"
}

# assert_no_send - the recording spy captured no send_alert call.
assert_no_send() {
  if [[ -s $SEND_ALERT_LOG ]]; then
    printf 'expected NO dispatch (an empty store is silent), but send_alert was called: %s\n' \
      "$(cat "$SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# assert_silent_success - the B1 behavior in one intent-named assertion: the
# builder exits 0 AND sends nothing.
assert_silent_success() {
  run run_digest
  if [[ $status -ne 0 ]]; then
    printf 'expected the builder to exit 0 (silent success), got %s: %s\n' "$status" "$output" >&2
    return 1
  fi
  assert_no_send
}

@test "an absent digest store produces no message and exits 0" {
  given_absent_store
  assert_silent_success
}

@test "a zero-byte digest store produces no message and exits 0" {
  given_empty_store
  assert_silent_success
}

@test "a whitespace-only digest store produces no message and exits 0" {
  given_whitespace_only_store
  assert_silent_success
}
