#!/usr/bin/env bats
# Log-only / no-deliver tier: retained in results.log, never paged, never digested.

load lib

setup() { setup_harness; }
teardown() { teardown_harness; }

@test "T-LOG-sip-no-page: a sip_state OFF row never pages (both name variants)" {
  # SIP is intentionally off on this host, so an on->off transition cannot occur;
  # the snapshot floor would otherwise page forever. The vestigial stale-name
  # variant must be handled identically, not silently classified elsewhere.
  run_alerter "$(row pack_security-policy-regression_sip_state added 1 '{"enabled":"0"}')"
  assert_no_page
  run_alerter "$(row pack_security-regression_sip_state added 1 '{"enabled":"0"}')"
  assert_no_page
  assert_digest_count 0
}

@test "T-LOG-firewall-pack-no-page: a firewall_state OFF pack row is log-only (the poller owns the page)" {
  # The security-policy pack also runs a differential firewall_state query. The tier
  # matrix routes it to log-only because the dedicated firewall/Gatekeeper poller (60s)
  # is the page owner — paging here too would fire TWO #priority pages for one disable.
  run_alerter "$(row pack_security-policy-regression_firewall_state added 1 '{"global_state":"0","stealth_enabled":"1","logging_enabled":"1"}')"
  assert_no_page
  assert_digest_count 0
}

@test "T-LOG-gatekeeper-pack-no-page: a gatekeeper_state OFF pack row is log-only (the poller owns the page)" {
  run_alerter "$(row pack_security-policy-regression_gatekeeper_state added 1 '{"assessments_enabled":"0","dev_id_enabled":"1"}')"
  assert_no_page
  assert_digest_count 0
}

@test "T-LOG-kext-no-deliver: a kext load/unload is never delivered" {
  # The kernel_extensions table lists LOADED kexts, which load/unload on demand —
  # a 657-event firehose. Wrong signal: not page, not digest, not even #osquery.
  run_alerter "$(row pack_intrusion-detection_kernel_extensions_new added 1 '{"name":"com.foo.kext"}')"
  assert_no_dispatch
  assert_digest_count 0
}

@test "T-LOG-es: es_launchd_writes is never delivered (forensic-only)" {
  run_alerter "$(row es_launchd_writes added 1 '{"path":"/usr/bin/foo","filename":"com.bar.plist"}')"
  assert_no_dispatch
  assert_digest_count 0
}

@test "T-LOG-crontab-no-promote: a new crontab/startup item stays log-only even when its path is untrusted" {
  # persistence_startup_items_crontab is log-only per the matrix (noisy, 116 real events).
  # It must not reach the enricher and get promoted NOTICE->CRIT on an unsigned path — its
  # sibling noisy detectors (kext, es_launchd) all have explicit log-only arms; this one
  # was missing, so an unsigned new startup item would page "New startup/cron entry".
  install_untrusted_enricher
  run_alerter "$(row pack_intrusion-detection_persistence_startup_items_crontab added 1 '{"name":"com.foo","command":"/bin/foo","path":"/Users/x/.local/bin/unsigned"}')"
  assert_no_dispatch
  assert_digest_count 0
}

@test "T-LOG-default-silent: there is no #osquery channel — non-page findings are silent" {
  # v2 dispatches ONLY confirmed criticals (#priority). Everything else digests or
  # stays log-only. A NOTICE (new crontab entry) and an INFO (app install) must
  # produce zero delivery — not a NOTICE/INFO line to a quiet channel.
  run_alerter "$(row pack_intrusion-detection_persistence_startup_items_crontab added 1 '{"name":"com.foo","command":"/bin/foo"}')"
  assert_no_dispatch
  run_alerter "$(row pack_installed-software-drift_installed_apps added 1 '{"name":"Foo.app","bundle_short_version":"1.0"}')"
  assert_no_dispatch
  assert_digest_count 0
}
