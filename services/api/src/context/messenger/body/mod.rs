mod button;
mod new_user;
#[cfg(feature = "plus")]
mod server_stats;

pub use button::ButtonBody;
pub use new_user::NewUserBody;
#[cfg(feature = "plus")]
pub use server_stats::ServerStatsBody;

pub trait FmtBody {
    fn text(&self) -> String;
    fn html(&self) -> String;
}

pub enum Body {
    Button(Box<ButtonBody>),
    NewUser(NewUserBody),
    #[cfg(feature = "plus")]
    ServerStats(ServerStatsBody),
}

impl FmtBody for Body {
    fn text(&self) -> String {
        match self {
            Self::Button(body) => body.text(),
            Self::NewUser(body) => body.text(),
            #[cfg(feature = "plus")]
            Self::ServerStats(body) => body.text(),
        }
    }

    fn html(&self) -> String {
        match self {
            Self::Button(body) => body.html(),
            Self::NewUser(body) => body.html(),
            #[cfg(feature = "plus")]
            Self::ServerStats(body) => body.html(),
        }
    }
}
