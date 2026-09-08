use super::super::ENGINE;
use super::Sandbox;
use std::process::Command;

impl Sandbox {
    /// The engine pointed at the stubs, with its state directory pinned inside
    /// this sandbox.
    pub fn pns_stateful(&self) -> Command {
        let mut command = self.pns();
        command.env("PNS_STATE_DIR", self.state());
        command
    }

    /// The engine, pointed at the stubs and away from the desk.
    pub fn pns(&self) -> Command {
        let mut command = self.bare();
        command
            .env("PNS_CHANNELS_DIR", self.root.join("channels"))
            .env("PNS_IDLE_SECS", "99999")
            // The phone's clock is read by walking the DEVELOPER'S OWN live
            // mosh sessions, so the suite states it instead: untouched for a
            // day. A test about the phone overrides this with its own age.
            .env("PNS_PHONE_INPUT_AGE", "99999")
            // No live condenser: a Stop hook spawns one for real, and the
            // suite must never reach the operator's own Codex.
            .env("CODEX_BIN", "/nonexistent/codex");
        command
    }

    /// The engine with NOTHING pointing it at stubs, which is the only way to
    /// reach the native plugins.
    ///
    /// EVERYTHING is cleared and only what the binary genuinely needs is put
    /// back, so a developer's environment cannot decide a verdict. The old
    /// blocklist named the variables to remove, which meant every new
    /// override had to be added here too or it would leak in silently; this
    /// states what a test keeps instead, and a new override is excluded by
    /// default.
    pub fn bare(&self) -> Command {
        let mut command = Command::new(ENGINE);
        command.env_clear();
        command.env("HOME", &self.root);
        // MOSHI-HOOK IS FENCED OFF BY DEFAULT, pointed at a path inside this
        // sandbox that nothing ever creates. Unset, the binary falls back to
        // `/opt/homebrew/bin/moshi-hook`, which on this machine EXISTS and is
        // the operator's own: a test that forgot to stub raised a real card on
        // a real phone during slice 11, and a second one was found by review in
        // the daemon suite. A default here makes that structural rather than
        // remembered, and every test that wants a stub still overrides it,
        // because this is set before the caller's own `env` calls.
        command.env("MOSHI_HOOK_BIN", self.root.join("no-moshi-hook-here"));
        // PATH survives because the binary resolves herdr and terminal-notifier
        // through it, and a test that stubs either one prepends to this.
        if let Some(path) = std::env::var_os("PATH") {
            command.env("PATH", path);
        }
        command
    }
}
