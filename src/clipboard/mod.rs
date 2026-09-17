mod arboard;

pub use arboard::paste_with;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
