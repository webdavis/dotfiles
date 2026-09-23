use std::sync::Mutex;

/// A desk notifier that records each spawn, program first, and raises nothing.
#[derive(Default)]
pub(crate) struct RecordingNotifier(Mutex<Vec<Vec<String>>>);

impl pns_application::CommandRunner for RecordingNotifier {
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        let mut call = vec![program.to_owned()];
        call.extend(args.iter().map(|arg| (*arg).to_owned()));
        self.0.lock().unwrap().push(call);
        Some(String::new())
    }
}

impl RecordingNotifier {
    pub(crate) fn calls(&self) -> Vec<Vec<String>> {
        self.0.lock().unwrap().clone()
    }
}
