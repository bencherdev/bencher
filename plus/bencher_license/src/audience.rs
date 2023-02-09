use bencher_plus::BENCHER_DEV;

#[derive(Debug, Copy, Clone)]
pub enum Audience {
    Bencher,
}

impl ToString for Audience {
    fn to_string(&self) -> String {
        match self {
            Self::Bencher => BENCHER_DEV.into(),
        }
    }
}

impl From<Audience> for String {
    fn from(audience: Audience) -> Self {
        audience.to_string()
    }
}
