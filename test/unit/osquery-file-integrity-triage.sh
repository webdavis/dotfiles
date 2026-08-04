#!/usr/bin/env bash
#
# file_integrity_triage (results-alerter/file-integrity-triage.sh) is the
# DISPLAY-ONLY triage fact-finder for a file-integrity page. The page it feeds
# fires whenever a watched file diverges from its root-owned known-good
# manifest, and until this existed a routine vendor update and a tamper rendered
# the SAME body: a path and a verb, with nothing to tell them apart.
#
# It answers three questions and nothing else:
#   recorded  the sha256 the governing manifest binds to this exact path, short
#   ondisk    the sha256 the file carries right now, short
#   upgrade   whether a RECORDED unattended upgrade plausibly explains it
#
# WHAT IT NEVER DOES, pinned here because the whole value is in the limit: it
# never says a change is safe, never suppresses, and never returns non-zero.
# It is read by route.sh AFTER the verdict has already decided to page, so a
# correlation that fails must cost the page a LINE, never the page itself.
#
# THE UPGRADE RECORD IS NOT A TRUST INPUT. It lives in the operator-writable
# state dir, so whoever tampered a manifested file could also write a record
# claiming an upgrade explains it. That is why the matched line says the name
# match is not proof, and why nothing here changes a tier.
#
# Unit test: the helper in isolation under a temp HOME, with fixture manifests
# and fixture record files. No sleeps, no network, no live osquery state.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/file-integrity-triage.sh"
PIPELINE_HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh"

fail() {
  printf 'osquery-file-integrity-triage: FAIL -- %s\n' "$*" >&2
  exit 1
}

refute() {
  local needle="$1" haystack="$2" message="$3"
  if grep -qF "$needle" <<<"$haystack"; then
    printf '=== output ===\n%s\n' "$haystack" >&2
    fail "$message"
  fi
}

for h in "$HELPER" "$PIPELINE_HELPER"; do
  [[ -f $h ]] || fail "missing file: $h"
done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
home="$work/home"
mkdir -p "$home/.local/libexec/osquery"

target="$home/.local/libexec/osquery/results-alerter.sh"
printf 'echo tampered\n' >"$target"
chmod 755 "$target"
disk_hash="$(shasum -a 256 "$target" | awk '{print $1}')"
recorded_hash="1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"

manifest="$work/pipeline-known-good.sha256"
printf '%s 0755 %s %s\n' "$recorded_hash" "$(id -u)" "$target" >"$manifest"

record="$work/last-upgrade-changes.tsv"

# triage <target> -- run the helper in a fresh subshell against the fixtures.
# stderr is kept separate so the LOUD-degradation cases can assert on it while
# stdout stays parseable JSON.
triage_stderr="$work/stderr"
triage() {
  : >"$triage_stderr"
  HOME="$home" OSQUERY_PIPELINE_MANIFEST="$manifest" \
    OSQUERY_MANAGED_BIN_MANIFEST="$work/no-bin-manifest.sha256" \
    OSQUERY_UPGRADE_RECORD="$record" \
    bash -c '
      # The REAL alerter entry sources its helpers under errexit, so every case
      # here runs under it too. Without this line the arithmetic guards inside
      # the row loop are exercised in a mode production never uses, and a helper
      # that aborts mid-record would pass here and lose facts in the field.
      set -euo pipefail
      source "$1"
      source "$2"
      file_integrity_triage "$3"
    ' _ "$PIPELINE_HELPER" "$HELPER" "$1" 2>"$triage_stderr"
}

# assert_json <output> -- every rendered value reaches the page through
# render-page.sh, whose jq program is bash SINGLE-QUOTED. An apostrophe anywhere
# in this output would end that quoting and break the renderer, so the ban is
# pinned on the producer side too, not only by reading the renderer.
assert_json() {
  local out="$1" what="$2"
  jq -e . >/dev/null 2>&1 <<<"$out" || fail "$what: the output is not valid JSON -- $out"
  refute "'" "$out" "$what: the output holds an apostrophe, which breaks the single-quoted render jq"
}

iso_at() { # <epoch> -> the ISO 8601 UTC stamp for it, the shape the record holds
  date -u -r "$1" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null ||
    date -u -d "@$1" +%Y-%m-%dT%H:%M:%SZ
}

now="$(date +%s)"
recent_epoch=$((now - 3600))
recent_iso="$(iso_at "$recent_epoch")"
stale_epoch=$((now - 30 * 86400))
stale_iso="$(iso_at "$stale_epoch")"

