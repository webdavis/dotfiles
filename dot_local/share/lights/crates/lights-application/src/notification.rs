use lights_domain::Action;

pub trait Notifier {
    fn announce(&self, action: &Action);
}
