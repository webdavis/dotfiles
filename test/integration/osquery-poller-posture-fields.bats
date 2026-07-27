#!/usr/bin/env bats
# The security-posture poller's DECLARED controls (slice 6): the poller reads
# ~/.local/libexec/osquery/posture-controls.json (the chezmoi render of
# .chezmoidata/macos_posture_controls.yaml) and monitors each record with the
# reader it names, against the value it declares. Adding a control is a data
# change; the poller carries no control list in its body.
#
# The rules pinned here:
#   - a control deviating from its declared value pages EXACTLY ONE CRIT naming
#     it, stays quiet while the deviation persists, and re-pages after a
#     restore-then-regress (the persisted baseline is the page-once marker);
#   - a probe that exits nonzero is INDETERMINATE regardless of what it printed
#     (unit suite: macos-posture-indeterminate.sh asserts this per control);
#   - a missing/malformed/mis-tiered controls file, an unknown reader, and an
#     unparseable or ambiguous probe are all MONITORING GAPS: page once,
#     preserve the baseline, never a silent pass (fail-open is the cardinal
#     sin);
#   - a non-verify tier is refused BEFORE any probe runs: the poller only READS
#     controls, so an enforce record in its file is a declaration error;
#   - the poller never invokes a mutating command (recording spies + an empty
#     violation log), and no system-read value reaches a notification body
#     unneutralized.

load ../fixtures/osquery-poller-lib

setup() { setup_poller_harness; }
teardown() { teardown_poller_harness; }

# The four repo-like records (same ids/readers/expects as
# .chezmoidata/macos_posture_controls.yaml; remedies shortened). The
# poller-vs-repo-data agreement lives in macos-posture-controls-agreement.sh.
declare_posture_controls() {
  set_posture_controls '[
    {"id":"filevault","description":"FileVault disk encryption","tier":"verify","reader":"fdesetup_status","expect":"on","remedy":"Re-enable it: System Settings, Privacy & Security, FileVault"},
    {"id":"sip","description":"System Integrity Protection","tier":"verify","reader":"csrutil_status","expect":"disabled","remedy":"Update the declared expect or investigate"},
    {"id":"autologin","description":"Automatic login at the login window","tier":"verify","reader":"defaults_autologin","expect":"off","remedy":"Turn it off: System Settings, Users & Groups"},
    {"id":"guest","description":"The macOS Guest account","tier":"verify","reader":"sysadminctl_guest","expect":"disabled","remedy":"Disable it: System Settings, Users & Groups"}
  ]'
}

# A baseline where every legacy field and every declared control is healthy.
# Each control persists as a value plus the "<id>:expect" it was recorded
# under, so a changed declaration re-arms the control (first observation).
healthy_seed='{"firewall":"1","gatekeeper":"1","screenlock":"1","filevault":"on","filevault:expect":"on","sip":"disabled","sip:expect":"disabled","autologin":"off","autologin:expect":"off","guest":"disabled","guest:expect":"disabled"}'

@test "T-PCTL-healthy-reads-and-baseline: a healthy tick reads every declared control once and persists each value into the baseline" {
  declare_posture_controls
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected exit 0 on a healthy tick, got $status: $output"
    false
  }

  assert_no_page
  # One status probe per declared control.
  assert_probe_calls fdesetup 1
  assert_probe_calls csrutil 1
  assert_probe_calls sysadminctl 1
  assert_probe_calls defaults 1
  # The baseline carries the legacy trio AND one field per declared control.
  assert_baseline_scalar firewall 1
  assert_baseline_scalar filevault on
  assert_baseline_scalar sip disabled
  assert_baseline_scalar autologin off
  assert_baseline_scalar guest disabled
}

@test "T-PCTL-legacy-baseline-gains-control-fields: a valid legacy-only baseline plus healthy controls stays silent and gains the control fields (the add-a-control-later path)" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  declare_posture_controls
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected exit 0, got $status: $output"
    false
  }

  assert_no_page # healthy first observation of the new controls: silent seed
  assert_baseline_scalar filevault on
  assert_baseline_scalar guest disabled
}

