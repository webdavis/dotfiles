#!/usr/bin/env bash
# macos-defaults-injection.sh -- the macos_defaults.yaml data file is UNTRUSTED
# input to two code generators: the Tier 1 runner template, which renders records
# into shell source, and the shared library, which renders them into a
# separator-joined record stream. This suite pins the properties that keep a
# record from becoming code.
#
# Why it matters here specifically: a system-scope record makes the runner cache
# a sudo credential in a `sudo -v` prelude that runs BEFORE any write, so
# anything the runner is tricked into executing afterwards runs with a
# passwordless root escalation already available.
#
# The properties pinned, each one previously unpinned:
#
#   A. A `type` outside the closed set fails the render on BOTH scopes, and no
#      write line is emitted. The type is the one field rendered as a bare word,
#      so the closed set is the only thing standing between it and shell syntax.
#   B. Command substitution, backticks, and parameter expansion in `domain`,
#      `key`, `value`, `plist_path`, and a killall entry never reach shell source
#      as live syntax. Rendering is not enough evidence, so the render is EXECUTED
#      under stubs: no marker file may appear, and every stub must have received
#      the literal text.
#   C. The template refuses the targets it cannot vouch for: a traversal domain,
#      a traversal plist_path, a plist_path on a user-scope record, and the
#      degenerate targets (empty/all-dot domain, a plist_path of exactly "/").
#   D. One YAML record produces exactly one write. A record whose value carries
#      unit separators and a newline can otherwise forge a SECOND, fully
#      attacker-controlled root write that the template never renders, leaving
#      the tools and the runner disagreeing about how many records exist.
#
# Real chezmoi and yq; `defaults`, `sudo`, `osascript`, and `killall` are
# stubbed. Never runs real sudo, never touches /Library.
#
# This test deals in LITERAL shell-injection payloads and stub-script bodies, so
# `$(...)` / `$@` inside single quotes is deliberate (they must NOT expand here).
# shellcheck disable=SC2016
set -euo pipefail

# Scrubbed at SCRIPT scope, before any git or chezmoi call. Git exports GIT_DIR
# to every hook it runs and this suite can still inherit one; the
# library's own override may be exported on a developer machine.
unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl"
LIB="$REPO_ROOT/dot_local/bin/macos-defaults-lib.sh"
APPLY="$REPO_ROOT/dot_local/bin/executable_macos-defaults-apply.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# A bare `! grep` is dead under `set -e` unless it happens to be the last
# statement, so every negative below goes through this helper.
refute_file_contains() { # <file> <fixed-string> <message>
  if grep -qF -- "$2" "$1"; then
    fail "$3"
  fi
}

assert_file_contains() { # <file> <fixed-string> <message>
  grep -qF -- "$2" "$1" || fail "$3"
}

for tool in chezmoi yq; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'SKIP: %s not on PATH; cannot exercise the data-file injection guards\n' "$tool"
    exit 0
  }
done
for required_file in "$TEMPLATE" "$LIB" "$APPLY"; do
  [[ -f $required_file ]] || fail "missing file: $required_file"
done

# Canonicalize away macOS's /var -> /private/var symlink.
sandbox="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$sandbox"' EXIT
render_home="$sandbox/render-home"
mkdir -p "$render_home"

make_source_dir() {
  local source_dir
  source_dir="$(mktemp -d "$sandbox/src.XXXXXX")"
  mkdir -p "$source_dir/.chezmoidata"
  cat >"$source_dir/.chezmoidata/macos_defaults.yaml"
  printf '%s\n' "$source_dir"
}

render_template() { # <source-dir> <out-file> <err-file>
  HOME="$render_home" chezmoi --source "$1" execute-template --no-tty <"$TEMPLATE" >"$2" 2>"$3"
}

render_error="$sandbox/render.err"

# Non-darwin hosts render an empty body, so there is nothing to exercise. Probed
# once, with a minimal fixture, before any assertion depends on a real render.
probe_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.probe
      key: ProbeKey
      type: bool
      value: true
      tier: enforce
  killall: []
EOF
)"
render_template "$probe_src" "$sandbox/rendered-probe" "$render_error" ||
  fail "the probe render must succeed (stderr: $(cat "$render_error"))"
