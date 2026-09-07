pub use pns_adapters::{
    PRESENCE_READ_MAX as READ_MAX, PRESENCE_STATE_FILE as STATE_FILE, ROOM_MAX,
    parse_presence_line, render_presence_line as render, room_fits,
};
pub use pns_domain::{Edge, RawPresence};
