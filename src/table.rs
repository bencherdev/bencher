pub type Uuid = ulid::Ulid;

pub struct Table<T> {
    id: Uuid,
    name: String,
    columns: Vec<Uuid>,
    headers: HashMap<Uuid, Header>,
    rows: Vec<HashMap<Uuid, T>>,
}

pub struct Header {
    id: Uuid,
    name: String,
    data_type: DataType,
}

pub enum DataType {
    Text,
    Number,
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
