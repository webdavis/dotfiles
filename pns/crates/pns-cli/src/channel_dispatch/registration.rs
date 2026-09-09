use super::EXECUTABLE_DEADLINE;
use pns_adapters::ExecutableDestination;
use pns_application::{DeliveryRequest, DestinationId, Destinations, NotificationDestination};
use pns_domain::{
    Delivery,
    registry::{PluginKind, Routing, Selection},
};
use std::path::Path;

pub(super) fn choose<D: NotificationDestination + 'static>(
    native: D,
    forced_directory: Option<&Path>,
    refusal: Option<String>,
    json: bool,
) -> Box<dyn NotificationDestination> {
    // A refused backend runs neither seam, including an explicitly selected executable.
    if let Some(line) = refusal {
        return Box::new(Refused {
            id: *native.id(),
            capabilities: native.capabilities(),
            line,
        });
    }
    match forced_directory {
        Some(directory) => Box::new(executable(
            *native.id(),
            native.capabilities(),
            directory,
            json,
        )),
        None => Box::new(native),
    }
}

pub(super) fn assemble(
    selection: &Selection,
    mut native: Vec<Box<dyn NotificationDestination>>,
    directory: &Path,
    json: bool,
) -> Destinations<Box<dyn NotificationDestination>> {
    let mut destinations = Destinations::new();
    for registration in selection.iter() {
        let PluginKind::Channel(capabilities) = registration.kind else {
            continue;
        };
        if !capabilities.event_dispatched {
            continue;
        }
        let destination = match native
            .iter()
            .position(|entry| entry.id().as_str() == registration.name)
        {
            Some(index) => native.remove(index),
            None => Box::new(executable(
                DestinationId::new(registration.name),
                capabilities,
                directory,
                json,
            )),
        };
        destinations
            .register(destination)
            .expect("a validated selection has unique channel names");
    }
    destinations
}

fn executable(
    id: DestinationId,
    capabilities: Routing,
    directory: &Path,
    json: bool,
) -> ExecutableDestination {
    ExecutableDestination::new(
        id,
        capabilities,
        directory.join(format!("{}.sh", id.as_str())),
        EXECUTABLE_DEADLINE,
    )
    .stdout_to_stderr(json)
}

struct Refused {
    id: DestinationId,
    capabilities: Routing,
    line: String,
}

impl NotificationDestination for Refused {
    fn id(&self) -> &DestinationId {
        &self.id
    }
    fn capabilities(&self) -> Routing {
        self.capabilities
    }
    fn deliver(&self, _request: &DeliveryRequest<'_>) -> Delivery {
        Delivery::Failed(self.line.clone())
    }
}
