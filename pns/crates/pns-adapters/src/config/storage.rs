use super::*;

/// `[storage]`'s one key, in `parse_remind`'s shape: an unknown key inside the
/// table and a value of the wrong shape are each refused BY NAME.
pub(super) fn parse_storage(value: toml::Value) -> Result<Duration, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`storage` is not a table".to_string()));
    };
    let mut busy_deadline = DEFAULT_BUSY_DEADLINE;
    for (key, setting) in table {
        admits_flat("storage", &key)?;
        match key.as_str() {
            "busy_deadline" => {
                busy_deadline =
                    duration_value("storage", "busy_deadline", &setting, busy_deadline_range())?;
            }
            _ => return Err(unknown_key("storage", "storage", &key)),
        }
    }
    Ok(busy_deadline)
}

/// `busy_deadline`, BOUNDED ON BOTH SIDES with zero carved out.
///
/// ZERO IS SQLITE'S OWN STATEMENT and is not an error: `sqlite3_busy_timeout`
/// with a non-positive argument turns the busy handler off, so a contended
/// write is refused the instant it is contended rather than waited on.
///
/// THE FLOOR IS TEN MILLISECONDS. SQLite's default busy handler sleeps in
/// steps of a millisecond and upward, so a bound below a handful of them
/// expires inside the first sleep and is the same instruction as zero without
/// saying so.
///
/// THE CEILING IS A MINUTE. `record_decision` runs in-process on the hook path
/// a harness is blocked on and takes this bound per lock acquisition, so a
/// wait past a minute stalls an interactive hook for longer than any operator
/// would read as pns working. A holder that long is a wedged machine, which is
/// what the shipped default's own reasoning already says.
fn busy_deadline_range() -> RangeInclusive<Duration> {
    Duration::from_millis(MIN_BUSY_DEADLINE_MS)..=Duration::from_secs(MAX_BUSY_DEADLINE_SECS)
}

/// How long a writer waits for the database's write lock when nothing says
/// otherwise. See `SqliteStore`'s own note on what this number bounds.
pub const DEFAULT_BUSY_DEADLINE: Duration = Duration::from_secs(5);

/// The shortest wait anyone may set. See `busy_deadline_range`.
const MIN_BUSY_DEADLINE_MS: u64 = 10;

/// The longest. See `busy_deadline_range`.
const MAX_BUSY_DEADLINE_SECS: u64 = 60;
