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
use crate::studio::uuid::Uuid;

const DEFAULT_TABLE_NAME: &str = "New Table";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    id: Uuid,
    name: String,
    columns: Vector<Uuid>,
    headers: HashMap<Uuid, Header>,
    rows: Vector<Row>,
}

impl Table {
    pub fn new() -> Table {
        Table {
            id: Uuid::new(),
            name: DEFAULT_TABLE_NAME.to_owned(),
            columns: Vector::new(),
            headers: HashMap::new(),
            rows: Vector::new(),
        }
    }

    pub fn load(data: &str) -> Result<Table> {
        Ok(from_str(data)?)
    }

    pub fn save(&self) -> Result<String> {
        let pretty = PrettyConfig::new();
        Ok(to_string_pretty(&self, pretty)?)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
    }

    pub fn add_column(&mut self) -> Uuid {
        let name = english_numbers::convert(
            (self.columns.len() + 1) as i64,
            english_numbers::Formatting {
                title_case: true,
                spaces: true,
                conjunctions: false,
                commas: false,
                dashes: false,
            },
        );
        let header = Header::new(&name, DataType::Text);
        let id = header.id().clone();
        self.columns.push_back(id.clone());
        for row in self.rows.iter_mut() {
            row.insert(id.clone(), Cell::Text(TextCell::new()));
        }
        self.headers.insert(id.clone(), header);
        id
    }

    pub fn delete_column(&mut self, id: &Uuid) -> Result<()> {
        let mut index = None;
        for (column_index, column_id) in self.columns.iter().enumerate() {
            if *id == *column_id {
                index = Some(column_index);
                break;
            }
        }
        self.columns.remove(index.ok_or(anyhow!(
            "Delete column failed to find id {} in columns",
            *id
        ))?);
        self.headers.remove(id).ok_or(anyhow!(
            "Delete column failed to find id {} in headers",
            *id
        ))?;
        for (row_index, row) in self.rows.iter_mut().enumerate() {
            row.remove(id).ok_or(anyhow!(
                "Delete column failed to find id {} in row {}",
                *id,
                row_index,
            ))?;
        }
        Ok(())
    }

    pub fn update_header_name(&mut self, id: &Uuid, name: &str) -> Result<()> {
        let header = self.headers.get_mut(id).ok_or(anyhow!(
            "Update header name failed to find id {} in headers",
            *id
        ))?;
        header.set_name(name);
        Ok(())
    }

    pub fn update_header_data_type(&mut self, id: &Uuid, data_type: DataType) -> Result<()> {
        for (index, row) in self.rows.iter_mut().enumerate() {
            let cell = row.get(id).ok_or(anyhow!(
                "Update header data type failed to find id {} in row {}",
                *id,
                index
            ))?;
            let cell = match data_type {
                DataType::Text => Cell::from(cell.clone()),
                DataType::Number => Cell::from(cell.clone()),
            };
            row.insert(*id, cell).ok_or(anyhow!(
                "Update header data type failed to find previous id {} in row {}",
                *id,
                index
            ))?;
        }
        let header = self.headers.get_mut(id).ok_or(anyhow!(
            "Update header data type failed to find id {} in headers",
            *id
        ))?;
        header.set_data_type(data_type);
        Ok(())
    }

    pub fn add_row(&mut self) -> Result<()> {
        let mut row = Row::new();
        for id in self.columns.iter() {
            let data_value = match self
                .headers
                .get(id)
                .ok_or(anyhow!("Add row could not find header for column {}", id))?
                .data_type()
            {
                DataType::Text => Cell::Text(TextCell::new()),
                DataType::Number => Cell::Number(NumberCell::new()),
            };
            row.insert(id.clone(), data_value);
        }
        self.rows.push_back(row);
        Ok(())
    }

    pub fn delete_row(&mut self, index: usize) -> Result<()> {
        let len = self.rows.len();
        if index >= len {
            bail!("Delete row index {} out of bounds of rows {}", index, len);
        }
        self.rows.remove(index);
        Ok(())
    }

