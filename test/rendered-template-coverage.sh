#!/usr/bin/env bash
# rendered-template-coverage.sh. The treefmt rendered-template lint formatter
# must render and lint EVERY safely renderable shell template, not a hand-picked
# subset. The 2026-07-10 audit found the old hand-list covered 6 of ~20 shell
# templates, hiding four render failures.
#
# "Safely renderable" = every `*.sh.tmpl` plus the shell `dot_*.tmpl` (first line
# a shell shebang or a `# shellcheck shell=` directive, OR a Go-template
# directive whose first non-directive line is such a shebang), MINUS any
# template that itself, or a `.chezmoitemplates/` partial it includes, invokes
# keepassxc through a Go-template directive (those need an interactive KeePassXC
# unlock and cannot render in the headless Nix check sandbox).
#
# This test builds that universe independently and fails when a member is
# neither covered by the formatter nor listed in EXCLUDED below with a reason.
# It reads the formatter's ACTUAL include list straight from treefmt.nix via
# `nix eval` (with a stub `pkgs`), so it tracks the programmatic discovery
# exactly: red against the old 6-template hand-list, green once discovery covers
# them all. A fixture layer (test/fixtures/render-coverage) drives the SAME
# classifier against synthetic templates so shared discovery/test blind spots
# stay visible.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1 && pwd)"
cd "$REPO_ROOT" || exit 1

FIXTURE_DIR="test/fixtures/render-coverage"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# ---- classifier (shared by the universe enumeration and the fixture oracle) --

# A template invokes keepassxc only through a Go-template directive: a line
# containing `{{` followed (after optional trim markers, whitespace, pipes, or
# grouping parens) by `keepassxc`/`keepassxcAttribute`. A bare comment that
# merely mentions keepassxc does NOT count, so it stays covered. Mirrors the
# `directCallsKeepassxc` predicate in treefmt.nix.
line_calls_keepassxc() { # <file>
  grep -Eq '[{][{][-(|[:space:]]*keepassxc' "$1"
}

# includeTemplate "<literal>" partial names a template references (literal
# strings in this repo). Mirrors `includeTemplateNames` in treefmt.nix.
include_template_names() { # <file>
  grep -Eo 'includeTemplate[[:space:]]+"[^"]+"' "$1" | sed -E 's/.*"([^"]+)"/\1/'
}

# A template is unsafe if it, OR any `.chezmoitemplates/` partial it includes
# (one level; base overridable for the fixture oracle), calls keepassxc.
calls_keepassxc() { # <file> [include_base]
  local file="$1" base="${2:-.chezmoitemplates}" name partial
  line_calls_keepassxc "$file" && return 0
  while IFS= read -r -u3 name; do
    [[ -n $name ]] || continue
    partial="$base/$name"
    [[ -f $partial ]] || continue
    line_calls_keepassxc "$partial" && return 0
  done 3< <(include_template_names "$file")
  return 1
}

