use super::*;

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
