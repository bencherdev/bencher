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
use crate::studio::table::nameless::nameless;
use crate::studio::table::row::Row;
use crate::studio::table::tab::{Tab, TabType};
use crate::studio::uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    id: Uuid,
    name: Option<String>,
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
            name: None,
            tab_id: id,
            tabs_vec: vec,
            tabs_map: map,
        }
    }

    pub fn name(&self) -> &Option<String> {
        &self.name
    }

    pub fn set_name(&mut self, name: Option<String>) {
        self.name = name;
    }

    pub fn tab_id(&self) -> &Uuid {
        &self.tab_id
    }

    pub fn tabs_vec(&self) -> &Vector<Uuid> {
        &self.tabs_vec
    }

    pub fn tabs_map(&self) -> &HashMap<Uuid, Tab> {
        &self.tabs_map
    }

    pub fn add_tab(&mut self) {
        let tab = Tab::new();
        self.tabs_vec.push_back(tab.id().clone());
        self.tab_id = tab.id().clone();
        self.tabs_map.insert(tab.id().clone(), tab);
    }

    pub fn delete_tab(&mut self, id: &Uuid) -> Result<()> {
        let mut index = None;
        for (tab_index, tab_id) in self.tabs_vec.iter().enumerate() {
            if *id == *tab_id {
                index = Some(tab_index);
                break;
            }
        }
        self.tabs_vec.remove(index.ok_or(anyhow!(
            "Delete tab failed to find id {} in tabs vector",
            *id
        ))?);
        self.tabs_map
            .remove(id)
            .ok_or(anyhow!("Delete tab failed to find id {} in tabs map", *id))?;

        if self.tabs_vec.len() == 0 {
            let tab = Tab::new();
            self.tabs_vec.push_back(tab.id().clone());
            self.tab_id = tab.id().clone();
            self.tabs_map.insert(tab.id().clone(), tab);
        } else if self.tab_id == *id {
            self.tab_id = self
                .tabs_vec
                .get(0)
                .ok_or(anyhow!(
                    "Delete tab failed to find next tab after id {}",
                    *id
                ))?
                .clone()
        }

        Ok(())
    }

    pub fn load(data: &str) -> Result<Table> {
        Ok(from_str(data)?)
    }

    pub fn save(&self) -> Result<String> {
        let pretty = PrettyConfig::new();
        Ok(to_string_pretty(&self, pretty)?)
    }

    pub fn to_code(&self) -> Result<String> {
        let mut code = String::new();
        // If there is more than one Tab, then the Table is an Enum
        // If there is only one Tab, then the Table is simply transparent to its only Tab
        if self.tabs_vec.len() > 1 {
            code.push_str(&format!("enum Enum{} {{", self.id));
            for tab_id in &self.tabs_vec {
                let tab = self.tabs_map.get(tab_id).ok_or(anyhow!(
                    "Table to code failed to find tab {} in map",
                    *tab_id
                ))?;
                // Create a Variant for each Tab
                code.push_str(&format!("Variant{}", *tab_id,));
                if tab.columns().len() == 0 {
                    // If the Tab has no Columns, it is a Variant with no associated data type
                    code.push_str(", ");
                } else {
                    // If the Tab has columns, it is a Variant with an associated data type
                    code.push_str(&format!("({}{}), ", *tab.tab_type(), *tab_id));
                }
            }
            code.push_str("}");
        }

        // Add all of the Tabs to the code
        for tab_id in &self.tabs_vec {
            let tab = self.tabs_map.get(tab_id).ok_or(anyhow!(
                "Table to code failed to find tab {} in map",
                *tab_id
            ))?;
            code.push_str(&tab.to_code()?);
        }

        Ok(code)
    }

    pub fn nameless(index: usize) -> String {
        format!("Table {}", nameless(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
}
