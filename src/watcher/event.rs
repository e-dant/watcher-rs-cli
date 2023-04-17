use std::{fmt, path::Path, time::Duration};

#[derive(Clone, Eq, PartialEq)]
pub struct Event {
    pub path: Box<Path>,
    pub what: What,
    pub kind: Kind,
    pub when: Duration,
}

#[allow(dead_code)]
impl Event {
    pub fn is_last(&self) -> bool {
        self.what == What::Destroy && self.kind == Kind::Watcher
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            r#""{}":{{"where":"{}","what":"{}","kind":"{}"}}{}"#,
            self.when.as_nanos(),
            self.path.to_string_lossy().replace('\0', ""),
            self.what,
            self.kind,
            if self.is_last() { "" } else { "," },
        )
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[allow(dead_code)]
pub enum What {
    Rename,
    Modify,
    Create,
    Destroy,
    Owner,
    Other,
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[allow(dead_code)]
pub enum Kind {
    Dir,
    File,
    HardLink,
    SymLink,
    Watcher,
    Other,
}

impl<'a> From<&'a str> for What {
    fn from(s: &'a str) -> What {
        match s {
            "rename" => What::Rename,
            "modify" => What::Modify,
            "create" => What::Create,
            "destroy" => What::Destroy,
            "owner" => What::Owner,
            "other" => What::Other,
            _ => What::Other,
        }
    }
}

impl From<String> for What {
    fn from(s: String) -> What {
        What::from(s.as_str())
    }
}

impl fmt::Display for What {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            What::Rename => write!(f, "rename"),
            What::Modify => write!(f, "modify"),
            What::Create => write!(f, "create"),
            What::Destroy => write!(f, "destroy"),
            What::Owner => write!(f, "owner"),
            What::Other => write!(f, "other"),
        }
    }
}

impl<'a> From<&'a str> for Kind {
    fn from(s: &'a str) -> Kind {
        match s {
            "dir" => Kind::Dir,
            "file" => Kind::File,
            "hard_link" => Kind::HardLink,
            "sym_link" => Kind::SymLink,
            "watcher" => Kind::Watcher,
            "other" => Kind::Other,
            _ => Kind::Other,
        }
    }
}

impl From<String> for Kind {
    fn from(s: String) -> Kind {
        Kind::from(s.as_str())
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Kind::Dir => write!(f, "dir"),
            Kind::File => write!(f, "file"),
            Kind::HardLink => write!(f, "hard_link"),
            Kind::SymLink => write!(f, "sym_link"),
            Kind::Watcher => write!(f, "watcher"),
            Kind::Other => write!(f, "other"),
        }
    }
}

// A callback takes an event and returns nothing.
// A callback is a synchronized, sendable function.
// pub type Callback = dyn Fn(Event) + Send + Sync + 'static;

// pub type Callback = dyn Fn(Event) + Send + Sync;
// pub trait Callback: Fn(Event) + Send + Sync + 'static{}
