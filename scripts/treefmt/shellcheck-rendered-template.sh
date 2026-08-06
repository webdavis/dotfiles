#!/usr/bin/env bash
# treefmt formatter: render chezmoi shell templates and shellcheck the result.
#
# The old treefmt.nix discovered the safe-to-render set at nix eval time via
# scripts/render-coverage-classifier.nix. Standalone treefmt has no eval stage,
# so treefmt.toml hands this script EVERY .chezmoiscripts/*.sh.tmpl and root
# dot_*.tmpl, and the classification happens here per file:
#   - not a shell template (no shell shebang / shellcheck directive in the
#     leading lines, template directives skipped)      -> skip
#   - the file, or any .chezmoitemplates partial it includes transitively,
#     invokes keepassxc (needs an interactive unlock)  -> skip
#   - otherwise render and shellcheck (blank successful render = skip, render
#     failure = fatal), via scripts/treefmt/lib-shellcheck-rendered-template.sh.
set -uo pipefail

lib="$(dirname "${BASH_SOURCE[0]}")/lib-shellcheck-rendered-template.sh"
# shellcheck source=/dev/null
source "$lib"

# chezmoi needs a writable HOME (its read-source-state pre hook chdirs there).
HOME="$(mktemp -d)"
export HOME

is_shell_template() {
  local file="$1" line n=0
  while IFS= read -r line && ((n < 10)); do
    n=$((n + 1))
    # Pure template-directive or blank lines don't decide either way.
    [[ $line =~ ^[[:space:]]*$ ]] && continue
    [[ $line =~ ^\{\{.*\}\}[[:space:]]*$ ]] && continue
    [[ $line =~ ^#! ]] && { [[ $line =~ sh ]] && return 0 || return 1; }
    [[ $line =~ ^#[[:space:]]*shellcheck[[:space:]]+shell= ]] && return 0
    return 1
  done <"$file"
  return 1
}

# Transitive keepassxc scan: the file plus every includeTemplate partial it
# reaches. Grep is deliberately broad (any mention inside the file); the cost
# of over-matching is one skipped lint, never a false failure.
renders_unsafe() {
  local queue=("$1") seen=() f name partial
  while ((${#queue[@]})); do
    f="${queue[0]}"
    queue=("${queue[@]:1}")
    for s in "${seen[@]:-}"; do [[ $s == "$f" ]] && continue 2; done
    seen+=("$f")
    [[ -r $f ]] || continue
    grep -q 'keepassxc' "$f" && return 0
    while IFS= read -r name; do
      partial=".chezmoitemplates/$name"
      queue+=("$partial")
    done < <(grep -o 'includeTemplate "[^"]*"' "$f" | sed 's/includeTemplate "//; s/"$//')
  done
  return 1
}

status=0
for file; do
  is_shell_template "$file" || continue
  renders_unsafe "$file" && continue
  render_and_shellcheck_one "$file" || status=1
done
exit "$status"