    pub fn to_html(&self) -> Node<TableMsg> {
        div![
            attrs![At::Class => "table-container"],
            table![
                attrs![At::Class => "table is-bordered is-hoverable is-narrow"],
                self.get_headers(),
                self.get_body(),
            ],
        ]
    }

    fn get_headers(&self) -> Node<TableMsg> {
        thead![
            tr![th![
                attrs![At::ColSpan => (self.columns.len() + 2).to_string()],
                style![St::TextAlign => "center"],
                self.name()
            ]],
            tr![self.columns.iter().map(|column_id| {
                th![
                    attrs![At::Scope => "column"],
                    match self.headers.get(column_id) {
                        Some(header) => header.name(),
                        None => "ERROR",
                    }
                ]
            })],
            tr![self.columns.iter().map(|column_id| {
                th![
                    attrs![At::Scope => "column"],
                    match self.headers.get(column_id) {
                        Some(header) => header.data_type().to_html(),
                        None => DataType::Text.to_html(),
                    }
                ]
            })],
        ]
    }

    fn get_body(&self) -> Node<TableMsg> {
        tbody![
            tr![
                th![attrs![At::Scope => "row"], "[ ]"],
                IF!(self.columns.len() > 2 =>
                   td![attrs![At::ColSpan => (self.columns.len() - 2).to_string()],]),
                td![style![St::TextAlign => "center"], "+"]
            ],
            self.rows.iter().map(|row| {
                tr![self.columns.iter().enumerate().map(|(index, column_id)| {
                    div![
                        th![attrs![At::Scope => "row"], (index + 1).to_string()],
                        td![match row.get(column_id) {
                            Some(cell) => match cell {
                                Cell::Text(text) => text.value().to_owned(),
                                Cell::Number(number) => number.value().to_string(),
                            },
                            None => "ERROR".to_owned(),
                        }]
                    ]
                })]
            }),
            tr![td![
                attrs![At::ColSpan => (self.columns.len() + 2).to_string()],
                style![St::TextAlign => "center"],
                "+"
            ]],
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

    enum Dimensions {
        Column,
        Row,
        Both,
    }

    fn get_table(dimensions: Dimensions) -> (Table, Option<Uuid>) {
        let mut table = Table::new();
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(0, table.columns.len());
        assert_eq!(0, table.headers.len());
        assert_eq!(0, table.rows.len());

        let header_name = "One";

        let mut id = None;
        match dimensions {
            Dimensions::Column => {
                let column_id = table.add_column();
                assert_eq!(DEFAULT_TABLE_NAME, table.name());
                assert_eq!(1, table.columns.len());
                assert_eq!(1, table.headers.len());
                assert_eq!(0, table.rows.len());

                let header = table.headers.get(&column_id).expect("test");
                assert_eq!(header_name, header.name());
                match header.data_type() {
                    DataType::Text => {}
                    _ => panic!("test"),
                }

                id = Some(column_id);
            }
            Dimensions::Row => {
                table.add_row().expect("test");

                assert_eq!(DEFAULT_TABLE_NAME, table.name());
                assert_eq!(0, table.columns.len());
                assert_eq!(0, table.headers.len());
                assert_eq!(1, table.rows.len());

                let row = table.rows.get(0).expect("test");
                assert_eq!(0, row.len());
            }
            Dimensions::Both => {
                let (mut c_table, c_id) = get_table(Dimensions::Column);
                let c_id = c_id.expect("test");

                c_table.add_row().expect("test");
                assert_eq!(DEFAULT_TABLE_NAME, c_table.name());
                assert_eq!(1, c_table.columns.len());
                assert_eq!(1, c_table.headers.len());
                assert_eq!(1, c_table.rows.len());

                let header = c_table.headers.get(&c_id).expect("test");
                assert_eq!(header_name, header.name());
                match header.data_type() {
                    DataType::Text => {}
                    _ => panic!("test"),
                }

                let row = c_table.rows.get(0).expect("test");
                let cell = row.get(&c_id).expect("test");
                match cell {
                    Cell::Text(text) => assert_eq!("", text.value()),
                    _ => panic!("test"),
                }

                let (mut r_table, _) = get_table(Dimensions::Row);

                let r_id = r_table.add_column();
                assert_eq!(DEFAULT_TABLE_NAME, r_table.name());
                assert_eq!(1, r_table.columns.len());
                assert_eq!(1, r_table.headers.len());
                assert_eq!(1, r_table.rows.len());

                let header = r_table.headers.get(&r_id).expect("test");
                assert_eq!(header_name, header.name());
                match header.data_type() {
                    DataType::Text => {}
                    _ => panic!("test"),
                }

                let row = r_table.rows.get(0).expect("test");
                let cell = row.get(&r_id).expect("test");
                match cell {
                    Cell::Text(text) => assert_eq!("", text.value()),
                    _ => panic!("test"),
                }

                assert!(c_id != r_id);

                table = c_table;
                id = Some(c_id);
            }
        }

        (table, id)
    }

    #[test]
    pub fn test_table_new() {
        let table = Table::new();
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(0, table.columns.len());
        assert_eq!(0, table.headers.len());
        assert_eq!(0, table.rows.len());
    }

    #[test]
    pub fn test_table_save_load() {
        let saved = r#"(
    id: "01F3TNWZE89WQKSMQXTTAZ8V8X",
    name: "New Table",
    columns: [],
    headers: {},
    rows: [],
)"#;
        let mut table = Table::load(saved).expect("test");
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(0, table.columns.len());
        assert_eq!(0, table.headers.len());
        assert_eq!(0, table.rows.len());

        let last_saved = table.save().expect("test");
        assert_eq!(saved, last_saved);
    }

    #[test]
    pub fn test_table_name() {
        let mut table = Table::new();
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(0, table.columns.len());
        assert_eq!(0, table.headers.len());
        assert_eq!(0, table.rows.len());

        let table_name = "People";
        table.set_name(table_name);
        assert_eq!(table_name, table.name());
        assert_eq!(0, table.columns.len());
        assert_eq!(0, table.headers.len());
        assert_eq!(0, table.rows.len());
    }

    #[test]
    pub fn test_table_add_column() {
        let (mut table, first_id) = get_table(Dimensions::Column);
        let first_id = first_id.expect("test");

        let second_id = table.add_column();
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(2, table.columns.len());
        assert_eq!(2, table.headers.len());
        assert_eq!(0, table.rows.len());

        let header = table.headers.get(&first_id).expect("test");
        assert_eq!("One", header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let header = table.headers.get(&second_id).expect("test");
        assert_eq!("Two", header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let (mut table, first_id) = get_table(Dimensions::Both);
        let first_id = first_id.expect("test");

        let second_id = table.add_column();
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(2, table.columns.len());
        assert_eq!(2, table.headers.len());
        assert_eq!(1, table.rows.len());

        let header = table.headers.get(&first_id).expect("test");
        assert_eq!("One", header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let header = table.headers.get(&second_id).expect("test");
        assert_eq!("Two", header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let row = table.rows.get(0).expect("test");
        let cell = row.get(&first_id).expect("test");
        match cell {
            Cell::Text(text) => assert_eq!("", text.value()),
            _ => panic!("test"),
        }
        let cell = row.get(&second_id).expect("test");
        match cell {
            Cell::Text(text) => assert_eq!("", text.value()),
            _ => panic!("test"),
        }

        let saved = table.save().expect("test");
        table = Table::load(&saved).expect("test");

        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(2, table.columns.len());
        assert_eq!(2, table.headers.len());
        assert_eq!(1, table.rows.len());

        let header = table.headers.get(&first_id).expect("test");
        assert_eq!("One", header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let header = table.headers.get(&second_id).expect("test");
        assert_eq!("Two", header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let row = table.rows.get(0).expect("test");
        let cell = row.get(&first_id).expect("test");
        match cell {
            Cell::Text(text) => assert_eq!("", text.value()),
            _ => panic!("test"),
        }
        let cell = row.get(&second_id).expect("test");
        match cell {
            Cell::Text(text) => assert_eq!("", text.value()),
            _ => panic!("test"),
        }
    }

    #[test]
    pub fn test_table_delete_column() {
        let (mut table, first_id) = get_table(Dimensions::Column);
        let first_id = first_id.expect("test");

        let second_id = table.add_column();
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(2, table.columns.len());
        assert_eq!(2, table.headers.len());
        assert_eq!(0, table.rows.len());

        let header = table.headers.get(&first_id).expect("test");
        assert_eq!("One", header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let header = table.headers.get(&second_id).expect("test");
        assert_eq!("Two", header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        table.delete_column(&second_id).expect("test");
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(1, table.columns.len());
        assert_eq!(1, table.headers.len());
        assert_eq!(0, table.rows.len());

        let header = table.headers.get(&first_id).expect("test");
        assert_eq!("One", header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        match table.headers.get(&second_id) {
            Some(_) => panic!("test"),
            None => {}
        }

        match table.delete_column(&second_id) {
            Ok(_) => panic!("test"),
            Err(_) => {}
        }
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(1, table.columns.len());
        assert_eq!(1, table.headers.len());
        assert_eq!(0, table.rows.len());

        let header = table.headers.get(&first_id).expect("test");
        assert_eq!("One", header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        match table.headers.get(&second_id) {
            Some(_) => panic!("test"),
            None => {}
        }

        table.delete_column(&first_id).expect("test");
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(0, table.columns.len());
        assert_eq!(0, table.headers.len());
        assert_eq!(0, table.rows.len());

        let saved = table.save().expect("test");
        table = Table::load(&saved).expect("test");

        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(0, table.columns.len());
        assert_eq!(0, table.headers.len());
        assert_eq!(0, table.rows.len());
    }

    #[test]
    pub fn test_table_update_header_name() {
        let (mut table, id) = get_table(Dimensions::Column);
        let id = id.expect("test");

        let header_name = "First";
        table.update_header_name(&id, header_name).expect("test");
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(1, table.columns.len());
        assert_eq!(1, table.headers.len());
        assert_eq!(0, table.rows.len());

        let header = table.headers.get(&id).expect("test");
        assert_eq!(header_name, header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let header_name = "Last";
        table.update_header_name(&id, header_name).expect("test");
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(1, table.columns.len());
        assert_eq!(1, table.headers.len());
        assert_eq!(0, table.rows.len());

        let header = table.headers.get(&id).expect("test");
        assert_eq!(header_name, header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let saved = table.save().expect("test");
        table = Table::load(&saved).expect("test");

        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(1, table.columns.len());
        assert_eq!(1, table.headers.len());
        assert_eq!(0, table.rows.len());

        let header = table.headers.get(&id).expect("test");
        assert_eq!(header_name, header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }
    }

    #[test]
    pub fn test_table_update_header_data_type() {
        let (mut table, id) = get_table(Dimensions::Column);
        let id = id.expect("test");

        let header_name = "One";
        table
            .update_header_data_type(&id, DataType::Text)
            .expect("test");
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(1, table.columns.len());
        assert_eq!(1, table.headers.len());
        assert_eq!(0, table.rows.len());

        let header = table.headers.get(&id).expect("test");
        assert_eq!(header_name, header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        table
            .update_header_data_type(&id, DataType::Number)
            .expect("test");
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(1, table.columns.len());
        assert_eq!(1, table.headers.len());
        assert_eq!(0, table.rows.len());

        let header = table.headers.get(&id).expect("test");
        assert_eq!(header_name, header.name());
        match header.data_type() {
            DataType::Number => {}
            _ => panic!("test"),
        }

        let saved = table.save().expect("test");
        table = Table::load(&saved).expect("test");

        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(1, table.columns.len());
        assert_eq!(1, table.headers.len());
        assert_eq!(0, table.rows.len());

        let header = table.headers.get(&id).expect("test");
        assert_eq!(header_name, header.name());
        match header.data_type() {
            DataType::Number => {}
            _ => panic!("test"),
        }
    }

    #[test]
    pub fn test_table_add_row() {
        let (mut table, _) = get_table(Dimensions::Row);

        table.add_row().expect("test");
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(0, table.columns.len());
        assert_eq!(0, table.headers.len());
        assert_eq!(2, table.rows.len());

        assert_eq!(0, table.headers.len());
        let row = table.rows.get(0).expect("test");
        assert_eq!(0, row.len());
        let row = table.rows.get(1).expect("test");
        assert_eq!(0, row.len());

        let (mut table, id) = get_table(Dimensions::Both);
        let id = id.expect("test");

        table.add_row().expect("test");
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(1, table.columns.len());
        assert_eq!(1, table.headers.len());
        assert_eq!(2, table.rows.len());

        let header = table.headers.get(&id).expect("test");
        assert_eq!("One", header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let row = table.rows.get(0).expect("test");
        let cell = row.get(&id).expect("test");
        match cell {
            Cell::Text(text) => assert_eq!("", text.value()),
            _ => panic!("test"),
        }

        let row = table.rows.get(1).expect("test");
        let cell = row.get(&id).expect("test");
        match cell {
            Cell::Text(text) => assert_eq!("", text.value()),
            _ => panic!("test"),
        }

        let saved = table.save().expect("test");
        table = Table::load(&saved).expect("test");

        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(1, table.columns.len());
        assert_eq!(1, table.headers.len());
        assert_eq!(2, table.rows.len());

        let header = table.headers.get(&id).expect("test");
        assert_eq!("One", header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let row = table.rows.get(0).expect("test");
        let cell = row.get(&id).expect("test");
        match cell {
            Cell::Text(text) => assert_eq!("", text.value()),
            _ => panic!("test"),
        }

        let row = table.rows.get(1).expect("test");
        let cell = row.get(&id).expect("test");
        match cell {
            Cell::Text(text) => assert_eq!("", text.value()),
            _ => panic!("test"),
        }
    }

    #[test]
    pub fn test_table_delete_row() {
        let (mut table, id) = get_table(Dimensions::Both);
        let id = id.expect("test");

        table.add_row().expect("test");
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(1, table.columns.len());
        assert_eq!(1, table.headers.len());
        assert_eq!(2, table.rows.len());

        let header = table.headers.get(&id).expect("test");
        assert_eq!("One", header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let row = table.rows.get(0).expect("test");
        let cell = row.get(&id).expect("test");
        match cell {
            Cell::Text(text) => assert_eq!("", text.value()),
            _ => panic!("test"),
        }

        let row = table.rows.get(1).expect("test");
        let cell = row.get(&id).expect("test");
        match cell {
            Cell::Text(text) => assert_eq!("", text.value()),
            _ => panic!("test"),
        }

        table.delete_row(1).expect("test");
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(1, table.columns.len());
        assert_eq!(1, table.headers.len());
        assert_eq!(1, table.rows.len());

        let header = table.headers.get(&id).expect("test");
        assert_eq!("One", header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let row = table.rows.get(0).expect("test");
        let cell = row.get(&id).expect("test");
        match cell {
            Cell::Text(text) => assert_eq!("", text.value()),
            _ => panic!("test"),
        }

        match table.rows.get(1) {
            Some(_) => panic!("test"),
            None => {}
        }

        match table.delete_row(1) {
            Ok(_) => panic!("test"),
            Err(_) => {}
        }

        table.delete_row(0).expect("test");
        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(1, table.columns.len());
        assert_eq!(1, table.headers.len());
        assert_eq!(0, table.rows.len());

        let header = table.headers.get(&id).expect("test");
        assert_eq!("One", header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        match table.rows.get(0) {
            Some(_) => panic!("test"),
            None => {}
        }

        match table.rows.get(1) {
            Some(_) => panic!("test"),
            None => {}
        }

        let saved = table.save().expect("test");
        table = Table::load(&saved).expect("test");

        assert_eq!(DEFAULT_TABLE_NAME, table.name());
        assert_eq!(1, table.columns.len());
        assert_eq!(1, table.headers.len());
        assert_eq!(0, table.rows.len());

        let header = table.headers.get(&id).expect("test");
        assert_eq!("One", header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        match table.rows.get(0) {
            Some(_) => panic!("test"),
            None => {}
        }

        match table.rows.get(1) {
            Some(_) => panic!("test"),
            None => {}
        }
    }
}