if [[ -z "$(tr -d '[:space:]' <"$sandbox/rendered-probe")" ]]; then
  # An empty render is only legitimate OFF darwin. On darwin this template
  # must produce output, so empty means the template broke, and skipping
  # would report a broken render as a pass. Assert the host, do not infer it.
  if [[ $(uname -s) == Darwin ]]; then
    printf 'FAIL: the render came back EMPTY on darwin, where this template must produce output; a broken darwin render must not pass as a skip\n' >&2
    exit 1
  fi
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# assert_render_rejects <label> <expected-stderr-fragment> -- feed a fixture on
# stdin, require the render to FAIL, require the message to name the reason, and
# require that no write line was emitted. The last part is what makes this an
# injection assertion rather than a status check: a render that failed only
# after emitting a write line has already written the attacker's script.
assert_render_rejects() { # <label> <expected-stderr-fragment>   (fixture on stdin)
  local label="$1" expected_fragment="$2"
  local source_dir out_file status=0
  source_dir="$(make_source_dir)"
  out_file="$sandbox/rejected-render"
  render_template "$source_dir" "$out_file" "$render_error" || status=$?
  [[ $status -ne 0 ]] ||
    fail "$label: the render must fail (got 0, render: $(cat "$out_file"))"
  assert_file_contains "$render_error" "$expected_fragment" \
    "$label: the failure must name the reason '$expected_fragment' (stderr: $(cat "$render_error"))"
  if grep -qE '^(sudo )?defaults ' "$out_file"; then
    fail "$label: a rejected render must emit no write line (got: $(grep -E '^(sudo )?defaults ' "$out_file"))"
  fi
}

# ---- A: a hostile type fails the render on both scopes ----------------------

# The payload is a valid `defaults` option word followed by a shell command
# separator, so an unvalidated type renders a legitimate write and then the
# attacker's command. Quoting cannot fix it: `defaults` needs a BARE option
# word, which is why the closed set exists.
assert_render_rejects 'hostile type, user scope' 'unsupported type' <<'EOF'
macos:
  defaults:
    - domain: com.example.usertype
      key: UserTypeKey
      type: 'bool true; touch hostile-user-type-marker #'
      value: true
      tier: enforce
  killall: []
EOF

assert_render_rejects 'hostile type, system scope' 'unsupported type' <<'EOF'
macos:
  defaults:
    - domain: com.example.systype
      key: SysTypeKey
      type: 'bool true; touch hostile-system-type-marker #'
      value: true
      scope: system
      tier: enforce
  killall: []
EOF

# Control for A: the closed set still admits every supported type, so the
# rejections above are not satisfied by a template that refuses all of them.
every_type_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.t1
      key: K1
      type: array
      value: v
      tier: enforce
    - domain: com.example.t2
      key: K2
      type: bool
      value: true
      tier: enforce
    - domain: com.example.t3
      key: K3
      type: data
      value: v
      tier: enforce
    - domain: com.example.t4
      key: K4
      type: date
      value: v
      tier: enforce
    - domain: com.example.t5
      key: K5
      type: dict
      value: v
      tier: enforce
    - domain: com.example.t6
      key: K6
      type: float
      value: 1.5
      tier: enforce
    - domain: com.example.t7
      key: K7
      type: int
      value: 7
      tier: enforce
    - domain: com.example.t8
      key: K8
      type: string
      value: v
      tier: enforce
  killall: []
EOF
)"
render_template "$every_type_src" "$sandbox/rendered-every-type" "$render_error" ||
  fail "every supported type must still render (stderr: $(cat "$render_error"))"
every_type_write_count="$(grep -cE '^defaults ' "$sandbox/rendered-every-type" || true)"
[[ $every_type_write_count -eq 8 ]] ||
  fail "the eight supported types must each render a write (got $every_type_write_count)"

# ---- B: shell metacharacters never reach shell source as live syntax --------

# Every data field carries a payload that WOULD run if the rendered field were
# double-quoted (bash expands $(...), backticks and $VAR inside double quotes)
# or unquoted. Each payload writes a distinctly named marker, so a failure names
# the field that leaked.
injection_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: "com.example.userdomain$(touch user-domain-marker)"
      key: "UserKey$(touch user-key-marker)"
      type: string
      value: "$(touch user-value-marker)`touch user-tick-marker`$INJECTED_VARIABLE"
      tier: enforce
    - domain: "com.example.sysdomain$(touch system-domain-marker)"
      key: SysDomainKey
      type: bool
      value: true
      scope: system
      tier: enforce
    - domain: com.example.syspath
      key: "SysPathKey$(touch system-key-marker)"
      type: string
      value: "$(touch system-value-marker)"
      scope: system
      plist_path: "/Library/Preferences/sys$(touch system-path-marker).plist"
      tier: enforce
  killall:
    - "Dock$(touch killall-marker)`touch killall-tick-marker`"
