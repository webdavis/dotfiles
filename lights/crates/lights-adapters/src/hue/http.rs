use super::{HueLightController, snapshot};
use lights_application::LightControlError;
use serde_json::Value;

impl HueLightController {
    pub(super) fn read(&self) -> Result<Vec<snapshot::Resource>, LightControlError> {
        let response = self
            .agent
            .get(&self.base)
            .header("hue-application-key", &self.key)
            .call()
            .map_err(unreachable)?;
        snapshot::decode(envelope(response)?)
    }

    pub(super) fn write(&self, path: &str, body: Value) -> Result<(), LightControlError> {
        let response = self
            .agent
            .put(format!("{}/{path}", self.base))
            .header("hue-application-key", &self.key)
            .content_type("application/json")
            .send(body.to_string())
            .map_err(unreachable)?;
        for value in envelope(response)? {
            snapshot::reference(&value)?;
        }
        Ok(())
    }
}

fn unreachable(error: ureq::Error) -> LightControlError {
    // A REFUSED CERTIFICATE OUTRANKS THE GENERIC SENTENCE, and the fingerprints
    // are spelled out ONCE: a walk over five rooms refuses five handshakes, and
    // five copies of the same block would bury the one that matters.
    if super::mismatch::refused_mismatch().is_some() {
        return LightControlError::Refused {
            detail: match super::mismatch::unspoken_mismatch() {
                Some(mismatch) => super::mismatch::report(&mismatch),
                None => "bridge certificate refused, as reported above".into(),
            },
        };
    }
    let detail = if matches!(error, ureq::Error::Timeout(_)) {
        "bridge request timed out"
    } else {
        "bridge request failed"
    };
    LightControlError::Unreachable {
        detail: detail.into(),
    }
}

fn envelope(
    mut response: ureq::http::Response<ureq::Body>,
) -> Result<Vec<Value>, LightControlError> {
    if !response.status().is_success() {
        return Err(LightControlError::Refused {
            detail: format!("bridge returned status {}", response.status().as_u16()),
        });
    }
    let bytes = response.body_mut().read_to_vec().map_err(unreachable)?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| snapshot::malformed())?;
    let errors = value["errors"].as_array().ok_or_else(snapshot::malformed)?;
    let data = value["data"].as_array().ok_or_else(snapshot::malformed)?;
    if !errors.is_empty() {
        return Err(LightControlError::Refused {
            detail: "bridge refused request".into(),
        });
    }
    Ok(data.clone())
}
