use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use uu_domain::LaneReport;

pub(super) fn start(root: &Path) -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let run = format!("{}-{}", std::process::id(), now.as_nanos());
    fs::write(root.join("run.json"), json!({"run":run}).to_string())
        .map_err(|e| format!("smoke run identity: {e}"))?;
    Ok(run)
}

pub(super) fn verify(
    root: &Path,
    run: &str,
    lock: &str,
    report: &mut LaneReport,
) -> Result<(), String> {
    let path = root.join("completion.json");
    let bytes = fs::read(&path)
        .map_err(|e| format!("missing verifier completion {}: {e}", path.display()))?;
    let result: Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("unreadable verifier completion: {e}"))?;
    if result["run"] != run || result["lock"] != lock || result["vim_enter"] != true {
        return Err(
            "verifier completion does not identify this run, candidate lock and VimEnter".into(),
        );
    }
    let errors = result["errors"]
        .as_array()
        .ok_or("completion has no startup diagnostics")?;
    if !errors.is_empty() {
        return Err(format!(
            "startup errors: {} (raw: {})",
            Value::Array(errors.clone()),
            path.display()
        ));
    }
    let health_errors = result["health_errors"]
        .as_u64()
        .ok_or("completion has no health error count")?;
    let warnings = result["health_warnings"]
        .as_u64()
        .ok_or("completion has no health warning count")?;
    report.noted(format!("smoke test passed; candidate lock: {}; health: {health_errors} ERROR, {warnings} WARNING; {}",root.join("c/nvim/lazy-lock.json").display(),root.join("checkhealth.txt").display()));
    Ok(())
}
