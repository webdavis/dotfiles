use crate::config::NvimSmokeTestLane;
use crate::lanes::{CommandRunner, LaneAdapter, Verdict};
use std::fs;
use std::path::{Path, PathBuf};
use uu_domain::{LaneReport, RunFacts};
mod completion;
mod tree;

impl LaneAdapter for NvimSmokeTestLane {
    fn parse(label: &str, fields: toml::Table) -> Result<Self, crate::ConfigError> {
        Self::parse(label, fields)
    }
    fn keys() -> &'static [&'static str] {
        Self::KEYS
    }
    fn diagnostic_program(&self) -> Option<&str> {
        Some(&self.host.nvim)
    }
    fn run(&self, name: &str, _facts: &RunFacts, runner: &dyn CommandRunner) -> LaneReport {
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
        match data {
            Some(data) => run_smoke(self, &data, name, runner),
            None => {
                let mut report = LaneReport::new(name);
                report.failed("smoke test needs HOME or XDG_DATA_HOME to copy Mason".into());
                report
            }
        }
    }
}

fn run_smoke(
    lane: &NvimSmokeTestLane,
    data: &Path,
    name: &str,
    runner: &dyn CommandRunner,
) -> LaneReport {
    let mut report = LaneReport::new(name);
    if let Err(why) = execute(lane, data, runner, &mut report) {
        report.failed(why);
    }
    report
}

fn execute(
    lane: &NvimSmokeTestLane,
    data: &Path,
    runner: &dyn CommandRunner,
    report: &mut LaneReport,
) -> Result<(), String> {
    let tree = tree::Candidate::prepare(lane, data).map_err(|e| format!("smoke candidate: {e}"))?;
    let run = completion::start(&tree.root)?;
    child(&tree, lane, true, runner, report)?;
    let lock = fs::read_to_string(tree.config.join("lazy-lock.json"))
        .map_err(|e| format!("candidate lock: {e}"))?;
    child(&tree, lane, false, runner, report)?;
    completion::verify(&tree.root, &run, &lock, report)?;
    if fs::read_to_string(tree.config.join("lazy-lock.json"))
        .ok()
        .as_ref()
        != Some(&lock)
    {
        return Err("completion candidate lock changed during verification".into());
    }
    Ok(())
}

fn child(
    tree: &tree::Candidate,
    lane: &NvimSmokeTestLane,
    prepare: bool,
    runner: &dyn CommandRunner,
    report: &mut LaneReport,
) -> Result<(), String> {
    let mut args = tree.environment();
    args.extend([
        lane.host.nvim.clone(),
        "--headless".into(),
        "-u".into(),
        tree.config.join("init.lua").to_string_lossy().into_owned(),
    ]);
    let script = tree
        .config
        .join("lua/uu/smoke_test.lua")
        .to_string_lossy()
        .into_owned();
    if prepare {
        args.extend(["-l".into(), script, "prepare".into()]);
    } else {
        args.extend(["-c".into(), format!("lua dofile({})", lua_string(&script))]);
    }
    let args = args.iter().map(String::as_str).collect::<Vec<_>>();
    let ran = runner.run_with_input("/usr/bin/env", &args, "")?;
    for line in ran.stdout.lines() {
        report.noted(line.into());
    }
    if !prepare {
        fs::write(tree.root.join("startup.stderr"), &ran.stderr)
            .map_err(|e| format!("startup stderr could not be retained: {e}"))?;
        if !ran.stderr.is_empty() {
            return Err(format!(
                "startup stderr: {} (raw: {})",
                crate::lanes::failure_reason("verification", &ran.stderr),
                tree.root.join("startup.stderr").display()
            ));
        }
    }
    match ran.verdict {
        Verdict::Clean => Ok(()),
        Verdict::Failed(why) | Verdict::Deferred(why) | Verdict::Pending(why) => Err(why),
    }
}

fn lua_string(text: &str) -> String {
    let mut quoted = String::from("'");
    for c in text.chars() {
        match c {
            '\'' => quoted.push_str("\\'"),
            '\\' => quoted.push_str("\\\\"),
            c if c.is_ascii_control() => quoted.push_str(&format!("\\{:03}", c as u32)),
            c => quoted.push(c),
        }
    }
    quoted.push('\'');
    quoted
}

#[cfg(test)]
mod tests;
