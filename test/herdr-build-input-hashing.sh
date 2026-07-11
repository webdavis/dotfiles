#!/usr/bin/env bash
# herdr-build-input-hashing.sh: the rebuild-decision hash (embedded in the
# rendered run_onchange trigger) must cover EVERY input to the Rust build, so a
# change to any one forces a rebuild. The build partial
# (.chezmoitemplates/herdr-plugin-build.sh.tmpl) is includeTemplate'd into both
# run_onchange_after_55/57 scripts and pins the trigger to the hashes of the
# plugin source. The four inputs are:
#
#   src/main.rs          the plugin source
#   Cargo.lock           the resolved dependency graph
#   Cargo.toml           the manifest (deps, edition, package); a change here
#                        (e.g. a bumped dependency) MUST force a rebuild
#   herdr-plugin.toml    the plugin manifest (build/events/actions)
#
# This renders each plugin build script with the host chezmoi (same mechanics as
# the treefmt rendered-template lint: scratch HOME, CI=1) and asserts the render
# carries the sha256 of ALL FOUR inputs. If any input's hash is absent, changing
# that input would not change the rendered trigger and chezmoi would not
# rebuild: the exact defect (Cargo.toml omitted) this test guards against.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# script -> plugin id
SCRIPTS=(
  "run_onchange_after_55-build-herdr-last-workspace-plugin.sh.tmpl:herdr-last-workspace"
  "run_onchange_after_57-build-herdr-smart-nav-plugin.sh.tmpl:herdr-smart-nav"
)

# The build inputs, relative to a plugin dir. Every one must be hashed.
INPUTS=(
  "src/main.rs"
  "Cargo.lock"
  "Cargo.toml"
  "herdr-plugin.toml"
)

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# Host-tool guards: plain test/*.sh scripts run outside the Nix shell.
for tool in chezmoi shasum; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'SKIP: %s not on PATH; cannot render/hash the plugin build inputs\n' "$tool"
    exit 0
  fi
done

# chezmoi's `sha256sum` template function returns the lowercase hex digest, the
# same value `shasum -a 256` prints in its first field.
host_sha256() {
  shasum -a 256 "$1" | awk '{print $1}'
}

scratch_home="$(mktemp -d)"
trap 'rm -rf "$scratch_home"' EXIT

for pair in "${SCRIPTS[@]}"; do
  script_name="${pair%%:*}"
  plugin_id="${pair##*:}"
  script="$REPO_ROOT/.chezmoiscripts/$script_name"
  plugin_dir="$REPO_ROOT/dot_local/share/herdr/plugins/$plugin_id"
  [[ -f $script ]] || fail "missing template: $script"

  rendered="$(HOME="$scratch_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$script")" ||
    fail "chezmoi failed to render $script"
  [[ -n $rendered ]] || fail "empty render (non-darwin?): $script"

  for input in "${INPUTS[@]}"; do
    input_path="$plugin_dir/$input"
    [[ -f $input_path ]] || fail "$plugin_id: missing build input $input"
    hash="$(host_sha256 "$input_path")"
    grep -qF "$hash" <<<"$rendered" ||
      fail "$plugin_id: rendered trigger omits the hash of $input ($hash); a change to it would not force a rebuild"
  done
done

printf 'PASS: both plugin build triggers hash all four build inputs (main.rs, Cargo.lock, Cargo.toml, herdr-plugin.toml)\n'