is_shell_shebang_line() { # <line>
  [[ $1 == '#!'*sh* || $1 == '# shellcheck shell='* ]]
}
is_go_directive_line() { # <line>
  [[ $1 =~ ^[[:space:]]*\{\{ ]]
}

# A template is a shell template when its first line is a shell shebang, OR its
# first line is a Go-template directive and its first NON-directive line is a
# shell shebang (the osquery-loader shape). Mirrors `isShellTemplate` in
# treefmt.nix.
is_shell_template() { # <file>
  local file="$1" first firstnon
  first="$(head -n 1 "$file")"
  is_shell_shebang_line "$first" && return 0
  if is_go_directive_line "$first"; then
    firstnon="$(grep -vE '^[[:space:]]*\{\{' "$file" | head -n 1)"
    is_shell_shebang_line "$firstnon" && return 0
  fi
  return 1
}

# ---- excluded set -----------------------------------------------------------

# Shell templates that are deliberately NOT standalone-covered. Key =
# repo-relative path; value = the reason. Each is re-validated below (must be a
# real, non-keepassxc shell template that the formatter does NOT cover) so this
# list cannot rot.
declare -A EXCLUDED=(
  [".chezmoitemplates/herdr-plugin-build.sh.tmpl"]='includeTemplate fragment: needs a (dict "id" ...) arg, so it never renders standalone; exercised through its includers run_onchange_after_55/57, which ARE covered'
  [".chezmoitemplates/herdr-health-check.sh.tmpl"]='includeTemplate fragment: defines a shell function with no standalone shebang entry point, so it never renders on its own; exercised through its covered includer run_after_58-herdr-migration-verify'
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
  if calls_keepassxc "$path"; then
    return 0
  fi
  safe+=("$path")
  safe_set["$path"]=1
}

# Every tracked *.sh.tmpl anywhere in the tree (fixtures excluded).
while IFS= read -r path; do
  [[ $path == "$FIXTURE_DIR"/* ]] && continue
  consider "$path"
done < <(git ls-files '*.sh.tmpl' | sort)

# Shell dot_*.tmpl (by basename), classified as a shell template (fixtures excluded).
while IFS= read -r path; do
  [[ $path == "$FIXTURE_DIR"/* ]] && continue
  [[ -f $path ]] || continue
  [[ $(basename "$path") == dot_*.tmpl ]] || continue
  is_shell_template "$path" && consider "$path"
done < <(git ls-files '*.tmpl' | sort)

[[ ${#safe[@]} -gt 0 ]] || fail "found no safely renderable shell templates (universe enumeration is broken)"

# The discovery does NOT scan extensionless executable_*.tmpl shell templates
# (none are tracked today). Assert that stays true, so the arrival of one forces
# a decision (add the shape to treefmt discovery + this universe, or keep it
# out) instead of silently escaping coverage. Fixtures are exempt.
declare -a stray_exec=()
while IFS= read -r path; do
  [[ $path == "$FIXTURE_DIR"/* ]] && continue
  [[ -f $path ]] || continue
  [[ $(basename "$path") == executable_*.tmpl ]] || continue
  [[ $(basename "$path") == *.sh.tmpl ]] && continue
  is_shell_template "$path" && stray_exec+=("$path")
done < <(git ls-files '*.tmpl' | sort)
if [[ ${#stray_exec[@]} -gt 0 ]]; then
  printf 'FAIL: extensionless executable_*.tmpl shell template(s) exist, but discovery does not scan that shape:\n' >&2
  printf '  - %s\n' "${stray_exec[@]}" >&2
  fail "decide: add executable_*.tmpl to treefmt discovery AND this universe, or keep them out of the shell set"
fi

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

# 2. No covered entry is a phantom, an untracked file, or an unsafe template.
for path in "${!covered[@]}"; do
  [[ -f $path ]] || fail "treefmt covers a non-existent file: $path"
  [[ -n ${safe_set["$path"]:-} ]] && continue
  if ! git ls-files --error-unmatch "$path" >/dev/null 2>&1; then
    fail "treefmt covers '$path', but it is an untracked template; git add it (the universe is built from git ls-files)"
  fi
  fail "treefmt covers '$path', which is not in the safely renderable universe (keepassxc caller, or not a shell template)"
done

# 3. Every EXCLUDED entry is real, safe, and genuinely uncovered (no stale rows).
for path in "${!EXCLUDED[@]}"; do
  [[ -f $path ]] || fail "EXCLUDED lists a non-existent file: $path"
  [[ -n ${safe_set["$path"]:-} ]] ||
    fail "EXCLUDED entry '$path' is not a safely renderable shell template; drop it"
  if [[ -n ${covered["$path"]:-} ]]; then
    fail "EXCLUDED entry '$path' is also covered by treefmt; remove it from EXCLUDED"
  fi
done

# 4. Fixture oracle: run the SAME classifier against synthetic templates so the
# vacuous discovery clauses (shell classification, keepassxc filter, transitive
# include) keep a permanent oracle. verdict = covered when the classifier would
# admit the template to the safe universe, excluded otherwise.
fixture_verdict() { # <file> <include_base>
  if calls_keepassxc "$1" "$2"; then
    printf 'excluded'
    return
  fi
  if [[ $1 == *.sh.tmpl ]] || is_shell_template "$1"; then
    printf 'covered'
    return
  fi
  printf 'excluded'
}

declare -A FIXTURE_EXPECT=(
  ["executable_hook.tmpl"]=covered
  ["dot_conditional.tmpl"]=covered
  ["covered_comment.sh.tmpl"]=covered
  ["excluded_keepassxc.sh.tmpl"]=excluded
  ["excluded_include.sh.tmpl"]=excluded
  ["plain_non_shell.tmpl"]=excluded
)

for name in "${!FIXTURE_EXPECT[@]}"; do
  file="$FIXTURE_DIR/$name"
  [[ -f $file ]] || fail "missing fixture: $file"
  got="$(fixture_verdict "$file" "$FIXTURE_DIR")"
  want="${FIXTURE_EXPECT[$name]}"
  [[ $got == "$want" ]] ||
    fail "fixture $name classified '$got', expected '$want' (classifier regressed)"
done

printf 'rendered-template-coverage: OK (%d safely renderable, %d covered, %d excluded-with-reason, %d fixtures)\n' \
  "${#safe[@]}" "${#covered[@]}" "${#EXCLUDED[@]}" "${#FIXTURE_EXPECT[@]}"
