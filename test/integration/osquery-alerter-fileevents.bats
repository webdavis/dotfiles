#!/usr/bin/env bats
# file_events routing (FX1 verb matrix + FX2 manifest tuple + FX3 directory watches).
#
# FX1: the old gate only let CREATED/UPDATED through, but the live log is dominated by
# MOVED_TO (atomic replacement), ROOT_CHANGED (a parent dir renamed) and
# ATTRIBUTES_MODIFIED — so the tamper detector and the sshd page could never fire.
# Every production verb now routes to its category's tier; DELETED is destructive.
#
# FX3: watches are now CONTAINING DIRECTORIES (~/.ssh, ~/.local/bin, ~/.config/osquery),
# so the alerter filters target paths at routing time — authorized_keys pages, other
# ~/.ssh files digest, and a neighbor file in a watched dir is log-only.
#
# FX2: legitimacy is the EXACT (target_path, sha256) tuple in the root-owned manifest;
# a valid hash lifted onto a DIFFERENT tracked file must not pass.

load ../fixtures/osquery-alerter-lib

setup() { setup_harness; }
teardown() { teardown_harness; }

# --- FX3: ~/.ssh directory watch, split by basename ------------------------------

@test "T-PAGE-authkeys: an authorized_keys file CREATED pages" {
  run_alerter "$(file_event_row ssh /Users/x/.ssh/authorized_keys CREATED)"
  assert_page_has authorized_keys
  assert_digest_count 0
}

@test "T-PAGE-authkeys-movedto: an authorized_keys atomic replacement (MOVED_TO) pages" {
  run_alerter "$(file_event_row ssh /Users/x/.ssh/authorized_keys MOVED_TO "")"
  assert_page_has authorized_keys
}

@test "T-PAGE-authkeys-delete: deleting authorized_keys pages (destructive verb)" {
  # FX1 treats DELETED as destructive: removing the key file could be an attacker
  # locking the operator out or covering tracks. It routes to the page tier, not dropped.
  run_alerter "$(file_event_row ssh /Users/x/.ssh/authorized_keys DELETED "")"
  assert_page_has authorized_keys
}

@test "T-DIG-ssh-idrsa: a change to ~/.ssh/id_rsa digests (restored broad coverage), never pages" {
  # FX3 restores ~/.ssh coverage lost when the watch narrowed to authorized_keys only.
  # Private keys / config / known_hosts are sensitive but operator-churned → digest.
  run_alerter "$(file_event_row ssh /Users/x/.ssh/id_rsa UPDATED)"
  assert_no_page
  assert_digest_count 1
}

@test "T-DIG-ssh-config: a change to ~/.ssh/config digests, never pages" {
  run_alerter "$(file_event_row ssh /Users/x/.ssh/config UPDATED)"
  assert_no_page
  assert_digest_count 1
}

# --- FX1: sshd_config verb coverage ----------------------------------------------

@test "T-PAGE-sshd: an sshd_config UPDATED pages" {
  run_alerter "$(file_event_row sshd_config /etc/ssh/sshd_config UPDATED)"
  assert_page_has sshd_config
  assert_page_has UPDATED # the real FSEvents verb, not the constant outer "added"
  assert_digest_count 0
}

@test "T-PAGE-sshd-attrmod: an sshd_config ATTRIBUTES_MODIFIED pages (was silently dropped)" {
  # Live log: the only sshd_config event on this host was ATTRIBUTES_MODIFIED, which the
  # CREATED/UPDATED-only gate discarded — the sshd page could never fire.
  run_alerter "$(file_event_row sshd_config /private/etc/ssh/sshd_config ATTRIBUTES_MODIFIED "")"
  assert_page_has sshd_config
}

# --- FX1 + FX2 + FX3: pipeline_integrity -----------------------------------------

@test "T-PAGE-pipeline-movedto: a pipeline script atomic replacement (live MOVED_TO, empty sha256) pages" {
  # The dominant live pipeline_integrity verb (22 rows) — MOVED_TO with an empty sha256.
  # It carries no hash, so legitimacy cannot be confirmed → page (fail-safe loud). This
  # is the exact row the old CREATED/UPDATED gate dropped, blinding the tamper detector.
  seed_manifest "aaaa1111  /Users/x/.local/bin/osquery-alert-dispatch.sh"
  run_alerter "$(file_event_row pipeline_integrity /Users/x/.local/bin/osquery-alert-dispatch.sh MOVED_TO "")"
  assert_page_has osquery-alert-dispatch.sh
  assert_digest_count 0
}

