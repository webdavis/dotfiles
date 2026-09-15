use super::*;

#[test]
fn a_recap_is_posted_once_on_the_default_route_whatever_that_post_answered() {
    // ONE ROUTE, SO NO SECOND ATTEMPT. The `pns-recap` route retired with its
    // Discord channel on 2026-09-15, and the fallback that once carried a
    // refused recap to the default route has nowhere left to fall from: a
    // refusal is REPORTED by the leg's own mode and never re-posted, or a
    // gateway having a bad minute would post every recap twice.
    for answer in [
        Delivery::Failed("failed".into()),
        Delivery::Unlaunched("missing".into()),
        Delivery::Silent,
    ] {
        let mut sent = Vec::new();
        assert_eq!(
            post_return_recap("body", |body, route| {
                sent.push((body.to_string(), route.to_string()));
                vec![answer.clone()]
            }),
            0
        );
        assert_eq!(sent, [("body".to_string(), String::new())]);
    }
}
