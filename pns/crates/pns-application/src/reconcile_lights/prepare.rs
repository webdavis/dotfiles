use super::*;
use crate::lamp_narrowing::narrow_to_presence;
use crate::lamp_routing::routing_complaints;

impl<B: LampBridge, H, T, P: PresenceDecisions> ReconcileLights<'_, B, H, T, P> {
    pub(super) fn breathing(
        &self,
        tick: &TickReading<'_>,
        complaints: &mut Vec<String>,
    ) -> Option<Vec<Breathing>> {
        let TickReading {
            lights,
            active,
            reading,
            held_before,
            now_ms,
            presence,
        } = *tick;
        let mut breathing: Vec<Breathing> = Vec::new();
        if !active.is_empty() {
            // The doctor is where an unreachable bridge is REPORTED; this process
            // runs unattended and has no reader for that sentence.
            let mut routing = self
                .bridge
                .inventory()
                .map(|inventory| pns_domain::lamps::resolve(&inventory, lights))?;
            // OFF THE WHOLE RESOLUTION, before the narrowing, for
            // `run_pulse_writes`'s reason: a name the bridge could not answer is a
            // typo whether or not the operator is standing in that room.
            complaints.extend(routing_complaints(&routing));
            // WHAT THIS TICK WOULD ACTUALLY ARM, in `run_pulse_writes`'s shape and
            // for its reason: presence has to narrow over the lamps this house
            // state reaches, or a room whose only lamp carries some other state
            // reads as narrowable and then breathes on nothing.
            let breath_for = |routed: &pns_domain::lamps::Routed| -> Option<(
                pns_domain::lights::held::Held,
                pns_domain::lamps::Showing,
            )> {
                if pns_domain::lamps::muted_now(&routed.lamp, reading.muted) {
                    return None;
                }
                let held = pns_domain::lights::held::shown(active, &routed.shows)?;
                let showing = pns_domain::lamps::dim_showing(
                    routed.dim.as_ref(),
                    held.behaviour(),
                    reading.minutes_now,
                );
                (showing != pns_domain::lamps::Showing::Dark).then_some((held, showing))
            };
            routing.lamps.retain(|routed| breath_for(routed).is_some());
            let routing = narrow_to_presence(self.presence, routing, presence);
            for routed in &routing.lamps {
                let Some((held, showing)) = breath_for(routed) else {
                    continue;
                };
                let (color, cycle) = pns_domain::lamps::held_render(held, lights, showing);
                let path = pns_domain::lamps::fixture_path(&pns_domain::lamps::Fixture::Light(
                    routed.lamp.id.clone(),
                ));
                // A LAMP NOT NAMED IN LAST TICK'S RECORD, OR NAMED THERE WITH NO
                // PHASE, RESUMES AT THE DEFAULT: a fresh arm, an external switch,
                // a killed child's bare token and a dim-window shape change all
                // read the same way, and all cost at most one fade of motion.
                let previous =
                    held_before.and_then(|entries| entries.iter().find(|entry| entry.path == path));
                let resume = pns_domain::lights::phase::resume_from(previous, now_ms, held, &cycle);
                breathing.push(Breathing {
                    path,
                    held,
                    cycle,
                    color,
                    resume,
                });
            }
        }
        Some(breathing)
    }
}