# --- one page per regression, quiet while it persists, re-page after restore ---
# The persisted baseline is the page-once marker: a deviation pages on the
# transition tick, the baseline advances to the deviant value (quiet ticks
# follow), a restore advances it back (clearing the marker), and a second
# regression pages again.

@test "T-PCTL-filevault-lifecycle: FileVault off pages one CRIT naming it, stays quiet while off, and re-pages after restore-then-regress" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  export POLLER_FDESETUP_OUTPUT="FileVault is Off."
  run run_poller # tick 1: the regression pages
  [[ $status -eq 0 ]] || {
    echo "tick 1 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has '`FileVault disk encryption`: now off, declared on'
  assert_page_body_has 'Re-enable it: System Settings, Privacy & Security, FileVault'
  assert_page_body_lacks 'Guest'            # only the control that changed is named
  assert_page_body_lacks 'Automatic login'
  # Notify-before-persist: at page time the baseline still held the prior value.
  assert_page_saw_baseline "$healthy_seed"
  assert_baseline_scalar filevault off

  run run_poller # tick 2: still off, quiet
  [[ $status -eq 0 ]] || {
    echo "tick 2 status $status: $output"
    false
  }
  assert_page_count 1

  unset POLLER_FDESETUP_OUTPUT
  run run_poller # tick 3: restored, silent, the marker (baseline) clears
  [[ $status -eq 0 ]] || {
    echo "tick 3 status $status: $output"
    false
  }
  assert_page_count 1
  assert_baseline_scalar filevault on

  export POLLER_FDESETUP_OUTPUT="FileVault is Off."
  run run_poller # tick 4: a later regression pages again
  [[ $status -eq 0 ]] || {
    echo "tick 4 status $status: $output"
    false
  }
  assert_page_count 2
}

@test "T-PCTL-filevault-restart-forms-classify: the real fdesetup restart-transition outputs classify explicitly instead of gapping forever" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  # Deferred enablement: the data is NOT yet encrypted, so this is off, a
  # real deviation page, never a permanently-indeterminate monitoring gap.
  export POLLER_FDESETUP_OUTPUT="FileVault is Off, but will be enabled after the next restart."
  run run_poller
  [[ $status -eq 0 ]] || {
    echo "tick 1 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_body_has '`FileVault disk encryption`: now off, declared on'
  assert_page_body_lacks 'monitoring gap'
  assert_baseline_scalar filevault off

  export POLLER_FDESETUP_OUTPUT="FileVault is On, but needs to be restarted to finish."
  run run_poller # an On transition form: classifies on, silent recovery
  [[ $status -eq 0 ]] || {
    echo "tick 2 status $status: $output"
    false
  }
  assert_page_count 1
  assert_baseline_scalar filevault on

  export POLLER_FDESETUP_OUTPUT="FileVault is Off, but needs to be restarted to finish."
  run run_poller # the Off transition form: a regression again
  [[ $status -eq 0 ]] || {
    echo "tick 3 status $status: $output"
    false
  }
  assert_page_count 2
  assert_baseline_scalar filevault off
}

