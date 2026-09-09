#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlReader {
    FileVault,
    SystemIntegrity,
    AutoLogin,
    GuestAccount,
    OverSight,
    LuluExtension,
    LuluRule,
    LuluResolvedRule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlValue {
    On,
    Off,
    Enabled,
    Disabled,
    Running,
    Stopped,
    Present,
    Absent,
}

impl ControlReader {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "fdesetup_status" => Self::FileVault,
            "csrutil_status" => Self::SystemIntegrity,
            "defaults_autologin" => Self::AutoLogin,
            "sysadminctl_guest" => Self::GuestAccount,
            "pgrep_oversight" => Self::OverSight,
            "pgrep_lulu_extension" => Self::LuluExtension,
            "lulu_rule_present" => Self::LuluRule,
            "lulu_rule_resolved_present" => Self::LuluResolvedRule,
            _ => return None,
        })
    }
    pub fn domain(self) -> &'static str {
        match self {
            Self::FileVault | Self::AutoLogin => "on off",
            Self::SystemIntegrity | Self::GuestAccount => "enabled disabled",
            Self::OverSight | Self::LuluExtension => "running stopped",
            Self::LuluRule | Self::LuluResolvedRule => "present absent",
        }
    }
    pub fn requires_target(self) -> bool {
        matches!(self, Self::LuluRule | Self::LuluResolvedRule)
    }
    pub fn value(self, value: &str) -> Option<ControlValue> {
        let parsed = ControlValue::parse(value)?;
        self.domain()
            .split(' ')
            .any(|item| item == value)
            .then_some(parsed)
    }
}

impl ControlValue {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "on" => Self::On,
            "off" => Self::Off,
            "enabled" => Self::Enabled,
            "disabled" => Self::Disabled,
            "running" => Self::Running,
            "stopped" => Self::Stopped,
            "present" => Self::Present,
            "absent" => Self::Absent,
            _ => return None,
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Present => "present",
            Self::Absent => "absent",
        }
    }
}
