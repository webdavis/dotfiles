use crate::PostureTrio;
use crate::legacy_json::{ProjectionFields, ProjectionInput, compact_row};
use posture_domain::{BaselineUpdate, trusted_poll_baseline};
use serde_json::value::RawValue;

pub(super) fn encode(
    update: &BaselineUpdate,
    current: &PostureTrio,
    prior: Option<&str>,
) -> Option<String> {
    let clean = current.exit == 0
        && trusted_poll_baseline(Some(0o600), true, current.reading().values, &[]).is_some();
    let rows = if clean {
        current.baseline_rows.clone()
    } else {
        let [fw, gk, sl] = update.trio.values().map(|n| n.to_string());
        format!("{{\"firewall\":\"{fw}\",\"gatekeeper\":\"{gk}\",\"screenlock\":\"{sl}\"}}")
    };
    if !update.preserve_prior_fields && update.controls.is_empty() {
        return Some(rows);
    }
    let input = ProjectionInput::new(rows.as_bytes())?;
    let documents = serde_json::Deserializer::from_str(&input.text)
        .into_iter::<&RawValue>()
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    // --argjson accepts one fresh trio when folding an unknown declaration set.
    if update.preserve_prior_fields && documents.len() != 1 {
        return None;
    }
    let mut output = Vec::new();
    for document in documents {
        let mut fields = if update.preserve_prior_fields {
            let input = ProjectionInput::new(prior?.as_bytes())?;
            object(&input, serde_json::from_str(&input.text).ok()?)?
        } else {
            vec![]
        };
        for (key, value) in object(&input, document)? {
            insert(&mut fields, key, value);
        }
        for control in &update.controls {
            for (key, value) in [
                (control.id.clone(), control.value.as_str()),
                (format!("{}:expect", control.id), control.expect.as_str()),
            ] {
                insert(&mut fields, key, serde_json::to_string(value).ok()?);
            }
            if !control.target.is_empty() {
                insert(
                    &mut fields,
                    format!("{}:target", control.id),
                    serde_json::to_string(&control.target).ok()?,
                );
            }
        }
        let fields = fields
            .into_iter()
            .map(|(key, value)| Some(format!("{}:{value}", serde_json::to_string(&key).ok()?)))
            .collect::<Option<Vec<_>>>()?;
        output.push(format!("{{{}}}", fields.join(",")));
    }
    Some(output.join("\n"))
}
fn object(input: &ProjectionInput, row: &RawValue) -> Option<Vec<(String, String)>> {
    serde_json::from_str::<ProjectionFields<'_>>(row.get())
        .ok()?
        .0
        .into_iter()
        .map(|(key, value)| Some((key, compact_row(input, value)?)))
        .collect()
}
fn insert(fields: &mut Vec<(String, String)>, key: String, value: String) {
    if let Some(stored) = fields.iter_mut().find(|(name, _)| name == &key) {
        stored.1 = value;
    } else {
        fields.push((key, value));
    }
}
