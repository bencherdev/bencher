use seed::{prelude::*, *};

use chrono::prelude::*;

// ------ ------
//     Init
// ------ ------

pub fn init(url: Url, _: &mut impl Orders<Msg>) -> Model {
    Model {
        changes_status: ChangesStatus::NoChanges,
        errors: Vec::new(),

        form: Form {
            username: String::new(),
            email: String::new(),
            password: String::new(),
            confirm_password: String::new(),

            errors: FormErrors::default(),
        },
    }
}

// ------ ------
//     Model
// ------ ------

pub struct Model {
    changes_status: ChangesStatus,
    errors: Vec<FetchError>,

    form: Form,
}

enum ChangesStatus {
    NoChanges,
    Saving { requests_in_flight: usize },
    Saved(DateTime<Local>),
}

struct Form {
    username: String,
    email: String,
    password: String,
    confirm_password: String,

    errors: FormErrors,
}

#[derive(Default)]
struct FormErrors {
    username: Option<String>,
    email: Option<String>,
    password: Option<String>,
    confirm_password: Option<String>,
}

// ------ ------
//    Update
// ------ ------

pub enum Msg {
    ChangesSaved(Option<FetchError>),
    ClearErrors,

    UsernameChanged(String),
    EmailChanged(String),
    PasswordChanged(String),
    ConfirmPasswordChanged(String),

    Save,
    DeleteAccount,
}

pub fn update(msg: Msg, model: &mut Model, _: &mut impl Orders<Msg>) {
    match msg {
        Msg::ChangesSaved(None) => {}
        Msg::ChangesSaved(Some(fetch_error)) => {}
        Msg::ClearErrors => {}

        Msg::UsernameChanged(username) => {}
        Msg::EmailChanged(email) => {}
        Msg::PasswordChanged(password) => {}
        Msg::ConfirmPasswordChanged(confirm_password) => {}

        Msg::Save => {}
        Msg::DeleteAccount => {}
    }
}

// ------ ------
//     View
// ------ ------

pub fn view(model: &Model) -> Node<Msg> {
    div!["Settings view"]
}
