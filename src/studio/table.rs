use std::collections::HashMap;

pub type Uuid = ulid::Ulid;

pub struct Table {
    id: Uuid,
    name: String,
    columns: Vec<Uuid>,
    headers: HashMap<Uuid, Header>,
    rows: Vec<HashMap<Uuid, DataType>>,
}

pub struct Header {
    id: Uuid,
    name: String,
    data_type: DataType,
}

pub enum DataType {
    Text(String),
    Number(f64),
    // List,
    // Dictionary,
    // Table,
    // Function,
    // Percentage,
    // Currency,
    // Date,
    // Time,
    // DateTime,
    // Duration,
    // Email,
    // URL,
    // Phone Number,
    // Address,
}
