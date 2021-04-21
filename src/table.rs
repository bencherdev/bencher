pub type Uuid = uuid::Uuid;

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
    Number,
    String,
    Date,
    Time,
    DateTime,
}
