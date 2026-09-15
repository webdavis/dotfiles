use crate::TapFailure;
use pns_protocol::{TapInstall, TapInstallStep};
use std::ffi::CStr;

#[cfg(test)]
mod tests;

fn unavailable() -> TapFailure {
    TapFailure::new(
        "install_context_unavailable",
        "the executable, host or account name is unavailable",
    )
}

pub fn tap_install() -> Result<TapInstall, TapFailure> {
    let binary = std::env::current_exe()
        .map_err(|_| unavailable())?
        .into_os_string()
        .into_string()
        .map_err(|_| unavailable())?;
    let mut host = [0_u8; 256];
    // SAFETY: host is writable for the stated length. Termination is checked below.
    if unsafe { libc::gethostname(host.as_mut_ptr().cast(), host.len()) } != 0 {
        return Err(unavailable());
    }
    let host = CStr::from_bytes_until_nul(&host)
        .map_err(|_| unavailable())?
        .to_str()
        .map_err(|_| unavailable())?;
    // SAFETY: getuid has no preconditions. getpwuid_r writes only into the supplied
    // passwd and buffer; a successful non-null result points into those live objects.
    let user = unsafe {
        let mut entry: libc::passwd = std::mem::zeroed();
        let mut buffer = vec![0_u8; 16_384];
        let mut result = std::ptr::null_mut();
        if libc::getpwuid_r(
            libc::getuid(),
            &mut entry,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        ) != 0
            || result.is_null()
            || entry.pw_name.is_null()
        {
            return Err(unavailable());
        }
        CStr::from_ptr(entry.pw_name)
            .to_str()
            .map_err(|_| unavailable())?
            .to_string()
    };
    install_guide(&binary, host, &user)
}

fn install_guide(binary: &str, host: &str, user: &str) -> Result<TapInstall, TapFailure> {
    if !std::path::Path::new(binary).is_absolute()
        || [binary, host, user]
            .iter()
            .any(|text| text.is_empty() || text.chars().any(char::is_control))
    {
        return Err(unavailable());
    }
    // First quote one shell argument, then escape the authorized_keys option string.
    let command = format!("'{}' tap", binary.replace('\'', "'\\''"));
    let command = command.replace('"', "\\\"");
    let line = format!("command=\"{command}\",restrict ssh-ed25519 <your-Shortcut-public-key>");
    let step = |title: &str, blurb: &str, lines: Vec<String>| TapInstallStep {
        title: title.into(),
        blurb: blurb.into(),
        lines,
    };
    Ok(TapInstall {
        binary: binary.into(), host: host.into(), user: user.into(), authorized_key_line: line.clone(),
        shortcut_url: None, verified_ios: None,
        steps: vec![
            step("1. This Mac", "the authorized_keys line", vec![
                "Enable Remote Login for this account first, in System Settings, General, Sharing, Remote Login. Nothing below works without it.".into(),
                "Paste this line into ~/.ssh/authorized_keys yourself. Replace an existing entry for this dedicated key; do not add a duplicate.".into(),
                line,
                "command= runs this pns binary with tap whenever the key connects.".into(),
                "restrict disables forwarding, a terminal and other optional SSH facilities.".into(),
                "Replace the public-key placeholder with the key from the phone's SSH action. pns does not read or edit this file.".into(),
            ]),
            step("2. Your phone", "the PNS Tap shortcut", vec![
                "A Shortcut install link is not included. Use the manual setup below.".into(),
                "Use Run Script Over SSH in Shortcuts with these fields:".into(),
                format!("Host: {host} (local hostname; verify it is reachable from the phone)"),
                "Port: the SSH port configured for this Mac".into(),
                format!("User: {user}"),
                "Auth: SSH Key; select the key whose public half you pasted on this Mac".into(),
                "Script: pns tap".into(),
                "sshd ignores this script text and runs the forced command. Display its output only after success; show SSH errors as failures.".into(),
                "For JSON output, change the forced command on this Mac to pns tap --json. Changing the phone's script alone has no effect.".into(),
                "If the Mac cannot answer, the SSH action fails and the Shortcut stops there, so the phone shows that SSH error and never the success notification. Check Remote Login first, then host, port and connectivity.".into(),
            ]),
            step("3. Trigger methods", "Back Tap, Action Button, others", vec![
                "Attach the Shortcut to Back Tap, an Action Button, a Lock Screen widget, Control Center or Siri where your device supports it.".into(),
                "These iOS settings paths have not been verified yet. Check them on your phone before relying on this setup.".into(),
            ]),
        ],
        undo: vec![
            "To undo an accidental tap, type on the unlocked Mac. Newer desk input wins.".into(),
            "To disable this integration, detach its phone triggers and manually remove only its dedicated authorization entry.".into(),
            "To restore a previous command for the same still-trusted key, review that command and make its marker path match the reader. Do not restore the whole trust file.".into(),
            "Revert a custom marker path in its owning configuration source and restore any environment override in its originating context. Leave marker files in place.".into(),
            "Keep Remote Login enabled if another workflow uses it. Deleting a marker can move Mobile to Away and cause more phone cards.".into(),
        ],
    })
}
