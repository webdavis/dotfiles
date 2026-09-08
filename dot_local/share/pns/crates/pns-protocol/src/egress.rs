use serde::{Deserialize, Serialize};

use crate::envelope::{Opened, Rejected, Rejection, encode, open};
use crate::identifiers::{RequestId, SchemaId};

fn schema() -> SchemaId {
    SchemaId {
        name: "pns.egress".to_string(),
        major: 1,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EgressMode {
    #[serde(rename = "async")]
    Silent,
    #[serde(rename = "sync")]
    ReportOutcome,
}

// Field order preserves the sorted JSON object that executable channels read (S126).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedEvent {
    pub agent: String,
    pub branch: String,
    pub detail: String,
    pub message: String,
    pub mode: EgressMode,
    pub pane: String,
    pub preview: String,
    pub project: String,
    pub state: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EgressEnvelope {
    pub request_id: RequestId,
    pub body: RenderedEvent,
}

#[derive(Serialize)]
struct Wire<'a> {
    schema: String,
    #[serde(flatten)]
    egress: &'a EgressEnvelope,
}

impl EgressEnvelope {
    pub fn encode(&self) -> Result<String, Rejected> {
        let wire = Wire {
            schema: schema().to_string(),
            egress: self,
        };
        encode(&wire, &schema())
    }
}

pub fn decode(bytes: &[u8]) -> Result<EgressEnvelope, Rejected> {
    let Opened { value, request_id } = open(bytes, &schema())?;
    serde_json::from_value(value).map_err(|error| Rejected {
        request_id,
        reason: Rejection::Invalid(error.to_string()),
    })
}

#[cfg(test)]
mod tests;
