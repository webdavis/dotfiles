#!/usr/bin/env bash
#
# run_after_50-setup-osquery.sh
# Converge /var/osquery to the desired state this repo deploys, on every apply.
# All of the work, and every decision, lives in the deployed tool
# (~/.local/libexec/osquery/osquery-converge.sh); this is the apply-time caller.
#
# THREE THINGS ABOUT THIS FILE'S NAME ARE LOAD-BEARING.
#
# NOT A TEMPLATE, deliberately, for the reason run_after_05 spells out: the
# mandated agent apply is `chezmoi apply --exclude=templates`, which skips every
# *.sh.tmpl script. The property this whole slice exists to hold is that an
# external wipe of /var/osquery is repaired by the next apply OF ANY FLAVOR, and
# a templated runner would be skipped by the flavor the operator runs daily.
# Darwin is therefore gated at runtime rather than with a Go-template guard.
#
# NOT run_onchange_, which is what it used to be. The desired state has moved
# out of this file into ordinary chezmoi targets, so this script's own content no
# longer changes when the osquery configuration does: an onchange runner would
# fire once and then never again, which is a slower version of the bug being
# fixed. A plain run_ script runs every apply, and the tool it calls is a silent
# no-op when nothing has drifted, so that costs an apply nothing.
#
# The AFTER phase, keeping the target name's `50`. The tool and the desired
# state are ordinary file targets, written while the target state is applied, so
# a before-phase runner would read the PREVIOUS apply's staging and would find
# neither of them at all on a fresh machine. Nothing in the osquery pipeline
# orders against this script, and run_after_05 already creates /var/osquery
# itself when it has to, so moving phases changes no ordering that exists.
#
# OUTPUT. The tool prints one line per repaired path and nothing at all when
# there is nothing to repair, so this script prints nothing of its own. It
# deliberately does NOT adopt the shared G2 section header from
# .chezmoitemplates/cli-print-style-lib.sh.tmpl: that library is inlined with
# includeTemplate, which would make this a template again and cost the
# any-flavor property above. Every line the tool prints names itself instead.
set -euo pipefail

[[ "$(uname)" == Darwin ]] || exit 0

converge="${CHEZMOI_HOME_DIR:-$HOME}/.local/libexec/osquery/osquery-converge.sh"

# A missing tool is stated rather than fatal: aborting the apply would stop every
# later script over a file this same apply is supposed to have deployed, and the
# next apply installs it. Loud, because a silently skipped converge is exactly
# the invisible drift this replaced.
if [[ ! -x $converge ]]; then
  printf '50-setup-osquery: %s is not deployed, so /var/osquery was NOT converged. It is a plain file, so any apply deploys it; re-run the apply.\n' \
    "$converge" >&2
  exit 0
fi

exec "$converge"
