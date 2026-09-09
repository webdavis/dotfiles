mod focus;
pub use focus::{FocusReading, focus_now};

mod clock;
pub(crate) mod desk;
pub(crate) mod phone;
pub use clock::{local_minutes_since_midnight, utc_timestamp};
