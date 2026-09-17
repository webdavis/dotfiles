//! The notifications listing, off the wire.
//!
//! SYNTAX ONLY, the presence state file's own split: what a thread LOOKS like
//! is this module's, and what one MEANS is `pns_domain::github::notifications`.
//! The two change for different reasons, and a parse that also judged would be
//! edited by both.

use pns_domain::github::notifications::NotificationThread;
use serde_json::Value;

/// Every thread in one listing, in the order the API stated them (newest
/// first), skipping any entry the shape does not fit.
///
/// A MALFORMED ENTRY IS SKIPPED RATHER THAN FAILING THE LISTING. One thread
/// carrying an unexpected null must not cost the other twenty, and a thread
/// naming no repository is dropped by the policy anyway.
///
/// A BODY THAT IS NOT A LISTING IS NO THREADS, which the caller reads as an
/// answer that said nothing rather than as an error: the cursor is not
/// advanced on it either way.
pub fn notification_threads(body: &str) -> Vec<NotificationThread> {
    serde_json::from_str::<Value>(body)
        .ok()
        .as_ref()
        .and_then(Value::as_array)
        .map(|entries| entries.iter().filter_map(thread).collect())
        .unwrap_or_default()
}

/// One entry as a thread, or nothing when it is not one.
fn thread(entry: &Value) -> Option<NotificationThread> {
    let subject = entry.get("subject");
    let repository = entry.get("repository");
    Some(NotificationThread {
        id: text(entry.get("id"))?,
        reason: text(entry.get("reason")).unwrap_or_default(),
        subject_type: text(subject.and_then(|it| it.get("type"))).unwrap_or_default(),
        subject_title: text(subject.and_then(|it| it.get("title"))).unwrap_or_default(),
        subject_url: text(subject.and_then(|it| it.get("url"))).unwrap_or_default(),
        repo_full_name: text(repository.and_then(|it| it.get("full_name"))).unwrap_or_default(),
        repo_html_url: text(repository.and_then(|it| it.get("html_url"))).unwrap_or_default(),
        updated_at: text(entry.get("updated_at"))
            .as_deref()
            .and_then(epoch_from_instant)
            .unwrap_or_default(),
    })
}

/// One string field, however the API spelled it.
///
/// A NUMBER READS AS ITS DIGITS, because `id` is documented as a string and
/// has been observed as one, and a build that refused a numeric one would
/// drop every thread the day that changed.
fn text(stated: Option<&Value>) -> Option<String> {
    match stated? {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// `2026-09-14T15:16:27Z` as epoch seconds, or nothing for anything else.
///
/// THE ONE SHAPE GITHUB SENDS, parsed strictly rather than leniently: this is
/// the inverse of `macos::clock::utc_timestamp`, and a lenient reader would
/// turn a format change into a plausible wrong instant instead of a zero the
/// caller can see. UTC only, which is what the trailing `Z` states.
pub fn epoch_from_instant(stated: &str) -> Option<u64> {
    let stated = stated.strip_suffix('Z')?;
    let (date, time) = stated.split_once('T')?;
    let mut date = date.split('-');
    let (year, month, day) = (
        number(date.next()?, 4)?,
        number(date.next()?, 2)?,
        number(date.next()?, 2)?,
    );
    if date.next().is_some() {
        return None;
    }
    let mut time = time.split(':');
    let (hour, minute, second) = (
        number(time.next()?, 2)?,
        number(time.next()?, 2)?,
        number(time.next()?, 2)?,
    );
    if time.next().is_some() || month == 0 || day == 0 || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let days = days_from_civil(year, month, day)?;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// One fixed-width run of digits.
///
/// THE WIDTH IS PART OF THE CONTRACT. `2026-9-14T15:16:27Z` parses to a
/// perfectly plausible instant under a lenient reader, and a producer sending
/// that has changed format: it must read as nothing rather than as an answer.
fn number(text: &str, width: usize) -> Option<u64> {
    (text.len() == width && text.chars().all(|c| c.is_ascii_digit())).then(|| text.parse().ok())?
}

/// Days from 1970-01-01 to this civil date, by Howard Hinnant's `days_from_civil`.
///
/// ARITHMETIC RATHER THAN `libc`, because this crate's callers include tests
/// that must give the same answer on any host, and a timezone database has no
/// part in reading a `Z` instant.
fn days_from_civil(year: u64, month: u64, day: u64) -> Option<u64> {
    if year < 1970 || month > 12 || day > 31 {
        return None;
    }
    let shifted = if month <= 2 { year - 1 } else { year };
    let era = shifted / 400;
    let year_of_era = shifted - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
}

#[cfg(test)]
mod tests;
