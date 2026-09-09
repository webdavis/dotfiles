use super::*;
use std::path::PathBuf;
use std::process::{Command, Output};
use uu_adapters::{SkillsBuildMode, SkillsGenerationStore, SkillsRoster};
pub(super) struct Fixture {
    pub home: Home,
    pub binary: PathBuf,
    store: SkillsGenerationStore,
    roster: SkillsRoster,
}
impl Fixture {
    pub fn new(name: &str) -> Self {
        let home = Home::new(name);
        let binary = home.dir.join("uu-copy");
        std::fs::copy(env!("CARGO_BIN_EXE_uu"), &binary).unwrap();
        let roster = home.dir.join("roster.json");
        std::fs::write(&roster, r#"{"version":2,"npxTracked":{"alpha":{"repo":"owner/repo"}},"clawhubTracked":{},"tiers":{"alpha":"core"},"hermesProfiles":{"alpha":["default"]},"forks":{}}"#).unwrap();
        let agents = home.dir.join(".agents");
        std::fs::create_dir_all(agents.join("skills/alpha")).unwrap();
        std::fs::write(agents.join("skills/alpha/SKILL.md"), "flat").unwrap();
        let routing = home.write_stub("routing", "printf called >\"$HOME/routing-called\"\n");
        let installer = home.write_stub("installer", "printf called >\"$HOME/installer-called\"\nprintf 'unexpected installer\n' >&2\nexit 1\n");
        let other = home.write_stub("other", "exit 0\n");
        let text = format!(
            "[lanes.mine]\ntype = \"skills\"\nlock = {roster:?}\nagents = {agents:?}\nclaude_skills = {:?}\nhermes = {:?}\nnpx = {installer:?}\nskills_cli_version = \"1.5.22\"\nclawhub = {installer:?}\nhermes_cli = {other:?}\ncua_driver = {other:?}\nrouting = {routing:?}\n",
            home.dir.join(".claude/skills"),
            home.dir.join(".hermes")
        );
        let home = home.with_config(&text);
        let roster = SkillsRoster::read(&roster).unwrap();
        let store = SkillsGenerationStore::capture(&agents, &roster.hash, &binary).unwrap();
        Self {
            home,
            binary,
            store,
            roster,
        }
    }
    pub fn ready(&self, id: &str) {
        let candidate = self.store.candidate(id).unwrap();
        std::fs::create_dir_all(candidate.agents().join("skills/alpha")).unwrap();
        std::fs::write(candidate.agents().join("skills/alpha/SKILL.md"), "ready").unwrap();
        std::fs::write(
            candidate.agents().join(".skill-lock.json"),
            r#"{"skills":{"alpha":{"skillFolderHash":"ready"}}}"#,
        )
        .unwrap();
        candidate
            .absorb_store_entries(&self.store, &self.roster)
            .unwrap();
        self.store
            .mark_ready(
                &candidate,
                id,
                "2026-09-07T00:00:00Z",
                SkillsBuildMode::Full,
            )
            .unwrap();
    }
    pub fn published(&self) -> String {
        std::fs::read_to_string(
            self.home
                .dir
                .join(".agents/.skills-current/generation.json"),
        )
        .unwrap()
    }
    pub fn invoke(&self, args: &[&str]) -> Output {
        let mut command = Command::new(&self.binary);
        command
            .args(args)
            .env_clear()
            .env("HOME", &self.home.dir)
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null");
        for key in [
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_STATE_HOME",
            "XDG_CACHE_HOME",
            "XDG_RUNTIME_DIR",
            "XDG_CONFIG_DIRS",
            "XDG_DATA_DIRS",
            "CLAUDE_CONFIG_DIR",
            "TMPDIR",
            "TMP",
            "TEMP",
        ] {
            let path = self.home.dir.join(key);
            std::fs::create_dir_all(&path).unwrap();
            command.env(key, path);
        }
        command.output().unwrap()
    }
}
