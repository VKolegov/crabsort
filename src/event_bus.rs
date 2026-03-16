use std::{cell::RefCell, rc::Rc};

pub struct Event {
    pub source: &'static str,
    pub payload: String,
}

#[derive(Clone)]
pub struct EventBus {
    events: Rc<RefCell<Vec<Event>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            events: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn push(&self, source: &'static str, payload: String) {
        self.events.borrow_mut().push(Event { source, payload });
    }

    pub fn drain(&self) -> Vec<Event> {
        self.events.borrow_mut().drain(..).collect()
    }
}
