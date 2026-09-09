use super::*;
/// candidate for one is a legal marker name.
pub fn render(job: &Job) -> String {
    let mut fields = vec![
        format!("id={}", job.id),
        format!("due={}", job.due),
        format!("until={}", job.until),
    ];
    if let Some(every) = job.every {
        fields.push(format!("every={every}"));
    }
    if let Some(marker) = &job.unless_marker {
        fields.push(format!("marker={marker}"));
    }
    // LAST, and the only field whose value can be long: nothing about the
    // parse depends on the order, but a reader scanning a spool file sees the
    // short scalars first.
    fields.push(format!(
        "args={}",
        serde_json::to_string(&job.args).unwrap_or_else(|_| "[]".to_string())
    ));
    fields.join("\t")
}

/// One line back into a job, or the reason it is not one.
///
/// REFUSED, NEVER GUESSED AT, in `parse_config`'s style: a missing field, a
/// repeated one, an unknown one and a value of the wrong shape are each an
/// error NAMING the offender. A record half-read is a job whose remaining
/// fields somebody else's edit decided, and the daemon re-executes this binary
/// from it.
pub fn parse(line: &str) -> Result<Job, String> {
    if line.len() > RECORD_MAX {
        return Err(format!(
            "the record is {} bytes, past the {RECORD_MAX}-byte cap",
            line.len()
        ));
    }
    if line.is_empty() {
        return Err("the record is empty".to_string());
    }
    let mut id = None;
    let mut due = None;
    let mut until = None;
    let mut every = None;
    let mut marker = None;
    let mut args = None;
    for field in line.split('\t') {
        let (key, value) = field
            .split_once('=')
            .ok_or_else(|| format!("field `{field}` is not `key=value`"))?;
        // A REPEAT IS AN ERROR RATHER THAN A LAST-WINS, which is the whole
        // reason each slot is filled through this helper: taking the last of
        // two `due` fields is a guess about which one the writer meant.
        match key {
            "id" => fill(&mut id, key, value.to_string())?,
            "due" => fill(&mut due, key, count(key, value)?)?,
            "until" => fill(&mut until, key, count(key, value)?)?,
            "every" => fill(&mut every, key, count(key, value)?)?,
            "marker" => fill(&mut marker, key, value.to_string())?,
            "args" => fill(
                &mut args,
                key,
                serde_json::from_str::<Vec<String>>(value)
                    .map_err(|_| "field `args` is not a JSON list of words".to_string())?,
            )?,
            _ => return Err(format!("unknown field `{key}`")),
        }
    }
    Ok(Job {
        id: required(id, "id")?,
        due: required(due, "due")?,
        until: required(until, "until")?,
        every,
        unless_marker: marker,
        args: required(args, "args")?,
    })
}

/// One slot, filled once. A second value for the same key is refused by name.
pub(super) fn fill<T>(slot: &mut Option<T>, key: &str, value: T) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("field `{key}` appears more than once"));
    }
    *slot = Some(value);
    Ok(())
}

/// A required field, or the name of the one that is missing.
pub(super) fn required<T>(slot: Option<T>, key: &str) -> Result<T, String> {
    slot.ok_or_else(|| format!("field `{key}` is missing"))
}

/// One numeric field, through the crate's own strict count.
///
/// `pns_domain::count::parse_count` RATHER THAN `str::parse`, which is the same choice
/// every other reading in this crate makes: it refuses a leading `+`, a
/// leading zero, surrounding whitespace and anything past what the shell this
/// ports can hold, so a numeral nobody wrote as a plain count is unknown
/// rather than coerced.
pub(super) fn count(key: &str, value: &str) -> Result<u64, String> {
    pns_domain::count::parse_count(value)
        .ok_or_else(|| format!("field `{key}` is not a plain count"))
}
