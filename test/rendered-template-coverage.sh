#!/usr/bin/env bash
# rendered-template-coverage.sh — the treefmt `shellcheck-rendered-template`
# formatter must render+shellcheck EVERY safely renderable shell template, not a
# hand-picked subset. The 2026-07-10 audit found the old hand-list covered 6 of
# ~20 shell templates, hiding four render failures.
#
# "Safely renderable" = every `*.sh.tmpl` plus the shell `dot_*.tmpl` (first line
# a shell shebang or a `# shellcheck shell=` directive), MINUS any template that
# calls `keepassxc` (those need an interactive KeePassXC unlock and cannot render
# in the headless Nix check sandbox).
#
# This test builds that universe independently and fails when a member is neither
# covered by the formatter nor listed in EXCLUDED below with a reason. It reads
# the formatter's ACTUAL include list straight from treefmt.nix via `nix eval`
# (with a stub `pkgs`), so it tracks the programmatic discovery exactly: red
# against the old 6-template hand-list, green once discovery covers them all.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# Shell *.sh.tmpl templates that are deliberately NOT standalone-covered. Key =
# repo-relative path; value = the reason. Each is re-validated below (must be a
# real, non-keepassxc shell template that the formatter does NOT cover) so this
# list cannot rot.
declare -A EXCLUDED=(
  [".chezmoitemplates/herdr-plugin-build.sh.tmpl"]='includeTemplate fragment: needs a (dict "id" ...) arg, so it never renders standalone; exercised through its includers run_onchange_after_55/57, which ARE covered'
)

# Host-tool guards: plain test/*.sh scripts run with host tools.
for tool in nix git; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'SKIP: %s not on PATH; cannot check rendered-template coverage\n' "$tool"
    exit 0
  fi
done

# ---- universe: every safely renderable shell template -----------------------
declare -A seen=()
declare -a safe=()
declare -A safe_set=()

consider() { # add a candidate template to the universe once, dropping keepassxc callers
  local path="$1"
  [[ -n ${seen["$path"]:-} ]] && return 0
  seen["$path"]=1
  [[ -f $path ]] || return 0
  if grep -q 'keepassxc' "$path"; then
    return 0
  fi
  safe+=("$path")
  safe_set["$path"]=1
}

# Every tracked *.sh.tmpl anywhere in the tree.
while IFS= read -r path; do
  consider "$path"
done < <(git ls-files '*.sh.tmpl' | sort)

# Shell dot_*.tmpl (by basename), first line a shebang or a shellcheck directive.
while IFS= read -r path; do
  [[ $(basename "$path") == dot_*.tmpl ]] || continue
  first="$(head -n 1 "$path")"
  if [[ $first == '#!'*sh* || $first == '# shellcheck shell='* ]]; then
    consider "$path"
  fi
done < <(git ls-files '*.tmpl' | sort)

[[ ${#safe[@]} -gt 0 ]] || fail "found no safely renderable shell templates — universe enumeration is broken"

# ---- covered set: the formatter's ACTUAL includes, read from treefmt.nix -----
covered_raw="$(
  nix eval --impure --raw --expr \
    'builtins.concatStringsSep "\n" ((import ./treefmt.nix { pkgs = {}; }).settings.formatter.shellcheck-rendered-template.includes)'
)" || fail "nix eval of treefmt.nix shellcheck-rendered-template.includes failed"

declare -A covered=()
if [[ -n $covered_raw ]]; then
  while IFS= read -r path; do
    covered["$path"]=1
  done <<<"$covered_raw"
fi

[[ ${#covered[@]} -gt 0 ]] || fail "treefmt.nix declares no rendered-template includes at all"

# ---- assertions -------------------------------------------------------------

# 1. Every safely renderable template is covered OR excluded-with-reason.
declare -a missing=()
for path in "${safe[@]}"; do
  [[ -n ${covered["$path"]:-} ]] && continue
  [[ -n ${EXCLUDED["$path"]:-} ]] && continue
  missing+=("$path")
done
if [[ ${#missing[@]} -gt 0 ]]; then
  printf 'FAIL: %d shell template(s) neither covered by treefmt shellcheck-rendered-template nor EXCLUDED:\n' \
    "${#missing[@]}" >&2
  printf '  - %s\n' "${missing[@]}" >&2
  fail "extend the treefmt discovery to cover them, or add each to EXCLUDED with a documented reason"
fi

# 2. No covered entry is a phantom or an unsafe (keepassxc/non-shell) template.
for path in "${!covered[@]}"; do
  [[ -f $path ]] || fail "treefmt covers a non-existent file: $path"
  [[ -n ${safe_set["$path"]:-} ]] ||
    fail "treefmt covers '$path', which is not in the safely renderable universe (keepassxc caller, or not a shell template)"
done

# 3. Every EXCLUDED entry is real, safe, and genuinely uncovered (no stale rows).
for path in "${!EXCLUDED[@]}"; do
  [[ -f $path ]] || fail "EXCLUDED lists a non-existent file: $path"
  [[ -n ${safe_set["$path"]:-} ]] ||
    fail "EXCLUDED entry '$path' is not a safely renderable shell template — drop it"
  if [[ -n ${covered["$path"]:-} ]]; then
    fail "EXCLUDED entry '$path' is also covered by treefmt — remove it from EXCLUDED"
  fi
done

printf 'rendered-template-coverage: OK (%d safely renderable, %d covered, %d excluded-with-reason)\n' \
  "${#safe[@]}" "${#covered[@]}" "${#EXCLUDED[@]}"
