#!/usr/bin/env bash
# Run on every apply, after builder 58 has installed posture and refreshed its
# manifest. The converge command repairs drift and is silent when nothing changed.
set -euo pipefail

[[ "$(uname)" == Darwin ]] || exit 0

converge="${CHEZMOI_HOME_DIR:-$HOME}/.cargo/bin/posture"
if [[ ! -x $converge ]]; then
  printf '59-setup-osquery: %s is not deployed, so /var/osquery was NOT converged. Resolve the deferred posture build and re-run the apply.\n' \
    "$converge" >&2
  exit 0
fi

exec "$converge" converge