write_record() { # <epoch> <iso> <tsv-row...>
  local epoch="$1" iso="$2"
  shift 2
  printf '%s\t%s\n' "$epoch" "$iso" >"$record"
  local row
  for row in "$@"; do printf '%s\n' "$row" >>"$record"; done
}

# ---- 1. The hashes. The page has to say WHICH bytes disagree, in a form a human
#      can read back off a terminal, so both sides are the short prefix of the
#      full sha256 and neither is invented when it cannot be read. ----
write_record "$recent_epoch" "$recent_iso"
out="$(triage "$target")"
assert_json "$out" "hashes"
[[ "$(jq -r '.recorded' <<<"$out")" == "${recorded_hash:0:12}" ]] ||
  fail "the recorded hash is not the manifest tuple prefix -- $out"
[[ "$(jq -r '.ondisk' <<<"$out")" == "${disk_hash:0:12}" ]] ||
  fail "the on-disk hash is not the current file prefix -- $out"

# ---- 2. A recorded upgrade whose package NAME matches the flagged file renders
#      the transition, the timestamp, AND the statement that a name match is not
#      proof. Naming it without that qualifier would be the one thing this must
#      never do: read as an all-clear. ----
write_record "$recent_epoch" "$recent_iso" \
  "$(printf 'results-alerter.sh\tchanged\t1.2.3\t1.2.4')" \
  "$(printf 'jq\tchanged\t1.7.1\t1.8.0')"
out="$(triage "$target")"
assert_json "$out" "matched upgrade"
upgrade="$(jq -r '.upgrade' <<<"$out")"
grep -qF 'recorded upgrade: results-alerter.sh 1.2.3 -> 1.2.4' <<<"$upgrade" ||
  fail "a matching recorded upgrade did not render the transition -- $upgrade"
grep -qF "$recent_iso" <<<"$upgrade" ||
  fail "a matching recorded upgrade did not render its timestamp -- $upgrade"
grep -qF 'not proof' <<<"$upgrade" ||
  fail "a matching recorded upgrade read as a verdict instead of a lead -- $upgrade"
refute 'safe' "$upgrade" "the upgrade line called a change safe"

# ---- 3. A record with no matching name says so, and still states WHAT the run
#      changed. The mapping from a file to the package that ships it is not
#      knowable from this record, so the honest answer is the recent activity,
#      never a claimed mapping. ----
write_record "$recent_epoch" "$recent_iso" \
  "$(printf 'jq\tchanged\t1.7.1\t1.8.0')" \
  "$(printf 'yq\tchanged\t4.53.3\t4.54.0')"
out="$(triage "$target")"
assert_json "$out" "unmatched upgrade"
upgrade="$(jq -r '.upgrade' <<<"$out")"
grep -qF 'no recorded upgrade names this file' <<<"$upgrade" ||
  fail "an unmatched record did not say so -- $upgrade"
grep -qF 'jq' <<<"$upgrade" ||
  fail "an unmatched record did not state what the run DID change -- $upgrade"

# ---- 4. A record older than the correlation window cannot explain anything, and
#      says which window it fell out of rather than going quiet. ----
write_record "$stale_epoch" "$stale_iso" \
  "$(printf 'results-alerter.sh\tchanged\t1.2.3\t1.2.4')"
out="$(triage "$target")"
assert_json "$out" "stale record"
upgrade="$(jq -r '.upgrade' <<<"$out")"
grep -qF 'no recorded upgrade in the last' <<<"$upgrade" ||
  fail "a record outside the window was not reported as out of window -- $upgrade"
refute 'recorded upgrade: results-alerter.sh' "$upgrade" \
  "a record outside the window was still offered as an explanation"

# ---- 5. NO RECORD AT ALL. The page must still render, with a line saying the
#      correlation had nothing to work with. ----
rm -f "$record"
out="$(triage "$target")"
assert_json "$out" "absent record"
grep -qF 'no upgrade record' <<<"$(jq -r '.upgrade' <<<"$out")" ||
  fail "an absent record did not render a stated no-record line -- $out"

# ---- 6. A MALFORMED record degrades LOUDLY: the page keeps its line, the helper
#      still exits 0, and the reason lands on stderr (which is the alerter's
#      launchd log). A correlation that cannot be trusted must not be rendered as
#      one, and must not take the page down with it. ----
printf 'not-an-epoch\tnot-an-iso\nrubbish\n' >"$record"
rc=0
out="$(triage "$target")" || rc=$?
[[ $rc -eq 0 ]] || fail "a malformed record made the helper exit $rc; a page must never be lost to it"
assert_json "$out" "malformed record"
grep -qF 'could not be read' <<<"$(jq -r '.upgrade' <<<"$out")" ||
  fail "a malformed record did not render a stated could-not-read line -- $out"