@test "T-PCTL-sip-lifecycle: SIP deviating from its DECLARED state (disabled) pages once, stays quiet, and re-pages after restore-then-regress" {
  # SIP is deliberately disabled on this machine: expect is the operator's
  # declaration, not a blanket enabled. SIP turning ON is therefore the
  # deviation here.
  seed_baseline "$healthy_seed"
  declare_posture_controls
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  export POLLER_CSRUTIL_OUTPUT="System Integrity Protection status: enabled."
  run run_poller
  [[ $status -eq 0 ]] || {
    echo "tick 1 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has '`System Integrity Protection`: now enabled, declared disabled'
  assert_page_saw_baseline "$healthy_seed"
  assert_baseline_scalar sip enabled

  run run_poller # still deviant, quiet
  assert_page_count 1

  unset POLLER_CSRUTIL_OUTPUT
  run run_poller # restored to the declared state, silent
  assert_page_count 1
  assert_baseline_scalar sip disabled

  export POLLER_CSRUTIL_OUTPUT="System Integrity Protection status: enabled."
  run run_poller
  assert_page_count 2
}

@test "T-PCTL-autologin-lifecycle: a DECLARED auto-login user pages once naming it, stays quiet, and re-pages after restore-then-regress" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  export POLLER_DEFAULTS_AUTOLOGIN_MODE=present # autoLoginUser declared: stephen
  run run_poller
  [[ $status -eq 0 ]] || {
    echo "tick 1 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has '`Automatic login at the login window`: now on, declared off'
  # The username from the probe output is data the page must NOT carry.
  assert_page_body_lacks 'stephen'
  assert_baseline_scalar autologin on

  run run_poller
  assert_page_count 1

  unset POLLER_DEFAULTS_AUTOLOGIN_MODE # back to absent: the declaration removed
  run run_poller
  assert_page_count 1
  assert_baseline_scalar autologin off

  export POLLER_DEFAULTS_AUTOLOGIN_MODE=present
  run run_poller
  assert_page_count 2
}

@test "T-PCTL-autologin-reads-declared-intent: a declared auto-login pages even while FileVault forces manual login (the effective state is not the question)" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
  # autoLoginUser IS set while sysadminctl would report "Automatic login is
  # disabled because FileVault is enabled." (a FileVault-driven message printed
  # regardless of the declaration; verified in the binary's strings). An
  # effective-state reader reads this machine healthy, and the auto-login
  # activates unflagged the moment FileVault goes off. The control means
  # "auto-login is not DECLARED", so the declaration must win.
  export POLLER_DEFAULTS_AUTOLOGIN_MODE=present
  export POLLER_SYSADMINCTL_AUTOLOGIN_OUTPUT="2026-07-27 00:00:00.000 sysadminctl[100:100] Automatic login is disabled because FileVault is enabled."

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_body_has '`Automatic login at the login window`: now on, declared off'
}

@test "T-PCTL-autologin-unreadable-gaps: a defaults failure that is NOT the canonical absent diagnostic is indeterminate, never a silent healthy" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  snapshot_baseline
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
  # Nonzero exit without "autoLoginUser) does not exist": absent must be
  # distinguished from unreadable, so this is a monitoring gap.
  export POLLER_DEFAULTS_AUTOLOGIN_MODE=unreadable

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_body_has 'monitoring gap'
  assert_page_body_has 'autologin'
  assert_baseline_unchanged
}

@test "T-PCTL-guest-lifecycle: the Guest account turning on pages once naming it, stays quiet, and re-pages after restore-then-regress" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  export POLLER_SYSADMINCTL_GUEST_OUTPUT="2026-07-27 00:00:00.000 sysadminctl[100:100] Guest account enabled."
  run run_poller
  [[ $status -eq 0 ]] || {
    echo "tick 1 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has '`The macOS Guest account`: now enabled, declared disabled'
  assert_baseline_scalar guest enabled

  run run_poller
  assert_page_count 1

  unset POLLER_SYSADMINCTL_GUEST_OUTPUT
  run run_poller
  assert_page_count 1
  assert_baseline_scalar guest disabled

  export POLLER_SYSADMINCTL_GUEST_OUTPUT="2026-07-27 00:00:00.000 sysadminctl[100:100] Guest account enabled."
  run run_poller
  assert_page_count 2
}

@test "T-PCTL-first-observation-deviation-pages: a control already deviant with no prior baseline pages as a first observation, then seeds and quiets" {
  declare_posture_controls
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
  export POLLER_FDESETUP_OUTPUT="FileVault is Off."

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "tick 1 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has '`FileVault disk encryption`: off at first observation, declared on'
  assert_page_body_lacks 'now off' # a first observation, not a transition
  assert_baseline_scalar filevault off

  run run_poller # identical next tick: seeded, steady-deviant, silent
  assert_page_count 1
}

