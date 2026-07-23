#!/usr/bin/env bash
#
# The pipeline-manifest runner (.chezmoiscripts/run_after_67-osquery-pipeline-
# manifest.sh.tmpl) regenerates the manifest from the real deployed tree on every
# apply and installs it root-owned, but only when the content actually changed - so
# a no-op apply does no privileged write. It runs as root via `sudo install`, which
# this test must NOT actually run: a PATH-shadowed `sudo` stub records the argv and
# emulates the copy without privilege, so the diff-guard and the exact ownership/
# mode are pinned without touching /var.
#
# Integration test (rendered runner, stubbed privilege): render the darwin-gated
# runner, point it at a fixture HOME (the deployed generator + a pipeline tree) and
# a scratch manifest dest, and drive three applies:
#   1. dest absent          -> installs (root:wheel 0644), dest = generator output;
#   2. tree unchanged        -> NO privileged write (the diff-guard holds);
#   3. one pipeline file edited -> installs again, dest refreshed.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNNER="$REPO_ROOT/.chezmoiscripts/run_after_67-osquery-pipeline-manifest.sh.tmpl"
GENERATOR_SRC="$REPO_ROOT/dot_local/libexec/osquery/executable_generate-pipeline-manifest.sh"

fail() {
  printf 'osquery-pipeline-manifest-install: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $RUNNER ]] || fail "missing runner: $RUNNER"
[[ -f $GENERATOR_SRC ]] || fail "missing generator: $GENERATOR_SRC"
command -v chezmoi >/dev/null 2>&1 || {
  printf 'osquery-pipeline-manifest-install: SKIP -- chezmoi not on PATH\n'
  exit 0
}

# Render the runner exactly as at apply time. On a non-darwin host it renders to
# nothing (the darwin gate), so there is nothing to drive: skip.
rendered="$(CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$RUNNER")" ||
  fail "chezmoi failed to render the runner"
if [[ -z ${rendered//[[:space:]]/} ]]; then
  printf 'osquery-pipeline-manifest-install: SKIP -- runner renders empty (non-darwin host)\n'
  exit 0
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
home="$work/home"
libexec="$home/.local/libexec/osquery"
agents="$home/Library/LaunchAgents"
mkdir -p "$libexec/results-alerter" "$agents" "$work/bin"

# The deployed generator (executable_ prefix dropped in the target) + a pipeline tree.
install -m 0755 "$GENERATOR_SRC" "$libexec/generate-pipeline-manifest.sh"
printf 'echo digest\n' >"$libexec/digest.sh"
printf 'true\n' >"$libexec/results-alerter/normalize.sh"
printf '<plist>digest</plist>\n' >"$agents/com.webdavis.osquery-digest.plist"

# PATH-shadowed sudo: record argv, then emulate `install ... <src> <dest>` by
# copying, so no real privilege is used and no /var write happens.
sudo_log="$work/sudo.log"
cat >"$work/bin/sudo" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$SUDO_LOG"
dest="${!#}"
src_pos=$(($# - 1))
src="${!src_pos}"
cp "$src" "$dest"
STUB
chmod +x "$work/bin/sudo"

runner="$work/runner.sh"
printf '%s\n' "$rendered" >"$runner"
dest="$work/pipeline-known-good.sha256"

# apply <fresh-log>: run the rendered runner with the stub sudo first on PATH and
# the manifest dest redirected into the scratch dir.
apply() {
  : >"$sudo_log"
  HOME="$home" PATH="$work/bin:$PATH" \
    OSQUERY_PIPELINE_MANIFEST="$dest" SUDO_LOG="$sudo_log" \
    bash "$runner" || fail "the runner exited non-zero"
}

# 1. First apply: dest is absent -> a privileged install of the freshly generated
#    manifest, root:wheel 0644.
apply
[[ -s $sudo_log ]] || fail "first apply did not install the manifest (no sudo call)"
grep -qF -- 'install -o root -g wheel -m 0644' "$sudo_log" ||
  fail "the install is not root:wheel 0644 (sudo argv: $(cat "$sudo_log"))"
[[ -f $dest ]] || fail "first apply did not create the manifest dest"
diff <(HOME="$home" "$libexec/generate-pipeline-manifest.sh") "$dest" >/dev/null ||
  fail "the installed manifest does not equal the generator's output"

# 2. Second apply, nothing changed: the diff-guard must skip the privileged write.
apply
[[ ! -s $sudo_log ]] ||
  fail "an unchanged tree still triggered a privileged write (sudo argv: $(cat "$sudo_log"))"

# 3. Edit one pipeline file: the manifest content changes -> install fires again.
printf 'echo changed\n' >>"$libexec/digest.sh"
apply
[[ -s $sudo_log ]] ||
  fail "a changed pipeline file did not trigger a re-install (the diff-guard is stuck)"
diff <(HOME="$home" "$libexec/generate-pipeline-manifest.sh") "$dest" >/dev/null ||
  fail "after an edit the installed manifest was not refreshed to the new content"

printf 'osquery-pipeline-manifest-install: OK (installs root:wheel 0644 on first apply; the diff-guard skips the privileged write when unchanged; re-installs after a pipeline file changes)\n'
