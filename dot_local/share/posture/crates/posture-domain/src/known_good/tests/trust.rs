use super::*;

#[test]
fn only_root_owned_manifests_without_group_or_world_write_are_trusted() {
    for mode in [0o0000, 0o0600, 0o0644, 0o0755] {
        assert!(manifest_trustworthy(
            ManifestAuthority::Protected,
            Some("0"),
            Some(mode)
        ));
    }
    for mode in [0o0664, 0o0646, 0o0020, 0o0002, 0o7777] {
        assert!(!manifest_trustworthy(
            ManifestAuthority::Protected,
            Some("0"),
            Some(mode)
        ));
    }
    for uid in [None, Some(""), Some("501"), Some("00")] {
        assert!(!manifest_trustworthy(
            ManifestAuthority::Protected,
            uid,
            Some(0o0644)
        ));
    }
    assert!(!manifest_trustworthy(
        ManifestAuthority::Protected,
        Some("0"),
        None
    ));
}

#[test]
fn explicit_fixture_authority_skips_trust_attributes() {
    assert!(manifest_trustworthy(
        ManifestAuthority::ExplicitOverride,
        None,
        None
    ));
    assert!(manifest_trustworthy(
        ManifestAuthority::ExplicitOverride,
        Some("501"),
        Some(0o0666)
    ));
}
