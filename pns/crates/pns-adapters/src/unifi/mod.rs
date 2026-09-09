mod client;
mod parse;
pub use client::UniFiRouter;
pub use parse::{first_site_id, parse_clients};

#[cfg(test)]
mod tests;

mod memory;
pub use memory::HomeStaleness;
