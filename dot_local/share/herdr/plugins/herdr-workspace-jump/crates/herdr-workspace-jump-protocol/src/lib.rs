mod focused;
mod wire;

pub use focused::parse_focused_id;
pub use wire::{
    ProtocolError, WireWorkspace, create_request, focus_request, list_request, result_of,
    workspaces_of,
};
