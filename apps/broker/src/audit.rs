//! JSON-lines audit trail on stdout (journald). Never carries secrets: callers
//! pass only user names, addresses, host names, ports and failure classes.

use std::io::Write;
use std::sync::Mutex;

use serde_json::{Map, Value};

pub struct Audit {
    sink: Sink,
}

enum Sink {
    Stdout,
    Memory(Mutex<Vec<Value>>),
}

impl Audit {
    pub fn stdout() -> Audit {
        Audit { sink: Sink::Stdout }
    }

    pub fn memory() -> Audit {
        Audit { sink: Sink::Memory(Mutex::new(Vec::new())) }
    }

    /// `fields` must be a JSON object; `ts` and `event` are added.
    pub fn event(&self, event: &str, fields: Value) {
        let mut m = Map::new();
        m.insert("ts".into(), Value::from(crate::util::unix_now()));
        m.insert("audit".into(), Value::from("plank-broker"));
        m.insert("event".into(), Value::from(event));
        if let Value::Object(f) = fields {
            m.extend(f);
        }
        let v = Value::Object(m);
        match &self.sink {
            Sink::Stdout => {
                let mut out = std::io::stdout().lock();
                let _ = writeln!(out, "{v}");
                let _ = out.flush();
            }
            Sink::Memory(buf) => buf.lock().unwrap().push(v),
        }
    }

    /// Recorded events (memory sink only).
    pub fn events(&self) -> Vec<Value> {
        match &self.sink {
            Sink::Memory(buf) => buf.lock().unwrap().clone(),
            Sink::Stdout => Vec::new(),
        }
    }

    pub fn count(&self, event: &str) -> usize {
        self.events().iter().filter(|e| e["event"] == event).count()
    }
}
