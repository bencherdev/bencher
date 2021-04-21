// (Lines like the one below ignore selected Clippy rules
//  - it's useful when you want to check your code with `cargo make verify`
// but some rules are too "annoying" or are not applicable for your case.)
#![allow(clippy::wildcard_imports)]

use seed::{prelude::*, *};

use crate::studio::table::Table;

// ------ ------
//     Init
// ------ ------

// `init` describes what should happen when your app started.
pub fn init(_: Url, _: &mut impl Orders<Msg>) -> Model {
    Model::default()
}

// ------ ------
//     Model
// ------ ------

// `Model` describes our app state.
pub type Model = i32;

// ------ ------
//    Update
// ------ ------

// (Remove the line below once any of your `Msg` variants doesn't implement `Copy`.)
#[derive(Copy, Clone)]
// `Msg` describes the different events you can modify state with.
pub enum Msg {
    Increment,
}

// `update` describes how to handle each `Msg`.
pub fn update(msg: Msg, model: &mut Model, _: &mut impl Orders<Msg>) {
    match msg {
        Msg::Increment => *model += 1,
    }
}

// ------ ------
//     View
// ------ ------

#[allow(clippy::trivially_copy_pass_by_ref)]
pub fn view(model: &Model) -> Node<Msg> {
    div![
        attrs![At::Class => "columns"],
        div![
            attrs![At::Class => "column is-half"],
            div![
                attrs![At::Class => "content"],
                div![
                    attrs![At::Class => "table-container"],
                    table![
                        attrs![At::Class => "table is-bordered is-hoverable is-narrow"],
                        thead![tr![th!["Names"]],],
                        thead![tr![th!["First"], th!["Last"],],],
                        tbody![tr![td!["Saul"], td!["Goodman"],],],
                    ],
                ],
            ],
        ]
    ]
}
