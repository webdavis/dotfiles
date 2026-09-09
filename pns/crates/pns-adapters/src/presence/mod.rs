mod bridge;
mod instant;
mod lock;
mod state_file;
pub use bridge::{poll as poll_bridge_presence, reading as read_bridge_presence};
pub use instant::instant_from_utc;
pub use lock::{
    Claim as PresenceClaim, LOCK_FILE as PRESENCE_LOCK_FILE, claim as claim_presence_poll,
};
pub use state_file::{
    READ_MAX as PRESENCE_READ_MAX, STATE_FILE as PRESENCE_STATE_FILE, parse_presence_line,
    render as render_presence_line,
};

mod poll;
pub use poll::BridgePresencePoll;
