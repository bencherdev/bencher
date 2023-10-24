use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Audience {
    Bencher,
}

impl ToString for Audience {
    fn to_string(&self) -> String {
        match self {
            Self::Bencher => "bencher".into(),
        }
    }
}
