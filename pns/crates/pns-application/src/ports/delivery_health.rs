#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryHealth {
    pub pending_legs: u64,
    pub deadlettered_legs: u64,
    pub growth_streak: u8,
    pub alarm_generation: Option<u64>,
    pub recording_gap: bool,
}
