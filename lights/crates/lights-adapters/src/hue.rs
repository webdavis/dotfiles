mod http;
mod snapshot;

use crate::settings::HueSettings;
use lights_application::{
    BrightnessChange, LightControlError, LightController, RoomRef, RoomState, SceneRef, SceneState,
};
use lights_domain::Direction;
use lights_domain::RoomName;
use serde_json::json;
use snapshot::{Resource, malformed};
use std::cell::OnceCell;
use ureq::unversioned::{resolver::Resolver, transport::Connector};

pub struct HueLightController {
    agent: ureq::Agent,
    base: String,
    key: String,
    snapshot: OnceCell<Result<Vec<Resource>, LightControlError>>,
}
impl HueLightController {
    pub fn new(settings: &HueSettings) -> Self {
        Self::with_agent(
            settings,
            config(std::time::Duration::from_secs(settings.timeout_secs)).new_agent(),
        )
    }
    pub fn with_transport(
        settings: &HueSettings,
        connector: impl Connector,
        resolver: impl Resolver,
    ) -> Self {
        Self::with_agent(
            settings,
            ureq::Agent::with_parts(
                config(std::time::Duration::from_secs(settings.timeout_secs)),
                connector,
                resolver,
            ),
        )
    }
    fn with_agent(settings: &HueSettings, agent: ureq::Agent) -> Self {
        Self {
            agent,
            base: format!("https://{}/clip/v2/resource", settings.address),
            key: settings.key().into(),
            snapshot: OnceCell::new(),
        }
    }
}
fn config(timeout: std::time::Duration) -> ureq::config::Config {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .proxy(None)
        .max_redirects(0)
        .http_status_as_error(false)
        .https_only(true)
        // Approved bridge-specific certificate exception. This disables server authentication;
        // it does not establish that certificate verification is impossible for Hue.
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .disable_verification(true)
                .build(),
        )
        .build()
}
impl HueLightController {
    fn resources(&self) -> Result<&[Resource], LightControlError> {
        self.snapshot
            .get()
            .ok_or(LightControlError::InvalidReference)?
            .as_deref()
            .map_err(Clone::clone)
    }
    fn grouped_id(&self, room: &RoomRef) -> Result<&str, LightControlError> {
        match self.resources()?.get(room.index()) {
            Some(Resource::Room { grouped, .. }) => Ok(grouped),
            _ => Err(LightControlError::InvalidReference),
        }
    }
}
impl LightController for HueLightController {
    fn room(&self, name: &RoomName) -> Result<RoomState, LightControlError> {
        // Keep a failed attempt only to prevent a retry. No malformed snapshot is published.
        let resources = self
            .snapshot
            .get_or_init(|| self.read())
            .as_deref()
            .map_err(Clone::clone)?;
        let (index, grouped) = resources
            .iter()
            .enumerate()
            .find_map(|(index, resource)| match resource {
                Resource::Room {
                    name: found,
                    grouped,
                    ..
                } if found == name.as_str() => Some((index, grouped)),
                _ => None,
            })
            .ok_or_else(|| LightControlError::UnknownRoom {
                name: name.as_str().into(),
            })?;
        resources
            .iter()
            .find_map(|resource| match resource {
                Resource::Grouped { id, on, brightness } if id == grouped => Some(RoomState {
                    room: RoomRef::from_index(index),
                    on: *on,
                    brightness: *brightness,
                }),
                _ => None,
            })
            .ok_or_else(malformed)
    }
    fn scenes(&self, room: &RoomRef) -> Result<Vec<SceneState>, LightControlError> {
        let resources = self.resources()?;
        let Some(Resource::Room { id: room_id, .. }) = resources.get(room.index()) else {
            return Err(LightControlError::InvalidReference);
        };
        Ok(resources
            .iter()
            .enumerate()
            .filter_map(|(index, resource)| match resource {
                Resource::Scene {
                    room, name, active, ..
                } if room == room_id => Some(SceneState {
                    scene: SceneRef::from_index(index),
                    name: name.clone(),
                    active: *active,
                }),
                _ => None,
            })
            .collect())
    }
    fn set_power(&self, room: &RoomRef, on: bool) -> Result<(), LightControlError> {
        self.write(
            &format!("grouped_light/{}", self.grouped_id(room)?),
            json!({"on":{"on":on}}),
        )
    }
    fn set_brightness(
        &self,
        room: &RoomRef,
        change: BrightnessChange,
    ) -> Result<(), LightControlError> {
        let id = self.grouped_id(room)?;
        let body = match change {
            BrightnessChange::Absolute(level) => json!({"dimming":{"brightness":level.percent()}}),
            BrightnessChange::Step { direction, percent } => {
                if !(1..=100).contains(&percent) {
                    return Err(malformed());
                }
                json!({"dimming_delta":{"action":match direction { Direction::Up => "up", Direction::Down => "down" },"brightness_delta":percent}})
            }
        };
        self.write(&format!("grouped_light/{id}"), body)
    }
    fn set_scene(&self, scene: &SceneRef) -> Result<(), LightControlError> {
        let Some(Resource::Scene { id, .. }) = self.resources()?.get(scene.index()) else {
            return Err(LightControlError::InvalidReference);
        };
        self.write(
            &format!("scene/{id}"),
            json!({"recall":{"action":"active"}}),
        )
    }
}

#[cfg(test)]
mod tests;
