use anyhow::{anyhow, bail, Result};
use english_numbers;
use im::hashmap::HashMap;
use im::vector::Vector;
use ron::de::from_str;
use ron::ser::{to_string_pretty, PrettyConfig};
use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};

use crate::studio::table::cell::{Cell, NumberCell, TextCell};
use crate::studio::table::header::{DataType, Header};
use crate::studio::table::row::Row;
use crate::studio::table::tab::Tab;
use crate::studio::uuid::Uuid;

const DEFAULT_TABLE_NAME: &str = "New Table";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    id: Uuid,
    name: String,
    tab_id: Uuid,
    tabs_vec: Vector<Uuid>,
    tabs_map: HashMap<Uuid, Tab>,
}

impl Table {
    pub fn new() -> Table {
        let tab = Tab::new();
        let id = tab.id().clone();
        let mut vec = Vector::new();
        vec.push_back(id.clone());
        let mut map = HashMap::new();
        map.insert(id.clone(), tab);
        Table {
            id: Uuid::new(),
            name: DEFAULT_TABLE_NAME.to_owned(),
            tab_id: id,
            tabs_vec: vec,
            tabs_map: map,
        }
    }

    pub fn to_html(&self) -> Node<TableMsg> {
        let tab = match self.tabs_map.get(&self.tab_id) {
            Some(tab) => tab,
            None => return div![],
        };

        let col_span = if self.tabs_vec.len() > tab.columns() {
            self.tabs_vec.len()
        } else {
            tab.columns()
        };

        div![
            attrs![At::Class => "table-container"],
            table![
                attrs![At::Class => "table is-bordered is-hoverable is-narrow"],
                thead![
                    tr![th![
                        attrs![At::ColSpan => (col_span + 2).to_string() ],
                        style![St::TextAlign => "center"],
                        &self.name
                    ]],
                    tr![
                        th![tab.name()],
                        th!["+"],
                        th![
                            attrs![At::ColSpan => (if col_span > 1 { col_span - 1 } else { 1 }).to_string()
                            ],
                        ]
                    ],
                    tab.get_headers(col_span)
                ],
                tab.get_rows(col_span)
            ],
        ]
    }
}

pub enum TableMsg {
    Increment,
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
}
