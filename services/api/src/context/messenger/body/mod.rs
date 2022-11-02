mod button;

pub use button::ButtonBody;

pub trait FmtBody {
    fn text(&self) -> String;
    fn html(&self) -> String;
}

pub enum Body {
    Button(ButtonBody),
}

impl FmtBody for Body {
    fn text(&self) -> String {
        match self {
            Self::Button(body) => body.text(),
        }
    }

    fn html(&self) -> String {
        match self {
            Self::Button(body) => body.html(),
        }
    }
}
