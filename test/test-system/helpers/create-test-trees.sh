# shellcheck shell=bash
# Make throwaway test/ folder structures for the test-system suite, so a test can
# drive the checker or runner against a scratch tree. Sourced; no main.

# make_test_tree <parent> <name> [subdir...] -- create <parent>/<name>/test with
# the named subdirs (each a path under test/, e.g. unit or fixtures/lib) and
# print the path to that test/ dir.
make_test_tree() {
  local parent="$1" name="$2"
  shift 2
  local root="$parent/$name/test"
  mkdir -p "$root"
  local subdir
  for subdir in "$@"; do
    mkdir -p "$root/$subdir"
  done
  printf '%s\n' "$root"
}
