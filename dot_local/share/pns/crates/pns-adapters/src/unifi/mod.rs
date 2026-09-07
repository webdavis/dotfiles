use pns_application::Router;
use pns_domain::home::{DeviceIdentity, HomeReading, home_reading};
mod client;
mod parse;
pub use client::UniFiRouter;
pub use parse::{first_site_id, parse_clients};

/// One reading: fetch through the seam, parse, judge.
pub fn read_home<R: Router>(router: &R, device: &DeviceIdentity) -> HomeReading {
    home_reading(
        router.clients_json().as_deref().and_then(parse_clients),
        device,
    )
}

#[cfg(test)]
mod tests;
