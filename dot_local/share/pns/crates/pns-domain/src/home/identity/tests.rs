use super::{Client, DeviceIdentity, DeviceIdentityError, DeviceKey};
use crate::home::reading::{HomePresence, home_reading};

#[test]
fn a_public_identity_normalizes_mac_before_matching_the_router() {
    let device = DeviceIdentity::new(None, None, Some("2E-11-AB-6D-B0-4F".to_string()))
        .expect("a valid device identity");
    let clients = Some(vec![Client {
        name: None,
        ipv4: None,
        mac: Some("2E-11-AB-6D-B0-4F".to_string()),
    }]);
    assert_eq!(
        home_reading(clients, &device).presence,
        HomePresence::Home {
            matched_by: DeviceKey::Mac,
            value: "2e:11:ab:6d:b0:4f".to_string(),
        }
    );
}

#[test]
fn a_public_identity_requires_at_least_one_identifier() {
    assert_eq!(
        DeviceIdentity::new(None, None, None),
        Err(DeviceIdentityError::NoIdentifier)
    );
}

#[test]
fn a_public_identity_refuses_an_empty_hostname_even_with_an_address() {
    assert_eq!(
        DeviceIdentity::new(
            Some(String::new()),
            Some("192.168.1.169".parse().unwrap()),
            None
        ),
        Err(DeviceIdentityError::InvalidKey {
            key: DeviceKey::Hostname,
            value: String::new(),
        })
    );
}

#[test]
fn a_public_identity_refuses_a_malformed_mac_even_with_a_hostname() {
    assert_eq!(
        DeviceIdentity::new(
            Some("mister".to_string()),
            None,
            Some("not-a-mac".to_string())
        ),
        Err(DeviceIdentityError::InvalidKey {
            key: DeviceKey::Mac,
            value: "not-a-mac".to_string(),
        })
    );
}
