#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvergeFile {
    Configuration,
    Flags,
    AgentAttackSurface,
    InstalledSoftwareDrift,
    IntrusionDetection,
    SecurityPolicyRegression,
}

impl ConvergeFile {
    pub const ALL: [Self; 6] = [
        Self::Configuration,
        Self::Flags,
        Self::AgentAttackSurface,
        Self::InstalledSoftwareDrift,
        Self::IntrusionDetection,
        Self::SecurityPolicyRegression,
    ];

    pub fn relative_path(self) -> &'static str {
        match self {
            Self::Configuration => "osquery.conf",
            Self::Flags => "osquery.flags",
            Self::AgentAttackSurface => "packs/agent-attack-surface.conf",
            Self::InstalledSoftwareDrift => "packs/installed-software-drift.conf",
            Self::IntrusionDetection => "packs/intrusion-detection.conf",
            Self::SecurityPolicyRegression => "packs/security-policy-regression.conf",
        }
    }

    pub fn from_relative_path(path: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|file| file.relative_path() == path)
    }
}
