use super::*;

pub(super) const DELIVERY: Table = Table {
    name: "delivery",
    prose: "# These request classes may pass your timed mute and named Focus for\n\
            # banners and phone cards. Presence and delivery scope still decide\n\
            # which surface receives them; muted lights stay off and hermes is\n\
            # unchanged. Names match exactly. An empty list allows no bypass.\n\
            # Exhausted delivery retries remain available for operator review.\n",
    opt_in: false,
    keys: &[
        Key {
            name: "bypass_silence_classes",
            prose: "",
            sample: Sample::Default("[\"security\"]"),
        },
        Key {
            name: "max_attempts",
            prose: "# Retry claims after the initial send, including interrupted retries. Zero permits no retries.\n",
            sample: Sample::Default("20"),
        },
        Key {
            name: "max_age_secs",
            prose: "# Stop retrying only after this original age is exceeded. Zero and future epochs do not expire.\n",
            sample: Sample::Default("604800"),
        },
        Key {
            name: "retry_base_secs",
            prose: "# Queued retries wait this many seconds times their retry count.\n",
            sample: Sample::Default("60"),
        },
        Key {
            name: "retry_random_secs",
            prose: "# Add one bounded random delay per failed retry. Zero disables it.\n# When omitted, it follows retry_base_secs.\n",
            sample: Sample::Default("60"),
        },
    ],
};
