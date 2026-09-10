use uu_adapters::{
    BrewLane, CargoLane, ClaudePluginsLane, CommandLane, HerdrLane, LaneRegistration, NpmLane,
    NvimMasonLane, NvimParsersLane, NvimPluginsLane, NvimSmokeTestLane, SkillsConfig, UvLane,
};

pub(crate) const LANES: &[LaneRegistration] = &[
    LaneRegistration::new::<BrewLane>("brew"),
    LaneRegistration::new::<CargoLane>("cargo"),
    LaneRegistration::new::<ClaudePluginsLane>("claude-plugins"),
    LaneRegistration::new::<CommandLane>("command"),
    LaneRegistration::new::<HerdrLane>("herdr"),
    LaneRegistration::new::<NpmLane>("npm"),
    LaneRegistration::new::<NvimMasonLane>("nvim-mason"),
    LaneRegistration::new::<NvimParsersLane>("nvim-parsers"),
    LaneRegistration::new::<NvimPluginsLane>("nvim-plugins"),
    LaneRegistration::new::<NvimSmokeTestLane>("nvim-smoke-test"),
    LaneRegistration::new::<SkillsConfig>("skills"),
    LaneRegistration::new::<UvLane>("uv"),
];
