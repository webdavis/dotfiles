use super::*;

/// What `[stale]` carries: how long a block stands before it is escalated, and
/// the route the page about it takes.
pub(super) struct Escalation {
    pub enabled: bool,
    pub escalate_after_secs: u64,
    pub route: Option<String>,
}

/// `[stale]`'s keys, refused BY NAME on the same terms as `[remind]` beside it.
pub(super) fn parse_stale(value: toml::Value) -> Result<Escalation, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`stale` is not a table".to_string()));
    };
    let mut escalation = Escalation {
        enabled: DEFAULT_STALE_ENABLED,
        escalate_after_secs: DEFAULT_ESCALATE_AFTER_SECS,
        route: None,
    };
    for (key, setting) in table {
        admits_flat("stale", &key)?;
        match key.as_str() {
            "enabled" => {
                escalation.enabled = setting.as_bool().ok_or_else(|| {
                    ConfigError::Invalid(format!(
                        "`stale` key `enabled` has type `{}`, not boolean",
                        setting.type_str()
                    ))
                })?;
            }
            "escalate_after" => {
                escalation.escalate_after_secs = nonzero_duration_key(
                    "stale",
                    "escalate_after",
                    &setting,
                    escalate_after_range(),
                )?;
            }
            "route" => escalation.route = Some(route_name(&setting)?),
            _ => {
                return Err(unknown_key("stale", "stale", &key));
            }
        }
    }
    Ok(escalation)
}

/// `escalate_after`, BOUNDED ON BOTH SIDES, with zero refused by name: the
/// window is not the switch, `enabled` beside it is.
///
/// THE FLOOR IS A MINUTE. Below that this is a nudge rather than an
/// escalation, and the nudge is the table beside it; a minute is also low
/// enough to drill the whole path in a minute.
///
/// THE CEILING IS A DAY, which is `jobs::EVERY_MAX_SECS`'s own reading of "a
/// mistyped value rather than a schedule". The job's lease is one more window
/// past its due second, so a day still sits far inside the daemon's
/// registration window (`jobs::DUE_WINDOW_SECS`, thirty days).
fn escalate_after_range() -> RangeInclusive<Duration> {
    Duration::from_secs(MIN_ESCALATE_AFTER_SECS)..=Duration::from_secs(MAX_ESCALATE_AFTER_SECS)
}

/// The route the page takes, on `[routes]`'s own terms: a name that could not
/// stand as a URL path segment is refused rather than swapped for another.
fn route_name(setting: &toml::Value) -> Result<String, ConfigError> {
    let Some(route) = setting.as_str() else {
        return Err(ConfigError::Invalid(format!(
            "`stale` key `route` has type `{}`, not a string",
            setting.type_str()
        )));
    };
    if !pns_domain::safety::route_name_is_usable(route) {
        return Err(ConfigError::Invalid(
            "`stale` key `route` is not a usable route name; \
             a route is letters, digits, `-` and `_`"
                .to_string(),
        ));
    }
    Ok(route.to_string())
}

/// How long a block stands before it is escalated when nothing says otherwise
/// (design, 2026-09-14).
///
/// DEFAULT ON, where the nudge in `[remind]` is default off, and the difference
/// is which mistake each default makes. It is also why this table needs
/// `enabled`: an unset window means an hour rather than off, so nothing about
/// leaving keys out could say the page is unwanted. A nudge nobody asked for interrupts a
/// session the operator is already watching; a page nobody asked for arrives
/// about a session that has been stuck for an hour, which is the one thing
/// they would want to know.
pub(super) const DEFAULT_ESCALATE_AFTER_SECS: u64 = 3_600;

/// The shortest escalation anyone may schedule. See `escalate_after_range`.
pub(super) const MIN_ESCALATE_AFTER_SECS: u64 = 60;

/// The longest. See `escalate_after_range`.
pub(super) const MAX_ESCALATE_AFTER_SECS: u64 = pns_domain::jobs::EVERY_MAX_SECS;

/// Whether the page is raised when nothing says otherwise. See
/// `DEFAULT_ESCALATE_AFTER_SECS`.
pub(super) const DEFAULT_STALE_ENABLED: bool = true;