@test "T-PCTL-expect-change-rearms-control: changing a declared expect makes the next tick a first observation, never a silent steady-deviant" {
  # Baseline and live SIP are both disabled, recorded under expect=disabled;
  # the operator then TIGHTENS the declaration to expect=enabled. The prior
  # was recorded under the OLD declaration, so it must not read as
  # steady-deviant: that would turn a hardening change into a silent no-op.
  seed_baseline "$healthy_seed"
  set_posture_controls '[
    {"id":"filevault","description":"FileVault disk encryption","tier":"verify","reader":"fdesetup_status","expect":"on","remedy":"Re-enable it: System Settings, Privacy & Security, FileVault"},
    {"id":"sip","description":"System Integrity Protection","tier":"verify","reader":"csrutil_status","expect":"enabled","remedy":"Update the declared expect or investigate"},
    {"id":"autologin","description":"Automatic login at the login window","tier":"verify","reader":"defaults_autologin","expect":"off","remedy":"Turn it off: System Settings, Users & Groups"},
    {"id":"guest","description":"The macOS Guest account","tier":"verify","reader":"sysadminctl_guest","expect":"disabled","remedy":"Disable it: System Settings, Users & Groups"}
  ]'
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller # tick 1: live sip=disabled deviates from the NEW declaration
  [[ $status -eq 0 ]] || {
    echo "tick 1 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has '`System Integrity Protection`: disabled at first observation, declared enabled'
  assert_baseline_scalar sip disabled
  assert_baseline_scalar 'sip:expect' enabled # the new declaration is recorded

  run run_poller # tick 2: recorded under the new declaration: steady-deviant, silent
  [[ $status -eq 0 ]] || {
    echo "tick 2 status $status: $output"
    false
  }
  assert_page_count 1
}

@test "T-PCTL-out-of-domain-control-prior-distrusted: an out-of-domain prior for one control becomes a first observation, never a fabricated transition" {
  # A prior of "wombat" is outside the fdesetup_status domain. Trusting it
  # would fabricate a "now off" transition line reading Was: wombat; the prior
  # is distrusted instead, so this is a first observation of an already-off
  # control.
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1","filevault":"wombat","filevault:expect":"on","sip":"disabled","sip:expect":"disabled","autologin":"off","autologin:expect":"off","guest":"disabled","guest:expect":"disabled"}'
  declare_posture_controls
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
  export POLLER_FDESETUP_OUTPUT="FileVault is Off."

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_page_body_has 'off at first observation'
  assert_page_body_lacks 'wombat' # the untrusted prior never reaches a page
}

@test "T-PCTL-multi-deviation-single-page: a legacy protection and a declared control regressing in one tick share a single counted CRIT page" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  set_posture '[{"firewall":"0","gatekeeper":"1","screenlock":"1"}]'
  export POLLER_FDESETUP_OUTPUT="FileVault is Off."

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }

  assert_page_count 1 # one page for the tick, never one per control
  assert_page_body_has 'Firewall turned OFF'
  assert_page_body_has '`FileVault disk encryption`: now off, declared on'
  assert_page_body_has '· 2' # the count title marks two deviations in one page
}

# --- gaps: a control the poller cannot read or trust is NEVER a silent pass ---

@test "T-PCTL-missing-controls-file-gaps: a missing controls file pages a monitoring gap and preserves the baseline" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  snapshot_baseline
  rm "$OSQUERY_POSTURE_CONTROLS"
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }

  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'monitoring gap'
  assert_page_body_has 'posture-controls file missing'
  assert_gap_marker
  assert_baseline_unchanged # a blind poller must never advance state
}

@test "T-PCTL-malformed-controls-file-gaps: a controls file that is not a JSON array pages a monitoring gap" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  snapshot_baseline
  set_posture_controls '{"not":"an array"}'
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }

  assert_page_count 1
  assert_page_body_has 'monitoring gap'
  assert_page_body_has 'not a JSON array'
  assert_baseline_unchanged
}

