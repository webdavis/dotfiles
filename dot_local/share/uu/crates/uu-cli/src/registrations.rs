use uu_adapters::{
    BrewLane, CommandLane, HerdrLane, LaneRegistration, NpmLane, NvimMasonLane, NvimPluginsLane,
    UvLane,
};

pub(crate) const LANES: &[LaneRegistration] = &[
    LaneRegistration::new::<BrewLane>("brew"),
    LaneRegistration::new::<CommandLane>("command"),
    LaneRegistration::new::<HerdrLane>("herdr"),
    LaneRegistration::new::<NpmLane>("npm"),
    LaneRegistration::new::<NvimMasonLane>("nvim-mason"),
    LaneRegistration::new::<NvimPluginsLane>("nvim-plugins"),
    LaneRegistration::new::<UvLane>("uv"),
];
