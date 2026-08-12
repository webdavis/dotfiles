#!/usr/bin/env bats
# What happens to a banner value BEFORE terminal-notifier's own code sees it,
# and why banner.rs arms every value with a leading backslash.
#
# WHAT KIND OF FILE THIS IS. A CONTRACT TEST: a characterization of the
# ingestion half of our terminal-notifier dependency. It runs Apple's real
# NSUserDefaults argument parsing against edges we choose, the way a
# URLProtocol-based test runs real URLSession code against responses it
# controls. It is NOT a spy, and it is NOT a stub of terminal-notifier, which
# offers no sanctioned extension point to stub inside.
#
# WHICH LAYER MAY CLAIM WHAT, so no file here overstates its reach:
#
#   SPIES, at our OWN seams (the Bridge and Sleeper recorders, the RELAY_BIN
#     stub, the stub terminal-notifier on PATH): our composition only. What
#     argv pns builds and in what order. They cannot speak for what the
#     dependency then does with it.
#   THIS CONTRACT TEST: the dependency's parsing, expected failures included.
#     Real Foundation behavior (layer 1), plus a REPLICATED one-line copy of
#     terminal-notifier's leading-backslash unescape, cited in the fixture
#     (layer 2). If upstream's rule drifts from that line, nothing here
#     notices.
#   THE DRILL, plus a Notification Center readback: end-to-end render truth,
#     the only thing that proves a banner appeared. That is the drill protocol
#     and the P8 screenshot matrix in the session ledger, not this file.
#
# One measured case does not match the P4-P6 reading and is pinned as measured
# rather than as expected: a leading SPACE survives layer 1 intact. Its live
# failure therefore happens above this layer, in terminal-notifier or in the
# notification service, and this file cannot see it. See the test that says so.

setup_file() {
  [[ -x /usr/bin/clang ]] || {
    printf 'clang is missing: install the Xcode command line tools with `xcode-select --install`\n' >&2
    return 1
  }
  export PROBE="$BATS_FILE_TMPDIR/nsuserdefaults-probe"
  # ~0.8s, once for the whole file; every test below is a millisecond exec.
  /usr/bin/clang -framework Foundation -o "$PROBE" \
    "$BATS_TEST_DIRNAME/../fixtures/nsuserdefaults-probe/main.m" || {
    printf 'the NSUserDefaults probe did not build\n' >&2
    return 1
  }
}

# The twelve shapes probe P8 fired, as INTENDED final text.
shapes() {
  printf '%s\0' \
    '(leading paren' \
    '[leading bracket' \
    '{leading brace' \
    '-leading dash' \
    '<leading angle' \
    '"leading quote' \
    ' leading space then (paren' \
    '9 leading digit' \
    'a leading letter' \
    "$(printf '​')(zero width space then paren" \
    '\a leading backslash' \
    'text with (parens) in the middle'
}

# readback <title-arg> <message-arg>: the two values as the parsing yields
# them, NUL separated so a value carrying spaces reads back intact.
readback() {
  local -a out=()
  mapfile -d '' -t out < <("$PROBE" -title "$1" -message "$2")
  printf '%s\0%s' "${out[0]-}" "${out[1]-}"
}

# as_message <arg>: what -message came back as, for one raw argument.
as_message() {
  local -a out=()
  mapfile -d '' -t out < <(readback 'a title' "$1")
  printf '%s' "${out[1]-}"
}

# as_title <arg>: the same for -title.
as_title() {
  local -a out=()
  mapfile -d '' -t out < <(readback "$1" 'a message')
  printf '%s' "${out[0]-}"
}

# refute_survives <text>: <text> sent UNARMORED must not come back as itself.
# A plain call, not `! as_message ...`, because bats and errexit both ignore an
# inverted pipeline, which is how a refutation goes dead.
refute_survives() {
  local got
  got="$(as_message "$1")"
  if [[ $got == "$1" ]]; then
    printf 'expected %q to be eaten unarmored, but it survived as %q\n' "$1" "$got" >&2
    return 1
  fi
}

# --- armored: every shape survives, which is the whole point of the armor ----

@test "every P8 shape survives the parsing when it is armored" {
  local shape got
  while IFS= read -r -d '' shape; do
    got="$(as_message "\\$shape")"
    [[ $got == "$shape" ]] || {
      printf 'armored %q came back as %q\n' "$shape" "$got" >&2
      return 1
    }
  done < <(shapes)
}

@test "an armored value that already starts with a backslash keeps exactly one" {
  # The armor prepends one and the unescape strips one, so a value whose own
  # first character is a backslash is delivered unchanged rather than doubled
  # or stripped bare (P8-H).
  [ "$(as_message '\\a leading backslash')" = '\a leading backslash' ]
}

@test "the title is armored on the same terms as the message" {
  # The escaped-title case: -title is read through the identical parsing, so a
  # title beginning with a killer character needs the same prefix.
  [ "$(as_title '\(leading paren')" = '(leading paren' ]
  [ "$(as_title '(leading paren')" = 'NOT-A-STRING' ]
}

# --- unarmored: the bug shape itself, kept visible and machine-checked -------

@test "the killer set is eaten when it is not armored" {
  # This is the defect the armor exists for. Measured: each of these yields no
  # string at all, so terminal-notifier renders the notification title-only.
  local shape
  for shape in '(leading paren' '[leading bracket' '{leading brace' \
    '-leading dash' '<leading angle' '"leading quote'; do
    refute_survives "$shape"
  done
}

@test "a zero-width space does not escape a leading paren" {
  refute_survives "$(printf '​')(zero width space then paren"
}

@test "an unarmored leading backslash is silently eaten by the unescape" {
  # Not the plist parser this time, the unescape: text that legitimately
  # begins with a backslash arrives one character short unless it was armored.
  [ "$(as_message '\a leading backslash')" = 'a leading backslash' ]
}

@test "a leading SPACE survives this layer, so its live failure is above it" {
  # Measured 2026-08-12, and it does NOT match the P4-P6 reading that a
  # leading space fails to escape a paren. Both can be true: the value reaches
  # terminal-notifier intact and something above this layer drops it. Pinned
  # as measured so the disagreement stays visible instead of being asserted
  # away, and so a future macOS that DOES eat it here shows up as a change.
  [ "$(as_message ' leading space then (paren')" = ' leading space then (paren' ]
}

# --- controls: the rule really is first-character only -----------------------

@test "plain text and mid-text punctuation need no armor at all" {
  [ "$(as_message 'a leading letter')" = 'a leading letter' ]
  [ "$(as_message '9 leading digit')" = '9 leading digit' ]
  [ "$(as_message 'text with (parens) in the middle')" = 'text with (parens) in the middle' ]
}