@test "T-PCTL-multidoc-controls-file-gaps: a controls file holding two top-level arrays pages a gap instead of silently monitoring zero controls" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  snapshot_baseline
  # Two JSON documents: an empty array, then the real record list. Each parses
  # alone, so a per-document validation passes both; only a whole-file
  # single-array rule can refuse this shape, under which every declared
  # control would otherwise silently vanish.
  printf '%s\n%s\n' '[]' '[{"id":"guest","description":"The macOS Guest account","tier":"verify","reader":"sysadminctl_guest","expect":"disabled"}]' \
    >"$OSQUERY_POSTURE_CONTROLS"
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }

  assert_page_count 1
  assert_page_body_has 'monitoring gap'
  assert_page_body_has 'not a JSON array'
  assert_no_probe_calls # the refused file is refused whole, before any read
  assert_baseline_unchanged
}

@test "T-PCTL-nonverify-tier-refused-before-reads: an enforce-tier record pages a gap naming it and no probe ever runs" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  snapshot_baseline
  set_posture_controls '[{"id":"guest","description":"The macOS Guest account","tier":"enforce","reader":"sysadminctl_guest","expect":"disabled"}]'
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }

  assert_page_count 1
  assert_page_body_has 'monitoring gap'
  assert_page_body_has 'guest'
  assert_page_body_has 'not verify'
  assert_no_probe_calls # refusal precedes every read
  assert_baseline_unchanged
}

@test "T-PCTL-unknown-reader-gaps: a record naming a reader the poller lacks pages a gap and no probe ever runs" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  snapshot_baseline
  set_posture_controls '[{"id":"guest","description":"The macOS Guest account","tier":"verify","reader":"wombat_status","expect":"disabled"}]'
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }

  assert_page_count 1
  assert_page_body_has 'monitoring gap'
  assert_page_body_has 'unknown reader'
  assert_no_probe_calls
  assert_baseline_unchanged
}

@test "T-PCTL-duplicate-id-gaps: two records sharing an id page a gap (ids are baseline field names)" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  snapshot_baseline
  set_posture_controls '[
    {"id":"guest","description":"The macOS Guest account","tier":"verify","reader":"sysadminctl_guest","expect":"disabled"},
    {"id":"guest","description":"A second guest record","tier":"verify","reader":"sysadminctl_guest","expect":"disabled"}
  ]'
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }

  assert_page_count 1
  assert_page_body_has 'monitoring gap'
  assert_page_body_has 'collides'
  assert_baseline_unchanged
}

@test "T-PCTL-builtin-id-collision-gaps: a record whose id shadows a built-in field pages a gap" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  snapshot_baseline
  set_posture_controls '[{"id":"firewall","description":"A collision","tier":"verify","reader":"sysadminctl_guest","expect":"disabled"}]'
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }

  assert_page_count 1
  assert_page_body_has 'monitoring gap'
  assert_page_body_has 'collides'
  assert_baseline_unchanged
}

@test "T-PCTL-out-of-domain-expect-gaps: a record expecting a value outside its reader domain pages a gap" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  snapshot_baseline
  set_posture_controls '[{"id":"guest","description":"The macOS Guest account","tier":"verify","reader":"sysadminctl_guest","expect":"wombat"}]'
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }

  assert_page_count 1
  assert_page_body_has 'monitoring gap'
  assert_page_body_has 'outside'
  assert_baseline_unchanged
}

@test "T-PCTL-unparseable-probe-gaps: a zero-exit probe whose output matches no known state is indeterminate and pages a gap naming the control" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  snapshot_baseline
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
  export POLLER_FDESETUP_OUTPUT="FileVault is Wombat."

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }

  assert_page_count 1
  assert_page_body_has 'monitoring gap'
  assert_page_body_has 'filevault'
  assert_baseline_unchanged
}

@test "T-PCTL-ambiguous-probe-gaps: a probe printing BOTH state needles is indeterminate (never guessed at) and pages a gap" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  snapshot_baseline
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
  export POLLER_FDESETUP_OUTPUT=$'FileVault is On.\nFileVault is Off.'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }

  assert_page_count 1
  assert_page_body_has 'monitoring gap'
  assert_page_body_has 'filevault'
  assert_baseline_unchanged
}

