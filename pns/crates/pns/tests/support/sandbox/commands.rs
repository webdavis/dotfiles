use super::super::ENGINE;
use super::Sandbox;
use std::ffi::OsString;
use std::process::Command;

/// A loopback port nothing serves: a post there is refused at once.
const UNSERVED_URL: &str = "http://127.0.0.1:1/";

/// The system directories every sandbox command searches after its own `bin`.
const SYSTEM_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

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
            .env("PNS_SCREEN_IDLE", "99999")
            // The phone's clock is read by walking the DEVELOPER'S OWN live
            // mosh sessions, so the suite states it instead: untouched for a
            // day. A test about the phone overrides this with its own age.
            .env("PNS_PHONE_INPUT_MAX_AGE", "24h");
        command
    }

    /// The engine with no channels directory, which is the only way to reach
    /// the native plugins.
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
        command.env("PNS_MOSHI_HOOK_BIN", self.root.join("no-moshi-hook-here"));
        // THE PHONE PUSH IS FENCED THE SAME WAY, at a loopback port nothing
        // serves. Unset, the binary posts to moshi's real API. A test that
        // captures the push overrides these.
        command
            .env("PNS_MOSHI_URL", UNSERVED_URL)
            .env("PNS_MOSHI_UPLOAD_URL", UNSERVED_URL);
        // No live summarizer: a Stop hook spawns one for real, and the suite
        // must never reach the operator's own Codex.
        command.env("PNS_CODEX_BIN", "/nonexistent/codex");
        // PATH IS THE SANDBOX'S `bin` AND THE SYSTEM DIRECTORIES, never the
        // developer's own. Every banner, a failure notice included, reaches the
        // recording `terminal-notifier` in `bin`, and a detached recap child
        // that outlives this sandbox finds no notifier at all once `Drop` has
        // taken `bin`. `git` resolves from `/usr/bin`; `herdr` and `gh` are a
        // test's stub in `bin` or absent.
        let mut path = OsString::from(self.path("bin"));
        path.push(":");
        path.push(SYSTEM_PATH);
        command.env("PATH", path);
        command
    }
}
