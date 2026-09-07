use pns_domain::home::Client;

/// The clients in a UniFi `/clients` listing, or `None` when the text is not
/// one. `None` and an empty list are DIFFERENT readings: an empty list is a
/// parsed answer ("nobody is on the wifi"), while `None` is no answer.
pub fn parse_clients(clients_json: &str) -> Option<Vec<Client>> {
    let listing = serde_json::from_str::<serde_json::Value>(clients_json).ok()?;
    let clients = listing.get("data")?.as_array()?;
    // An INCOMPLETE PAGE is no answer: totalCount counts every client the
    // router knows, and a device beyond this page would read as departed,
    // which is a false transition. Completeness is judged on the ENTRIES,
    // which is what it has always been judged on.
    if let Some(total) = listing
        .get("totalCount")
        .and_then(serde_json::Value::as_u64)
        && total > clients.len() as u64
    {
        return None;
    }
    Some(
        clients
            .iter()
            .map(|client| {
                let field =
                    |key: &str| -> Option<String> { client.get(key)?.as_str().map(str::to_string) };
                Client {
                    name: field("name"),
                    ipv4: field("ipAddress"),
                    mac: field("macAddress"),
                }
            })
            .collect(),
    )
}

/// The first site's id out of the sites listing, which on a UDR is the one
/// `default` site. Validated as id-shaped before it is joined into a path:
/// this is the one place a router answer becomes part of a URL.
pub fn first_site_id(sites_json: &str) -> Option<String> {
    let id = serde_json::from_str::<serde_json::Value>(sites_json)
        .ok()?
        .get("data")?
        .as_array()?
        .first()?
        .get("id")?
        .as_str()?
        .to_string();
    (!id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-'))
    .then_some(id)
}
