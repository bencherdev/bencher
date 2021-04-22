// (Lines like the one below ignore selected Clippy rules
//  - it's useful when you want to check your code with `cargo make verify`
// but some rules are too "annoying" or are not applicable for your case.)
#![allow(clippy::wildcard_imports)]

use std::convert::TryFrom;

use seed::{prelude::*, *};

use crate::studio::exec::exec;
use crate::studio::table::cell::{Cell, NumberCell, TextCell};
use crate::studio::table::header::{DataType, Header};
use crate::studio::table::nameless::nameless;
use crate::studio::table::tab::Tab;
use crate::studio::table::table::Table;

const ESCAPE_KEY: &str = "Escape";
const TAB_KEY: &str = "Tab";
const ENTER_KEY: &str = "Enter";

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
pub struct Model {
    table: Table,
    selected: Option<Selected>,
}

impl Model {
    pub fn new() -> Model {
        Model {
            table: Table::new(),
            selected: None,
        }
    }
}
struct Selected {
    selectable: Selectable,
    input_element: ElRef<web_sys::HtmlInputElement>,
}

// ------ ------
//    Update
// ------ ------

#[derive(Debug, Clone)]
pub enum Msg {
    Deselect,
    SelectTableName,
    UpdateTableName(String),
}

#[derive(Debug, Clone)]
pub enum Selectable {
    TableName,
}

// `update` describes how to handle each `Msg`.
pub fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::Deselect => {
            model.selected = None;
        }
        Msg::SelectTableName => {
            let input_element = ElRef::new();

            model.selected = Some(Selected {
                selectable: Selectable::TableName,
                input_element: input_element.clone(),
            });

            let field = match model.table.name() {
                Some(name) => name.to_owned(),
                None => Table::nameless(0),
            };

            let title_length = u32::try_from(field.len()).expect("field length as u32");
            orders.after_next_render(move |_| {
                let input_element = input_element.get().expect("input_element");

                input_element.focus().expect("focus input_element");

                input_element
                    .set_selection_range(title_length, title_length)
                    .expect("move cursor to the end of input_element");
            });
        }
        Msg::UpdateTableName(field) => {
            model.table.set_name(Some(field));
            let mut code = "fn main() {".to_owned();
            code.push_str(&model.table.to_code().unwrap());
            code.push_str("}");
            log(code.clone());
            log(exec(code));
        }
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
            div![attrs![At::Class => "content"], view_table(model),],
        ]
    ]
}

pub fn view_table(model: &Model) -> Node<Msg> {
    let tab = match model.table.tabs_map().get(&model.table.tab_id()) {
        Some(tab) => tab,
        None => return div![],
    };

    let col_span = if model.table.tabs_vec().len() > tab.columns().len() {
        model.table.tabs_vec().len()
    } else {
        tab.columns().len()
    };

    let table_name = th![
        el_key(&format!("table-name")),
        attrs![At::ColSpan => (col_span + 2).to_string() ],
        style![St::TextAlign => "center"],
        match model.table.name() {
            Some(name) => b![name.to_owned()],
            None => i![Table::nameless(0)],
        },
        ev(Ev::Click, move |_| Msg::SelectTableName)
    ];

    div![
        attrs![At::Class => "table-container"],
        table![
            attrs![At::Class => "table is-bordered is-hoverable is-narrow"],
            thead![
                tr![
                    el_key(&format!("table-name")),
                    match &model.selected {
                        Some(selected) => match &selected.selectable {
                            Selectable::TableName => {
                                th![
                                    el_key(&format!("table-name")),
                                    attrs![At::ColSpan => (col_span + 2).to_string() ],
                                    style![St::TextAlign => "center"],
                                    input![
                                        C!["edit"],
                                        el_ref(&selected.input_element),
                                        attrs! {At::Value => match model.table.name() {
                                            Some(name) => name.to_owned(),
                                            None => Table::nameless(0),
                                        }},
                                        input_ev(Ev::Input, Msg::UpdateTableName),
                                        keyboard_ev(Ev::KeyDown, |keyboard_event| {
                                            Some(match keyboard_event.key().as_str() {
                                                // TODO add selection of Tabs when hitting enter
                                                ESCAPE_KEY | TAB_KEY | ENTER_KEY => Msg::Deselect,
                                                _ => return None,
                                            })
                                        }),
                                    ]
                                ]
                            }
                            _ => table_name,
                        },
                        None => table_name,
                    }
                ],
                tr![
                    el_key(&format!("tabs")),
                    model
                        .table
                        .tabs_vec()
                        .iter()
                        .enumerate()
                        .map(|(index, tab_id)| {
                            match model.table.tabs_map().get(tab_id) {
                                Some(tab) => match tab.name() {
                                    Some(name) => th![name.to_owned()],
                                    None => th![i![format!("Tab {}", nameless(index))]],
                                },
                                None => th!["Error".to_owned()],
                            }
                        }),
                    th![
                        style![St::TextAlign => "center", St::VerticalAlign => "middle"],
                        "+"
                    ],
                    th![
                        attrs![At::ColSpan => (if col_span > 1 { col_span - 1 } else { 1 }).to_string()
                        ],
                    ]
                ],
                view_headers(tab, col_span)
            ],
            view_rows(tab, col_span)
        ],
    ]
}

