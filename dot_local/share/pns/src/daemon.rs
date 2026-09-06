//! The daemon's core: what a scheduled job is, whether it fires, what a repeat
//! re-arms to, and the spool directory the two sides talk through.
//!
//! THE IPC IS A DIRECTORY, and that is the whole design. A short-lived process
//! registers work by writing ONE file; the daemon reads the directory on its
//! tick. There is no connection, no handshake, no reply and NOTHING FOR A HOOK
//! TO WAIT ON, which is the property every other choice here falls out of: a
//! daemon that is dead, wedged or mid-restart changes nothing about the write.
//!
//! The write is `main.rs`'s `publish_state_line` shape (a private 0600 temp
//! named by pid, then a rename) and the read is its `claim_by_rename` shape (a
//! rename decides the owner, because a plain unlink does NOT arbitrate on APFS:
//! measured, eight racing unlinkers were every one of them told they had
//! succeeded). Both are re-stated here rather than reused because both are
//! private to the composition root and these are library functions the hooks
//! call directly.

// The job POLICY moved to `pns-domain`: what a job is, what the loop decides
// about one, what a heartbeat says, and the bounds. The codec, the spool's
// transactions and `validate_shape` stay here, the last because its record cap
// is a fact about the rendered line.
pub use pns_domain::jobs::{
    ARGS_BYTES_MAX, ARGS_MAX, DUE_WINDOW_SECS, EVERY_MAX_SECS, HEARTBEAT_STALE_SECS, Heartbeat,
    ID_MAX, Job, MIN_EVERY_SECS, RECORD_MAX, Reason, Verdict, decide, name_is_safe,
    parse_heartbeat, rearm, render_heartbeat,
};
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

// The id bound moved to `pns-domain`, because the nag derives its own name
// cap from it and a member crate never reaches back into this package.
/// The rules a job must satisfy WHEREVER it came from: the registration that
/// wrote it and the loop that read it back.
///
/// THE LOOP APPLIES IT TOO, which is the whole reason it is a function rather
/// than a check inside the registration. A hand-edited spool file must not be
/// able to do what a registration could not.
///
/// IT TAKES NO CLOCK, so it says the same thing at write time and at read
/// time. The bound that IS a function of now (`due` inside a window) lives in
/// `validate_registration`, because a job re-armed hours ago and read back on
/// a woken laptop is a lease decision, not a malformed record.
pub fn validate_shape(job: &Job) -> Result<(), String> {
    if !name_is_safe(&job.id) {
        return Err(format!(
            "`id` must be 1 to {ID_MAX} characters of letters, digits, `.`, `_`, `:` or `-`, \
             with no leading `.` and no `..`"
        ));
    }
    if let Some(marker) = &job.unless_marker
        && !name_is_safe(marker)
    {
        return Err(format!(
            "`marker` must be 1 to {ID_MAX} characters of letters, digits, `.`, `_`, `:` or `-`, \
             with no leading `.` and no `..`"
        ));
    }
    // BOUNDED ON BOTH SIDES. A repeat under the tick is a job the loop would
    // re-arm into the past on every pass, which is a spin; one past the
    // ceiling is a lease-length repeat nobody meant to write.
    if let Some(every) = job.every
        && !(MIN_EVERY_SECS..=EVERY_MAX_SECS).contains(&every)
    {
        return Err(format!(
            "`every` must be between {MIN_EVERY_SECS} and {EVERY_MAX_SECS} seconds"
        ));
    }
    if job.until < job.due {
        return Err("`until` is before `due`, so the lease ends before it starts".to_string());
    }
    if job.args.is_empty() {
        return Err("`args` is empty, so the job would re-execute pns with no event".to_string());
    }
    if job.args.len() > ARGS_MAX {
        return Err(format!("`args` has more than {ARGS_MAX} words"));
    }
    let bytes: usize = job.args.iter().map(String::len).sum();
    if bytes > ARGS_BYTES_MAX {
        return Err(format!("`args` is longer than {ARGS_BYTES_MAX} bytes"));
    }
    // THE RENDERED RECORD, NOT THE FIELDS THAT WENT INTO IT, which is the only
    // length the parser will ever see. `render` JSON-escapes the argv, so one
    // control character becomes six bytes and a run of them expands past this
    // cap while every field bound above is still satisfied. Checked here rather
    // than at the write, so a registration is refused BY NAME instead of being
    // accepted, written, and dropped by the daemon as unparseable on the next
    // tick.
    let rendered = render(job).len();
    if rendered > RECORD_MAX {
        return Err(format!(
            "the rendered record is {rendered} bytes, past the {RECORD_MAX}-byte cap"
        ));
    }
    Ok(())
}

/// The shape rules PLUS the one bound that needs a clock.
///
/// A `due` FAR FROM NOW IS REFUSED IN BOTH DIRECTIONS, per the two-sided-bound
/// rule: far in the future parks a job the lease can never expire, and far in
/// the past is a clock jump or a corrupt field rather than a schedule.
pub fn validate_registration(job: &Job, now: u64) -> Result<(), String> {
    validate_shape(job)?;
    if job.due.abs_diff(now) > DUE_WINDOW_SECS {
        return Err(format!(
            "`due` is more than {DUE_WINDOW_SECS} seconds from now"
        ));
    }
    Ok(())
}

/// One slot, filled once. A second value for the same key is refused by name.
fn fill<T>(slot: &mut Option<T>, key: &str, value: T) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("field `{key}` appears more than once"));
    }
    *slot = Some(value);
    Ok(())
}

/// A required field, or the name of the one that is missing.
fn required<T>(slot: Option<T>, key: &str) -> Result<T, String> {
    slot.ok_or_else(|| format!("field `{key}` is missing"))
}

/// One numeric field, through the crate's own strict count.
///
/// `crate::parse_count` RATHER THAN `str::parse`, which is the same choice
/// every other reading in this crate makes: it refuses a leading `+`, a
/// leading zero, surrounding whitespace and anything past what the shell this
/// ports can hold, so a numeral nobody wrote as a plain count is unknown
/// rather than coerced.
fn count(key: &str, value: &str) -> Result<u64, String> {
    crate::parse_count(value).ok_or_else(|| format!("field `{key}` is not a plain count"))
}

#[cfg(test)]
mod record_tests;

mod spool;

pub use spool::{
    Peeked, Startup, cancel, claim, hand_back, heartbeat_path, job_count, marker_dir,
    marker_exists, peek, prepare_spool, publish_heartbeat, schedule, spool_dir, spool_entries,
};
