use super::*;
use pns_domain::{lamps::Inventory, lights::phase::HeldEntry};
use std::cell::RefCell;

struct Held {
    entries: Option<Vec<HeldEntry>>,
    calls: RefCell<Vec<String>>,
}
impl HeldLamps for Held {
    fn read(&self) -> Option<Vec<HeldEntry>> {
        self.calls.borrow_mut().push("read".into());
        self.entries.clone()
    }
    fn remember(&self, entries: &[HeldEntry]) -> Result<(), String> {
        assert!(entries.is_empty());
        self.calls.borrow_mut().push("forget".into());
        Err("unwritable".into())
    }
}
struct Bridge<'a>(&'a Held);
impl LampBridge for Bridge<'_> {
    fn inventory(&self) -> Option<Inventory> {
        panic!("clears never read inventory")
    }
    fn write(&self, path: &str, write: &LampWrite) {
        assert!(matches!(write, LampWrite::Clear));
        self.0.calls.borrow_mut().push(path.into());
    }
}

#[test]
fn an_unreadable_or_empty_held_record_never_constructs_a_bridge_or_forgets_it() {
    for entries in [None, Some(Vec::new())] {
        let held = Held {
            entries,
            calls: RefCell::new(Vec::new()),
        };
        clear_held_lamps::<Bridge<'_>>(&held, || panic!("no bridge without named lamps"));
        assert_eq!(*held.calls.borrow(), ["read"]);
    }
}

#[test]
fn missing_bridge_credentials_keep_the_named_held_record() {
    let held = Held {
        entries: Some(vec![HeldEntry::bare("light/a")]),
        calls: RefCell::new(Vec::new()),
    };
    clear_held_lamps::<Bridge<'_>>(&held, || None);
    assert_eq!(*held.calls.borrow(), ["read"]);
}

#[test]
fn a_clear_writes_every_recorded_name_before_forgetting_without_an_inventory_read() {
    let held = Held {
        entries: Some(vec![HeldEntry::bare("light/a"), HeldEntry::bare("light/b")]),
        calls: RefCell::new(Vec::new()),
    };
    clear_held_lamps(&held, || Some(Bridge(&held)));
    assert_eq!(
        *held.calls.borrow(),
        ["read", "light/a", "light/b", "forget"]
    );
}
