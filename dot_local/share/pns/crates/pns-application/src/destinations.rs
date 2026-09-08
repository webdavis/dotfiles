use crate::NotificationDestination;
use pns_domain::registry::{RegistryError, Routing};
use pns_domain::routing::ReportMode;
use pns_domain::{Delivery, Event};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DestinationId(&'static str);

impl DestinationId {
    pub const fn new(name: &'static str) -> Self {
        // Identifiers are compiled registrations and become channel basenames.
        // A malformed declaration is a programming error, never operator input.
        assert!(!name.is_empty(), "a destination needs a name");
        let bytes = name.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            let byte = bytes[index];
            assert!(
                byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_',
                "a destination name must be a safe channel basename"
            );
            index += 1;
        }
        Self(name)
    }

    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

pub struct DeliveryRequest<'a> {
    pub producer: &'a str,
    pub request_id: Option<&'a str>,
    pub event: &'a Event,
    pub route: &'a str,
    pub mode: ReportMode,
}

pub struct Recorded<D, R> {
    destination: D,
    record: R,
}

impl<D, R> Recorded<D, R> {
    pub fn new(destination: D, record: R) -> Self {
        Self {
            destination,
            record,
        }
    }
}

impl<D, R> NotificationDestination for Recorded<D, R>
where
    D: NotificationDestination,
    R: Fn(&DeliveryRequest<'_>, &Delivery) -> Result<(), String> + Send + Sync,
{
    fn id(&self) -> &DestinationId {
        self.destination.id()
    }

    fn capabilities(&self) -> Routing {
        self.destination.capabilities()
    }

    fn deliver(&self, request: &DeliveryRequest<'_>) -> Delivery {
        let outcome =
            crate::deliver_guarded(self.id().as_str(), || self.destination.deliver(request));
        // The recorder reports its own storage failures. A failed write cannot
        // change whether the destination confirmed delivery.
        let _ = (self.record)(request, &outcome);
        outcome
    }
}

pub struct Destinations<D> {
    entries: Vec<D>,
}

impl<D: NotificationDestination> Destinations<D> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn register(&mut self, destination: D) -> Result<(), RegistryError> {
        if self
            .entries
            .iter()
            .any(|entry| entry.id() == destination.id())
        {
            return Err(RegistryError::Duplicate(destination.id().as_str().into()));
        }
        self.entries.push(destination);
        Ok(())
    }

    pub fn deliver(&self, name: &str, request: &DeliveryRequest<'_>) -> Delivery {
        match self.get(name) {
            Some(destination) => destination.deliver(request),
            None => Delivery::Unlaunched(format!(
                "the {name} destination is not registered; nothing was sent"
            )),
        }
    }

    pub fn get(&self, name: &str) -> Option<&D> {
        self.entries
            .iter()
            .find(|entry| entry.id().as_str() == name)
    }

    pub fn iter(&self) -> impl Iterator<Item = &D> {
        self.entries.iter()
    }

    pub fn durable(&self) -> Option<&D> {
        self.entries
            .iter()
            .find(|entry| entry.capabilities().durable)
    }
}

impl<D: NotificationDestination> Default for Destinations<D> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D: NotificationDestination + ?Sized> NotificationDestination for Box<D> {
    fn id(&self) -> &DestinationId {
        (**self).id()
    }

    fn capabilities(&self) -> Routing {
        (**self).capabilities()
    }

    fn deliver(&self, request: &DeliveryRequest<'_>) -> Delivery {
        (**self).deliver(request)
    }
}

impl<D: NotificationDestination + ?Sized> NotificationDestination for &D {
    fn id(&self) -> &DestinationId {
        (**self).id()
    }

    fn capabilities(&self) -> Routing {
        (**self).capabilities()
    }

    fn deliver(&self, request: &DeliveryRequest<'_>) -> Delivery {
        (**self).deliver(request)
    }
}

#[cfg(test)]
mod tests;