[[ -s $triage_stderr ]] ||
  fail "a malformed record degraded SILENTLY; it must say so on stderr"

# ---- 7. A file that is GONE (the DELETED verb reaches this helper too) still
#      yields a rendered fact rather than an empty field or a crash. ----
write_record "$recent_epoch" "$recent_iso"
rc=0
out="$(triage "$home/.local/libexec/osquery/vanished.sh")" || rc=$?
[[ $rc -eq 0 ]] || fail "a vanished target made the helper exit $rc"
assert_json "$out" "vanished target"
[[ "$(jq -r '.ondisk' <<<"$out")" == "absent" ]] ||
  fail "a vanished target did not render its on-disk state as absent -- $out"
[[ "$(jq -r '.recorded' <<<"$out")" == "not in the manifest" ]] ||
  fail "an unmanifested target did not say the manifest has no tuple for it -- $out"

# ---- 8. ALL THREE ROW KINDS render the transition in the direction the run
#      actually moved. The producer writes the absent side of an add or a remove
#      as an EMPTY column, and a tab is an IFS WHITESPACE character, so decoding
#      a row with `IFS=$'\t' read -r name state before after` collapsed the two
#      tabs of an `added` row into one delimiter and shifted the version into the
#      BEFORE field: a package that had just appeared rendered as "14.1.1 ->
#      none", i.e. as a package that had just been REMOVED. Only `changed` rows
#      were covered before, which is the one shape that survives the collapse. --
added_target="$home/.local/bin/ripgrep"
removed_target="$home/.local/bin/oldpkg"
changed_target="$home/.local/bin/jq"
write_record "$recent_epoch" "$recent_iso" \
  "$(printf 'ripgrep\tadded\t\t14.1.1')" \
  "$(printf 'oldpkg\tremoved\t9.9\t')" \
  "$(printf 'jq\tchanged\t1.7.1\t1.8.0')"

out="$(triage "$added_target")"
assert_json "$out" "added row"
grep -qF 'recorded upgrade: ripgrep none -> 14.1.1' <<<"$(jq -r '.upgrade' <<<"$out")" ||
  fail "an ADDED package did not render as none -> version -- $(jq -r '.upgrade' <<<"$out")"

out="$(triage "$removed_target")"
assert_json "$out" "removed row"
grep -qF 'recorded upgrade: oldpkg 9.9 -> none' <<<"$(jq -r '.upgrade' <<<"$out")" ||
  fail "a REMOVED package did not render as version -> none -- $(jq -r '.upgrade' <<<"$out")"

out="$(triage "$changed_target")"
assert_json "$out" "changed row"
grep -qF 'recorded upgrade: jq 1.7.1 -> 1.8.0' <<<"$(jq -r '.upgrade' <<<"$out")" ||
  fail "a CHANGED package did not render its version transition -- $(jq -r '.upgrade' <<<"$out")"

# triage_bounded <target> -- run the helper with a WATCHDOG, so a record shape
# that blocks fails this suite instead of wedging it. A plain triage() call is a
# command substitution, which waits forever; the cases below are exactly the
# shapes that never return, so the call has to be bounded from the outside.
#
# The bound is a killer subshell rather than a poll loop, so the passing path
# sleeps for NOTHING: `wait` returns the instant the helper exits and the killer
# is torn down unused. The killer detaches its own stdin and stdout so a lingering
# sleep can never hold the suite runner's pipe open.
triage_bounded_out=""
triage_bounded() {
  local target="$1" child killer status=0
  : >"$triage_stderr"
  HOME="$home" OSQUERY_PIPELINE_MANIFEST="$manifest" \
    OSQUERY_MANAGED_BIN_MANIFEST="$work/no-bin-manifest.sha256" \
    OSQUERY_UPGRADE_RECORD="$record" \
    bash -c '
      set -euo pipefail
      source "$1"
      source "$2"
      file_integrity_triage "$3"
    ' _ "$PIPELINE_HELPER" "$HELPER" "$target" >"$work/bounded.out" 2>"$triage_stderr" &
  child=$!
  {
    sleep 10
    kill -KILL "$child" 2>/dev/null || true
  } </dev/null >/dev/null 2>&1 &
  killer=$!
  wait "$child" || status=$?
  kill "$killer" 2>/dev/null || true
  triage_bounded_out="$(cat "$work/bounded.out")"
  return "$status"
}