@test "T-PCTL-gap-on-one-control-never-blinds-another: a SIP probe outage never suppresses a FileVault regression on a later tick (per-member gaps)" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  export POLLER_CSRUTIL_EXIT=1
  run run_poller # tick 1: the SIP probe fails, one gap page naming sip
  [[ $status -eq 0 ]] || {
    echo "tick 1 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_body_has 'sip'
  assert_gap_marker

  export POLLER_FDESETUP_OUTPUT="FileVault is Off."
  run run_poller # tick 2: SIP still failing AND FileVault regresses: the regression pages
  [[ $status -eq 0 ]] || {
    echo "tick 2 status $status: $output"
    false
  }
  assert_page_count 2
  assert_page_body_has '`FileVault disk encryption`: now off, declared on'
  assert_baseline_scalar filevault off
  assert_baseline_scalar sip disabled # the gapped member's prior is preserved, never dropped

  unset POLLER_FDESETUP_OUTPUT
  run run_poller # tick 3: FileVault restores while SIP still gaps: silent recovery
  [[ $status -eq 0 ]] || {
    echo "tick 3 status $status: $output"
    false
  }
  assert_page_count 2
  assert_baseline_scalar filevault on

  unset POLLER_CSRUTIL_EXIT
  run run_poller # tick 4: SIP recovers: clean tick, marker clears, no page
  [[ $status -eq 0 ]] || {
    echo "tick 4 status $status: $output"
    false
  }
  assert_page_count 2
  assert_no_gap_marker
}

@test "T-PCTL-new-gap-member-pages-during-ongoing-gap: a second probe breaking during an ongoing gap pages again (the marker covers members, not the world)" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  export POLLER_CSRUTIL_EXIT=1
  run run_poller # tick 1: sip gaps, pages once
  [[ $status -eq 0 ]] || {
    echo "tick 1 status $status: $output"
    false
  }
  assert_page_count 1

  export POLLER_FDESETUP_EXIT=1
  run run_poller # tick 2: filevault ALSO gaps: a new member, so it pages
  [[ $status -eq 0 ]] || {
    echo "tick 2 status $status: $output"
    false
  }
  assert_page_count 2
  assert_page_body_has 'filevault'

  run run_poller # tick 3: same two members gapped: covered, no re-page
  [[ $status -eq 0 ]] || {
    echo "tick 3 status $status: $output"
    false
  }
  assert_page_count 2
}

@test "T-PCTL-controls-file-gap-never-blinds-builtins: a refused controls file never suppresses a firewall-off page (the trio still compares)" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  rm "$OSQUERY_POSTURE_CONTROLS"
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller # tick 1: the controls file is missing, gap page
  [[ $status -eq 0 ]] || {
    echo "tick 1 status $status: $output"
    false
  }
  assert_page_count 1
  assert_page_body_has 'posture-controls file missing'

  set_posture '[{"firewall":"0","gatekeeper":"1","screenlock":"1"}]'
  run run_poller # tick 2: file still missing AND the firewall turns off: it pages
  [[ $status -eq 0 ]] || {
    echo "tick 2 status $status: $output"
    false
  }
  assert_page_count 2
  assert_page_body_has 'Firewall turned OFF'
  assert_baseline_scalar firewall 0
}

@test "T-PCTL-gap-recovery-then-regression-pages: after a control gap recovers, a real later regression still pages (the gap never poisoned the baseline)" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  export POLLER_FDESETUP_EXIT=1
  run run_poller # tick 1: indeterminate probe, gap page
  assert_page_count 1
  assert_gap_marker

  unset POLLER_FDESETUP_EXIT
  run run_poller # tick 2: recovered, marker clears, steady state
  [[ $status -eq 0 ]] || {
    echo "tick 2 status $status: $output"
    false
  }
  assert_page_count 1
  assert_no_gap_marker

  export POLLER_FDESETUP_OUTPUT="FileVault is Off."
  run run_poller # tick 3: a REAL regression against the preserved baseline
  assert_page_count 2
  assert_page_body_has '`FileVault disk encryption`: now off, declared on'
}

