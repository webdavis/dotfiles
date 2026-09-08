use crate::Fetched;

/// One external source's three states, said in the type the body reads.
///
/// THE OUTER `Option` IS THE KEY AND THE INNER ONE IS THE READ, which is what
/// keeps "nobody configured this" and "this would not answer" apart all the way
/// from the config to the message. An empty `Vec` is neither: it is a source
/// that answered with nothing.
pub(super) fn found(fetched: &Option<Option<Fetched>>) -> pns_domain::recap::external::Found<'_> {
    match fetched {
        None => pns_domain::recap::external::Found::Unconfigured,
        Some(None) => pns_domain::recap::external::Found::Unavailable,
        Some(Some(fetched)) => pns_domain::recap::external::Found::Read(&fetched.sources),
    }
}

/// Whether what `found` holds is a floor. A source nobody configured and one
/// that would not answer are neither: there is no count to qualify.
pub(super) fn truncated(fetched: &Option<Option<Fetched>>) -> bool {
    matches!(fetched, Some(Some(fetched)) if fetched.truncated)
}

/// What a source actually held, for the two callers that only have something to
/// do when it held anything.
pub(super) fn read_sources(
    fetched: &Option<Option<Fetched>>,
) -> Option<&[pns_domain::recap::external::Sourced]> {
    Some(fetched.as_ref()?.as_ref()?.sources.as_slice()).filter(|sources| !sources.is_empty())
}
