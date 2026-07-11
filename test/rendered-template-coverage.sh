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
# them all. A fixture layer (test/fixtures/render-coverage) drives the classifier
# against synthetic templates BOTH ways — the bash mirror in this file and the
# production Nix predicates in scripts/render-coverage-classifier.nix (via
# `nix eval`) — so weakening either side fails a fixture instead of passing
# silently.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1 && pwd)"
cd "$REPO_ROOT" || exit 1

FIXTURE_DIR="test/fixtures/render-coverage"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# ---- classifier (shared by the universe enumeration and the fixture oracle) --

# A line invokes keepassxc when keepassxc/keepassxcAttribute appears ANYWHERE
# inside a Go-template action `{{ ... }}` (not only as the first token, so
# `{{ $e := keepassxc "x" }}` counts), and the line is not a Go-template comment
# (`{{/* ... */}}`, in any trim form). A bare shell `#` comment that merely names
# keepassxc carries no `{{` and does not count, so it stays covered. Mirrors
# `lineCallsKeepassxc` in scripts/render-coverage-classifier.nix.
line_calls_keepassxc() { # <file>
  local hits
  hits="$(grep -E '[{][{][^}]*keepassxc' "$1" 2>/dev/null |
    grep -vE '^[[:space:]]*[{][{]-?[[:space:]]*/[*]' || true)"
  [[ -n $hits ]]
}

# Parse includeTemplate directives, one emitted line per directive:
#   L<TAB><name>  a double-quoted OR backtick raw-string literal partial name
#   D             a name that is not a static string literal (a $var, (expr), or
#                 .field) and so cannot be resolved statically
# Anchored on `{{` so prose mentioning includeTemplate inside a comment body
# (which carries no `{{` on that line) is not treated as a directive. Mirrors
# `parseIncludeLine`/`includeDirectives` in the classifier.
include_directives() { # <file>
  local inc_re='[{][{]-?[[:space:]]*includeTemplate[[:space:]]+(.*)'
  # SC2016: the backtick in bt_re is a literal ERE atom (backtick raw-string
  # delimiter), deliberately not command substitution.
  # shellcheck disable=SC2016
  local dq_re='^"([^"]*)"' bt_re='^`([^`]*)`'
  local line rest
  while IFS= read -r line; do
    [[ $line =~ $inc_re ]] || continue
    rest="${BASH_REMATCH[1]}"
    if [[ $rest =~ $dq_re ]]; then
      printf 'L\t%s\n' "${BASH_REMATCH[1]}"
    elif [[ $rest =~ $bt_re ]]; then
      printf 'L\t%s\n' "${BASH_REMATCH[1]}"
    else
      printf 'D\n'
    fi
  done <"$1"
}

# A template cannot render headless (is UNSAFE) when it, or any partial it
# includeTemplates transitively (literal names resolved against <base>), calls
# keepassxc, OR when any include name is dynamic (unresolvable). Cycle-protected
# via a visited set so a cyclic include pair terminates. Mirrors `rendersUnsafe`
# in scripts/render-coverage-classifier.nix.
renders_unsafe() { # <file> [include_base]
  local -A _visited=()
  _renders_unsafe "$1" "${2:-.chezmoitemplates}"
}
_renders_unsafe() { # <file> <base>  (shares _visited with renders_unsafe)
  local file="$1" base="$2" line kind name partial
  [[ -n ${_visited["$file"]:-} ]] && return 1
  _visited["$file"]=1
  line_calls_keepassxc "$file" && return 0
  while IFS= read -r -u3 line; do
    kind="${line%%$'\t'*}"
    if [[ $kind == D ]]; then
      return 0
    fi
    name="${line#*$'\t'}"
    partial="$base/$name"
    [[ -f $partial ]] || continue
    _renders_unsafe "$partial" "$base" && return 0
  done 3< <(include_directives "$file")
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
  if renders_unsafe "$path"; then
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

# 4. Fixture oracle: the SAME classifier runs THREE ways per fixture and all
# three must agree — the expected verdict, the bash mirror above, and the
# PRODUCTION Nix classifier (scripts/render-coverage-classifier.nix) evaluated
# via `nix eval`. Driving the real Nix predicates here (not just the bash mirror)
# is what makes weakening EITHER side fail the matrix instead of passing
# silently. verdict = covered when a shell template can render headless, excluded
# otherwise.
fixture_verdict() { # <file> <include_base>
  if renders_unsafe "$1" "$2"; then
    printf 'excluded'
    return
  fi
  if [[ $1 == *.sh.tmpl ]] || is_shell_template "$1"; then
    printf 'covered'
    return
  fi
  printf 'excluded'
}

# The production Nix classifier's verdict for a fixture, read straight from
# scripts/render-coverage-classifier.nix so a weakened Nix predicate fails here.
nix_classify() { # <fixture_file>
  nix eval --impure --raw --expr \
    "(import ./scripts/render-coverage-classifier.nix).classify (./${FIXTURE_DIR}) (./$1)"
}

declare -A FIXTURE_EXPECT=(
  ["executable_hook.tmpl"]=covered
  ["dot_conditional.tmpl"]=covered
  ["covered_comment.sh.tmpl"]=covered
  ["excluded_keepassxc.sh.tmpl"]=excluded
  ["excluded_include.sh.tmpl"]=excluded
  ["plain_non_shell.tmpl"]=excluded
  ["assignment_keepassxc.sh.tmpl"]=excluded
  ["raw_string_include.sh.tmpl"]=excluded
  ["chain_root.sh.tmpl"]=excluded
  ["dynamic_include.sh.tmpl"]=excluded
  ["cyclic_a.sh.tmpl"]=covered
)

for name in "${!FIXTURE_EXPECT[@]}"; do
  file="$FIXTURE_DIR/$name"
  [[ -f $file ]] || fail "missing fixture: $file"
  want="${FIXTURE_EXPECT[$name]}"
  got_bash="$(fixture_verdict "$file" "$FIXTURE_DIR")"
  [[ $got_bash == "$want" ]] ||
    fail "fixture $name: bash mirror classified '$got_bash', expected '$want' (bash classifier regressed)"
  got_nix="$(nix_classify "$file")" ||
    fail "fixture $name: nix eval of the classifier failed"
  [[ $got_nix == "$want" ]] ||
    fail "fixture $name: Nix classifier classified '$got_nix', expected '$want' (Nix classifier regressed)"
done

printf 'rendered-template-coverage: OK (%d safely renderable, %d covered, %d excluded-with-reason, %d fixtures bash+nix)\n' \
  "${#safe[@]}" "${#covered[@]}" "${#EXCLUDED[@]}" "${#FIXTURE_EXPECT[@]}"
