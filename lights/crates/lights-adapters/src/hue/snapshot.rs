use lights_application::LightControlError;
use lights_domain::ReportedBrightness;
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) enum Resource {
    Room {
        id: String,
        name: String,
        grouped: String,
    },
    Grouped {
        id: String,
        on: bool,
        brightness: Option<ReportedBrightness>,
    },
    Scene {
        id: String,
        room: String,
        name: String,
        active: bool,
    },
    Other,
}

pub(super) fn malformed() -> LightControlError {
    LightControlError::Malformed {
        detail: "invalid bridge response".into(),
    }
}

pub(super) fn decode(data: Vec<Value>) -> Result<Vec<Resource>, LightControlError> {
    let mut seen = BTreeSet::new();
    let mut resources = Vec::new();
    for value in data {
        let id = identifier(&value, "id")?.to_owned();
        if !seen.insert(id.clone()) {
            return Err(malformed());
        }
        resources.push(match text(&value, "type")? {
            "room" => {
                let name = name(&value)?;
                text(&value["metadata"], "archetype")?;
                for child in value["children"].as_array().ok_or_else(malformed)? {
                    reference(child)?;
                }
                let services = value["services"].as_array().ok_or_else(malformed)?;
                let mut grouped = None;
                for service in services {
                    let (rid, kind) = reference(service)?;
                    if kind == "grouped_light" && grouped.is_none() {
                        grouped = Some(rid.to_owned());
                    }
                }
                Resource::Room {
                    id,
                    name,
                    grouped: grouped.ok_or_else(malformed)?,
                }
            }
            "grouped_light" => {
                reference(&value["owner"])?;
                let on = value["on"]["on"].as_bool().ok_or_else(malformed)?;
                let brightness = match value.get("dimming") {
                    None => None,
                    Some(dimming) => Some(
                        ReportedBrightness::new(
                            dimming["brightness"].as_f64().ok_or_else(malformed)?,
                        )
                        .map_err(|_| malformed())?,
                    ),
                };
                Resource::Grouped { id, on, brightness }
            }
            "scene" => {
                reference(&value["owner"])?;
                let (room, kind) = reference(&value["group"])?;
                if !["room", "zone"].contains(&kind) {
                    return Err(malformed());
                }
                value["actions"].as_array().ok_or_else(malformed)?;
                let speed = value["speed"].as_f64().ok_or_else(malformed)?;
                if !(0.0..=1.0).contains(&speed) {
                    return Err(malformed());
                }
                value["auto_dynamic"].as_bool().ok_or_else(malformed)?;
                let active = match text(&value["status"], "active")? {
                    "static" => true,
                    "inactive" | "dynamic_palette" => false,
                    _ => return Err(malformed()),
                };
                Resource::Scene {
                    id,
                    room: room.into(),
                    name: name(&value)?,
                    active,
                }
            }
            _ => Resource::Other,
        });
    }
    for resource in &resources {
        if let Resource::Room { grouped, .. } = resource
            && !resources
                .iter()
                .any(|r| matches!(r, Resource::Grouped { id, .. } if id == grouped))
        {
            return Err(malformed());
        }
    }
    Ok(resources)
}

fn name(value: &Value) -> Result<String, LightControlError> {
    let name = text(&value["metadata"], "name")?;
    if name.trim().is_empty() || name.chars().any(char::is_control) {
        return Err(malformed());
    }
    Ok(name.into())
}

fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str, LightControlError> {
    value[field].as_str().ok_or_else(malformed)
}

fn identifier<'a>(value: &'a Value, field: &str) -> Result<&'a str, LightControlError> {
    let id = text(value, field)?;
    if id.len() != 36
        || !id.bytes().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == b'-'
            } else {
                c.is_ascii_digit() || (b'a'..=b'f').contains(&c)
            }
        })
    {
        return Err(malformed());
    }
    Ok(id)
}

pub(super) fn reference(value: &Value) -> Result<(&str, &str), LightControlError> {
    Ok((identifier(value, "rid")?, text(value, "rtype")?))
}
