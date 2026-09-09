use super::{ConfigError, admits, non_empty, table_of};

/// `[records]`: where the weekly what-happened entry is posted, and the key it
/// is signed with.
///
/// THE KEY IS REQUIRED once the block exists. A records block that cannot sign
/// is a record path that can never land, and the whole point of the record is
/// that its absence means something.
#[derive(Debug, Clone, PartialEq)]
pub struct Records {
    pub url: String,
    pub key: String,
    pub failure_webhook: Option<String>,
}

/// The gateway route the record goes to when no key states one.
pub const DEFAULT_RECORD_URL: &str = "http://127.0.0.1:8644/webhooks/unattended-upgrades";

pub(super) fn parse_records(value: toml::Value) -> Result<Records, ConfigError> {
    let table = table_of("records", value)?;
    let mut url = DEFAULT_RECORD_URL.to_string();
    let mut key = None;
    let mut failure_webhook = None;
    for (name, setting) in table {
        admits("records", "records", &name)?;
        match name.as_str() {
            "url" => url = non_empty("records", &name, &setting)?,
            "failure_webhook" => failure_webhook = Some(non_empty("records", &name, &setting)?),
            "key" => key = Some(non_empty("records", &name, &setting)?),
            // `admits` above is the ONE gate; nothing reaches here.
            _ => {}
        }
    }
    // A RECORDS BLOCK THAT CANNOT SIGN IS REFUSED, not quietly demoted to
    // log-only. The record's absence is what the operator reads as a dead
    // machine, so a path that can never post has to say so at load.
    let key = key.ok_or_else(|| {
        ConfigError::Invalid(
            "`records` has no `key`, so nothing it posts could be signed; remove the table to \
             switch records off"
                .to_string(),
        )
    })?;
    Ok(Records {
        url,
        key,
        failure_webhook,
    })
}