# --- bounded probes: a WEDGED status tool becomes a gap, not silent blindness ---
# One hang test per probe path (fdesetup, csrutil, sysadminctl, defaults),
# mirroring the osqueryi hang test: the stub execs into a 30s sleep, the 1s
# bound kills it, and the control gaps. The wallclock guard is what makes an
# UNBOUNDED probe fail here: without it, the 30s sleep would end, the probe
# would return empty at exit 0, and the very same gap assertions would pass
# late, hiding a removed run_bounded.

assert_bounded_hang_gaps() { # <control-id> <started-seconds>
  local elapsed=$((SECONDS - $2))
  [[ $elapsed -lt 10 ]] || {
    echo "the poller ran ${elapsed}s: the wedged probe was not killed at the bound"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'monitoring gap'
  assert_page_body_has "$1"
  assert_baseline_unchanged
}

@test "T-PCTL-fdesetup-hang-pages-gap: a wedged fdesetup is killed at the bound and gaps the filevault control" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  snapshot_baseline
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
  export OSQUERY_POSTURE_TIMEOUT=1
  export POLLER_FDESETUP_SLEEP=30

  local started=$SECONDS
  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_bounded_hang_gaps filevault "$started"
}

@test "T-PCTL-csrutil-hang-pages-gap: a wedged csrutil is killed at the bound and gaps the sip control" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  snapshot_baseline
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
  export OSQUERY_POSTURE_TIMEOUT=1
  export POLLER_CSRUTIL_SLEEP=30

  local started=$SECONDS
  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_bounded_hang_gaps sip "$started"
}

@test "T-PCTL-sysadminctl-hang-pages-gap: a wedged sysadminctl is killed at the bound and gaps the guest control" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  snapshot_baseline
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
  export OSQUERY_POSTURE_TIMEOUT=1
  export POLLER_SYSADMINCTL_SLEEP=30

  local started=$SECONDS
  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_bounded_hang_gaps guest "$started"
}

@test "T-PCTL-defaults-hang-pages-gap: a wedged defaults read is killed at the bound and gaps the autologin control" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  snapshot_baseline
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
  export OSQUERY_POSTURE_TIMEOUT=1
  export POLLER_DEFAULTS_SLEEP=30

  local started=$SECONDS
  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }
  assert_bounded_hang_gaps autologin "$started"
}

# --- the poller never mutates, and system-read text never reaches a page raw ---

@test "T-PCTL-no-mutating-invocation: healthy, deviant, and gap ticks invoke only status probes; the violation log stays empty" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller # healthy tick
  [[ $status -eq 0 ]]
  export POLLER_FDESETUP_OUTPUT="FileVault is Off."
  run run_poller # deviant tick (pages)
  [[ $status -eq 0 ]]
  export POLLER_CSRUTIL_EXIT=1
  run run_poller # gap tick (indeterminate probe)
  [[ $status -eq 0 ]]

  # The probes DID run (the spies are live), and nothing but the exact
  # read-only status queries was ever invoked.
  assert_probe_calls fdesetup 3
  assert_probe_calls csrutil 3
  assert_probe_calls sysadminctl 3
  assert_probe_calls defaults 3
  assert_no_mutation_attempt
}

@test "T-PCTL-legacy-gap-values-neutralized: hostile bytes in an osqueryi scalar never reach the gap page body raw" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  snapshot_baseline
  # An out-of-domain firewall value carrying shell metacharacters: the gap body
  # quotes the offending values, so they must arrive neutralized (no dollar, no
  # backslash, no quotes, the value's OWN backticks stripped) inside the body's
  # inline-code span, with the page still firing.
  set_posture '[{"firewall":"0`touch HOME-pwned`$(reboot)\\\"x","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }

  assert_page_count 1
  assert_page_body_has 'monitoring gap'
  # The exact sanitized value inside our span: had any of the value's own
  # backticks survived, the span content (and this match) would break.
  assert_page_body_has '(firewall=`0touch HOME-pwned(reboot)x`'
  if grep -qF -- '$(' "$POLLER_SEND_ALERT_LOG"; then
    echo "a command substitution from the system read reached the page body"
    false
  fi
  if grep -qF -- '\' "$POLLER_SEND_ALERT_LOG"; then
    echo "a backslash from the system read reached the page body"
    false
  fi
  [[ ! -e HOME-pwned && ! -e $POLLER_HOME/HOME-pwned ]] || {
    echo "the hostile value executed"
    false
  }
  assert_baseline_unchanged
}

