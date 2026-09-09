use super::Fixture;

/// The CLIP resource path this fixture is written to.
///
/// WHICH IS THE WHOLE POINT OF THE DISTINCTION. Addressing either as the
/// other is a PUT to a resource id of the wrong type, which the bridge
/// answers by doing nothing and telling no one, because `put` is fire and
/// forget.
pub fn fixture_path(fixture: &Fixture) -> String {
    match fixture {
        Fixture::Grouped(id) => format!("grouped_light/{id}"),
        Fixture::Light(id) => format!("light/{id}"),
    }
}
