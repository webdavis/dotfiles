use super::{super::scalar::Scalar, families::Family};
// Parsing remains with each record's existing codec. Report unreadable imported
// state without quoting its contents into diagnostics.
pub(super) fn error(family: Family, body: &str) -> Option<&'static str> {
    let valid = match family {
        Family::Return => body.trim().parse::<u64>().is_ok(),
        Family::Scalar(Scalar::Quiet) => pns_domain::quiet::expiry_from_state(body).is_ok(),
        Family::Scalar(Scalar::News) => crate::lights_codec::parse_news(body).is_some(),
        Family::Scalar(Scalar::Streak) => crate::lights_codec::parse_streak(body).is_some(),
        Family::Muted => crate::lights_codec::muted_entries(body).is_ok(),
        _ => true,
    };
    (!valid).then_some("legacy record could not be decoded")
}
