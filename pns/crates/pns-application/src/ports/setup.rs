use pns_domain::Answers;

pub trait Terminal {
    fn is_terminal(&self) -> bool;
    fn say(&self, line: &str);
    fn ask(&self, question: &str) -> Result<String, String>;
    fn ask_hidden(&self, question: &str) -> Result<String, String>;
}

pub trait ConfigRenderer {
    fn compose(&self, answers: &Answers) -> String;
    fn validate(&self, composed: &str) -> Result<(), String>;
}

pub trait ConfigPublisher {
    fn path(&self) -> String;
    fn check(&self, force: bool) -> Result<(), String>;
    fn publish(&self, composed: &str, force: bool) -> Result<Option<String>, String>;
}
