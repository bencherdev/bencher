use std::fmt;

use anyhow::{anyhow, bail, Result};
use english_numbers;
use im::hashmap::HashMap;
use im::vector::Vector;
use ron::ser::{to_string_pretty, PrettyConfig};
use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};

use crate::studio::table::cell::{Cell, NumberCell, TextCell};
use crate::studio::table::header::{DataType, Header};
use crate::studio::table::nameless::nameless;
use crate::studio::table::row::Row;
use crate::studio::uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    id: Uuid,
    tab_type: TabType,
    name: Option<String>,
    columns: Vector<Uuid>,
    headers: HashMap<Uuid, Header>,
    rows: Vector<Row>,
}

impl Tab {
    pub fn new() -> Tab {
        Tab {
            id: Uuid::new(),
            tab_type: TabType::Tuple,
            name: None,
            columns: Vector::new(),
            headers: HashMap::new(),
            rows: Vector::new(),
        }
    }

    pub fn id(&self) -> &Uuid {
        &self.id
    }

    pub fn tab_type(&self) -> &TabType {
        &self.tab_type
    }

    pub fn set_tab_type(&mut self, tab_type: &TabType) {
        self.tab_type = tab_type.to_owned();
    }

    pub fn name(&self) -> &Option<String> {
        &self.name
    }

    pub fn set_name(&mut self, name: Option<String>) {
        self.name = name;
    }

    pub fn columns(&self) -> &Vector<Uuid> {
        &self.columns
    }

    pub fn headers(&self) -> &HashMap<Uuid, Header> {
        &self.headers
    }

    pub fn rows(&self) -> &Vector<Row> {
        &self.rows
    }

