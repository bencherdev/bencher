// (Lines like the one below ignore selected Clippy rules
//  - it's useful when you want to check your code with `cargo make verify`
// but some rules are too "annoying" or are not applicable for your case.)
#![allow(clippy::wildcard_imports)]

use seed::{prelude::*, *};

use crate::studio::table::table::{Table, TableMsg};

// ------ ------
//     Init
// ------ ------

// `init` describes what should happen when your app started.
pub fn init(_: Url, _: &mut impl Orders<Msg>) -> Model {
    Model::new()
}

// ------ ------
//     Model
// ------ ------

// `Model` describes our app state.
pub type Model = Table;

// ------ ------
//    Update
// ------ ------

pub type Msg = TableMsg;

// `update` describes how to handle each `Msg`.
pub fn update(msg: Msg, model: &mut Model, _: &mut impl Orders<Msg>) {
    match msg {
        TableMsg::Increment => {}
    };
}

// ------ ------
//     View
// ------ ------

pub fn view(model: &Model) -> Node<Msg> {
    div![
        attrs![At::Class => "columns"],
        div![
            attrs![At::Class => "column is-half"],
            div![attrs![At::Class => "content"], model.to_html(),],
        ]
    ]
}
