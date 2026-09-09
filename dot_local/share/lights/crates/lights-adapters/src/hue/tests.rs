mod reads;
mod transport;
mod writes;

use super::*;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use transport::{ScriptedConnector, ScriptedResolver};

fn fixture() -> Value {
    let mut data = vec![
        json!({"id":id(1),"type":"room","children":[],"metadata":{"name":"Studio","archetype":"office"},"services":[{"rid":id(2),"rtype":"grouped_light"}]}),
        json!({"id":id(2),"type":"grouped_light","owner":{"rid":id(1),"rtype":"room"},"on":{"on":true},"dimming":{"brightness":42.75}}),
        json!({"id":id(3),"type":"room","children":[],"metadata":{"name":"Bedroom","archetype":"bedroom"},"services":[{"rid":id(4),"rtype":"grouped_light"}]}),
        json!({"id":id(4),"type":"grouped_light","owner":{"rid":id(3),"rtype":"room"},"on":{"on":false}}),
    ];
    for (n, room, name, active) in [
        (5, 3, "Read", "static"),
        (6, 1, "Read", "static"),
        (7, 1, "Dimmed", "dynamic_palette"),
        (8, 1, "Energize", "inactive"),
        (9, 1, "Concentrate", "inactive"),
    ] {
        data.push(json!({"id":id(n),"type":"scene","owner":{"rid":id(room),"rtype":"room"},"metadata":{"name":name},"group":{"rid":id(room),"rtype":"room"},"status":{"active":active},"actions":[],"speed":0.5,"auto_dynamic":false}));
    }
    json!({"errors":[],"data":data})
}
fn id(n: usize) -> String {
    format!("00000000-0000-0000-0000-{n:012}")
}
fn room() -> RoomName {
    RoomName::new("Studio").unwrap()
}
fn setup(responses: Vec<(u16, Value)>) -> (HueLightController, Arc<Mutex<Vec<Vec<u8>>>>) {
    let connector = ScriptedConnector::new(responses);
    let requests = connector.requests.clone();
    let settings =
        crate::settings::parse("[controller]\ntype='hue'\naddress='192.0.2.1'\nkey='test-secret'")
            .unwrap();
    (
        HueLightController::with_transport(&settings.controller, connector, ScriptedResolver),
        requests,
    )
}

mod contract;
mod timeout;
