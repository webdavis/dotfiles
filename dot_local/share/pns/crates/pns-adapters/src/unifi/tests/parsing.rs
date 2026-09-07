use super::*;

// --- parsing the router's listing ---------------------------------------

#[test]
fn every_client_in_the_live_capture_is_read_with_all_three_of_its_fields() {
    let clients = parse_clients(CLIENTS_CAPTURE).expect("the live capture is a listing");
    assert_eq!(clients.len(), 4);
    assert_eq!(
        clients[2],
        Client {
            name: Some("mister".to_string()),
            ipv4: Some("192.168.1.169".to_string()),
            mac: Some("2e:11:ab:6d:b0:4f".to_string()),
        }
    );
    // An UNNAMED client STAYS in the listing now. The router has not
    // identified it, but it still carries the MAC and the address a
    // configured device can match on, and dropping it would answer
    // NotHome for a device sitting right there in the list.
    assert_eq!(
        parse_clients(r#"{"data":[{"macAddress":"2E:11:AB:6D:B0:4F"},{"name":"mister"}]}"#),
        Some(vec![
            Client {
                name: None,
                ipv4: None,
                mac: Some("2E:11:AB:6D:B0:4F".to_string()),
            },
            Client {
                name: Some("mister".to_string()),
                ipv4: None,
                mac: None,
            },
        ])
    );
}

#[test]
fn a_listing_that_is_not_json_is_no_answer_rather_than_an_empty_wifi() {
    // Unparseable and empty must stay distinct: empty means "nobody is
    // on the wifi" and would read as the phone having LEFT.
    assert_eq!(parse_clients("<html>router login</html>"), None);
    assert_eq!(parse_clients(""), None);
}

#[test]
fn a_json_answer_without_the_data_list_is_no_answer() {
    // The auth-failed shape: valid JSON, no clients in it.
    assert_eq!(parse_clients(r#"{"error":"unauthorized"}"#), None);
    assert_eq!(parse_clients(r#"{"data":"not-a-list"}"#), None);
}

#[test]
fn a_parsed_empty_list_is_an_answer_and_not_a_failure() {
    assert_eq!(
        parse_clients(r#"{"offset":0,"limit":200,"count":0,"totalCount":0,"data":[]}"#),
        Some(Vec::new())
    );
}
