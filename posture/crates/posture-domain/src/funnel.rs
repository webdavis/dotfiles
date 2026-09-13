mod gap;
pub use gap::{
    FUNNEL_CRITICAL_TITLE, FunnelReadFailure, funnel_corruption_gap, funnel_persistence_gap,
    funnel_read_gap,
};
#[derive(Debug, Clone, Copy)]
pub enum AllowFunnel<'a> {
    Omitted,
    Map(&'a [(&'a str, Option<bool>)]),
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunnelReading {
    Inactive,
    Active(Vec<String>),
    Gap,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunnelState {
    Active,
    Inactive,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunnelBaseline {
    Absent,
    Corrupt,
    Trusted(FunnelState),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunnelAlert {
    ReadGap,
    CorruptBaseline,
    Exposure(String),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunnelPlan {
    pub alert: Option<FunnelAlert>,
    pub next: Option<FunnelState>,
    pub clear_read_gap: bool,
}

// The wire owner supplies each document's AllowFunnel values at every depth.
// Absent/null/false are omitted by the Bash projection; every other non-map is invalid.
pub fn classify_funnel(documents: &[&[AllowFunnel<'_>]]) -> FunnelReading {
    let [values] = documents else {
        return FunnelReading::Gap;
    };
    let mut active = std::collections::BTreeSet::new();
    for value in *values {
        match value {
            AllowFunnel::Omitted => {}
            AllowFunnel::Invalid => return FunnelReading::Gap,
            AllowFunnel::Map(entries) => {
                for (key, enabled) in *entries {
                    match enabled {
                        Some(true) => {
                            active.insert((*key).to_owned());
                        }
                        Some(false) => {}
                        None => return FunnelReading::Gap,
                    }
                }
            }
        }
    }
    if active.is_empty() {
        FunnelReading::Inactive
    } else {
        FunnelReading::Active(active.into_iter().collect())
    }
}

pub fn funnel_baseline(
    present: bool,
    one_object_value: Option<&str>,
    persist_failed: bool,
) -> FunnelBaseline {
    if !present {
        return FunnelBaseline::Absent;
    }
    let value = match one_object_value {
        Some("active") => FunnelState::Active,
        Some("inactive") => FunnelState::Inactive,
        _ => return FunnelBaseline::Corrupt,
    };
    if persist_failed {
        FunnelBaseline::Absent
    } else {
        FunnelBaseline::Trusted(value)
    }
}

pub fn plan_funnel(
    reading: &FunnelReading,
    baseline: FunnelBaseline,
    gap_covered: bool,
) -> FunnelPlan {
    if *reading == FunnelReading::Gap {
        return FunnelPlan {
            alert: (!gap_covered).then_some(FunnelAlert::ReadGap),
            next: None,
            clear_read_gap: false,
        };
    }
    let next = match reading {
        FunnelReading::Active(_) => FunnelState::Active,
        _ => FunnelState::Inactive,
    };
    let alert = match reading {
        FunnelReading::Active(keys) if baseline != FunnelBaseline::Trusted(FunnelState::Active) => {
            Some(FunnelAlert::Exposure(
                render_funnel_exposure(keys).trim_end_matches('\n').into(),
            ))
        }
        _ if baseline == FunnelBaseline::Corrupt => Some(FunnelAlert::CorruptBaseline),
        _ => None,
    };
    // This is proposed state. The application must durably submit an exposure before writing it.
    // The legacy corruption warning is best effort and is followed by baseline repair.
    FunnelPlan {
        alert,
        next: Some(next),
        clear_read_gap: true,
    }
}

/// The most exposed keys a page renders in full before it summarizes the rest.
///
/// The wire caps one text field at 8,000 characters (`MAX_TEXT_CHARS` in
/// posture-pns-wire, refused by posture-adapters/src/pns_producer/request.rs),
/// and the application prefixes the 14-character critical title and a newline.
/// The header is 179 characters, the widest key line is 216 (a dash, a space,
/// a backtick, 200 characters, `…(truncated)` and a closing backtick), each
/// line after the header costs one newline, and the widest summary line is 22.
/// So the worst case is 15 + 179 + 32 * (216 + 1) + 22 = 7,160 characters,
/// 840 under the cap. Thirty-six keys would exceed it, so the margin here is
/// deliberate rather than the largest count that happens to fit.
pub const FUNNEL_EXPOSURE_KEY_LIMIT: usize = 32;

pub fn render_funnel_exposure(keys: &[String]) -> String {
    let keys: std::collections::BTreeSet<_> = keys.iter().collect();
    let omitted = keys.len().saturating_sub(FUNNEL_EXPOSURE_KEY_LIMIT);
    let exposed = if keys.is_empty() {
        "`(unknown)`".into()
    } else {
        keys.into_iter()
            .take(FUNNEL_EXPOSURE_KEY_LIMIT)
            .map(|key| {
                let clean: String = key
                    .chars()
                    .filter(|c| *c != '`')
                    .map(|c| {
                        if matches!(c, '\r' | '\n' | '\t') {
                            ' '
                        } else {
                            c
                        }
                    })
                    .collect();
                let suffix = if clean.chars().count() > 200 {
                    "…(truncated)"
                } else {
                    ""
                };
                format!(
                    "- `{}{suffix}`",
                    clean.chars().take(200).collect::<String>()
                )
            })
            .chain((omitted > 0).then(|| format!("- …and {omitted} more")))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "**Tailscale Funnel is exposing a local service to the PUBLIC internet.**\n- Did you set this up? If not, close it now: **tailscale funnel reset**\n- Exposed to the public internet:\n{exposed}\n"
    ).replace('\0', "")
}

#[cfg(test)]
mod tests;
