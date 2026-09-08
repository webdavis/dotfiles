use crate::lanes::skills::generation::metadata;
use crate::lanes::skills::tests::{config, directory, roster, write_roster};
use crate::{
    CommandRunner, Ran, SkillsBuildMode, SkillsCandidate, SkillsConfig, SkillsGenerationStore,
    SkillsRoster, Verdict,
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
pub(super) struct Fixture {
    pub root: PathBuf,
    pub config: SkillsConfig,
    pub store: SkillsGenerationStore,
    pub roster: SkillsRoster,
}
impl Fixture {
    pub fn new() -> Self {
        let root = directory();
        let config = config(&root);
        let mut value = roster();
        value["hermesRegistry"] = serde_json::json!({"hub":{"source":"hub","identifier":"display","lockKey":"@owner/hub","profiles":["default"]}});
        value["forks"] = serde_json::json!({"fork":{"sourceUrl":"https://fixture.invalid/repo","skillPath":".","lastComparedTreeHash":"old-tree"}});
        write_roster(Path::new(&config.lock), &value);
        let roster = SkillsRoster::read(Path::new(&config.lock)).unwrap();
        static UPDATER: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        let updater = UPDATER.get_or_init(|| {
            SkillsGenerationStore::capture(
                Path::new("/unused"),
                "",
                &std::env::current_exe().unwrap(),
            )
            .unwrap()
            .updater_hash
        });
        let store = SkillsGenerationStore {
            agents: PathBuf::from(&config.agents),
            roster_hash: roster.hash.clone(),
            updater_hash: updater.clone(),
        };
        metadata::directory(&store.agents.join("skills")).unwrap();
        std::os::unix::fs::symlink(root.join("app"), store.agents.join("skills/cua-driver"))
            .unwrap();
        Self {
            root,
            config,
            store,
            roster,
        }
    }
    pub fn candidate(&self, id: &str, hash: &str, extra: bool) -> SkillsCandidate {
        let c = self.store.candidate(id).unwrap();
        for name in ["alpha", "beta", "gamma"] {
            skill(&c.agents().join("skills").join(name), hash);
        }
        let mut lock = serde_json::json!({"skills":{"alpha":{"skillFolderHash":hash},"beta":{"skillFolderHash":hash}}});
        if extra {
            for name in ["delisted", "owned-real"] {
                skill(&c.agents().join("skills").join(name), "retained");
            }
            lock["skills"]["delisted"] = serde_json::json!({"skillFolderHash":"retained"});
        }
        metadata::write(&c.agents().join(".skill-lock.json"), &lock).unwrap();
        c.assert_overlays(&self.roster).unwrap();
        self.store
            .mark_ready(&c, id, "2026-09-07T00:00:00Z", SkillsBuildMode::Full)
            .unwrap();
        c
    }
    pub fn current(&self, extra: bool) {
        let c = self.candidate("old", "old", extra);
        std::fs::rename(c.agents(), self.store.agents.join(".skills-current")).unwrap();
        for name in ["alpha", "beta", "gamma"] {
            std::os::unix::fs::symlink(
                format!("../.skills-current/skills/{name}"),
                self.store.agents.join("skills").join(name),
            )
            .unwrap();
        }
        std::os::unix::fs::symlink(
            ".skills-current/.skill-lock.json",
            self.store.agents.join(".skill-lock.json"),
        )
        .unwrap();
        if extra {
            std::os::unix::fs::symlink(
                "../.skills-current/skills/delisted",
                self.store.agents.join("skills/delisted"),
            )
            .unwrap();
            skill(
                &self.store.agents.join("skills/owned-real"),
                "operator edit",
            );
            skill(&self.store.agents.join("skills/foreign"), "foreign");
            for path in [
                PathBuf::from(&self.config.claude_skills),
                PathBuf::from(&self.config.hermes).join("skills"),
            ] {
                metadata::directory(&path).unwrap();
                for name in ["delisted", "owned-real"] {
                    std::os::unix::fs::symlink(
                        format!("../../.agents/skills/{name}"),
                        path.join(name),
                    )
                    .unwrap();
                }
            }
        }
    }
}
pub(super) fn skill(path: &Path, content: &str) {
    metadata::directory(&path.join(".clawhub")).unwrap();
    std::fs::write(path.join("SKILL.md"), content).unwrap();
    if path.file_name().unwrap() == "gamma" {
        metadata::write(
            &path.join(".clawhub/origin.json"),
            &serde_json::json!({"installedVersion":content}),
        )
        .unwrap();
    }
}
pub(super) struct Effects {
    pub calls: RefCell<Vec<String>>,
    pub fail_candidate: bool,
    pub migrated: RefCell<bool>,
    pub root: PathBuf,
}
impl Effects {
    pub fn new(f: &Fixture) -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            fail_candidate: false,
            migrated: RefCell::new(false),
            root: f.root.clone(),
        }
    }
}
impl CommandRunner for Effects {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
        self.calls.borrow_mut().push(format!(
            "{} {}",
            Path::new(program).file_name().unwrap().to_string_lossy(),
            args.join(" ")
        ));
        Ok("registry updated".into())
    }
    fn run_with_input(&self, program: &str, args: &[&str], _: &str) -> Result<Ran, String> {
        let output = self.run(program, args)?;
        Ok(Ran {
            stdout: output,
            stderr: String::new(),
            verdict: Verdict::Clean,
        })
    }
    fn run_with_deadline(&self, _: &str, args: &[&str], _: Duration) -> Result<String, String> {
        self.calls.borrow_mut().push("fork comparison".into());
        Ok(if args.last() == Some(&"HEAD^{tree}") {
            "new-tree"
        } else {
            "head"
        }
        .into())
    }
    fn run_in(
        &self,
        program: &str,
        args: &[&str],
        env: &BTreeMap<String, String>,
    ) -> Result<String, String> {
        let name = Path::new(program).file_name().unwrap().to_str().unwrap();
        self.calls.borrow_mut().push(name.into());
        let home = PathBuf::from(&env["HOME"]);
        assert!(home.starts_with(self.root.join(".agents/.skills-generations")));
        assert_eq!(
            env["XDG_CONFIG_HOME"],
            home.join(".config").to_string_lossy()
        );
        if name == "npx" {
            *self.migrated.borrow_mut() = self.root.join(".agents/skills/alpha").is_symlink()
                && self
                    .root
                    .join(".agents/.skills-current/skills/alpha/SKILL.md")
                    .is_file();
            if self.fail_candidate {
                return Err("fixture npx failure".into());
            }
            let mut skills = serde_json::Map::new();
            for pair in args.windows(2).filter(|pair| pair[0] == "--skill") {
                let target = home.join(".agents/skills").join(pair[1]);
                if target.exists() {
                    std::fs::remove_dir_all(&target).unwrap();
                }
                skill(&target, "new");
                skills.insert(pair[1].into(), serde_json::json!({"skillFolderHash":"new"}));
            }
            let lock = Path::new(&env["XDG_STATE_HOME"]).join("skills/.skill-lock.json");
            metadata::directory(lock.parent().unwrap()).unwrap();
            metadata::write(&lock, &serde_json::json!({"skills":skills})).unwrap();
        } else {
            assert_eq!(name, "clawhub");
            let work =
                Path::new(args[args.iter().position(|arg| *arg == "--workdir").unwrap() + 1]);
            let target = work.join("skills").join(args.last().unwrap());
            skill(&target, "new");
        }
        Ok("installed".into())
    }
}