@test "T-PAGE-pipeline-rootchanged: a pipeline script ROOT_CHANGED (empty sha256) pages" {
  seed_manifest "aaaa1111  /Users/x/.local/bin/osquery-alert-dispatch.sh"
  run_alerter "$(file_event_row pipeline_integrity /Users/x/.local/bin/osquery-alert-dispatch.sh ROOT_CHANGED "")"
  assert_page_has osquery-alert-dispatch.sh
}

@test "T-PAGE-pipeline-mismatch: a tooling change whose hash is NOT in the manifest pages" {
  seed_manifest "aaaa1111  /Users/x/.local/bin/osquery-results-alerter.sh"
  run_alerter "$(file_event_row pipeline_integrity /Users/x/.local/bin/osquery-results-alerter.sh UPDATED novelhash9999)"
  assert_page_has osquery-results-alerter.sh
  assert_digest_count 0
}

@test "T-NEG-pipeline-match: the EXACT (path, hash) tuple in the manifest is silent (legit apply)" {
  seed_manifest "goodhash1234  /Users/x/.local/bin/osquery-results-alerter.sh"
  run_alerter "$(file_event_row pipeline_integrity /Users/x/.local/bin/osquery-results-alerter.sh UPDATED goodhash1234)"
  assert_no_page
  assert_digest_count 0
}

@test "T-SEC-pipeline-swap: a tracked file carrying ANOTHER tracked file's hash pages (FX2)" {
  # The probe: replace osquery-alert-dispatch.sh with a copy of osquery-heartbeat.sh.
  # heartbeat's hash IS in the manifest, so a hash-anywhere check wrongly suppresses it.
  # Binding the hash to ITS path makes (dispatch.sh, heartbeat-hash) an unknown tuple → page.
  seed_manifest \
    "dispatchhash  /Users/x/.local/bin/osquery-alert-dispatch.sh" \
    "heartbeathash  /Users/x/.local/bin/osquery-heartbeat.sh"
  run_alerter "$(file_event_row pipeline_integrity /Users/x/.local/bin/osquery-alert-dispatch.sh UPDATED heartbeathash)"
  assert_page_has osquery-alert-dispatch.sh
  assert_digest_count 0
}

@test "T-NEG-pipeline-neighbor: a non-pipeline file in the watched bin dir is log-only" {
  # ~/.local/bin/%% now fires for every script in the dir; only the osquery pipeline
  # scripts are tracked. A neighbor (relay.sh) changing must not page.
  seed_manifest "aaaa1111  /Users/x/.local/bin/osquery-alert-dispatch.sh"
  run_alerter "$(file_event_row pipeline_integrity /Users/x/.local/bin/relay.sh UPDATED "")"
  assert_no_dispatch
  assert_digest_count 0
}

@test "T-PAGE-pipeline-plist: a tracked osquery LaunchAgent change (launch_agents category) tamper-pages" {
  # Plists live under ~/Library/LaunchAgents (category launch_agents), one category, no
  # overlap. The alerter routes a tracked com.webdavis.osquery-*.plist to the same
  # tamper verdict as the scripts; an unconfirmable change pages.
  seed_manifest "aaaa1111  /Users/x/.local/bin/osquery-alert-dispatch.sh"
  run_alerter "$(file_event_row launch_agents /Users/x/Library/LaunchAgents/com.webdavis.osquery-digest.plist MOVED_TO "")"
  assert_page_has com.webdavis.osquery-digest.plist
}

@test "T-NEG-launchagent-neighbor: a non-osquery LaunchAgent change is log-only (persistence owns new items)" {
  run_alerter "$(file_event_row launch_agents /Users/x/Library/LaunchAgents/com.spotify.webhelper.plist UPDATED)"
  assert_no_dispatch
  assert_digest_count 0
}

# --- FX3: allowlist directory watch ----------------------------------------------

@test "T-DIG-allowlist-file: editing the page-allowlist digests, never pages" {
  run_alerter "$(file_event_row allowlist_file /Users/x/.config/osquery/page-launchd-allowlist.txt UPDATED)"
  assert_no_page
  assert_digest_count 1
}

@test "T-NEG-allowlist-neighbor: another ~/.config/osquery file is log-only" {
  # ~/.config/osquery/%% also holds webhook-secret (covered by agent_authfile_changed);
  # its change must not digest under allowlist_file.
  run_alerter "$(file_event_row allowlist_file /Users/x/.config/osquery/webhook-secret UPDATED)"
  assert_no_dispatch
  assert_digest_count 0
}
