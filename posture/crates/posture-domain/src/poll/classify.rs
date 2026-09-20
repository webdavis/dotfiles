use crate::ControlValue;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlReading {
    Known(ControlValue),
    Indeterminate,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LuluProfile {
    Base,
    Active,
    Unconfirmed,
}
pub fn classify_messages(
    output: &str,
    exit: i32,
    needles: &[(&str, ControlValue)],
) -> ControlReading {
    if exit != 0 {
        return ControlReading::Indeterminate;
    }
    let mut found = None;
    for &(needle, value) in needles {
        if output.contains(needle) {
            if found.is_some_and(|prior| prior != value) {
                return ControlReading::Indeterminate;
            }
            found = Some(value);
        }
    }
    found.map_or(ControlReading::Indeterminate, ControlReading::Known)
}
pub fn classify_filevault(output: &str, exit: i32) -> ControlReading {
    use ControlValue::{Off, On};
    classify_messages(
        output,
        exit,
        &[
            ("FileVault is On.", On),
            ("FileVault is On, but needs to be restarted to finish.", On),
            ("FileVault is Off.", Off),
            (
                "FileVault is Off, but will be enabled after the next restart.",
                Off,
            ),
            (
                "FileVault is Off, but needs to be restarted to finish.",
                Off,
            ),
        ],
    )
}
pub fn classify_pgrep(output: &str, exit: i32) -> ControlReading {
    if exit == 0
        && output
            .split('\n')
            .all(|line| !line.is_empty() && line.bytes().all(|b| b.is_ascii_digit()))
    {
        ControlReading::Known(ControlValue::Running)
    } else if exit == 1 && output.is_empty() {
        ControlReading::Known(ControlValue::Stopped)
    } else {
        ControlReading::Indeterminate
    }
}
/// `None` when the login window preferences could not be read.
pub fn classify_autologin(auto_login_user_declared: Option<bool>) -> ControlReading {
    match auto_login_user_declared {
        Some(true) => ControlReading::Known(ControlValue::On),
        Some(false) => ControlReading::Known(ControlValue::Off),
        None => ControlReading::Indeterminate,
    }
}
/// `None` when the LuLu base preferences could not be read.
pub fn classify_lulu_profile(current_profile_declared: Option<bool>) -> LuluProfile {
    match current_profile_declared {
        Some(true) => LuluProfile::Active,
        Some(false) => LuluProfile::Base,
        None => LuluProfile::Unconfirmed,
    }
}
