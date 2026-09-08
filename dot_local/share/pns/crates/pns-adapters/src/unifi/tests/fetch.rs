use super::*;

// --- the seam, end to end ------------------------------------------------

struct FakeRouter(Option<&'static str>);
impl Router for FakeRouter {
    fn clients(&self) -> Option<Vec<Client>> {
        self.0.and_then(parse_clients)
    }
}

#[test]
fn one_reading_runs_fetch_parse_judge_in_order() {
    let device = identity("device_hostname = \"mister\"\n");
    assert_eq!(
        read_home(&FakeRouter(Some(CLIENTS_CAPTURE)), &device).presence,
        HomePresence::Home {
            matched_by: DeviceKey::Hostname,
            value: "mister".to_string(),
        }
    );
    assert_eq!(
        read_home(&FakeRouter(None), &device).presence,
        HomePresence::Unknown
    );
}
