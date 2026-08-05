#!/usr/bin/env bash
# nested-chezmoi-dump-config-warning.sh: the nested chezmoi dump in
# run_after_05 must seed its throwaway persistent state with the config
# template's hash, or every full apply prints a false warning.
#
# WHY THIS EXISTS. Measured on dresden 2026-08-05: every `chezmoi apply`
# printed "config file template has changed, run chezmoi init to regenerate
# config file", and no `chezmoi init` ever cleared it. The emitter was not the
# apply but the nested dump inside run_after_05, which must run against a
# throwaway --persistent-state because the parent apply holds the real state
# lock. A fresh state has no configState record, and chezmoi's applyArgs warns
# whenever that record does not match the config template's hash (chezmoi
# v2.72.0, internal/cmd/config.go), so the nested dump warned unconditionally,
# at first-script time, indistinguishable from a startup warning. The fix seeds
# the throwaway state with the template's sha256 before dumping.
#
# Case 1 pins chezmoi's BEHAVIOR in a sandbox: unseeded dump warns, seeded dump
# is silent. If a future chezmoi stops warning here, case 1 goes red and the
# seeding is vestigial, which is exactly when a maintainer should remove it.
# Cases 2 and 3 pin OUR WIRING: the seed exists, targets the right bucket and
# key, and runs before the dump.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -r $SCRIPT ]] || fail "$SCRIPT is missing"
command -v chezmoi >/dev/null 2>&1 || fail "chezmoi is not on PATH"

# --- Case 1: chezmoi behavior, in a sandbox source with its own config
# template. No vault, no hooks, fully hermetic.
src="$work/src"
dest="$work/dest"
mkdir -p "$src" "$dest"
printf '[data]\n' >"$src/.chezmoi.toml.tmpl"
printf 'sandbox file\n' >"$src/dot_probe"

dump_warn_count() {
  # $1 = persistent-state path. Prints the number of config-template warning
  # lines the dump emits on stderr.
  chezmoi --source "$src" --destination "$dest" --persistent-state "$1" \
    dump --format=json 2>&1 >/dev/null | grep -c 'config file template has changed' || true
}

unseeded="$(dump_warn_count "$work/unseeded.boltdb")"
[[ $unseeded -eq 1 ]] ||
  fail "expected exactly 1 warning from a dump against an UNSEEDED throwaway state, saw $unseeded; if chezmoi stopped warning here, the seeding in run_after_05 is vestigial and this test should be retired with it"

template_sha256="$(shasum -a 256 "$src/.chezmoi.toml.tmpl" | awk '{print $1}')"
chezmoi --source "$src" --destination "$dest" --persistent-state "$work/seeded.boltdb" \
  state set --bucket=configState --key=configState \
  --value="{\"configTemplateContentsSHA256\":\"$template_sha256\"}" ||
  fail "could not seed the sandbox throwaway state"
seeded="$(dump_warn_count "$work/seeded.boltdb")"
[[ $seeded -eq 0 ]] ||
  fail "a dump against a SEEDED throwaway state still warned ($seeded line(s)); the seed value or key no longer matches what chezmoi compares"

# --- Case 2: the production script carries the seed, aimed at the right
# bucket and key, with the hash taken from the config template.
grep -Eq '^[[:space:]]*state set --bucket=configState --key=configState' "$SCRIPT" ||
  fail "run_after_05 no longer seeds configState into its throwaway dump state; the false warning returns on every apply"
grep -q 'configTemplateContentsSHA256' "$SCRIPT" ||
  fail "run_after_05's seed does not set configTemplateContentsSHA256, which is the key chezmoi compares"
grep -q '\.chezmoi\.toml\.tmpl' "$SCRIPT" ||
  fail "run_after_05's seed does not hash the config template itself"

# --- Case 3: the seed runs BEFORE the dump. A seed after the dump is the
# mutation that silently reintroduces the warning while cases 1 and 2 pass.
seed_line="$(grep -En '^[[:space:]]*state set --bucket=configState' "$SCRIPT" | head -1 | cut -d: -f1)"
dump_line="$(grep -n 'dump --format=json.*dump_json' "$SCRIPT" | head -1 | cut -d: -f1)"
[[ -n $seed_line && -n $dump_line && $seed_line -lt $dump_line ]] ||
  fail "the configState seed (line ${seed_line:-absent}) does not precede the dump (line ${dump_line:-absent})"

printf 'nested-chezmoi-dump-config-warning: OK (unseeded warns, seeded silent, seed wired before the dump)\n'
