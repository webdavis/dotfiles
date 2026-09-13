#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeRecordRefusal {
    Header,
    ClockUnavailable,
    TooManyRows,
    Row,
}

pub fn recorded_hash(text: &str, target: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let mut rest = line.trim_matches([' ', '\t']);
        let mut hash = "";
        for field in 0..3 {
            let end = rest.find([' ', '\t'])?;
            if field == 0 {
                hash = &rest[..end];
            }
            rest = rest[end..].trim_start_matches([' ', '\t']);
        }
        (rest == target && super::known_good::valid_digest(hash))
            .then(|| hash[..12].to_ascii_lowercase())
    })
}

pub fn upgrade_correlation(
    snapshot: &str,
    basename: &str,
    now: Option<u64>,
) -> Result<String, UpgradeRecordRefusal> {
    let mut rows = snapshot.trim_end_matches('\n').split('\n');
    let (epoch, iso) = header(rows.next().unwrap_or_default())?;
    let now = now.ok_or(UpgradeRecordRefusal::ClockUnavailable)?;
    if now.checked_sub(epoch).is_none_or(|age| age > 259_200) {
        return Ok(format!(
            "no recorded upgrade in the last 3 days; the newest record is from {iso}"
        ));
    }
    let mut names = Vec::new();
    let mut matched = None;
    for (index, row) in rows.enumerate() {
        let mut fields = row.splitn(4, '\t');
        let name = fields.next().unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        if index + 2 > 500 {
            return Err(UpgradeRecordRefusal::TooManyRows);
        }
        let state = fields.next().ok_or(UpgradeRecordRefusal::Row)?;
        let before = fields.next().ok_or(UpgradeRecordRefusal::Row)?;
        let after = fields.next().ok_or(UpgradeRecordRefusal::Row)?;
        if !matches!(state, "added" | "removed" | "changed") {
            return Err(UpgradeRecordRefusal::Row);
        }
        names.push(name);
        if matched.is_none() && name == basename {
            matched = Some((name, before, after));
        }
    }
    if let Some((name, before, after)) = matched {
        let before = if before.is_empty() { "none" } else { before };
        let after = if after.is_empty() { "none" } else { after };
        return Ok(format!(
            "recorded upgrade: {name} {before} -> {after} at {iso} (the name matches this file, which is not proof)"
        ));
    }
    let prefix = format!("no recorded upgrade names this file; the run at {iso}");
    if names.is_empty() {
        return Ok(format!("{prefix} recorded no package change"));
    }
    let mut shown = names[..names.len().min(5)].join(", ");
    if names.len() > 5 {
        shown.push_str(&format!(", and {} more", names.len() - 5));
    }
    Ok(format!("{prefix} changed: {shown}"))
}

fn header(line: &str) -> Result<(u64, &str), UpgradeRecordRefusal> {
    let (epoch, iso) = line.split_once('\t').ok_or(UpgradeRecordRefusal::Header)?;
    let shape = b"0000-00-00T00:00:00Z";
    if !(1..=11).contains(&epoch.len())
        || !epoch.bytes().all(|byte| byte.is_ascii_digit())
        || iso.len() != shape.len()
        || !iso.bytes().zip(shape).all(|(byte, expected)| {
            if *expected == b'0' {
                byte.is_ascii_digit()
            } else {
                byte == *expected
            }
        })
    {
        return Err(UpgradeRecordRefusal::Header);
    }
    Ok((
        epoch.parse().map_err(|_| UpgradeRecordRefusal::Header)?,
        iso,
    ))
}

#[cfg(test)]
mod tests;
