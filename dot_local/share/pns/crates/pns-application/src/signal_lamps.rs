use crate::lamp_narrowing::narrow_to_presence;
use crate::lamp_routing::routing_complaints;
use crate::{LampBridge, LampWrite, PresenceDecisions};

pub struct SignalLamps<'a, B, P> {
    pub bridge: &'a B,
    pub presence: &'a P,
}
impl<B: LampBridge, P: PresenceDecisions> SignalLamps<'_, B, P> {
    /// The event path's routed writes: one pulse body per lamp the behaviour is
    /// routed for, with the mute and the TICK'S held record each answered at the
    /// per-lamp decision, once.
    ///
    /// IT ANSWERS WITH THE RESOLUTION'S COMPLAINTS rather than printing or dropping
    /// them. This path resolves the map on every pulse, so it is where a mistyped
    /// name on a pulse-only config is met; the caller owns the say-once memory.
    ///
    /// A HELD RECORD OF `None` IS EVERY LAMP HELD, which is the fail-dark direction
    /// on the one gate that decides whether a blink writes over a breath. Read as
    /// nothing held, an unreadable record let the pulse flash straight over a lamp
    /// that was breathing about a question.
    pub fn run(
        &self,
        lights: &pns_domain::lamps::config::Lights,
        behaviour: pns_domain::lamps::config::Behaviour,
        reading: &pns_domain::lamps::Reading<'_>,
        held: Option<&[String]>,
        presence: Option<&pns_domain::Snapshot>,
    ) -> Vec<String> {
        // A BRIDGE THAT ANSWERED NOTHING RESOLVES NOTHING, and says nothing here.
        // The doctor is where an unreachable bridge is reported; a warning on every
        // notification for the rest of a machine's life is noise.
        let Some(routing) = self
            .bridge
            .inventory()
            .map(|inventory| pns_domain::lamps::resolve(&inventory, lights))
        else {
            return Vec::new();
        };
        // THE COMPLAINTS COME OFF THE WHOLE RESOLUTION, before the narrowing: a
        // lamp name the bridge could not answer is a typo in the config whether or
        // not the operator is standing in that room.
        let mut routing = routing;
        let complaints = routing_complaints(&routing);
        // WHAT THIS EVENT WOULD ACTUALLY WRITE, decided ONCE and used twice: as
        // the set presence narrows over, and as the write itself. It answers the
        // whole per-lamp question, the mute, the routing, the held record and the
        // dim window together, because "eligible" has to mean "would light" or the
        // narrowing's own fallback is judging the wrong set.
        let write_for = |routed: &pns_domain::lamps::Routed| -> Option<(String, LampWrite)> {
            let path = pns_domain::lamps::fixture_path(&pns_domain::lamps::Fixture::Light(
                routed.lamp.id.clone(),
            ));
            let lamp_is_held = held.is_none_or(|held| held.contains(&path));
            if pns_domain::lamps::muted_now(&routed.lamp, reading.muted)
                || !pns_domain::lights::held::pulse_fires(&routed.shows, behaviour, lamp_is_held)
            {
                return None;
            }
            let showing =
                pns_domain::lamps::dim_showing(routed.dim.as_ref(), behaviour, reading.minutes_now);
            let (color, pulse, brightness) =
                pns_domain::lamps::pulse_render(behaviour, lights, showing)?;
            Some((
                path,
                LampWrite::Pulse {
                    pulse,
                    color,
                    brightness,
                },
            ))
        };
        // NARROWED OVER THE ELIGIBLE SET AND NOT THE WHOLE ONE. A room holding a
        // lamp that carries some OTHER behaviour is a room this event lights
        // nothing in, so narrowing to it and filtering afterwards produced exactly
        // the silence the fallback exists to prevent.
        routing.lamps.retain(|routed| write_for(routed).is_some());
        let routing = narrow_to_presence(self.presence, routing, presence);
        for routed in &routing.lamps {
            if let Some((path, body)) = write_for(routed) {
                self.bridge.write(&path, &body);
            }
        }
        complaints
    }
}
