//! `uu schedule render`: the launchd job for the configured day and time.
//!
//! TWO SCHEDULE TRUTHS, and this module serves the standalone one. A machine
//! whose plist is managed elsewhere takes its timing from that plist; this
//! renders one for a machine that has none, so `uu schedule render > ~/Library/
//! LaunchAgents/<label>.plist` is the whole install.

use crate::config::Schedule;
use std::path::{Path, PathBuf};

/// The launchd label the rendered job carries.
pub const DEFAULT_LABEL: &str = "com.webdavis.uu";

/// Where the apply-time build puts the binary launchd runs.
pub fn installed_binary(home: &str) -> PathBuf {
    Path::new(home).join(".cargo/bin/uu")
}

/// Where the job's own output goes. The DIRECTORY is the operator's to make;
/// see `render_plist`.
pub fn log_path(home: &str) -> PathBuf {
    Path::new(home).join(".local/log/uu/uu.log")
}

/// What a lane's own child processes are given to search, since launchd hands
/// a job almost no environment at all. It mirrors the tracked plist's list:
/// the Node runtime first, then Cargo and the operator's bin, then system tools.
fn search_path(home: &str) -> String {
    format!(
        "{home}/.local/share/fnm/aliases/default/bin:{home}/.cargo/bin:{home}/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
    )
}

/// One launchd job, as a property list, for a standalone install under `home`.
///
/// THE ENVIRONMENT IS STATED, because launchd gives a job almost none. `uu
/// run` refuses outright without HOME, and a lane's child processes find
/// nothing without PATH, so a plist that omits them renders a job that cannot
/// work. Both mirror the tracked plist this machine loads.
///
/// THE LOG DIRECTORY IS THE OPERATOR'S TO MAKE, and the plist says so in a
/// comment above the paths. launchd creates the log FILE but never its
/// directory, and a job whose output cannot be opened does not start; on this
/// machine the loader script makes it, and a standalone install has no loader.
///
/// EVERY INTERPOLATED VALUE IS XML-ESCAPED. A home directory may legitimately
/// hold `&`, and an unescaped one renders a plist launchd refuses to parse at
/// all, which is a job that silently never loads.
pub fn render_plist(label: &str, home: &str, schedule: Schedule) -> String {
    let label = escape(label);
    let program = escape(&installed_binary(home).display().to_string());
    let log_path = escape(&log_path(home).display().to_string());
    let search_path = escape(&search_path(home));
    let home = escape(home);
    let Schedule {
        weekday,
        hour,
        minute,
    } = schedule;
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n  \
           <key>Label</key>\n  <string>{label}</string>\n  \
           <key>ProgramArguments</key>\n  <array>\n    \
             <string>{program}</string>\n    <string>run</string>\n  </array>\n  \
           <key>EnvironmentVariables</key>\n  <dict>\n    \
             <key>HOME</key>\n    <string>{home}</string>\n    \
             <key>PATH</key>\n    <string>{search_path}</string>\n  </dict>\n  \
           <key>RunAtLoad</key>\n  <false/>\n  \
           <key>StartCalendarInterval</key>\n  <dict>\n    \
             <key>Weekday</key>\n    <integer>{weekday}</integer>\n    \
             <key>Hour</key>\n    <integer>{hour}</integer>\n    \
             <key>Minute</key>\n    <integer>{minute}</integer>\n  </dict>\n  \
           <!-- launchd creates the log file but never its directory, and a \
                job whose output cannot be opened does not start:\n       \
                mkdir -p the directory holding the two paths below before \
                loading this job. -->\n  \
           <key>StandardOutPath</key>\n  <string>{log_path}</string>\n  \
           <key>StandardErrorPath</key>\n  <string>{log_path}</string>\n\
         </dict>\n</plist>\n"
    )
}

/// XML text-node escaping, the five characters that matter inside an element.
/// The ampersand goes FIRST, or every escape this function just wrote would be
/// escaped again.
fn escape(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests;
