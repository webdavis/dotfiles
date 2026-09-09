use super::{Bridge, breath_arm_body, clear_body, fade_body, inventory, pulse_body};
use pns_application::{LampBridge, LampWrite};
use pns_domain::lamps::Inventory;

pub struct TypedLampBridge<B>(pub B);
impl<B: Bridge> LampBridge for TypedLampBridge<B> {
    fn inventory(&self) -> Option<Inventory> {
        // ALL THREE OR NOTHING: failed listings are not evidence of an empty house.
        let rooms = self.0.get("room")?;
        let lamps = self.0.get("light")?;
        let zones = self.0.get("zone")?;
        Some(inventory(&rooms, &lamps, &zones))
    }
    fn write(&self, path: &str, write: &LampWrite) {
        let body = match write {
            LampWrite::Clear => clear_body(),
            LampWrite::Pulse {
                color,
                pulse,
                brightness,
            } => pulse_body(pulse, *color, *brightness),
            LampWrite::Fade {
                fade,
                color: Some(color),
            } => breath_arm_body(*color, fade),
            LampWrite::Fade { fade, color: None } => fade_body(fade),
        };
        self.0.put(path, &body);
    }
}