# ---- 9. A record path that is NOT A REGULAR FILE is refused rather than read.
#      The record lives in the operator-writable state dir, so its path is not a
#      trust input: a readable FIFO with no writer standing there makes `read`
#      block FOREVER, and it blocks while the alerter holds its single-instance
#      lock, so the page is never sent and the cursor never advances. Every page
#      on the machine stops behind one named pipe. A character device does the
#      same thing by never yielding a newline. ----
fifo_record="$work/record.fifo"
rm -f "$fifo_record"
mkfifo "$fifo_record"
record="$fifo_record"
rc=0
triage_bounded "$target" || rc=$?
[[ $rc -eq 0 ]] ||
  fail "a FIFO upgrade record blocked or failed the helper (exit $rc); every page waits behind it"
assert_json "$triage_bounded_out" "fifo record"
grep -qF 'could not be read' <<<"$(jq -r '.upgrade' <<<"$triage_bounded_out")" ||
  fail "a FIFO record did not degrade to a stated could-not-read line -- $triage_bounded_out"
[[ -s $triage_stderr ]] || fail "a FIFO record degraded SILENTLY; it must say so on stderr"

record="/dev/zero"
rc=0
triage_bounded "$target" || rc=$?
[[ $rc -eq 0 ]] ||
  fail "a character-device upgrade record blocked or failed the helper (exit $rc)"
assert_json "$triage_bounded_out" "device record"
grep -qF 'could not be read' <<<"$(jq -r '.upgrade' <<<"$triage_bounded_out")" ||
  fail "a character-device record did not degrade to a stated could-not-read line -- $triage_bounded_out"
record="$work/last-upgrade-changes.tsv"

# ---- 10. The record is read ONCE. It is published by an atomic rename, so two
#      opens can straddle a replacement and pair one generation's timestamp with
#      the other generation's package rows, which is a rendered correlation that
#      never existed. The interleaving is driven deterministically rather than
#      raced: the helper reads its clock between the two opens it used to do, so
#      a `date` shim on PATH is a hook that fires exactly there, and it swaps the
#      record for a second generation. The shim leaves a MARKER and the marker is
#      asserted, so this can never pass by quietly failing to drive the swap.
#      EPOCHSECONDS is unset for the same reason: with it set, bash answers the
#      clock read from its own variable and never forks the shim. ----
shim_bin="$work/shim-bin"
mkdir -p "$shim_bin"
gen_b_record="$work/generation-b.tsv"
{
  printf '%s\t%s\n' "$recent_epoch" "$recent_iso"
  printf 'ripgrep\tchanged\t14.1.0\t14.1.1\n'
} >"$gen_b_record"
cat >"$shim_bin/date" <<SHIM
#!/usr/bin/env bash
# Publish generation B over the record, the way the weekly job does.
: >"$work/date-shim-fired"
cp "$gen_b_record" "$record.next"
mv -f "$record.next" "$record"
exec /bin/date "\$@"
SHIM
chmod +x "$shim_bin/date"
# Generation A: the SAME timestamp, a DIFFERENT transition for the same package.
write_record "$recent_epoch" "$recent_iso" "$(printf 'ripgrep\tchanged\t13.0.0\t14.0.0')"
rm -f "$work/date-shim-fired"
out="$(PATH="$shim_bin:$PATH" HOME="$home" OSQUERY_PIPELINE_MANIFEST="$manifest" \
  OSQUERY_MANAGED_BIN_MANIFEST="$work/no-bin-manifest.sha256" \
  OSQUERY_UPGRADE_RECORD="$record" \
  bash -c '
    set -euo pipefail
    # Without this, bash answers the helper clock read from its own
    # EPOCHSECONDS, and the shim that drives the swap never runs.
    unset EPOCHSECONDS
    source "$1"
    source "$2"
    file_integrity_triage "$3"
  ' _ "$PIPELINE_HELPER" "$HELPER" "$home/.local/bin/ripgrep" 2>"$triage_stderr")"
[[ -e $work/date-shim-fired ]] ||
  fail "the record-swap hook never fired, so this case proved nothing about how often the record is opened"
assert_json "$out" "single read"
upgrade="$(jq -r '.upgrade' <<<"$out")"
grep -qF 'recorded upgrade: ripgrep 13.0.0 -> 14.0.0' <<<"$upgrade" ||
  fail "the helper re-opened the record and rendered a generation it did not date -- $upgrade"

printf 'osquery-file-integrity-triage: OK (short recorded/on-disk hashes; a name-matching record renders the transition and says it is not proof; added, removed and changed rows each render in the direction the run moved; an unmatched, stale, absent or malformed record each degrade to a stated line, loudly and without losing the page)\n'
