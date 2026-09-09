use pns_adapters::{TABLE_KEYS, strip_chezmoi_actions};

fn keys_of(table: &str) -> Option<&'static [&'static str]> {
    TABLE_KEYS
        .iter()
        .find(|(name, _)| *name == table)
        .map(|(_, keys)| *keys)
}

#[path = "contract_tests.rs"]
mod tests;
