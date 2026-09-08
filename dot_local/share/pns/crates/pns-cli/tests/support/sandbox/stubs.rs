use super::super::write_script;
use super::Sandbox;
use std::ffi::OsString;
use std::process::Command;

impl Sandbox {
    /// A recording channel, or any other body: the same shape the bats stubs
    /// wrote, one script per channel name.
    pub fn stub_channel(&self, channel: &str, body: &str) {
        write_script(
            &self.root.join("channels").join(format!("{channel}.sh")),
            body,
        );
    }

    /// A stub `herdr` first on PATH, answering the two calls the session view
    /// makes. `origin_visible` decides whether the event's pane sits on the
    /// tab being looked at or on another one, which is the whole input the
    /// visibility model takes.
    ///
    /// Nothing caller-relative is answered: `pane current`, and a `pane
    /// layout` that names no pane, both resolve against whoever asked, and
    /// the view must never use either. Both exit non-zero here, so a build
    /// that regresses to one reads the session as unreadable instead of
    /// passing on a stub that could not tell the difference.
    pub fn stub_herdr(&self, command: &mut Command, origin_visible: bool) {
        let origin_tab = if origin_visible { "t1" } else { "t9" };
        self.stub_on_path(
            command,
            "herdr",
            &format!(
                r#"case "$1 $2 $3" in
  "workspace list ")      printf '%s' '{{"result":{{"workspaces":[{{"active_tab_id":"t1","focused":true,"workspace_id":"w1"}}]}}}}' ;;
  "pane layout --pane")   printf '%s' '{{"result":{{"layout":{{"focused_pane_id":"t1:p1","tab_id":"{origin_tab}","zoomed":false}}}}}}' ;;
  *)                      exit 1 ;;
esac"#
            ),
        );
    }

    /// A stub binary of that name, first on PATH.
    pub fn stub_on_path(&self, command: &mut Command, name: &str, body: &str) {
        let stub_bin = self.path("bin");
        std::fs::create_dir_all(&stub_bin).expect("stub bin");
        write_script(&stub_bin.join(name), body);
        let mut path = OsString::from(&stub_bin);
        path.push(":");
        path.push(
            command
                .get_envs()
                .find(|(key, _)| *key == "PATH")
                .and_then(|(_, value)| value)
                .map(OsString::from)
                .unwrap_or_else(|| std::env::var_os("PATH").unwrap_or_default()),
        );
        command.env("PATH", path);
    }

    /// Every binary the engine resolves through PATH, replaced by a spy that
    /// records the call and does nothing else.
    ///
    /// AN EMPTY LOG IS THE OBSERVABLE PROXY for "this path spawned nothing",
    /// which is how the help and usage paths are pinned without a wall clock.
    /// Its REACH IS STATED rather than implied: the system probes are absolute
    /// by design (`/usr/sbin/ioreg`, `/usr/bin/pgrep`, `/bin/ps`) so no PATH
    /// can stand in front of them, and what this does catch is the native
    /// banner's `terminal-notifier`, the session view's `herdr`, the branch
    /// lookup's `git` and the condenser's `codex`. The banner is the one that
    /// makes it bite: a usage path that reached the event path raised a real
    /// macOS notification reading "pns · done".
    pub fn spy_path(&self, command: &mut Command) {
        for name in ["herdr", "terminal-notifier", "git", "codex"] {
            self.stub_on_path(
                command,
                name,
                &format!(
                    "printf '%s %s\\n' \"${{0##*/}}\" \"$*\" >>\"{}/spawn.log\"",
                    self.display()
                ),
            );
        }
    }

    /// What the spies recorded, empty when nothing was spawned.
    pub fn spawned(&self) -> String {
        std::fs::read_to_string(self.path("spawn.log")).unwrap_or_default()
    }

    /// A stub `terminal-notifier` first on PATH, so the native banner's spawn
    /// is recorded instead of posting a real notification.
    pub fn stub_notifier(&self, command: &mut Command) {
        self.stub_on_path(
            command,
            "terminal-notifier",
            &format!(
                "printf '%s\\n' \"$*\" >\"{}/notifier.args\"",
                self.display()
            ),
        );
    }
}
