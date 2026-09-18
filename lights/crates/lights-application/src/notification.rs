use lights_domain::Action;

pub trait Notifier {
    fn announce(&self, action: &Action);
    /// Say out loud that something is wrong, where an action would have said
    /// what went right. A KEY PRESS HAS NO TERMINAL: a failure written to
    /// standard error is invisible when the caller is a keybinding, so the one
    /// failure that is not "the bridge is unplugged" is announced instead of
    /// only printed.
    fn alarm(&self, detail: &str);
}
