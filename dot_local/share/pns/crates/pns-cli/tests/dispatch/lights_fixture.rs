/// A port on the loopback nothing is listening on, so a bridge GET fails at
/// once instead of waiting out its deadline. NO REAL BRIDGE IS EVER NAMED in
/// this suite; the cases below are about what the engine SAYS, and the dial
/// they might make has to cost nothing.
pub(super) const DEAD_BRIDGE: &str = "127.0.0.1:9";
