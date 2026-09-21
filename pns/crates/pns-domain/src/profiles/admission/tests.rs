use super::*;

#[test]
fn a_surface_admits_what_its_word_says() {
    assert!(Admits::All.admits(false), "all admits an ordinary event");
    assert!(Admits::All.admits(true));
    assert!(
        !Admits::Priority.admits(false),
        "priority admits no ordinary event"
    );
    assert!(Admits::Priority.admits(true));
    assert!(!Admits::None.admits(false));
    assert!(
        !Admits::None.admits(true),
        "none admits nothing, which the floor refuses at load"
    );
}

#[test]
fn the_three_words_are_the_whole_vocabulary() {
    assert_eq!(Admits::parse("all"), Some(Admits::All));
    assert_eq!(Admits::parse("priority"), Some(Admits::Priority));
    assert_eq!(Admits::parse("none"), Some(Admits::None));
    assert_eq!(Admits::parse("All"), None, "the words are lower case");
    assert_eq!(Admits::parse("off"), None);
}

#[test]
fn only_discord_is_the_floor_because_the_other_three_are_presence_gated() {
    let silent = Profile {
        quiet: true,
        banner: Admits::None,
        discord: Admits::None,
        phone: Admits::None,
        lights: Admits::None,
    };
    assert!(!silent.admits_a_page());
    assert!(Profile::default().admits_a_page());
    assert!(
        !Profile {
            phone: Admits::Priority,
            ..silent.clone()
        }
        .admits_a_page(),
        "phone alone is not the floor: it never fires at the desk"
    );
    assert!(
        !Profile {
            banner: Admits::Priority,
            ..silent.clone()
        }
        .admits_a_page(),
        "banner alone is not the floor: it never fires off the desk"
    );
    assert!(
        Profile {
            discord: Admits::Priority,
            ..silent.clone()
        }
        .admits_a_page(),
        "discord alone is the floor: channel_plan never masks it by presence"
    );
}

#[test]
fn a_profiles_hush_never_reaches_a_priority_page() {
    let work = Profile {
        quiet: true,
        ..Profile::default()
    };
    assert!(work.hushes(false));
    assert!(!work.hushes(true));
    assert!(
        !Profile::default().hushes(false),
        "the default profile hushes nothing"
    );
}
