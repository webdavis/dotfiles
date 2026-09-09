use super::*;

#[test]
fn only_a_refused_thread_delivery_retries_once_on_the_default_route() {
    for first in [
        Delivery::Failed("failed".into()),
        Delivery::Unlaunched("missing".into()),
        Delivery::Silent,
    ] {
        let retries = matches!(first, Delivery::Failed(_) | Delivery::Unlaunched(_));
        let mut sent = Vec::new();
        assert_eq!(
            post_return_recap("body", true, |body, route| {
                sent.push((body.to_string(), route.to_string()));
                vec![first.clone()]
            }),
            0
        );
        assert_eq!(sent[0], ("body".into(), "pns-recap".into()));
        assert_eq!(sent.len(), if retries { 2 } else { 1 });
        if retries {
            assert_eq!(sent[1], (format!("body\n{THREAD_UNAVAILABLE}"), "".into()));
        }
    }
}

#[test]
fn a_plain_recap_attempts_only_the_default_route_even_when_it_refuses() {
    let mut routes = Vec::new();
    assert_eq!(
        post_return_recap("body", false, |body, route| {
            assert_eq!(body, "body");
            routes.push(route.to_string());
            vec![Delivery::Failed("failed".into())]
        }),
        0
    );
    assert_eq!(routes, [""]);
}
