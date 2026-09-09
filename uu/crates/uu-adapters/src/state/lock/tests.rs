use super::*;

#[test]
fn owner_release_unlocks_even_while_a_duplicate_descriptor_is_retained() {
    let home = std::env::temp_dir().join(format!("uu-lock-owner-{}", std::process::id()));
    let home = home.to_str().unwrap();
    let owner = acquire(home).unwrap_or_else(|error| panic!("first owner: {error:?}"));
    let retained = owner.0.try_clone().unwrap();
    assert!(matches!(acquire(home), Err(LockFailure::Contended(_))));
    drop(owner);
    let next = acquire(home)
        .unwrap_or_else(|error| panic!("owner returned but lock remains held: {error:?}"));
    drop(retained);
    assert!(matches!(acquire(home), Err(LockFailure::Contended(_))));
    drop(next);
    assert!(acquire(home).is_ok());
}