EOF
)"
rendered_injection="$sandbox/rendered-injection"
render_template "$injection_src" "$rendered_injection" "$render_error" ||
  fail "the injection fixture must render (these are legal, if hostile, values; stderr: $(cat "$render_error"))"

# The stubs record the argument vector they were handed and do nothing else.
# `sudo` does NOT exec its argument list: nothing here should run for real.
stub_bin="$sandbox/bin"
mkdir -p "$stub_bin"
stub_log="$sandbox/stub.log"
for stubbed_command in defaults sudo osascript killall; do
  cat >"$stub_bin/$stubbed_command" <<EOF
#!/bin/bash
{ printf '$stubbed_command'; printf ' [%s]' "\$@"; printf '\n'; } >>"$stub_log"
exit 0
EOF
  chmod +x "$stub_bin/$stubbed_command"
done

# The rendered script runs with this directory as its working directory, so any
# payload that executes lands here, observable and contained.
marker_dir="$sandbox/markers"
mkdir -p "$marker_dir"
: >"$stub_log"
(
  cd "$marker_dir" || exit 1
  PATH="$stub_bin:$PATH" INJECTED_VARIABLE=EXPANDED_VARIABLE_VALUE \
    bash "$rendered_injection"
) || fail "the rendered injection script must run cleanly under the stubs (log: $(cat "$stub_log"))"

created_markers="$(find "$marker_dir" -mindepth 1 -print)"
[[ -z $created_markers ]] ||
  fail "no payload may execute; the rendered script created: $created_markers"

# The stubs must have seen the LITERAL payload text. Without this the emptiness
# check above is also satisfied by a template that dropped the fields entirely.
assert_file_contains "$stub_log" 'com.example.userdomain$(touch user-domain-marker)' \
  "the user domain must arrive as literal text (log: $(cat "$stub_log"))"
assert_file_contains "$stub_log" 'UserKey$(touch user-key-marker)' \
  "the user key must arrive as literal text (log: $(cat "$stub_log"))"
assert_file_contains "$stub_log" '$(touch user-value-marker)`touch user-tick-marker`$INJECTED_VARIABLE' \
  "the user value must arrive as literal text, backticks and $ included (log: $(cat "$stub_log"))"
assert_file_contains "$stub_log" '/Library/Preferences/com.example.sysdomain$(touch system-domain-marker)' \
  "the system domain must arrive as literal text inside the resolved path (log: $(cat "$stub_log"))"
assert_file_contains "$stub_log" 'SysPathKey$(touch system-key-marker)' \
  "the system key must arrive as literal text (log: $(cat "$stub_log"))"
assert_file_contains "$stub_log" '/Library/Preferences/sys$(touch system-path-marker).plist' \
  "the declared plist_path must arrive as literal text (log: $(cat "$stub_log"))"
assert_file_contains "$stub_log" 'Dock$(touch killall-marker)`touch killall-tick-marker`' \
  "the killall entry must arrive as literal text (log: $(cat "$stub_log"))"
refute_file_contains "$stub_log" 'EXPANDED_VARIABLE_VALUE' \
  'a $VAR in a data field must never be expanded against the runner environment'

# A value containing a single quote must survive the single-quoting intact.
# `squote` would wrap without escaping, which BREAKS OUT of the quotes; this is
# the case that separates correct escaping from the convenient-looking one.
apostrophe_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.apostrophe
      key: ApostropheKey
      type: string
      value: "it's $(touch apostrophe-marker) done"
      tier: enforce
  killall: []
EOF
)"
rendered_apostrophe="$sandbox/rendered-apostrophe"
render_template "$apostrophe_src" "$rendered_apostrophe" "$render_error" ||
  fail "the apostrophe fixture must render (stderr: $(cat "$render_error"))"
: >"$stub_log"
(
  cd "$marker_dir" || exit 1
  PATH="$stub_bin:$PATH" bash "$rendered_apostrophe"
) || fail "the apostrophe render must run cleanly under the stubs (log: $(cat "$stub_log"))"
created_markers="$(find "$marker_dir" -mindepth 1 -print)"
[[ -z $created_markers ]] ||
  fail "a value containing a single quote must not break out of its quoting; created: $created_markers"
assert_file_contains "$stub_log" "it's \$(touch apostrophe-marker) done" \
  "the apostrophe value must arrive intact and literal (log: $(cat "$stub_log"))"

# ---- C: the template refuses targets it cannot vouch for --------------------

