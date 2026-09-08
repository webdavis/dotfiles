use pns_domain::{
    Narrowing, Snapshot, lamps::Inventory, lamps::config::Pulse, lights::breath::Fade,
    lights::phase::HeldEntry, pulse::PulseColor,
};

pub trait LampBridge {
    fn inventory(&self) -> Option<Inventory>;
    fn write(&self, path: &str, write: &LampWrite);
}

pub enum LampWrite {
    Clear,
    Pulse {
        color: PulseColor,
        pulse: Pulse,
        brightness: u8,
    },
    Fade {
        fade: Fade,
        color: Option<PulseColor>,
    },
}

pub trait HeldLamps {
    fn read(&self) -> Option<Vec<HeldEntry>>;
    fn remember(&self, entries: &[HeldEntry]) -> Result<(), String>;
}

pub trait LampTickClaim {
    type Guard;
    fn claim(&self, now_secs: u64) -> Option<Self::Guard>;
}

pub trait PresenceDecisions {
    fn record(&self, snapshot: &Snapshot, decision: &Narrowing);
}
