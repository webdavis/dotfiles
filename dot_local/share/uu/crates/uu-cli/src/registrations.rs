use uu_adapters::{BrewLane, CommandLane, HerdrLane, LaneRegistration, NpmLane, UvLane};

pub(crate) const LANES: &[LaneRegistration] = &[
    LaneRegistration::new::<BrewLane>("brew"),
    LaneRegistration::new::<CommandLane>("command"),
    LaneRegistration::new::<HerdrLane>("herdr"),
    LaneRegistration::new::<NpmLane>("npm"),
    LaneRegistration::new::<UvLane>("uv"),
];
