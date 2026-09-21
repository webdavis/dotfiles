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

fn parsed(text: &str) -> Result<Profiles, ConfigError> {
    parse_profiles(toml::Value::Table(table(text)))
}

#[test]
fn the_shipped_shape_parses() {
    let read = parsed(
        "location_poll = \"30s\"\n\
         [default]\nquiet = false\n\
         [night]\nquiet = true\nbanner = \"none\"\ndiscord = \"priority\"\n\
         phone = \"priority\"\nlights = \"none\"\n\
         [locations]\nhome = \"00:11:22:aa:bb:cc\"\n\
         [[rules]]\nprofile = \"night\"\nhours = \"22:00-06:00\"\n\
         [[rules]]\nprofile = \"default\"\ndays = [\"Sat\", \"Sun\"]\n",
    )
    .expect("it parses");
    assert_eq!(read.location_poll_secs, 30);
    assert_eq!(
        read.locations.get("home").map(String::as_str),
        Some("00:11:22:aa:bb:cc")
    );
    assert_eq!(read.rules.len(), 2);
    assert_eq!(read.rules[0].profile, "night");
    assert!(read.rules[0].hours.is_some());
    assert_eq!(
        read.rules[1].days,
        vec![6, 0],
        "Saturday is 6 and Sunday is 0"
    );
}

#[test]
fn a_rule_naming_an_undefined_profile_is_refused_with_its_index() {
    let refusal = parsed("[default]\nquiet = false\n[[rules]]\nprofile = \"meeting\"\n")
        .expect_err("refused");
    assert_eq!(
        refusal.detail(),
        "rule 1 names profile `meeting`, which no `[profiles.meeting]` table defines"
    );
}

#[test]
fn a_malformed_window_and_an_unknown_weekday_each_say_what_they_are() {
    let hours =
        parsed("[default]\nquiet = false\n[[rules]]\nprofile = \"default\"\nhours = \"22:00\"\n")
            .expect_err("refused");
    assert_eq!(
        hours.detail(),
        "rule 1 has hours \"22:00\", which is not a HH:MM-HH:MM window"
    );
    let day =
        parsed("[default]\nquiet = false\n[[rules]]\nprofile = \"default\"\ndays = [\"Funday\"]\n")
            .expect_err("refused");
    assert_eq!(
        day.detail(),
        "rule 1 has day \"Funday\"; a day is Mon, Tue, Wed, Thu, Fri, Sat or Sun"
    );
}

#[test]
fn a_day_is_matched_case_insensitively_and_a_rule_may_name_nothing() {
    let read =
        parsed("[default]\nquiet = false\n[[rules]]\nprofile = \"default\"\ndays = [\"mon\"]\n")
            .expect("it parses");
    assert_eq!(read.rules[0].days, vec![1]);
    let bare = parsed("[default]\nquiet = false\n[[rules]]\nprofile = \"default\"\n")
        .expect("a rule may name no input");
    assert!(bare.rules[0].days.is_empty() && bare.rules[0].hours.is_none());
}

#[test]
fn a_key_that_is_not_a_table_is_refused_as_a_key_of_profiles() {
    let refusal = parsed("locaton_poll = \"30s\"\n").expect_err("refused");
    assert!(
        refusal
            .detail()
            .contains("unknown `profiles` key `locaton_poll`"),
        "it said: {}",
        refusal.detail()
    );
}
