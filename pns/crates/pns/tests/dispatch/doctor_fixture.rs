use super::*;

// --- the doctor -------------------------------------------------------------

/// The doctor's command, with its state directory inside the sandbox so a mute
/// can be planted where the engine will read it, and with no moshi-hook to
/// run unless the test hands it one.
pub(super) fn doctor_command(sandbox: &Sandbox) -> std::process::Command {
    let mut command = sandbox.pns();
    command.env("PNS_STATE_DIR", sandbox.path("state"));
    no_moshi_hook(sandbox, &mut command);
    command.arg("doctor");
    command
}

/// The moshi-hook EVERY doctor invocation gets unless it asked for a different
/// one: a path inside the sandbox that does not exist.
///
/// WITHOUT THIS THE SUITE READS THE DEVELOPER'S OWN MACHINE. The doctor
/// resolves the binary through `MOSHI_HOOK_BIN` over a Homebrew path, so an
/// unstubbed run would spawn the real moshi-hook, contact the moshi API, take
/// about five seconds doing it, and answer differently on every machine.
/// Absent is also a real state rather than a flag, and the check is inert on
/// the exit code for it, so no test here has its verdict decided by the stub.
pub(super) fn no_moshi_hook(sandbox: &Sandbox, command: &mut std::process::Command) {
    command.env("MOSHI_HOOK_BIN", sandbox.path("no-moshi-hook-here"));
}

/// A moshi-hook that answers both shapes of `status` from canned bytes and
/// APPENDS its argv to a record file.
///
/// Appending, rather than the hook suite's stub overwriting with `>`, is what
/// lets a test assert that exactly two invocations happened and that neither
/// of them was `probe`. It is a thin stub plus a spy and reasons about
/// nothing: the fixtures are the bytes the real 0.3.3 binary printed, so this
/// models the tool rather than what the check wishes the tool did.
///
/// THE ARGUMENTS ARE RECORDED WITH THEIR BOUNDARIES INTACT, one unit separator
/// after each and one line per invocation, because `"$*"` joins them with a
/// space: under it a single argument `status --json` and the two real ones
/// leave an identical record, so a spy reading it could not tell the shape the
/// doctor actually spawned from a shape it never would.
///
/// A FIXTURE MAY NOT CONTAIN AN APOSTROPHE. Both are interpolated into a
/// single-quoted shell string below, so one apostrophe ends that string and
/// the stub silently prints something else, or fails to parse at all. The
/// plausible case is not an attack but a contraction: a future `server:`
/// sentence reading "moshi can't reach the server" would do it.
pub(super) fn stub_moshi_hook(
    sandbox: &Sandbox,
    command: &mut std::process::Command,
    json: &str,
    plain: &str,
) {
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    let script = bin.join("moshi-hook");
    write_script(
        &script,
        &format!(
            "printf '%s\\x1f' \"$@\" >>\"{sandbox}/moshi-hook.argv\"\n\
             printf '\\n' >>\"{sandbox}/moshi-hook.argv\"\n\
             case \"$*\" in\n\
             *--json*) printf '%s' '{json}' ;;\n\
             *) printf '%s' '{plain}' ;;\n\
             esac",
            sandbox = sandbox.display()
        ),
    );
    command.env("MOSHI_HOOK_BIN", &script);
}

