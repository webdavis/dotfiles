use super::super::{rows::Ring, scalar::Scalar};
#[derive(Clone, Copy)]
pub(super) enum Family {
    Ring(Ring),
    Scalar(Scalar),
    Return,
    Held,
    Muted,
}
pub(super) const FAMILIES: [Family; 14] = [
    Family::Ring(Ring::Decisions),
    Family::Ring(Ring::Journal),
    Family::Ring(Ring::Activity),
    Family::Ring(Ring::Presence),
    Family::Ring(Ring::PolicyAudit),
    Family::Return,
    Family::Scalar(Scalar::Quiet),
    Family::Scalar(Scalar::Staleness),
    Family::Scalar(Scalar::LightsComplaint),
    Family::Scalar(Scalar::QuietComplaint),
    Family::Scalar(Scalar::News),
    Family::Scalar(Scalar::Streak),
    Family::Held,
    Family::Muted,
];
impl Family {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Ring(ring) => ring.file(),
            Self::Return => "last-present",
            Self::Scalar(scalar) => scalar.file(),
            Self::Held => "lights-held",
            Self::Muted => "lights-quiet",
        }
    }
    pub(super) fn limit(self) -> Option<u64> {
        match self {
            Self::Ring(Ring::Activity) => Some(crate::ACTIVITY_READ_MAX),
            Self::Ring(_) => Some(crate::RING_READ_MAX),
            _ => None,
        }
    }
}
