mod arboard;

pub use arboard::paste_with;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sensitivity {
    Sensitive,
    Public,
}

impl Sensitivity {
    pub fn from_public(public: bool) -> Self {
        if public {
            Self::Public
        } else {
            Self::Sensitive
        }
    }
}
