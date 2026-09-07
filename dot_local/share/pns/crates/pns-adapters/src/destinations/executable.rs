use pns_domain::Delivery;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;
/// Hand one channel its event on stdin. A channel that is missing, is not
/// executable, or fails is not an error: it is simply not installed, or it
/// declined, and neither may take down the siblings or the caller.
///
/// SILENT ON THE NOTIFICATION PATH whichever verdict it answers with: the
/// common failure here is a channel nobody installed, and reporting that on
/// every event would be noise. THE TWO ARE STILL DIFFERENT VERDICTS. A channel
/// that ran and said nothing is `Silent`; one that never started is
/// `Unlaunched`, which prints nowhere an event can see and is what lets a
/// hand-run check tell a delivery from a spawn that never happened. The exit
/// status of a channel that DID run is still dropped, because a channel
/// declining is its own business.
pub fn deliver_executable(channel: &Path, event: &str, deadline: Duration) -> Delivery {
    let expires_at = std::time::Instant::now() + deadline;
    let child = match Command::new(channel).stdin(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(error) => {
            return Delivery::Unlaunched(format!(
                "could not launch the channel at {} ({error}); nothing was sent",
                channel.display()
            ));
        }
    };
    // Bound both the write and wait. Keep newline framing, inherited output
    // and the silent verdict even when a launched channel fails or times out.
    let _ = crate::finish_bounded(child, Some(&format!("{event}\n")), expires_at, 0);
    Delivery::Silent
}
/// A path from the environment, defaulting like bash's `${VAR:-default}`:
/// EMPTY means the default as much as unset does, because joining a filename
/// to an empty path resolves into the current directory and quietly delivers
/// nothing.
pub fn resolve_path(candidate: Option<&str>, default: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(
        candidate
            .filter(|value| !value.is_empty())
            .unwrap_or(default),
    )
}

#[cfg(test)]
mod tests;
