use super::*;

pub(super) const DELIVERY: Table = Table {
    name: "delivery",
    prose: "# How a delivery that did not land is retried. Exhausted delivery\n\
            # retries remain available for operator review.\n",
    opt_in: false,
    children: &[],
    keys: &[
        Key {
            name: "event_max_age",
            prose: "# Stop retrying once the ORIGINAL EVENT is older than this. Zero and future\n# epochs do not expire.\n",
            sample: Sample::Default("\"168h\""),
        },
        Key {
            name: "max_retries",
            prose: "# Retries allowed after the initial send, including interrupted ones. Zero\n# permits no retry at all.\n",
            sample: Sample::Default("20"),
        },
        Key {
            name: "remote_deadline",
            prose: "# How long ONE remote call may take, in seconds, before the caller stops\n\
                         # waiting on it. Short because a hook or a shell prompt is blocked on it.\n\
                         # Zero is no deadline at all, which is your instruction rather than a\n\
                         # default: a wedged gateway then holds that caller for as long as it\n\
                         # takes.\n",
            sample: Sample::Default("5"),
        },
        Key {
            name: "retry_step",
            prose: "# Queued retries wait this step times their retry count. The wait is exact:\n# there is no random spread, because one local daemon draining one queue has\n# no herd to spread.\n",
            sample: Sample::Default("\"1m\""),
        },
    ],
};

/// One `[delivery_class.<name>]` declaration, whose name is the operator's
/// own. WRITTEN BY THE HARDCODED BRANCH in `render_delivery_classes`, the way
/// a lamp declaration is, because this layout cannot enumerate class names it
/// does not compile in.
pub(super) const DELIVERY_CLASS: Table = Table {
    name: crate::config::schema::DELIVERY_CLASS_KEYS,
    prose: "# What each delivery class DOES, one table per class, named by the word\n\
            # a producer sends as `--delivery-class` or JSON `delivery_class`.\n\
            # `route` is where a message of that class goes when its producer\n\
            # named no route of its own, and it is taken only while somebody is\n\
            # waiting on the event, so a weekly upgrade that went fine stays on\n\
            # the routine route; empty is the routine route at every state.\n\
            # `bypass_mute` lets it pass your timed mute and named Focus modes\n\
            # for banners and phone cards, and presence and delivery scope still\n\
            # decide which surface receives it; muted lights stay off and hermes\n\
            # is unchanged. A message naming NO class reads [delivery_class.default],\n\
            # and a class no table here defines is refused rather than delivered\n\
            # somewhere you did not intend.\n",
    opt_in: false,
    children: &[],
    keys: &[
        Key {
            name: "route",
            prose: "",
            sample: Sample::Default("\"\""),
        },
        Key {
            name: "bypass_mute",
            prose: "",
            sample: Sample::Default("false"),
        },
    ],
};

/// Written commented, and only when the caller declared no class of its own:
/// a real declaration is a better example than this one.
pub(crate) const EXAMPLE_CLASS: &str = "# [delivery_class.default]\n\
     # route = \"\"\n\
     # bypass_mute = false\n\n";
