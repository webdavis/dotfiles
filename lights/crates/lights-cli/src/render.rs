use lights_application::LightsError;

pub(super) fn error(error: LightsError) -> (u8, String) {
    match error {
        LightsError::UnknownRoom { name } => (2, format!("unknown room {name:?}")),
        LightsError::UnknownScene { name, room } => {
            (3, format!("unknown scene {name:?} in room {room:?}"))
        }
        LightsError::Unreachable { detail }
        | LightsError::Refused { detail }
        | LightsError::Malformed { detail } => (4, detail),
        LightsError::InvalidReference => (4, "invalid controller reference".into()),
    }
}
