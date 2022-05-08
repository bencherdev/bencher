use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::Serialize;
use wasm_bindgen::prelude::*;

use crate::Reports;

#[wasm_bindgen]
pub struct InventoryData {
    inventory: JsValue,
    data: JsValue,
}

type Inventory = Vec<String>;
type Data = Vec<Datum>;

#[derive(Debug, Serialize)]
struct Datum {
    date_time: DateTime<Utc>,
    duration: u64,
    name: String,
}

// TODO all that D3 js needs is three arrays of values.
// Update the D3 code and this code to simply just pass an x, y, and z array
impl InventoryData {
    pub(crate) fn new_latency(reports: &Reports) -> Self {
        let (inventory, data) = Self::latency(reports);
        Self {
            inventory: JsValue::from_serde(&inventory)
                .expect(&format!("Failed to serialize latency inventory JSON")),
            data: JsValue::from_serde(&data)
                .expect(&format!("Failed to serialize latency data JSON")),
        }
    }

    fn latency(reports: &Reports) -> (Inventory, Data) {
        let mut names_set = HashSet::new();
        let mut data = Vec::new();
        for (date_time, report) in reports.as_ref().iter() {
            for (name, metric) in report.metrics.iter() {
                if let Some(latency) = &metric.latency() {
                    names_set.insert(name.clone());
                    data.push(Datum {
                        date_time: date_time.clone(),
                        duration: latency.duration.as_micros() as u64,
                        name: name.clone(),
                    })
                }
            }
        }
        let mut names = Vec::from_iter(names_set);
        names.sort_unstable();
        (names, data)
    }
}

#[wasm_bindgen]
impl InventoryData {
    pub fn inventory(&self) -> JsValue {
        self.inventory.clone()
    }

    pub fn data(&self) -> JsValue {
        self.data.clone()
    }
}
