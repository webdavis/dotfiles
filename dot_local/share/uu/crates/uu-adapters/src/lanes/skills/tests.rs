use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
pub(super) fn directory() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "uu-skills-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    path
}
pub(super) fn roster() -> serde_json::Value {
    serde_json::json!({"version":2,"npxTracked":{"alpha":{"repo":"owner/repo"},"beta":{"repo":"owner/repo"}},"clawhubTracked":{"gamma":{"slug":"@owner/gamma","registry":"https://registry.invalid"}},"tiers":{"alpha":"on-demand","beta":"core","gamma":"on-demand"},"hermesProfiles":{"alpha":["default"]}})
}
pub(super) fn write_roster(path: &std::path::Path, value: &serde_json::Value) {
    std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

pub(super) fn config(root: &std::path::Path) -> crate::SkillsConfig {
    let path = |name| root.join(name).to_string_lossy().into_owned();
    crate::SkillsConfig {
        lock: path("roster"),
        agents: path(".agents"),
        claude_skills: path(".claude/skills"),
        hermes: path(".hermes"),
        npx: path("bin/npx"),
        skills_cli_version: "1.5.22".into(),
        clawhub: path("bin/clawhub"),
        hermes_cli: path("bin/hermes"),
        cua_driver: path("bin/cua-driver"),
        routing: path("bin/routing"),
    }
}
