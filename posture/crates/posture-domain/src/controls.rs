mod reader;
mod text;

pub use reader::{ControlReader, ControlValue};
pub(crate) use text::{control_span, sanitize_control};

#[derive(Debug, Clone, Copy)]
pub struct ControlRecord<'a> {
    pub id: &'a str,
    pub tier: &'a str,
    pub reader: &'a str,
    pub expect: &'a str,
    pub target: &'a str,
    pub description: &'a str,
    pub remedy: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub enum ControlsInput<'a> {
    Missing(&'a str),
    Malformed,
    Records(&'a [ControlRecord<'a>]),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    pub(crate) id: String,
    pub(crate) reader: ControlReader,
    pub(crate) expect: ControlValue,
    pub(crate) target: String,
    pub(crate) description: String,
    pub(crate) remedy: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlsRefusalKind {
    Missing,
    Malformed,
    Empty,
    Id,
    Collision,
    Tier,
    Reader,
    Expect,
    MissingTarget,
    UnexpectedTarget,
    RelativeTarget,
    MultilineTarget,
    Description,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlsRefusal {
    pub kind: ControlsRefusalKind,
    pub explanation: String,
}

pub fn validate_controls(input: ControlsInput<'_>) -> Result<Vec<Control>, ControlsRefusal> {
    use ControlsRefusalKind::*;
    let records = match input {
        ControlsInput::Missing(path) => return Err(refusal(Missing,
            format!("posture-controls file missing at {}", control_span(path)))),
        ControlsInput::Malformed => return Err(refusal(Malformed,
            "the posture-controls file is not a JSON array".into())),
        ControlsInput::Records([]) => return Err(refusal(Empty,
            "the posture-controls file declares zero controls; the declaration is never empty, so a blank render is refused instead of silently watching nothing".into())),
        ControlsInput::Records(records) => records,
    };
    let mut controls: Vec<Control> = Vec::new();
    for (index, record) in records.iter().enumerate() {
        let id = record.id;
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            return Err(refusal(
                Id,
                format!("posture-controls record {index} has a missing or malformed id"),
            ));
        }
        let prefix = format!("posture-controls record [{id}]");
        if ["firewall", "gatekeeper", "screenlock"].contains(&id)
            || controls.iter().any(|control| control.id == id)
        {
            return Err(refusal(
                Collision,
                format!("{prefix} collides with another monitored field"),
            ));
        }
        if record.tier != "verify" {
            return Err(refusal(
                Tier,
                format!(
                    "{prefix} declares tier {}, not verify; the poller only reads controls, so the record does not belong in its file",
                    control_span(record.tier)
                ),
            ));
        }
        let reader = ControlReader::parse(record.reader).ok_or_else(|| {
            refusal(
                Reader,
                format!(
                    "{prefix} names unknown reader {}",
                    control_span(record.reader)
                ),
            )
        })?;
        let expect = reader.value(record.expect).ok_or_else(|| {
            refusal(
                Expect,
                format!(
                    "{prefix} expects {}, outside the {} domain ({})",
                    control_span(record.expect),
                    record.reader,
                    reader.domain()
                ),
            )
        })?;
        if reader.requires_target() {
            if record.target.is_empty() {
                return Err(refusal(
                    MissingTarget,
                    format!(
                        "{prefix} names reader {}, which requires a target (the absolute binary path whose rule must exist)",
                        record.reader
                    ),
                ));
            }
            if !record.target.starts_with('/') {
                return Err(refusal(
                    RelativeTarget,
                    format!(
                        "{prefix} target {} must be an absolute path",
                        control_span(record.target)
                    ),
                ));
            }
            if record.target.contains(['\n', '\u{1f}']) {
                return Err(refusal(
                    MultilineTarget,
                    format!("{prefix} target contains a newline or a unit separator"),
                ));
            }
        } else if !record.target.is_empty() {
            return Err(refusal(
                UnexpectedTarget,
                format!(
                    "{prefix} declares a target its reader {} does not consume",
                    record.reader
                ),
            ));
        }
        let description = sanitize_control(record.description);
        if description.is_empty() {
            return Err(refusal(Description, format!("{prefix} has no description")));
        }
        controls.push(Control {
            id: id.into(),
            reader,
            expect,
            target: record.target.into(),
            description,
            remedy: sanitize_control(record.remedy),
        });
    }
    Ok(controls)
}

fn refusal(kind: ControlsRefusalKind, explanation: String) -> ControlsRefusal {
    ControlsRefusal { kind, explanation }
}

impl Control {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn reader(&self) -> ControlReader {
        self.reader
    }
    pub fn expect(&self) -> ControlValue {
        self.expect
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn description(&self) -> &str {
        &self.description
    }
    pub fn remedy(&self) -> &str {
        &self.remedy
    }
}

#[cfg(test)]
mod tests;
