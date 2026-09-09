use crate::CommandRunner;
use serde_json::Value;
use std::path::{Path, PathBuf};
use uu_domain::LaneReport;
mod clone;
#[derive(Debug)]
enum Input {
    Missing,
    Broken(String),
    Document(Value),
}
#[derive(Debug)]
pub struct SkillsForkWatch {
    lock: PathBuf,
    input: Input,
}
impl SkillsForkWatch {
    pub fn read(lock: &Path) -> Self {
        let input = match std::fs::read(lock) {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(value) if value.is_object() => Input::Document(value),
                Ok(_) => Input::Broken("expected one JSON object".into()),
                Err(why) => Input::Broken(why.to_string()),
            },
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => Input::Missing,
            Err(why) => Input::Broken(why.to_string()),
        };
        Self {
            lock: lock.into(),
            input,
        }
    }
    pub(super) fn snapshot(lock: &Path, value: Value) -> Self {
        Self {
            lock: lock.into(),
            input: Input::Document(value),
        }
    }
    pub fn report(&self, temporary: &Path, runner: &dyn CommandRunner, report: &mut LaneReport) {
        let location = self.lock.display();
        let value = match &self.input {
            Input::Missing => {
                report.pending(format!("fork-lock-missing: {location} is absent; restore the lock, no upstream was compared"));
                return;
            }
            Input::Broken(why) => {
                report.pending(format!(
                    "fork-lock-broken: {location}: {why}; repair the lock, no upstream was compared"
                ));
                return;
            }
            Input::Document(value) => value,
        };
        let Some(value) = value.get("forks") else {
            report.pending(format!("fork-table-absent: {location} has no forks table; restore it, or declare an empty object"));
            return;
        };
        let Some(entries) = value.as_object() else {
            report.pending(format!(
                "fork-lock-broken: {location}: forks must be an object; no upstream was compared"
            ));
            return;
        };
        let mut walked = 0;
        for (name, row) in entries {
            walked += 1;
            let name = crate::lanes::changes::section::code(name);
            let fields = match fields(row) {
                Ok(fields) => fields,
                Err(why) => {
                    report.pending(format!("fork-lock-broken: {name} in {location}: {why}; fix this entry, upstream not compared"));
                    continue;
                }
            };
            let (result, cleanup) = clone::inspect(temporary, &fields, runner);
            match result {
                Ok(hash) if hash == fields[2] => report.noted(format!("fork {name}: upstream unchanged since the last comparison")),
                Ok(hash) => report.pending(format!("fork-drift: {name}: {} -> {hash} at {}; compare and port wanted changes by hand, then advance lastComparedTreeHash", fields[2], fields[0])),
                Err((state, why)) => report.pending(format!("{state}: {name}: {why}")),
            }
            if let Some(why) = cleanup {
                report.noted(why);
            }
        }
        finish_walk(walked, entries.len(), report);
    }
}
fn fields(value: &Value) -> Result<[&str; 3], String> {
    let keys = ["sourceUrl", "skillPath", "lastComparedTreeHash"];
    let mut result = [""; 3];
    let mut broken = Vec::new();
    for (i, key) in keys.into_iter().enumerate() {
        match value
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty() && !s.chars().any(char::is_control))
        {
            Some(value) => result[i] = value,
            None => broken.push(key),
        }
    }
    if broken.is_empty() {
        Ok(result)
    } else {
        Err(format!("invalid {}", broken.join(", ")))
    }
}
fn finish_walk(walked: usize, expected: usize, report: &mut LaneReport) {
    if walked != expected {
        report.pending(format!("fork-walk-incomplete: only {walked} of {expected} entries were walked; the rest were not compared"));
    }
}
#[cfg(test)]
mod tests;
