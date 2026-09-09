use std::collections::{BTreeMap, BTreeSet};

/// Whether any asserted mode is one the config named.
///
/// TWO SPELLINGS ARE ACCEPTED for each entry in the list: the display name out
/// of the catalog ("Casually Concerned"), which is what the operator reads in
/// Control Center, and the raw `modeIdentifier`, which is the only handle a
/// mode with no name in the catalog has.
///
/// CASE-INSENSITIVE, because the name is transcribed by hand out of a user
/// interface and "sleep" is not a different Focus from "Sleep". See `same` for
/// exactly how far that goes, which is not as far as "any script".
///
/// AN IDENTIFIER ENTRY IS MATCHED WITH NO CATALOG AT ALL, and that is the
/// DESIGN rather than an oversight of the catalog's failure path. The raw
/// `modeIdentifier` is the ONLY handle a mode the catalog does not name has,
/// so it cannot be made to depend on the catalog without deleting the escape
/// hatch that exists for exactly the case where the catalog says nothing. The
/// consequence, stated so nobody has to rediscover it: a catalog that is
/// absent, gated or garbled leaves identifier entries working and NAME entries
/// inert. That silences LESS rather than more, which is this module's
/// direction, and `pns doctor` says the catalog failed in a clause of its own
/// so the inert half is never mistaken for health.
///
/// AN EMPTY LIST SILENCES NOTHING, which is the feature switched off and the
/// default state of every machine that never wrote a `[focus]` table.
pub fn silenced(
    active: &BTreeSet<String>,
    names: &BTreeMap<String, String>,
    silence: &[String],
) -> bool {
    active.iter().any(|identifier| {
        silence.iter().any(|listed| {
            same(listed, identifier) || names.get(identifier).is_some_and(|name| same(listed, name))
        })
    })
}

/// Two spellings of one Focus mode, compared the way a hand transcribing a
/// name out of Control Center would mean them.
///
/// FOLDED BOTH WAYS, because neither direction alone is enough. MEASURED:
/// "Straße" lowercases to "straße" while "STRASSE" lowercases to "strasse", so
/// a lowercase-only compare misses a name the operator typed in capitals;
/// upper-casing both agrees on "STRASSE". Agreement in EITHER direction is
/// taken as the same name.
///
/// AND THAT IS THE WHOLE OF IT, stated rather than overclaimed: this is case
/// mapping, NOT full Unicode case folding and NOT normalization. MEASURED,
/// both still false: "İstanbul" against "istanbul" (the dotted capital maps to
/// i plus a combining dot), and a decomposed "Cafe\u{301}" against a composed
/// "café". A name that either of those describes must be listed by its raw
/// `modeIdentifier`, which is compared by the same rule but is ASCII on every
/// mode macOS writes. Doing better needs a case-folding and normalization
/// dependency, which this crate does not carry for one config key.
fn same(left: &str, right: &str) -> bool {
    left.to_lowercase() == right.to_lowercase() || left.to_uppercase() == right.to_uppercase()
}
