use super::{SkillsBuildMode, SkillsCandidate, SkillsEnvironment, SkillsRoster, overlay};
use crate::CommandRunner;
use std::collections::BTreeMap;
use std::os::unix::fs::DirBuilderExt;
use std::sync::atomic::{AtomicU64, Ordering};

fn changed(result: &Result<String, String>) -> bool {
    match result {
        Ok(s) | Err(s) => s.contains("local changes"),
    }
}
fn clean(result: Result<String, String>) -> Result<(), String> {
    if changed(&result) {
        Err("refused over local changes".into())
    } else {
        result.map(|_| ())
    }
}
impl SkillsCandidate {
    pub fn install_clawhub(
        &self,
        binary: &str,
        roster: &SkillsRoster,
        mode: SkillsBuildMode,
        env: &SkillsEnvironment,
        runner: &dyn CommandRunner,
    ) -> Result<BTreeMap<String, String>, String> {
        let mut environment = env.variables.clone();
        environment.insert("CLAWHUB_DISABLE_TELEMETRY".into(), "1".into());
        let mut failed = BTreeMap::new();
        for (name, (slug, registry)) in &roster.clawhub {
            let store = self.agents().join("skills").join(name);
            let result = if !store.exists() {
                self.clawhub_install(binary, name, slug, registry, &environment, runner)
            } else if mode == SkillsBuildMode::Additive {
                Ok(())
            } else {
                self.clawhub_update(binary, name, &environment, runner)
            };
            if let Err(why) = result {
                failed.insert(name.clone(), why);
            }
        }
        Ok(failed)
    }
    fn clawhub_install(
        &self,
        binary: &str,
        name: &str,
        slug: &str,
        registry: &str,
        env: &BTreeMap<String, String>,
        runner: &dyn CommandRunner,
    ) -> Result<(), String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let work = self.home.join(".tmp").join(format!(
            "clawhub-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&work)
            .map_err(|e| e.to_string())?;
        clean(runner.run_in(
            binary,
            &[
                "--no-input",
                "--workdir",
                &work.to_string_lossy(),
                "--dir",
                "skills",
                "--registry",
                registry,
                "install",
                slug,
            ],
            env,
        ))?;
        let nested = work.join("skills").join(slug);
        let installed = if nested.exists() {
            nested
        } else {
            work.join("skills").join(name)
        };
        if !std::fs::symlink_metadata(&installed)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_dir()
            || !installed.join(".clawhub/origin.json").is_file()
        {
            return Err("installed skill has no directory or origin.json".into());
        }
        std::fs::rename(installed, self.agents().join("skills").join(name))
            .map_err(|e| e.to_string())
    }
    fn clawhub_update(
        &self,
        binary: &str,
        name: &str,
        env: &BTreeMap<String, String>,
        runner: &dyn CommandRunner,
    ) -> Result<(), String> {
        let agents = self.agents();
        match std::fs::remove_file(agents.join("skills").join(name).join(".DS_Store")) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        let work = agents.to_string_lossy();
        let args = [
            "--no-input",
            "--workdir",
            &work,
            "--dir",
            "skills",
            "update",
            name,
        ];
        let initial = runner.run_in(binary, &args, env);
        if !changed(&initial) {
            return clean(initial);
        }
        let path = agents.join("skills").join(name).join("agents/openai.yaml");
        let Some(_) = overlay::strip_owned(&path)? else {
            return clean(initial);
        };
        let retried = runner.run_in(binary, &args, env);
        overlay::reassert(&path)?;
        clean(retried)
    }
}
#[cfg(test)]
mod tests;
