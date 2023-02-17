#[cfg(feature = "plus")]
mod advice;
mod button;

#[cfg(feature = "plus")]
pub use advice::AdviceBody;
pub use button::ButtonBody;

pub trait FmtBody {
    fn text(&self) -> String;
    fn html(&self) -> String;
}

pub enum Body {
    Button(Box<ButtonBody>),
    #[cfg(feature = "plus")]
    Advice(AdviceBody),
}

impl FmtBody for Body {
    fn text(&self) -> String {
        match self {
            Self::Button(body) => body.text(),
            #[cfg(feature = "plus")]
            Self::Advice(body) => body.text(),
        }
    }

    fn html(&self) -> String {
        match self {
            Self::Button(body) => body.html(),
            #[cfg(feature = "plus")]
            Self::Advice(body) => body.html(),
        }
    }
}