    pub fn add_column(&mut self) -> Uuid {
        let header = Header::new(
            match self.tab_type {
                TabType::Tuple => None,
                TabType::Struct => Some(nameless(self.columns.len())),
            },
            DataType::Text,
        );
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

    pub fn update_header_name(&mut self, id: &Uuid, name: Option<String>) -> Result<()> {
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

    pub fn to_code(&self) -> Result<String> {
        if self.columns.len() == 0 {
            // If the Tab has no Columns then it is a Unit Struct
            // Note that a Table uses this same condition for Enum Variants
            // with no associated data type.
            // Therefore it will only be possible to have a Unit Struct outside of an Enum
            // This can be overcome by having all Enums have an associated data type
            return Ok(format!("struct Unit{}; ", self.id));
        }

        let mut open;
        let mut fields = String::new();
        let mut close;

        match self.tab_type {
            TabType::Tuple => {
                open = "(";
                close = ");";
                // For each Column, add a data type to the Tuple Struct
                for (index, column_id) in self.columns.iter().enumerate() {
                    let header = self.headers.get(column_id).ok_or(anyhow!(
                        "Tuple tab to code failed to find header {} in map",
                        *column_id
                    ))?;

                    match header.name() {
                        Some(name) => bail!(
                            "Tuple tab to code failed as header {} is named {}",
                            header.id(),
                            name,
                        ),
                        None => {}
                    }

                    if index != 0 {
                        fields.push_str(", ");
                    }
                    fields.push_str(&format!("{}", *header.data_type()));
                }
            }
            TabType::Struct => {
                open = "{";
                close = "}";
                // For each Column, add a field and its data type to the Struct
                for column_id in &self.columns {
                    let header = self.headers.get(column_id).ok_or(anyhow!(
                        "Struct tab to code failed to find header {} in map",
                        *column_id
                    ))?;
                    fields.push_str(&format!(
                        "field_{}: {}, ",
                        *header.id(),
                        *header.data_type(),
                    ));
                }
            }
        }

        let mut code = format!(
            "struct {}{} {} {} {}",
            self.tab_type, self.id, open, fields, close
        );
        Ok(code)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TabType {
    Tuple,
    Struct,
}

impl fmt::Display for TabType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TabType::Tuple => "Tuple",
                TabType::Struct => "Struct",
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    const DEFAULT_TAB_NAME: Option<String> = None;
    const DEFAULT_HEADER_NAME: Option<String> = None;

    enum Dimensions {
        Column,
        Row,
        Both,
    }

    fn get_tab(dimensions: Dimensions) -> (Tab, Option<Uuid>) {
        let mut tab = Tab::new();
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(0, tab.columns.len());
        assert_eq!(0, tab.headers.len());
        assert_eq!(0, tab.rows.len());

        let mut id = None;
        match dimensions {
            Dimensions::Column => {
                let column_id = tab.add_column();
                assert_eq!(DEFAULT_TAB_NAME, *tab.name());
                assert_eq!(1, tab.columns.len());
                assert_eq!(1, tab.headers.len());
                assert_eq!(0, tab.rows.len());

                let header = tab.headers.get(&column_id).expect("test");
                assert_eq!(DEFAULT_HEADER_NAME, *header.name());
                match header.data_type() {
                    DataType::Text => {}
                    _ => panic!("test"),
                }

                id = Some(column_id);
            }
            Dimensions::Row => {
                tab.add_row().expect("test");

                assert_eq!(DEFAULT_TAB_NAME, *tab.name());
                assert_eq!(0, tab.columns.len());
                assert_eq!(0, tab.headers.len());
                assert_eq!(1, tab.rows.len());

                let row = tab.rows.get(0).expect("test");
                assert_eq!(0, row.len());
            }
            Dimensions::Both => {
                let (mut c_tab, c_id) = get_tab(Dimensions::Column);
                let c_id = c_id.expect("test");

                c_tab.add_row().expect("test");
                assert_eq!(DEFAULT_TAB_NAME, *c_tab.name());
                assert_eq!(1, c_tab.columns.len());
                assert_eq!(1, c_tab.headers.len());
                assert_eq!(1, c_tab.rows.len());

                let header = c_tab.headers.get(&c_id).expect("test");
                assert_eq!(DEFAULT_HEADER_NAME, *header.name());
                match header.data_type() {
                    DataType::Text => {}
                    _ => panic!("test"),
                }

                let row = c_tab.rows.get(0).expect("test");
                let cell = row.get(&c_id).expect("test");
                match cell {
                    Cell::Text(text) => assert_eq!("", text.value()),
                    _ => panic!("test"),
                }

                let (mut r_tab, _) = get_tab(Dimensions::Row);

                let r_id = r_tab.add_column();
                assert_eq!(DEFAULT_TAB_NAME, *r_tab.name());
                assert_eq!(1, r_tab.columns.len());
                assert_eq!(1, r_tab.headers.len());
                assert_eq!(1, r_tab.rows.len());

                let header = r_tab.headers.get(&r_id).expect("test");
                assert_eq!(DEFAULT_HEADER_NAME, *header.name());
                match header.data_type() {
                    DataType::Text => {}
                    _ => panic!("test"),
                }

                let row = r_tab.rows.get(0).expect("test");
                let cell = row.get(&r_id).expect("test");
                match cell {
                    Cell::Text(text) => assert_eq!("", text.value()),
                    _ => panic!("test"),
                }

                assert!(c_id != r_id);

                tab = c_tab;
                id = Some(c_id);
            }
        }

        (tab, id)
    }

    #[test]
    pub fn test_tab_new() {
        let tab = Tab::new();
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(0, tab.columns.len());
        assert_eq!(0, tab.headers.len());
        assert_eq!(0, tab.rows.len());
    }

    #[test]
    pub fn test_tab_name() {
        let mut tab = Tab::new();
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(0, tab.columns.len());
        assert_eq!(0, tab.headers.len());
        assert_eq!(0, tab.rows.len());

        let tab_name = Some("Person".to_owned());
        tab.set_name(tab_name.clone());
        assert_eq!(tab_name, *tab.name());
        assert_eq!(0, tab.columns.len());
        assert_eq!(0, tab.headers.len());
        assert_eq!(0, tab.rows.len());
    }

    #[test]
    pub fn test_tab_add_column() {
        let (mut tab, first_id) = get_tab(Dimensions::Column);
        let first_id = first_id.expect("test");

        let second_id = tab.add_column();
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(2, tab.columns.len());
        assert_eq!(2, tab.headers.len());
        assert_eq!(0, tab.rows.len());

        let header = tab.headers.get(&first_id).expect("test");
        assert_eq!(None, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let header = tab.headers.get(&second_id).expect("test");
        assert_eq!(None, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let (mut tab, first_id) = get_tab(Dimensions::Both);
        let first_id = first_id.expect("test");

        let second_id = tab.add_column();
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(2, tab.columns.len());
        assert_eq!(2, tab.headers.len());
        assert_eq!(1, tab.rows.len());

        let header = tab.headers.get(&first_id).expect("test");
        assert_eq!(None, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let header = tab.headers.get(&second_id).expect("test");
        assert_eq!(None, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let row = tab.rows.get(0).expect("test");
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
    pub fn test_tab_delete_column() {
        let (mut tab, first_id) = get_tab(Dimensions::Column);
        let first_id = first_id.expect("test");

        let second_id = tab.add_column();
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(2, tab.columns.len());
        assert_eq!(2, tab.headers.len());
        assert_eq!(0, tab.rows.len());

        let header = tab.headers.get(&first_id).expect("test");
        assert_eq!(None, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let header = tab.headers.get(&second_id).expect("test");
        assert_eq!(None, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        tab.delete_column(&second_id).expect("test");
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(1, tab.columns.len());
        assert_eq!(1, tab.headers.len());
        assert_eq!(0, tab.rows.len());

        let header = tab.headers.get(&first_id).expect("test");
        assert_eq!(None, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        match tab.headers.get(&second_id) {
            Some(_) => panic!("test"),
            None => {}
        }

        match tab.delete_column(&second_id) {
            Ok(_) => panic!("test"),
            Err(_) => {}
        }
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(1, tab.columns.len());
        assert_eq!(1, tab.headers.len());
        assert_eq!(0, tab.rows.len());

        let header = tab.headers.get(&first_id).expect("test");
        assert_eq!(None, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        match tab.headers.get(&second_id) {
            Some(_) => panic!("test"),
            None => {}
        }

        tab.delete_column(&first_id).expect("test");
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(0, tab.columns.len());
        assert_eq!(0, tab.headers.len());
        assert_eq!(0, tab.rows.len());
    }

    #[test]
    pub fn test_tab_update_header_name() {
        let (mut tab, id) = get_tab(Dimensions::Column);
        let id = id.expect("test");

        let header_name = Some("First".to_owned());
        tab.update_header_name(&id, header_name.clone())
            .expect("test");
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(1, tab.columns.len());
        assert_eq!(1, tab.headers.len());
        assert_eq!(0, tab.rows.len());

        let header = tab.headers.get(&id).expect("test");
        assert_eq!(header_name, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let header_name = Some("Last".to_owned());
        tab.update_header_name(&id, header_name.clone())
            .expect("test");
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(1, tab.columns.len());
        assert_eq!(1, tab.headers.len());
        assert_eq!(0, tab.rows.len());

        let header = tab.headers.get(&id).expect("test");
        assert_eq!(header_name, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let header_name = DEFAULT_HEADER_NAME;
        tab.update_header_name(&id, header_name.clone())
            .expect("test");
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(1, tab.columns.len());
        assert_eq!(1, tab.headers.len());
        assert_eq!(0, tab.rows.len());

        let header = tab.headers.get(&id).expect("test");
        assert_eq!(header_name, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }
    }

    #[test]
    pub fn test_tab_update_header_data_type() {
        let (mut tab, id) = get_tab(Dimensions::Column);
        let id = id.expect("test");

        tab.update_header_data_type(&id, DataType::Text)
            .expect("test");
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(1, tab.columns.len());
        assert_eq!(1, tab.headers.len());
        assert_eq!(0, tab.rows.len());

        let header = tab.headers.get(&id).expect("test");
        assert_eq!(DEFAULT_HEADER_NAME, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        tab.update_header_data_type(&id, DataType::Number)
            .expect("test");
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(1, tab.columns.len());
        assert_eq!(1, tab.headers.len());
        assert_eq!(0, tab.rows.len());

        let header = tab.headers.get(&id).expect("test");
        assert_eq!(DEFAULT_HEADER_NAME, *header.name());
        match header.data_type() {
            DataType::Number => {}
            _ => panic!("test"),
        }
    }

    #[test]
    pub fn test_tab_add_row() {
        let (mut tab, _) = get_tab(Dimensions::Row);

        tab.add_row().expect("test");
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(0, tab.columns.len());
        assert_eq!(0, tab.headers.len());
        assert_eq!(2, tab.rows.len());

        assert_eq!(0, tab.headers.len());
        let row = tab.rows.get(0).expect("test");
        assert_eq!(0, row.len());
        let row = tab.rows.get(1).expect("test");
        assert_eq!(0, row.len());

        let (mut tab, id) = get_tab(Dimensions::Both);
        let id = id.expect("test");

        tab.add_row().expect("test");
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(1, tab.columns.len());
        assert_eq!(1, tab.headers.len());
        assert_eq!(2, tab.rows.len());

        let header = tab.headers.get(&id).expect("test");
        assert_eq!(None, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let row = tab.rows.get(0).expect("test");
        let cell = row.get(&id).expect("test");
        match cell {
            Cell::Text(text) => assert_eq!("", text.value()),
            _ => panic!("test"),
        }

        let row = tab.rows.get(1).expect("test");
        let cell = row.get(&id).expect("test");
        match cell {
            Cell::Text(text) => assert_eq!("", text.value()),
            _ => panic!("test"),
        }
    }

    #[test]
    pub fn test_tab_delete_row() {
        let (mut tab, id) = get_tab(Dimensions::Both);
        let id = id.expect("test");

        tab.add_row().expect("test");
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(1, tab.columns.len());
        assert_eq!(1, tab.headers.len());
        assert_eq!(2, tab.rows.len());

        let header = tab.headers.get(&id).expect("test");
        assert_eq!(None, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let row = tab.rows.get(0).expect("test");
        let cell = row.get(&id).expect("test");
        match cell {
            Cell::Text(text) => assert_eq!("", text.value()),
            _ => panic!("test"),
        }

        let row = tab.rows.get(1).expect("test");
        let cell = row.get(&id).expect("test");
        match cell {
            Cell::Text(text) => assert_eq!("", text.value()),
            _ => panic!("test"),
        }

        tab.delete_row(1).expect("test");
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(1, tab.columns.len());
        assert_eq!(1, tab.headers.len());
        assert_eq!(1, tab.rows.len());

        let header = tab.headers.get(&id).expect("test");
        assert_eq!(None, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        let row = tab.rows.get(0).expect("test");
        let cell = row.get(&id).expect("test");
        match cell {
            Cell::Text(text) => assert_eq!("", text.value()),
            _ => panic!("test"),
        }

        match tab.rows.get(1) {
            Some(_) => panic!("test"),
            None => {}
        }

        match tab.delete_row(1) {
            Ok(_) => panic!("test"),
            Err(_) => {}
        }

        tab.delete_row(0).expect("test");
        assert_eq!(DEFAULT_TAB_NAME, *tab.name());
        assert_eq!(1, tab.columns.len());
        assert_eq!(1, tab.headers.len());
        assert_eq!(0, tab.rows.len());

        let header = tab.headers.get(&id).expect("test");
        assert_eq!(None, *header.name());
        match header.data_type() {
            DataType::Text => {}
            _ => panic!("test"),
        }

        match tab.rows.get(0) {
            Some(_) => panic!("test"),
            None => {}
        }

        match tab.rows.get(1) {
            Some(_) => panic!("test"),
            None => {}
        }
    }
}
