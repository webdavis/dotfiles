# shellcheck shell=bash
# Per-file body of the treefmt `shellcheck-rendered-template` formatter, factored
# out so test/unit/rendered-template-shellcheck-wrapper.sh can drive it with a
# stubbed render_template and shellcheck.
#
# Skip semantic: after a SUCCESSFUL render, a blank (empty or whitespace-only)
# result means an OS-gated template on the other OS has nothing to lint, so the
# lint step is skipped (an empty body would fail SC2148). A render FAILURE stays
# fatal.
#
# Usage: render_and_shellcheck_one <file>  -> 0 on ok/skip, non-zero on failure.
render_and_shellcheck_one() {
  local file="$1"
  local rendered
  if ! rendered="$(render_template "$file")"; then
    printf 'shellcheck-rendered-template: chezmoi render failed: %s\n' "$file" >&2
    return 1
  fi
  if [[ -z ${rendered//[[:space:]]/} ]]; then
    return 0
  fi
  if ! printf '%s\n' "$rendered" | shellcheck -; then
    printf 'shellcheck-rendered-template: rendered template failed shellcheck: %s\n' "$file" >&2
    return 1
  fi
  return 0
}
