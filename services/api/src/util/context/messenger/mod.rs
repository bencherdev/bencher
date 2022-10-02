mod email;
mod message;

pub use email::Email;
pub use message::Message;

pub enum Messenger {
    StdOut,
    Email(Email),
}