@test "T-PCTL-probe-output-never-in-page: a hostile probe output is normalized away; only the fixed enum or a gap ever reaches a page" {
  seed_baseline "$healthy_seed"
  declare_posture_controls
  snapshot_baseline
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
  export POLLER_FDESETUP_OUTPUT='`touch probe-pwned`$(reboot) "hostile" FileVault is Wombat.'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }

  assert_page_count 1 # indeterminate -> gap page, never silent
  assert_page_body_has 'monitoring gap'
  if grep -qF -- '`' "$POLLER_SEND_ALERT_LOG"; then
    echo "a backtick from the probe output reached the page body: $(cat "$POLLER_SEND_ALERT_LOG")"
    false
  fi
  if grep -qF -- '$(' "$POLLER_SEND_ALERT_LOG"; then
    echo "a command substitution from the probe output reached the page body"
    false
  fi
  [[ ! -e probe-pwned && ! -e $POLLER_HOME/probe-pwned ]] || {
    echo "the hostile probe output executed"
    false
  }
  assert_baseline_unchanged
}

@test "T-PCTL-gap-values-inline-code-span: a system-read value crosses into a gap page only inside an inline-code span (no structure forgery)" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  snapshot_baseline
  # Markdown that survives character-stripping unchanged: emphasis, a link, a
  # mention. It must arrive WRAPPED in an inline-code span so none of it can
  # render as notification structure (a fake header, a live link, a ping).
  set_posture '[{"firewall":"x] **FAKE CRITICAL** [open](https://example.invalid) @everyone [","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }

  assert_page_count 1
  assert_page_body_has 'monitoring gap'
  assert_page_body_has '`x] **FAKE CRITICAL** [open](https://example.invalid) @everyone [`'
  assert_baseline_unchanged
}

@test "T-PCTL-description-inline-code-span: description and remedy from the data file reach a page only inside inline-code spans" {
  set_posture_controls '[{"id":"guest","description":"x] **FAKE CRITICAL** [open](https://example.invalid) @everyone [","tier":"verify","reader":"sysadminctl_guest","expect":"disabled","remedy":"r] **FAKE REMEDY** [r"}]'
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
  export POLLER_SYSADMINCTL_GUEST_OUTPUT="2026-07-27 00:00:00.000 sysadminctl[100:100] Guest account enabled."

  run run_poller # first observation of a deviant control: pages
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }

  assert_page_count 1
  assert_page_body_has '`x] **FAKE CRITICAL** [open](https://example.invalid) @everyone [`'
  assert_page_body_has '`r] **FAKE REMEDY** [r`'
}

# --- the Gatekeeper remedy names System Settings, never the removed CLI flag ---

@test "T-PCTL-gatekeeper-remedy-names-system-settings: the Gatekeeper-off page points at System Settings and says the CLI cannot re-enable it" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  set_posture '[{"firewall":"1","gatekeeper":"0","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }

  assert_page_body_has 'Gatekeeper turned OFF'
  assert_page_body_has 'System Settings → Privacy & Security'
  assert_page_body_has 'cannot enable Gatekeeper from the CLI'
  assert_page_body_lacks '--master-enable' # removed on macOS 15+; naming it would be a lie
}

@test "T-PCTL-gatekeeper-first-observation-remedy: the first-observation Gatekeeper page carries the same System Settings remedy" {
  set_posture '[{"firewall":"1","gatekeeper":"0","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "status $status: $output"
    false
  }

  assert_page_body_has 'Gatekeeper is OFF (first observation)'
  assert_page_body_has 'cannot enable Gatekeeper from the CLI'
  assert_page_body_lacks '--master-enable'
}

@test "T-PCTL-no-master-enable-in-source: the poller source never mentions spctl --master-enable" {
  if grep -qF -- 'master-enable' "$POLLER_TOOL"; then
    echo "the poller source still names the removed spctl --master-enable flag"
    false
  fi
}
