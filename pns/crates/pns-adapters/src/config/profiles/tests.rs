use super::*;

fn table(text: &str) -> toml::Table {
    text.parse::<toml::Table>().expect("a table")
}

#[test]
fn the_five_keys_parse() {
    let night = parse_profile(
        "night",
        &table(
            "quiet = true\nbanner = \"none\"\ndiscord = \"priority\"\n\
             phone = \"priority\"\nlights = \"none\"\n",
        ),
    )
    .expect("night parses");
    assert_eq!(
        night,
        Profile {
            quiet: true,
            banner: Admits::None,
            discord: Admits::Priority,
            phone: Admits::Priority,
            lights: Admits::None,
        }
    );
}

#[test]
fn an_unwritten_key_keeps_todays_behaviour() {
    let sparse = parse_profile("sparse", &table("quiet = true\n")).expect("it parses");
    assert_eq!(
        sparse,
        Profile {
            quiet: true,
            ..Profile::default()
        }
    );
}

#[test]
fn an_unknown_surface_word_is_refused_with_the_three_words() {
    let refusal = parse_profile("night", &table("phone = \"off\"\n")).expect_err("refused");
    assert_eq!(
        refusal.detail(),
        "unknown `profiles.night` value for `phone`; a surface is \"all\", \"priority\" or \"none\""
    );
}

#[test]
fn a_profile_whose_discord_is_none_is_refused_by_name() {
    let refusal = parse_profile("night", &table("discord = \"none\"\n")).expect_err("refused");
    assert_eq!(
        refusal.detail(),
        "profile `night`'s `discord` is \"none\"; a priority page has to reach the durable log \
         wherever you are, so `discord` must be \"all\" or \"priority\""
    );
}

#[test]
fn banner_and_phone_at_none_alone_do_not_trip_the_floor() {
    let banner_and_phone_off = parse_profile(
        "work",
        &table("banner = \"none\"\nphone = \"none\"\ndiscord = \"priority\"\n"),
    );
    assert!(
        banner_and_phone_off.is_ok(),
        "the floor is discord alone, not any one of the four"
    );
}

#[test]
fn an_unknown_key_is_refused_by_name() {
    let refusal = parse_profile("night", &table("lamps = \"none\"\n")).expect_err("refused");
    assert!(
        refusal
            .detail()
            .contains("unknown `profiles.night` key `lamps`"),
        "it said: {}",
        refusal.detail()
    );
}