pub fn view_headers(tab: &Tab, col_span: usize) -> Vec<Node<Msg>> {
    let tuple_toggle = th![
        el_key(&format!("tuple-toggle")),
        attrs![At::Scope => "row"],
        style![St::TextAlign => "center", St::VerticalAlign => "middle"],
        "(1 2 3)"
    ];
    let select_all = th![
        el_key(&format!("select-all-rows")),
        attrs![At::Scope => "row"],
        style![St::TextAlign => "center", St::VerticalAlign => "middle"],
        "[ ]"
    ];
    let add_column = td![
        el_key(&format!("add-column")),
        attrs![At::Scope => "row", At::RowSpan => 2],
        style![St::TextAlign => "center", St::VerticalAlign => "middle"],
        "+"
    ];
    let empty = th![
        el_key(&format!("empty")),
        attrs![At::Scope => "column", At::ColSpan => col_span.to_string() ],
        "⠀"
    ];

    if tab.columns().len() == 0 {
        return vec![
            tr![tuple_toggle.clone(), empty.clone(), add_column.clone(),],
            tr![select_all.clone(), empty],
        ];
    }

    vec![
        tr![
            el_key(&format!("column-names")),
            tuple_toggle,
            tab.columns().iter().enumerate().map(|(index, column_id)| {
                th![
                    el_key(&format!("column-name-{}", column_id)),
                    attrs![At::Scope => "column"],
                    match tab.headers().get(column_id) {
                        Some(header) => match header.name() {
                            Some(name) => name.to_owned(),
                            None => nameless(index),
                        },
                        None => "ERROR".to_owned(),
                    }
                ]
            }),
            add_column,
        ],
        tr![
            el_key(&format!("column-data-types")),
            select_all,
            tab.columns().iter().map(|column_id| {
                th![
                    el_key(&format!("column-data-type-{}", column_id)),
                    attrs![At::Scope => "column"],
                    match tab.headers().get(column_id) {
                        Some(header) => header.data_type().to_html(),
                        None => DataType::Text.to_html(),
                    }
                ]
            }),
        ],
    ]
}

pub fn view_rows(tab: &Tab, col_span: usize) -> Node<Msg> {
    tbody![
        tab.rows().iter().enumerate().map(|(index, row)| {
            tr![
                el_key(&format!("row-{}", index)),
                th![
                    el_key(&format!("row-{}-select", index)),
                    attrs![At::Scope => "row"],
                    (index + 1).to_string()
                ],
                tab.columns().iter().map(|column_id| {
                    td![
                        el_key(&format!("column-{}-row-{}", column_id, index)),
                        match row.get(column_id) {
                            Some(cell) => match cell {
                                Cell::Text(text) => text.value().to_owned(),
                                Cell::Number(number) => number.value().to_string(),
                            },
                            None => "ERROR".to_owned(),
                        }
                    ]
                })
            ]
        }),
        tr![
            el_key(&format!("add-row")),
            td![
                el_key(&format!("add-row")),
                attrs![At::ColSpan => (col_span + 2).to_string()],
                style![St::TextAlign => "center"],
                "+"
            ]
        ],
    ]
}
