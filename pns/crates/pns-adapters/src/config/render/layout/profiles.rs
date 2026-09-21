use super::*;

/// The heading every named profile writes under, and the one knob it serves
/// itself. A HARDCODED BRANCH writes the declarations, like the delivery
/// classes beside it, because the headings carry the operator's own names.
pub(crate) const PROFILES: Table = Table {
    name: "profiles",
    prose: "# What reaches you, bundled by name. One profile is active at a time: the\n\
            # first matching rule below chooses it, or `pns profile <name>` does by\n\
            # hand. Each surface takes \"all\" (everything that surface would have\n\
            # shown), \"priority\" (pages on the [routes] urgent route and nothing\n\
            # else) or \"none\". A profile only ever SUBTRACTS from what presence\n\
            # already decided, so it can never card a phone you are not near, and\n\
            # `discord` may never be \"none\": no profile can silence a page.\n",
    opt_in: false,
    children: &[],
    keys: &[Key {
        name: "location_poll",
        prose: "# How often the default gateway is fingerprinted, bounded \"5s\" to \"5m\".\n\
                # A network change moves the profile within one poll.\n",
        sample: Sample::Default("\"30s\""),
    }],
};

/// One `[profiles.<name>]` declaration's keys, in file order.
pub(crate) const PROFILE: Table = Table {
    name: crate::config::schema::PROFILE_KEYS,
    prose: "",
    opt_in: false,
    children: &[],
    keys: &[
        Key {
            name: "quiet",
            prose: "",
            sample: Sample::Default("false"),
        },
        Key {
            name: "banner",
            prose: "",
            sample: Sample::Default("\"all\""),
        },
        Key {
            name: "discord",
            prose: "",
            sample: Sample::Default("\"all\""),
        },
        Key {
            name: "phone",
            prose: "",
            sample: Sample::Default("\"all\""),
        },
        Key {
            name: "lights",
            prose: "",
            sample: Sample::Default("\"all\""),
        },
    ],
};

/// The prose above the two tables the branch writes itself.
pub(crate) const LOCATIONS_PROSE: &str = "# A named network, fingerprinted by the default gateway's hardware address\n\
     # rather than the Wi-Fi name, which macOS will not hand over without\n\
     # Location Services. `pns profile learn <name>` prints the row to paste here.\n";
pub(crate) const RULES_PROSE: &str = "# Which profile is active, first match wins. An input this machine cannot\n\
     # read matches nothing, so an unreadable probe falls through to `default`\n\
     # rather than silencing anything. A rule naming no input matches always.\n";
