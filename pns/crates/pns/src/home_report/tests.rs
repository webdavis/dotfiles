use super::{report, setup_report};
use pns_adapters::{
    SetupFailure, device_identity, enabled_router_table, parse_clients, router_api_key,
    router_settings,
};
use pns_domain::home::{
    DeviceIdentity, DeviceKey, HomePresence, HomeReading, home_reading, stale_identifiers,
};

mod presence;
mod router;
mod settings;

/// The live capture of 2026-08-20 from the UDR's
/// `/proxy/network/integration/v1/sites/{id}/clients`, verbatim: four
/// clients, the phone ("mister") among them with an iOS private MAC.
const CLIENTS_CAPTURE: &str = r#"{"offset":0,"limit":200,"count":4,"totalCount":4,"data":[{"type":"WIRED","id":"b6363aa8-67c6-326b-93da-90f8e9632d95","name":"hue-bridge-pro","connectedAt":"2026-08-14T03:01:20Z","ipAddress":"192.168.4.37","macAddress":"c4:29:96:bb:6d:cc","uplinkDeviceId":"4dd21a37-9e4e-35e8-8b94-5670c43cf93e","access":{"type":"DEFAULT"}},{"type":"WIRELESS","id":"f7585d92-906f-3c6c-84c3-89e2ce6b2eda","name":"dresden","connectedAt":"2026-08-19T05:44:57Z","ipAddress":"192.168.1.26","macAddress":"3c:06:30:0f:8a:bf","uplinkDeviceId":"4dd21a37-9e4e-35e8-8b94-5670c43cf93e","access":{"type":"DEFAULT"}},{"type":"WIRELESS","id":"71e95d68-f736-3554-b547-75fa1cdc7bf4","name":"mister","connectedAt":"2026-08-20T04:14:55Z","ipAddress":"192.168.1.169","macAddress":"2e:11:ab:6d:b0:4f","uplinkDeviceId":"4dd21a37-9e4e-35e8-8b94-5670c43cf93e","access":{"type":"DEFAULT"}},{"type":"WIRELESS","id":"84038944-790a-3965-9f7e-cfee3468308c","name":"mouse","connectedAt":"2026-08-20T05:18:12Z","ipAddress":"192.168.1.248","macAddress":"60:82:46:3c:fb:01","uplinkDeviceId":"4dd21a37-9e4e-35e8-8b94-5670c43cf93e","access":{"type":"DEFAULT"}}]}"#;

/// The configured device, through the FRONT DOOR: the identity a test
/// judges against is the one the operator's table would have produced.
fn identity(text: &str) -> DeviceIdentity {
    device_identity(&table(text)).expect("a valid device identity")
}

fn table(text: &str) -> toml::Table {
    text.parse().unwrap()
}
