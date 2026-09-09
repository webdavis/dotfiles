use pns_application::{DeliveryRequest, DestinationId, NotificationDestination};
use pns_domain::{Delivery, registry::Routing};
use pns_protocol::RequestId;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
pub struct ExecutableDestination {
    id: DestinationId,
    capabilities: Routing,
    channel: PathBuf,
    deadline: Duration,
    stdout_to_stderr: bool,
}

impl ExecutableDestination {
    pub fn stdout_to_stderr(mut self, enabled: bool) -> Self {
        self.stdout_to_stderr = enabled;
        self
    }

    pub fn new(
        id: DestinationId,
        capabilities: Routing,
        channel: PathBuf,
        deadline: Duration,
    ) -> Self {
        Self {
            id,
            capabilities,
            channel,
            deadline,
            stdout_to_stderr: false,
        }
    }
}

impl NotificationDestination for ExecutableDestination {
    fn id(&self) -> &DestinationId {
        &self.id
    }
    fn capabilities(&self) -> Routing {
        self.capabilities
    }

    fn deliver(&self, request: &DeliveryRequest<'_>) -> Delivery {
        let body = match request.request_id {
            None => Ok(crate::event_json(request.event, request.mode)),
            Some(_) => egress(request),
        };
        let body = match body {
            Ok(body) => body,
            Err(reason) => {
                return Delivery::Unlaunched(format!(
                    "could not encode the channel request ({reason}); nothing was sent"
                ));
            }
        };
        deliver_executable(
            &self.channel,
            &body,
            request.request_id,
            request.producer,
            self.deadline,
            self.stdout_to_stderr,
        )
    }
}

fn egress(request: &DeliveryRequest<'_>) -> Result<String, String> {
    RequestId::new(request.request_id.ok_or("request identity unavailable")?)
        .map_err(|error| format!("request id {error}"))?;
    Ok(crate::event_json(request.event, request.mode))
}

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
fn deliver_executable(
    channel: &Path,
    event: &str,
    request_id: Option<&str>,
    producer: &str,
    deadline: Duration,
    stdout_to_stderr: bool,
) -> Delivery {
    let expires_at = std::time::Instant::now() + deadline;
    let mut command = Command::new(channel);
    if stdout_to_stderr {
        use std::os::fd::AsFd;
        match std::io::stderr().as_fd().try_clone_to_owned() {
            Ok(stderr) => {
                command.stdout(Stdio::from(stderr));
            }
            Err(_) => {
                return Delivery::Unlaunched("channel output unavailable; nothing was sent".into());
            }
        }
    }
    command.env_remove("PNS_REQUEST_ID");
    if let Some(request_id) = request_id {
        command.env("PNS_REQUEST_ID", request_id);
    }
    command.env("PNS_PRODUCER", producer).stdin(Stdio::piped());
    // Bound both the write and wait. Keep newline framing, configured output
    // and the silent verdict even when a launched channel fails or times out.
    if let Err(error) =
        crate::finish_bounded(&mut command, Some(&format!("{event}\n")), expires_at, 0)
    {
        return Delivery::Unlaunched(format!(
            "could not launch the channel at {} ({error}); nothing was sent",
            channel.display()
        ));
    }
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
