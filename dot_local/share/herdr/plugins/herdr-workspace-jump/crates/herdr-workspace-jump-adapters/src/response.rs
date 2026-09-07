use herdr_workspace_jump_application::JumpError;
use herdr_workspace_jump_domain::Workspace;
use herdr_workspace_jump_protocol::{self as protocol, ProtocolError};
use serde_json::Value;

pub(crate) fn jump_error(error: ProtocolError) -> JumpError {
    match error {
        ProtocolError::Malformed(detail) => JumpError::Malformed(detail),
        ProtocolError::Server { code, message } => JumpError::Server { code, message },
    }
}

pub(crate) fn workspaces(result: &Value) -> Result<Vec<Workspace>, JumpError> {
    protocol::workspaces_of(result)
        .map(|records| {
            records
                .into_iter()
                .map(|record| Workspace {
                    workspace_id: record.workspace_id,
                    label: record.label,
                })
                .collect()
        })
        .map_err(jump_error)
}
