mod email;

pub use email::Email;

pub enum Messenger {
    StdOut,
    Email(Email),
}
