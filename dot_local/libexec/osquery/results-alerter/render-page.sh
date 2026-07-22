#!/usr/bin/env bash
#
# render-page.sh - a sourced helper for results-alerter.sh. Functions only, no
# main. It owns the page rendering: render_page reads the enriched findings as
# NDJSON on stdin, selects the CRIT ones, renders each into a focused #priority
# block (header + decision-relevant fields + one next-step), and prints the
# {pcount, pbody} JSON object. Display-only - it never changes detection or tier.
#
# Layout follows the ADHD surfacing research: one thing, glanceable, minimal
# fields, ending in a single action, no raw query jargon.
#
# Criterion 7 (basename-only): a secret/auth-file finding
# (agent_authfile_changed, agent_secretfile_changed) renders the file's BASENAME
# only - never its full path, and never a sha256/digest. Those detectors reach
# this renderer only as a page (a page-tier secret change), and the page body
# fans out to Discord, so it must not disclose the path or the content hash.
#
# Caps: every rendered value is backtick-sanitized and capped at 240 chars; the
# page is capped at 8 blocks (plus an "N more" marker) and then hard-capped at
# 1900 chars, so an over-length page can never wedge the 2000-char Discord budget.

render_page() {
  jq -s '
    # Wrap a value in Discord inline-code backticks. The value is attacker-controlled
    # (launchd label, path); strip backticks so it cannot break out of the inline-code
    # span and inject markdown. Display-only. Bound each field to 240 chars with an
    # explicit omission marker so one giant value cannot alone blow the 2000-char budget.
    def code:
      (gsub("`"; "")) as $v
      | "`" + (if ($v | length) > 240 then ($v[0:240] + "…(truncated)") else $v end) + "`";
    # Plain-English name of a macOS protection query, or null if the finding is not one.
    def protname:
      if (.q | test("^firewall")) then "Firewall"
      elif (.q | test("^gatekeeper")) then "Gatekeeper"
      elif (.q | test("^sip")) then "System Integrity Protection"
      elif (.q | test("^filevault")) then "FileVault"
      else null end;
    # Human header for a finding.
    def header:
      (protname) as $p |
      if $p != null then (if .sev == "CRIT" then "\($p) turned OFF" else "\($p) changed" end)
      elif .q == "persistence_launchd" then "New startup item"
      elif .q == "persistence_launchd_overrides" then "Startup override changed"
      elif .q == "persistence_startup_items_crontab" then "New startup/cron entry"
      elif .q == "suid_bin_unexpected" then "New setuid root binary"
      elif .q == "new_admin_user" then "New administrator account"
      elif .q == "agent_exposure_changed" then "Agent port exposed off-loopback"
      elif .q == "agent_authfile_changed" then "Agent credential changed"
      elif .q == "agent_secretfile_changed" then "Agent secret file changed"
      elif .q == "remote_access_sharing_state" then "Remote-access service enabled"
      elif .q == "kernel_extensions_new" then "New kernel extension"
      elif .q == "system_extensions_new" then "New system extension"
      elif .q == "listening_ports_non_loopback" then "New network listener"
      elif .q == "recent_logins" then "Login"
      elif .q == "installed_apps" then "New app"
      elif .q == "homebrew_packages" then "New Homebrew package"
      elif (.q | test("_extensions$|_addons$")) then "New browser extension"
      elif .q == "file_events_recent" then
        ((.cols.category // "") as $cat | ((.cols.target_path // "") | split("/") | last) as $bn |
          # A tracked pipeline file can arrive under pipeline_integrity OR (for our own
          # LaunchAgents) launch_agents/launch_daemons, so key the tooling header on the
          # basename, not only the category.
          if ($bn | test("^osquery-.*\\.sh$")) or ($bn | test("^com\\.webdavis\\.osquery-.*\\.plist$")) then "Security tooling changed"
          elif ($cat == "ssh" or $cat == "authorized_keys") then "SSH key file changed"
          elif $cat == "sudoers" then "sudoers changed"
          elif $cat == "sshd_config" then "sshd_config changed"
          elif $cat == "pipeline_integrity" then "Security tooling changed"
          elif $cat == "allowlist_file" then "Allowlist changed"
          elif ($cat == "launch_agents" or $cat == "launch_daemons") then "Startup folder changed"
          else "Watched file changed" end)
      elif .q == "es_launchd_writes" then "Startup item written by a process"
      else (.q | gsub("_"; " ")) end;
    # Best single identifier for a finding.
    def keyid:
      .cols as $c |
      ($c.label // $c.identifier // $c.name // $c.target_path // $c.path // $c.username // "?");
    # Decision-relevant "Label: value" lines for a #priority block. Values are wrapped
    # in Discord inline-code; an untrusted signing verdict is flagged and bolded.
    def fields:
      # Strip markdown metacharacters from the (attacker-influenceable) signing authority
      # so a crafted certificate subject cannot inject backticks/emphasis into the body.
      .cols as $c | ((.signing // null) | if type == "string" then gsub("[`*]"; "") else . end) as $sig |
      (if $sig then
         (if ($sig | test("unsigned|untrusted|ad-hoc|unverified|no authority"; "i"))
          then ["- **Signing:** ⚠️ **\($sig)**"] else ["- **Signing:** \($sig)"] end)
       else [] end) as $sg |
      if .q == "persistence_launchd" then ["- **What:** \(($c.label // "?") | code)", "- **Program:** \(($c.program // "?") | code)"] + $sg
      elif .q == "persistence_startup_items_crontab" then ["- **What:** \(($c.name // "?") | code)", "- **Command:** \(($c.command // "?") | code)"] + $sg
      elif .q == "suid_bin_unexpected" then ["- **Path:** \(($c.path // "?") | code)"] + $sg + ["- **Owner:** \(($c.username // "?") | code)"]
      elif .q == "new_admin_user" then ["- **User:** \(($c.username // "?") | code)", "- **UID:** \(($c.uid // "?") | code)"]
      elif .q == "agent_exposure_changed" then ["- **Process:** \(($c.name // "?") | code)", "- **Address:** \(($c.address // "?") | code)", "- **Port:** \(($c.port // "?") | code)"]
      # Criterion 7: secret/auth files render the BASENAME only, never the full path,
      # and never a sha256 (no hash field is rendered for these detectors at all).
      elif .q == "agent_authfile_changed" then ["- **File:** \((($c.path // "") | split("/") | last) | code)"]
      elif .q == "agent_secretfile_changed" then ["- **File:** \((($c.path // "") | split("/") | last) | code)"]
      elif .q == "remote_access_sharing_state" then ["- **Service:** \(($c.service // "?") | code)"]
      elif .q == "system_extensions_new" then ["- **Name:** \(($c.identifier // "?") | code)", "- **Team:** \(($c.team // "?") | code)"] + $sg
      elif .q == "kernel_extensions_new" then ["- **Name:** \(($c.name // "?") | code)", "- **Path:** \(($c.path // "?") | code)"] + $sg
      elif .q == "file_events_recent" then ["- **File:** \(($c.target_path // "?") | code)", "- **Action:** \($c.action // .act)"]
      elif .q == "es_launchd_writes" then ["- **Process:** \(($c.path // "?") | code)", "- **Wrote:** \(($c.filename // $c.dest_filename // "?") | code)"] + $sg
      elif (protname) != null then ["- **State:** **OFF**"]
      else $sg + ["- **What:** \((keyid) | code)"] end;
    # One or two instructive next-step lines for a #priority (always CRIT) block.
    def nextstep:
      (.ep // "") as $ep |
      if (protname) != null then
        ["- Did you turn this off? If not, something else did - **investigate now**.", "- Re-enable it in System Settings."]
      elif (.q == "system_extensions_new" or .q == "kernel_extensions_new") then
        ["- Did you install this? If not, **remove it** - an extension can intercept traffic or load at boot.", "- Manage at: System Settings → General → Login Items & Extensions"]
      elif .q == "suid_bin_unexpected" then
        ["- Did you create this? If not, it lets a program run as **root** - a backdoor.", "- **Inspect:** " + (("codesign -dv \"" + $ep + "\"") | code)]
      elif .q == "new_admin_user" then
        ["- Did you create this account? If not, someone gained **admin access** - investigate now.", "- Review accounts: System Settings → Users & Groups"]
      elif .q == "agent_exposure_changed" then
        ["- Did you expose this? If not, an agent API is reachable **off-box** - close it now.", "- Re-bind it to 127.0.0.1 or block the port at the firewall."]
      elif .q == "agent_authfile_changed" then
        ["- Did you rotate this? If not, an attacker may forge or mute alerts, or hijack remote access - **investigate now**."]
      elif .q == "agent_secretfile_changed" then
        ["- Did you rotate this? If not, an attacker may have your alerting or remote-access secret - **investigate now**."]
      elif .q == "remote_access_sharing_state" then
        ["- Did you enable this? If not, someone opened a remote-control path into this Mac - **disable it now**.", "- System Settings → General → Sharing"]
      elif .q == "file_events_recent" then
        (((.cols.target_path // "") | split("/") | last) as $bn |
         if ((.cols.category // "") == "pipeline_integrity")
            or ($bn | test("^osquery-.*\\.sh$")) or ($bn | test("^com\\.webdavis\\.osquery-.*\\.plist$"))
         then ["- Did you just apply your dotfiles? If not, your **security tooling was modified** - investigate now.", "- **Compare:** " + (("shasum -a 256 \"" + $ep + "\"") | code)]
         else ["- Did you change this? If not, someone altered who can log in or run as **root**.", "- **Review:** " + (("sudo cat \"" + $ep + "\"") | code)] end)
      elif (.q == "persistence_launchd" or .q == "persistence_startup_items_crontab") then
        ["- Did you set this up? If not, it **auto-runs at every login** - likely malware.", "- **Inspect:** " + (("cat \"" + $ep + "\"") | code)]
      elif .q == "es_launchd_writes" then
        ["- Did you run this? If not, a process is **installing persistence** - investigate it and remove the file.", "- **Inspect the writer:** " + (("codesign -dv \"" + $ep + "\"") | code)]
      elif ($ep != "") then ["- **Review:** " + ($ep | code)]
      else [] end;
    def block:
      (["**" + header + "**"] + fields + nextstep) | join("\n");
    ([.[] | select(.sev == "CRIT")]) as $crit |
    {
      pcount: ($crit | length),
      # Cap the page at eight blocks + a marker so a large simultaneous-CRIT batch cannot
      # exceed the Discord 2000-char limit and get stuck undelivered in the spool. The
      # dropped detail still lands in results.log. Mirrors the digest group cap.
      pbody: (
        (($crit[0:8] | map(block) | join("\n\n"))
         + (if ($crit | length) > 8
            then "\n\n… and \(($crit | length) - 8) more CRITICAL finding(s) - see results.log"
            else "" end)) as $full
        # The eight-block cap bounds COUNT, not length - eight blocks with long fields can
        # still exceed 2000. Apply a FINAL hard length cap; the truncated body is exactly
        # what is dispatched, so an over-length page can never wedge the spool.
        | if ($full | length) > 1900
          then ($full[0:1900] + "\n… (truncated to fit the 2000-char limit - see results.log)")
          else $full end
      )
    }
  '
}