# A blank YAML scalar parses as nil, and rendering it stringifies to the literal
# text <nil>. That would be WRITTEN, as root on a system record. It is caught
# here rather than left to look like a value, and each field is asserted
# separately so a guard covering only one of them fails.
assert_render_rejects 'blank value' 'has a blank value' <<'EOF'
macos:
  defaults:
    - domain: com.example.blankvalue
      key: BlankValueKey
      type: string
      value:
      scope: system
      tier: enforce
  killall: []
EOF

assert_render_rejects 'blank key' 'has a blank key' <<'EOF'
macos:
  defaults:
    - domain: com.example.blankkey
      key:
      type: string
      value: v
      tier: enforce
  killall: []
EOF

assert_render_rejects 'blank domain' 'has a blank domain' <<'EOF'
macos:
  defaults:
    - domain:
      key: BlankDomainKey
      type: string
      value: v
      tier: enforce
  killall: []
EOF

# The distinction the guard above must NOT lose: an empty STRING is a legitimate
# value, not a missing one, and has to keep rendering. A guard written as a
# plain truthiness test would reject both and this case is what catches that.
empty_string_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.emptystring
      key: EmptyStringKey
      type: string
      value: ""
      scope: system
      tier: enforce
  killall: []
EOF
)"
rendered_empty_string="$sandbox/rendered-empty-string"
render_template "$empty_string_src" "$rendered_empty_string" "$render_error" ||
  fail "an empty-string value must still render (stderr: $(cat "$render_error"))"
assert_file_contains "$rendered_empty_string" "-string ''" \
  'an empty-string value must render as an empty quoted argument, not be rejected'

assert_render_rejects 'traversal domain' 'contains a slash' <<'EOF'
macos:
  defaults:
    - domain: ../../tmp/owned
      key: OwnedKey
      type: bool
      value: true
      scope: system
      tier: enforce
  killall: []
EOF

assert_render_rejects 'traversal plist_path' 'parent-directory component' <<'EOF'
macos:
  defaults:
    - domain: com.example.traverse
      key: TraverseKey
      type: bool
      value: true
      scope: system
      plist_path: /Library/Preferences/../../etc/owned.plist
      tier: enforce
EOF

assert_render_rejects 'plist_path on a user-scope record' 'outside scope system' <<'EOF'
macos:
  defaults:
    - domain: com.example.userpath
      key: UserPathKey
      type: bool
      value: true
      plist_path: /Library/Preferences/com.example.userpath.plist
      tier: enforce
  killall: []
EOF

assert_render_rejects 'empty domain' 'domain' <<'EOF'
macos:
  defaults:
    - domain: ""
      key: EmptyDomainKey
      type: bool
      value: true
      scope: system
      tier: enforce
  killall: []
EOF

assert_render_rejects 'dot domain' 'domain' <<'EOF'
macos:
  defaults:
    - domain: "."
      key: DotDomainKey
      type: bool
      value: true
      scope: system
      tier: enforce
  killall: []
EOF

assert_render_rejects 'dot-dot domain' 'domain' <<'EOF'
macos:
  defaults:
    - domain: ".."
      key: DotDotDomainKey
      type: bool
      value: true
      scope: system
      tier: enforce
  killall: []
EOF

assert_render_rejects 'root plist_path' 'plist_path' <<'EOF'
macos:
  defaults:
    - domain: com.example.root
      key: RootKey
      type: bool
      value: true
      scope: system
      plist_path: "/"
      tier: enforce
  killall: []
EOF

# Control for C: the real Objective-See path a later slice depends on stays
# ACCEPTED, so none of the rejections above were bought with a blanket refusal.
lulu_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.lulu
      key: LuLuKey
      type: string
      value: block
      scope: system
      plist_path: /Library/Objective-See/LuLu/preferences.plist
      tier: enforce
  killall: []
EOF
)"
render_template "$lulu_src" "$sandbox/rendered-lulu" "$render_error" ||
  fail "the tracked Objective-See path must still render (stderr: $(cat "$render_error"))"
assert_file_contains "$sandbox/rendered-lulu" '/Library/Objective-See/LuLu/preferences.plist' \
  'the tracked Objective-See path must be the write target'

# ---- D: one record, one write ------------------------------------------------

# run_apply <source-dir> <err-file> -- run apply with the stubs first on PATH,
# from inside the sandbox so a path bug lands somewhere observable.
run_apply() { # <source-dir> <err-file>
  (
    cd "$sandbox" || exit 1
    MACOS_DEFAULTS_SOURCE_DIR="$1" HOME="$sandbox/apply-home" \
      PATH="$stub_bin:$PATH" bash "$APPLY"
  ) 2>"$2"
}
mkdir -p "$sandbox/apply-home"