/// Every argv the stub was ever handed, one vector per invocation, with the
/// argument boundaries the stub recorded preserved.
pub(super) fn moshi_hook_argv(sandbox: &Sandbox) -> Vec<Vec<String>> {
    std::fs::read_to_string(sandbox.path("moshi-hook.argv"))
        .map(|recorded| {
            recorded
                .lines()
                .map(|line| {
                    // TERMINATOR, not separator: the stub writes one after
                    // every argument, so a plain split would invent a trailing
                    // empty argument on every invocation.
                    line.split_terminator('\u{1f}')
                        .map(str::to_string)
                        .collect()
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `moshi-hook status --json` on this machine, moshi-hook 0.3.3, healthy. The
/// values the capture elided are elided here too; nothing reads them.
pub(super) const PAIRED_STATUS_JSON: &str = r#"{"baseUrl":"https://api.getmoshi.app/api/v1","displayName":"dresden","hooks":[],"hostId":"host_b14dd2bb0b1f45899d9eaa81a71ff874","logPath":"...","paired":true,"platform":"macos","secretStore":"keychain","socketPath":"..."}"#;

/// The same call with `HOME` pointed at an empty directory: no host id at all.
pub(super) const UNPAIRED_STATUS_JSON: &str = r#"{"baseUrl":"https://api.getmoshi.app/api/v1","hooks":[],"logPath":"...","paired":false,"platform":"macos","secretStore":"keychain","socketPath":"..."}"#;

/// `moshi-hook status` (plain), healthy. Only this shape carries a server
/// verdict; the JSON above is local-only and measured to do no network I/O.
pub(super) const PAIRED_STATUS_PLAIN: &str = "status:       paired
host id:      host_b14dd2bb0b1f45899d9eaa81a71ff874
display name: dresden
server:       Moshi Pro attached (usage scope: license)";

/// Plain `status` on an unpaired host. What was captured about this one is
/// that it leads `unpaired` and has NO `server:` line at all, which is the
/// only property anything here reads; the column padding is copied from the
/// paired capture above rather than measured, and nothing consumes it.
pub(super) const UNPAIRED_STATUS_PLAIN: &str = "status:       unpaired";

/// What the pairing check says when there is no moshi-hook to run at all,
/// which is what every doctor test above gets unless it stubs one.
pub(super) const NO_MOSHI_HOOK_LINE: &str = "pns doctor: moshi pairing: moshi-hook did not answer \
     (not installed, or it did not answer in time), so the approval path could not be checked.";

/// The pairing line a healthy dresden earns.
pub(super) const PAIRED_LINE: &str =
    "pns doctor: moshi pairing: paired as dresden (host_b14dd2bb0b1f45899d9eaa81a71ff874).";

/// The relayed line beside it, in moshi's own words.
pub(super) const MOSHI_SAYS_LINE: &str =
    "pns doctor: moshi says: Moshi Pro attached (usage scope: license)";

/// What the doctor says about Focus on a machine whose config names no mode,
/// which is every machine that never wrote a `[focus]` table.
pub(super) const FOCUS_OFF_LINE: &str =
    "pns doctor: focus awareness is off (no [focus] table names a mode to silence)";

/// What the doctor says about the clock on a machine where nothing has
/// bootstrapped the LaunchAgent, which is every sandbox in this file: the table
/// defaults ON and no daemon has ever written a beat here.
pub(super) const DAEMON_NEVER_RAN_LINE: &str =
    "pns doctor: the daemon is enabled and has not run yet";

/// And what it says about the nag on a machine whose config has no `[nag]`
/// table, which is every machine until an operator writes one: the feature
/// ships OFF. It sits IMMEDIATELY BELOW the daemon's line, which is the whole
/// mitigation for the one thing it does not say (a nag with a dead daemon never
/// fires): the two read as one paragraph.
pub(super) const NAG_OFF_LINE: &str = "pns doctor: the nag is off (no `[nag] after_secs`)";

/// And what it says about the lamps on a machine whose config has no `[lights]`
/// table, which is every machine that never wrote one.
pub(super) const LIGHTS_OFF_LINE: &str =
    "pns doctor: lights: off in the config, so the pulse uses the [plugins.hue] rooms";

/// Every channel an event dispatches, switched on. The sensor and the lights
/// are deliberately absent: the report has to name them anyway.
pub(super) const EVERY_DISPATCHED_CHANNEL: &str = "[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n\
     [plugins.macos-banner]\nenabled = true\n[plugins.hermes]\nenabled = true\n";

/// The report's own sentences, with the presentation taken back off.
///
/// THE TESTS BELOW ASSERT WHAT THE REPORT SAYS, not how it looks. The frame,
/// the headings, the marks and the closing list are the command's rendering and
/// have their own tests beside the renderer; reading them here would make every
/// expectation in this file a hostage to a change of glyph.
pub(super) fn report_rows(reported: &str) -> Vec<&str> {
    // The closing rule ends the report proper; everything under it repeats a
    // row already counted.
    let body = match reported.split_once("\n─") {
        Some((body, _)) => body,
        None => reported,
    };
    body.lines()
        .map(str::trim_start)
        .filter_map(|line| {
            ["✓ ", "✗ ", "⚠ ", "· ", "→ "]
                .iter()
                .find_map(|glyph| line.strip_prefix(glyph))
        })
        .collect()
}

/// The section headings, in the order they were printed.
pub(super) fn report_sections(reported: &str) -> Vec<&str> {
    reported
        .lines()
        .filter_map(|line| line.strip_prefix("◆ "))
        .filter_map(|line| line.split_once(" ──"))
        .map(|(title, _)| title)
        .collect()
}