# Control: one benign system record yields exactly one write. Without this row
# the rejection below is satisfied by an apply that writes nothing ever.
one_record_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.sys
      key: SysKey
      type: string
      value: v
      scope: system
      tier: enforce
  killall: []
EOF
)"
: >"$stub_log"
run_apply "$one_record_src" "$sandbox/apply-benign.err" ||
  fail "apply must succeed on one benign system record (stderr: $(cat "$sandbox/apply-benign.err"))"
benign_write_count="$(grep -cF 'sudo [defaults] [write]' "$stub_log" || true)"
[[ $benign_write_count -eq 1 ]] ||
  fail "one system record must produce exactly one root write (got $benign_write_count; log: $(cat "$stub_log"))"

# The forging payload: unit separators plus a newline inside ONE record's value.
# It is BALANCED on purpose, so both halves carry exactly eight fields and a
# field-count check alone waves them through; only comparing the number of
# emitted lines against the number of DECLARED records catches it.
forging_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.sys
      key: SysKey
      type: string
      value: "v\x1f\x1fsystem\x1f\x1fenforce\nEVIL.DOMAIN\x1fEVILKEY\x1fbool\x1ftrue"
      scope: system
      tier: enforce
  killall: []
EOF
)"
forging_data_file="$forging_src/.chezmoidata/macos_defaults.yaml"

stream_status=0
stream_output="$(bash -c 'source "$1"; defaults_records_unit_separated "$2"' _ \
  "$LIB" "$forging_data_file" 2>"$sandbox/stream.err")" || stream_status=$?
[[ $stream_status -ne 0 ]] ||
  fail "the record stream must reject a forged record (got 0, stream: $(printf '%s' "$stream_output" | cat -v))"
assert_file_contains "$sandbox/stream.err" 'com.example.sys' \
  "the rejection must name the offending record (stderr: $(cat "$sandbox/stream.err"))"
refute_file_contains "$sandbox/stream.err" 'EVIL.DOMAIN' \
  'the rejection must name the real record, not the forged one it produced'

: >"$stub_log"
forging_status=0
run_apply "$forging_src" "$sandbox/apply-forging.err" || forging_status=$?
[[ $forging_status -ne 0 ]] ||
  fail "apply must refuse a data file carrying a forged record (log: $(cat "$stub_log"))"
refute_file_contains "$stub_log" 'EVIL.DOMAIN' \
  'apply must never perform the forged root write'
refute_file_contains "$stub_log" '[write]' \
  'apply must perform NO write at all once the record stream is known to be malformed'

# The template must refuse the same file, at the EARLIER boundary. It used to
# render it as one write and leave the tools to refuse, which meant one data file
# rendered cleanly and then made every tool fail; the operator met the problem
# through a broken drift report rather than at apply time. Both consumers now
# reject the same record, and this asserts the template is the one that says so
# first.
rendered_forging="$sandbox/rendered-forging"
if render_template "$forging_src" "$rendered_forging" "$render_error"; then
  fail "the forging fixture must FAIL the render, not be left for the tools to catch"
fi
assert_file_contains "$render_error" 'newline or a unit separator' \
  "the render's refusal must name the reason (stderr: $(cat "$render_error"))"
refute_file_contains "$rendered_forging" 'EVIL' \
  'a refused render must not leave the forged record in its output'

# A lone unit separator, no newline, is caught by the field count rather than by
# the line count, so both halves of the guard are exercised.
separator_only_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.sep
      key: SepKey
      type: string
      value: "a\x1fb"
      tier: enforce
  killall: []
EOF
)"
stream_status=0
bash -c 'source "$1"; defaults_records_unit_separated "$2"' _ \
  "$LIB" "$separator_only_src/.chezmoidata/macos_defaults.yaml" \
  >/dev/null 2>"$sandbox/stream-separator.err" || stream_status=$?
[[ $stream_status -ne 0 ]] ||
  fail 'the record stream must reject a value containing a bare unit separator'
assert_file_contains "$sandbox/stream-separator.err" 'com.example.sep' \
  "the bare-separator rejection must name the record (stderr: $(cat "$sandbox/stream-separator.err"))"

printf 'macos-defaults-injection: OK (a hostile type fails the render on both scopes with no write emitted; command substitution, backticks and $VAR in every data field arrive as literal text and execute nothing; traversal, user-scope and degenerate targets are refused while the tracked Objective-See path still renders; one record yields exactly one write and a separator/newline forgery is refused before anything is written)\n'
